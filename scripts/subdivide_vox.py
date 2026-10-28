"""五倍保色保形细分 MagicaVoxel 场景：python -X utf8 scripts/subdivide_vox.py <src.vox> --target-unit 0.02 --staging <dir> [--publish]。

把按 0.1 米/格建模的体素场景重采样为 0.02 米/格（每格拆成 5×5×5 = 125 个子格，
颜色原样复制），并同步改写 .vox.meta 的 voxel_size，使运行时世界尺寸不变。
细分后画布任一轴超过 256 格时，该轴按 ≤256 的两块均分规划分块；分块以命名节点
之下的匿名 nTRN 平移承载，命名子资产 `#节点名` 返回完整分块子树。

安全边界：
- 只接受七个真实文件所需的格式（版本 150；SIZE/XYZI/RGBA/MATL/nTRN/nGRP/nSHP/LAYR）；
  多帧动画、多模型 nSHP、非空 ShapeModel 属性、未知块一律明确报错。
- 命名节点帧：`_t` 各分量 ×5，`_r` 原样保留；根节点的帧被运行时忽略，原样复制。
- 默认只写 staging（先 .vox 后 .vox.meta）；--publish 用 os.replace 原子替换源文件，
  任一步失败保留 staging 目录供取证，绝不降低源资产质量。
- 写出后用独立于写出路径的映射关系回读 staged 文件核对：每源格 125 个子格恰好
  各出现一次、颜色一致、总格数为 125 倍；任何不一致以非零退出码失败。
"""
from __future__ import annotations

import argparse
import bisect
import math
import os
import re
import struct
import sys
import tempfile
import time
import tracemalloc
from pathlib import Path

# 复用现有脚本的分块/字典字节编码（与 generate_enemy.py 同一 import 先例）。
from generate_ground import chunk, dictionary

# 每源格沿每轴拆出的子格数；细分因子固定为 5（0.1 米 → 0.02 米）。
FACTOR = 5
# 单轴分块上限；超过时按偶数/奇数画布取 K=256/255 两块均分。
CHUNK_LIMIT = 256
# 本工程固定的最终体素单位（米/格）；用于拒绝重复细分与校验单位比例。
TARGET_UNIT = 0.02
# 子格位集全覆盖标志：每源格 125 个子格各占一位。
FULL_BITS = (1 << (FACTOR ** 3)) - 1

# 允许出现的 MAIN 子块；其余（PACK/IMAP/动画等）一律拒绝。
KNOWN_CHUNKS = {'SIZE', 'XYZI', 'RGBA', 'MATL', 'nTRN', 'nGRP', 'nSHP', 'LAYR'}


class ConvertError(Exception):
    """转换前置条件不满足；消息即最终诊断，不再包装。"""


# ---------------------------------------------------------------------------
# 读取：字节流 → 结构化源
# ---------------------------------------------------------------------------

class Model:
    """一个 SIZE+XYZI 模型对：画布尺寸与 4 字节体素记录串（x,y,z,颜色索引）。"""

    def __init__(self, size: tuple[int, int, int], xyzi: bytes):
        self.size = size
        self.xyzi = xyzi

    @property
    def voxel_count(self) -> int:
        return len(self.xyzi) // 4


class Node:
    """场景图节点；kind ∈ {'trn','grp','shp'}，node_id 仅为诊断，引用按出现顺序。"""

    def __init__(self, kind: str, node_id: int, node_dict: dict):
        self.kind = kind
        self.node_id = node_id
        self.node_dict = node_dict
        # trn：child、layer、frames（帧字典列表，≤1 个）
        self.child = -1
        self.layer = 0
        self.frames: list[dict] = []
        # grp：children（出现顺序索引列表）
        self.children: list[int] = []
        # shp：[(model_id, attributes)]，仅接受单模型且属性为空
        self.models: list[tuple[int, dict]] = []


class VoxSource:
    """解析结果：模型列表、按出现顺序的节点列表、原样保留的 RGBA/MATL/LAYR 原始块。"""

    def __init__(self):
        self.models: list[Model] = []
        self.nodes: list[Node] = []
        self.raw_chunks: list[tuple[str, bytes]] = []  # RGBA/MATL/LAYR 原始内容


def read_dict(data: bytes, off: int) -> tuple[dict, int]:
    """读 VOX 节点字典：数量 u32，每项（键长+键+值长+值）。"""
    count, = struct.unpack_from('<I', data, off)
    off += 4
    result = {}
    for _ in range(count):
        klen, = struct.unpack_from('<I', data, off)
        off += 4
        key = data[off:off + klen].decode('ascii')
        off += klen
        vlen, = struct.unpack_from('<I', data, off)
        off += 4
        value = data[off:off + vlen].decode('ascii')
        off += vlen
        result[key] = value
    return result, off


def parse_vox(data: bytes, name: str) -> VoxSource:
    """严格解析：版本 150、白名单块、SIZE 必须紧跟 XYZI、场景图引用必须可达。"""
    if len(data) < 20 or data[:4] != b'VOX ':
        raise ConvertError(f'{name}: 不是 VOX 文件')
    version, = struct.unpack_from('<I', data, 4)
    if version != 150:
        raise ConvertError(f'{name}: 版本 {version} 不受支持，仅支持 150')
    main_tag, main_size, main_children = struct.unpack_from('<4sII', data, 8)
    if main_tag != b'MAIN' or main_size != 0:
        raise ConvertError(f'{name}: MAIN 块结构异常')
    off = 20
    if main_children == 0:
        raise ConvertError(f'{name}: MAIN 无子块')
    end = 20 + main_children
    if end > len(data):
        raise ConvertError(f'{name}: MAIN 子块长度越界')

    source = VoxSource()
    last_size: tuple[int, int, int] | None = None
    seen: set[str] = set()
    while off < end:
        tag_b, size, children = struct.unpack_from('<4sII', data, off)
        tag = tag_b.decode('ascii', errors='replace')
        body = off + 12
        nxt = body + size + children
        if nxt > end:
            raise ConvertError(f'{name}: 块 {tag} 长度越界')
        if tag not in KNOWN_CHUNKS:
            raise ConvertError(f'{name}: 不支持的块 {tag}')
        seen.add(tag)
        content = data[body:body + size]

        if tag == 'SIZE':
            if size != 12:
                raise ConvertError(f'{name}: SIZE 块长度异常')
            last_size = struct.unpack('<III', content)
        elif tag == 'XYZI':
            if last_size is None:
                raise ConvertError(f'{name}: XYZI 未跟在 SIZE 之后')
            count, = struct.unpack_from('<I', content, 0)
            if size != 4 + 4 * count:
                raise ConvertError(f'{name}: XYZI 数量与块长不一致')
            voxels = content[4:]
            for x, y, z, _c in struct.iter_unpack('<BBBB', voxels):
                if x >= last_size[0] or y >= last_size[1] or z >= last_size[2]:
                    raise ConvertError(
                        f'{name}: 体素 ({x},{y},{z}) 超出画布 {last_size}')
            source.models.append(Model(last_size, bytes(voxels)))
            last_size = None
        elif tag in ('RGBA', 'MATL', 'LAYR'):
            source.raw_chunks.append((tag, content))
        elif tag == 'nTRN':
            node_id, = struct.unpack_from('<i', content, 0)
            o = 4
            d, o = read_dict(content, o)
            child, _reserved, layer, nframes = struct.unpack_from('<iiii', content, o)
            o += 16
            frames = []
            for _ in range(nframes):
                fd, o = read_dict(content, o)
                frames.append(fd)
            node = Node('trn', node_id, d)
            node.child = child
            node.layer = layer
            node.frames = frames
            source.nodes.append(node)
        elif tag == 'nGRP':
            node_id, = struct.unpack_from('<i', content, 0)
            o = 4
            d, o = read_dict(content, o)
            count, = struct.unpack_from('<i', content, o)
            o += 4
            node = Node('grp', node_id, d)
            node.children = list(struct.unpack_from(f'<{count}i', content, o))
            source.nodes.append(node)
        elif tag == 'nSHP':
            node_id, = struct.unpack_from('<i', content, 0)
            o = 4
            d, o = read_dict(content, o)
            count, = struct.unpack_from('<i', content, o)
            o += 4
            node = Node('shp', node_id, d)
            for _ in range(count):
                mid, = struct.unpack_from('<i', content, o)
                o += 4
                md, o = read_dict(content, o)
                node.models.append((mid, md))
            source.nodes.append(node)
        off = nxt

    validate_scene(source, name)
    return source


def validate_scene(source: VoxSource, name: str) -> None:
    """结构校验：根为 nTRN、引用在界内且从根可达、帧/属性符合细分前提。"""
    if not source.nodes:
        raise ConvertError(f'{name}: 无场景图节点')
    root = source.nodes[0]
    if root.kind != 'trn':
        raise ConvertError(f'{name}: 根节点必须是 nTRN')

    # 从根 DFS 检查可达性与逐节点约束（无孤儿节点、无环、无越界引用）。
    visited = set()

    def walk(index: int, is_root: bool, path: str) -> None:
        if index in visited:
            raise ConvertError(f'{name}: 场景图存在环（{path}）')
        visited.add(index)
        node = source.nodes[index]
        if node.kind == 'trn':
            if not is_root and len(node.frames) > 1:
                raise ConvertError(f'{name}: 节点 {path} 含多帧动画')
            if not is_root:
                for frame in node.frames:
                    bad = set(frame) - {'_t', '_r'}
                    if bad:
                        raise ConvertError(f'{name}: 节点 {path} 帧含未支持键 {sorted(bad)}')
            if not 0 <= node.child < len(source.nodes):
                raise ConvertError(f'{name}: 节点 {path} 的 child 越界')
            walk(node.child, False, path + '/trn')
        elif node.kind == 'grp':
            if not node.children:
                # 空组允许（细分空模型会产生），仅校验即可。
                return
            for c in node.children:
                if not 0 <= c < len(source.nodes):
                    raise ConvertError(f'{name}: 节点 {path} 的子引用越界')
                walk(c, False, path + '/grp')
        else:
            if len(node.models) != 1:
                raise ConvertError(f'{name}: 节点 {path} 引用 {len(node.models)} 个模型，仅支持单模型')
            mid, attrs = node.models[0]
            if attrs:
                raise ConvertError(f'{name}: 节点 {path} 的模型属性非空（疑似动画）')
            if not 0 <= mid < len(source.models):
                raise ConvertError(f'{name}: 节点 {path} 的模型引用越界')

    walk(0, True, 'root')
    if len(visited) != len(source.nodes):
        raise ConvertError(f'{name}: 存在从根不可达的节点')


# ---------------------------------------------------------------------------
# 单位：meta 文本与比例校验
# ---------------------------------------------------------------------------

VOXEL_SIZE_RE = re.compile(r'(voxel_size\s*:\s*)([0-9.eE+-]+)')


def read_meta_unit(meta_path: Path) -> float:
    """从 .vox.meta 读 voxel_size；缺失或损坏都明确报错。"""
    text = meta_path.read_text(encoding='utf-8')
    match = VOXEL_SIZE_RE.search(text)
    if not match:
        raise ConvertError(f'{meta_path}: 未找到 voxel_size')
    try:
        return float(match.group(2))
    except ValueError:
        raise ConvertError(f'{meta_path}: voxel_size 无法解析') from None


def rewrite_meta_unit(text: str, target: float) -> str:
    """把 meta 文本中的 voxel_size 替换为目标单位；其余内容逐字保留。"""
    return VOXEL_SIZE_RE.sub(lambda m: m.group(1) + repr(target), text, count=1)


def check_units(source_unit: float, target_unit: float, name: str) -> None:
    """源单位必须恰好是目标单位的 5 倍；重复细分（源已等于目标）在此被拒绝。"""
    if not math.isfinite(source_unit) or source_unit <= 0:
        raise ConvertError(f'{name}: 源 voxel_size 非法：{source_unit}')
    if not math.isfinite(target_unit) or target_unit <= 0:
        raise ConvertError(f'{name}: 目标 voxel_size 非法：{target_unit}')
    if not math.isclose(source_unit, FACTOR * target_unit, rel_tol=1e-9):
        raise ConvertError(
            f'{name}: 源单位 {source_unit} 与目标单位 {target_unit} 不满足 {FACTOR} 倍关系；'
            '已细分模型重复执行会被此处拒绝')


# ---------------------------------------------------------------------------
# 细分：块规划与体素重采样
# ---------------------------------------------------------------------------

def chunk_axis_plan(full: int) -> tuple[tuple[int, int], ...]:
    """单轴分块规划：返回 [(起点, 长度)]。

    长度 ≤256 时不分块；超过时奇数画布取 K=255（奇+奇+余），偶数画布取 K=256
    （偶+偶+余），保证块数为两块均分时取整块最长。
    """
    if full <= CHUNK_LIMIT:
        return ((0, full),)
    k = CHUNK_LIMIT - 1 if full % 2 else CHUNK_LIMIT
    count = math.ceil(full / k)
    return tuple((i * k, full - i * k if i == count - 1 else k) for i in range(count))


def subdivide_model(model: Model) -> tuple[list[Model], list[tuple[int, ...]]]:
    """把一个模型细分为若干分块模型。

    返回（分块模型列表按 (ix,iy,iz) 字典序，每块平移 t）。分块平移满足：
    世界格坐标 = 局部子格坐标 + t + (full - chunk) / 2，与源模型居中对齐一致。
    """
    fx, fy, fz = (s * FACTOR for s in model.size)
    plans = (chunk_axis_plan(fx), chunk_axis_plan(fy), chunk_axis_plan(fz))
    keys = [(ix, iy, iz)
            for ix in range(len(plans[0]))
            for iy in range(len(plans[1]))
            for iz in range(len(plans[2]))]
    buffers = {key: bytearray() for key in keys}

    # 每子格坐标 → 所属块索引：轴上块起点有序，二分定位。
    origins = tuple([o for o, _c in plan] for plan in plans)
    for x, y, z, color in struct.iter_unpack('<BBBB', model.xyzi):
        for dx in range(FACTOR):
            sx = FACTOR * x + dx
            ix = bisect.bisect_right(origins[0], sx) - 1
            for dy in range(FACTOR):
                sy = FACTOR * y + dy
                iy = bisect.bisect_right(origins[1], sy) - 1
                for dz in range(FACTOR):
                    sz = FACTOR * z + dz
                    iz = bisect.bisect_right(origins[2], sz) - 1
                    # XYZI 记录字节序固定为 (x, y, z, 颜色索引)；
                    # 块内坐标 = 全画布子格坐标 − 块起点。
                    buffers[(ix, iy, iz)] += bytes((
                        sx - origins[0][ix], sy - origins[1][iy], sz - origins[2][iz], color))

    chunk_models, offsets = [], []
    for key in keys:
        lengths = (plans[0][key[0]][1], plans[1][key[1]][1], plans[2][key[2]][1])
        buffer = buffers[key]
        if not buffer:
            # 规划内不含任何体素的空块跳过，不为它写空模型（方案 §4.3）。
            continue
        chunk_models.append(Model(lengths, bytes(buffer)))
        # 平移使块在满细分画布中的世界位置与源模型居中一致：
        # 局部 c 映射到子格 s = c + origin，需 c − C/2 + t = s − F/2，
        # 即 t = origin + (C − F)/2（C 与 F 同奇偶，整除精确）。
        offsets.append(tuple(
            plan[o][0] + (plan[o][1] - full) // 2
            for plan, o, full in zip(plans, key, (fx, fy, fz))))
    return chunk_models, offsets


# ---------------------------------------------------------------------------
# 场景重建：命名节点下插入分块子树
# ---------------------------------------------------------------------------

def build_scene(source: VoxSource, factor: int) -> tuple[list[Model], list[Node]]:
    """重建场景：nTRN→nSHP 替换为 nTRN→nGRP→[nTRN(t)→nSHP]×块数。

    模型按写序重新编号；非根 nTRN 帧 `_t` ×factor、`_r` 原样；根帧原样复制
    （运行时忽略根帧）。返回新模型列表与新节点表（id 即表内序号）。
    """
    new_models: list[Model] = []
    out_nodes: list[Node] = []

    def alloc(kind: str, node_dict: dict) -> Node:
        node = Node(kind, len(out_nodes), node_dict)
        out_nodes.append(node)
        return node

    def walk(index: int, is_root: bool) -> int:
        src = source.nodes[index]
        if src.kind == 'trn':
            node = alloc('trn', dict(src.node_dict))
            node.layer = src.layer
            if is_root:
                # 根帧被 bevy_vox_scene 忽略，原样保留以免误改语义。
                node.frames = [dict(f) for f in src.frames]
            else:
                frames = []
                for frame in src.frames:
                    scaled = dict(frame)
                    if '_t' in frame:
                        tx, ty, tz = (int(v) for v in frame['_t'].split())
                        scaled['_t'] = ' '.join(str(factor * v) for v in (tx, ty, tz))
                    frames.append(scaled)
                node.frames = frames
            child = source.nodes[src.child]
            if child.kind == 'shp':
                # 单模型命名节点：插入分块 nGRP，其下每块一个匿名 nTRN+nSHP。
                group = alloc('grp', {})
                node.child = group.node_id
                src_model = source.models[child.models[0][0]]
                chunk_models, offsets = subdivide_model(src_model)
                group.children = []
                for chunk_model, offset in zip(chunk_models, offsets):
                    trn = alloc('trn', {})
                    trn.child = -1  # 稍后指向 shp
                    trn.layer = -1  # 块节点用默认层，可见性语义由父层承担
                    trn.frames = [{'_t': ' '.join(map(str, offset))}] if any(offset) else []
                    shp = alloc('shp', {})
                    shp.models = [(len(new_models), {})]
                    trn.child = shp.node_id
                    group.children.append(trn.node_id)
                    new_models.append(chunk_model)
            else:
                node.child = walk(src.child, False)
            return node.node_id
        if src.kind == 'grp':
            node = alloc('grp', dict(src.node_dict))
            node.children = [walk(c, False) for c in src.children]
            return node.node_id
        # shp 出现在非 trn 父下：validate_scene 已保证走 trn 分支，此处不可达。
        raise ConvertError('内部错误：意外的 nSHP 位置')

    walk(0, True)
    return new_models, out_nodes


# ---------------------------------------------------------------------------
# 写出与独立验证
# ---------------------------------------------------------------------------

def write_vox(models: list[Model], nodes: list[Node], raw_chunks, path: Path) -> None:
    """按（模型对 → RGBA/MATL/LAYR → 场景节点）顺序写出。"""
    children = b''
    for model in models:
        children += chunk('SIZE', struct.pack('<III', *model.size))
        children += chunk('XYZI', struct.pack('<I', model.voxel_count) + model.xyzi)
    for tag, content in raw_chunks:
        children += chunk(tag, content)
    for node in nodes:
        head = struct.pack('<i', node.node_id) + dictionary(node.node_dict)
        if node.kind == 'trn':
            # 无帧节点按 0 帧写出：运行时对无帧/空帧都取恒等变换，不补空帧。
            frames = node.frames
            content = head + struct.pack('<iiii', node.child, -1, node.layer, len(frames))
            content += b''.join(dictionary(f) for f in frames)
        elif node.kind == 'grp':
            content = head + struct.pack('<i', len(node.children))
            content += b''.join(struct.pack('<i', c) for c in node.children)
        else:
            (mid, attrs), = node.models
            content = head + struct.pack('<i', 1) + struct.pack('<i', mid) + dictionary(attrs)
        children += chunk({'trn': 'nTRN', 'grp': 'nGRP', 'shp': 'nSHP'}[node.kind], content)
    path.write_bytes(b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children))


def verify_staged(staged: bytes, source: VoxSource, factor: int, name: str) -> int:
    """回读 staged 输出，核对保形保色。返回输出体素总数。

    映射关系独立于写出路径：世界子格坐标 = 块局部 + 块平移 + (full−chunk)/2
    + 节点平移×factor；对每输出体素反解源格与子格位，逐源格用 125 位集合
    记录占用。终态每位恰好一次 ⇒ 无漏格、无重叠、总数为 125 倍。
    """
    staged_src = parse_vox(staged, f'{name} (staged)')
    stats = {'out': 0, 'expected': 0}

    def verify_chunks(src_shp: Node, out_node: Node, src_t, node_t5, path: str) -> None:
        """核对一个被细分的命名节点：分块组还原出的场景体素与源一一对应。"""
        src_model = source.models[src_shp.models[0][0]]
        stats['expected'] += src_model.voxel_count * factor ** 3
        src_voxels = {(x, y, z): c
                      for x, y, z, c in struct.iter_unpack('<BBBB', src_model.xyzi)}
        full = tuple(s * factor for s in src_model.size)
        bits: dict[tuple[int, int, int], int] = {}
        group = staged_src.nodes[out_node.child]
        if group.kind != 'grp':
            raise ConvertError(f'{name}: {path} 应展开为分块组')
        for chunk_id in group.children:
            chunk_trn = staged_src.nodes[chunk_id]
            if chunk_trn.kind != 'trn' or chunk_trn.node_dict:
                raise ConvertError(f'{name}: 分块节点结构异常（{path}）')
            t_chunk = frame_t(chunk_trn.frames)
            shp = staged_src.nodes[chunk_trn.child]
            model = staged_src.models[shp.models[0][0]]
            for x, y, z, color in struct.iter_unpack('<BBBB', model.xyzi):
                scene = tuple(
                    loc + t + (f - c) // 2 + t5
                    for loc, t, c, f, t5 in zip((x, y, z), t_chunk, model.size, full, node_t5))
                src_pos = tuple(s // factor - t for s, t in zip(scene, src_t))
                color_src = src_voxels.get(src_pos)
                if color_src is None:
                    raise ConvertError(
                        f'{name}: 输出体素 {scene} 反解出的源格 {src_pos} 不存在')
                if color_src != color:
                    raise ConvertError(
                        f'{name}: 源格 {src_pos} 颜色 {color_src} 与输出 {color} 不一致')
                d = tuple(s % factor for s in scene)
                bit = 1 << (d[0] + factor * d[1] + factor * factor * d[2])
                bits[src_pos] = bits.get(src_pos, 0) | bit
                stats['out'] += 1
        for pos, bitset in bits.items():
            if bitset != FULL_BITS:
                raise ConvertError(
                    f'{name}: 源格 {pos} 的子格覆盖不完整'
                    f'（{bin(bitset).count("1")}/{factor ** 3}）')
        if len(bits) != src_model.voxel_count:
            raise ConvertError(f'{name}: {path} 覆盖的源格数与源不符')

    def walk(src_idx: int, out_idx: int, is_root: bool, path: str) -> None:
        """源树与输出树并行走查：结构、字典、帧、几何逐节点对齐。"""
        src = source.nodes[src_idx]
        out = staged_src.nodes[out_idx]
        if src.kind != out.kind:
            raise ConvertError(f'{name}: 节点类型改变 {src.kind} → {out.kind}（{path}）')
        if out.node_dict != src.node_dict:
            raise ConvertError(f'{name}: 节点字典被改动（{path}）')
        if src.kind == 'trn':
            check_frames(out.frames, src.frames, is_root, name, factor)
            src_child = source.nodes[src.child]
            if src_child.kind == 'shp':
                verify_chunks(src_child, out, frame_t(src.frames), frame_t(out.frames), path)
            else:
                walk(src.child, out.child, False, path + '/child')
        elif src.kind == 'grp':
            if len(out.children) != len(src.children):
                raise ConvertError(f'{name}: 组子数改变（{path}）')
            for i, (sc, oc) in enumerate(zip(src.children, out.children)):
                walk(sc, oc, False, path + f'/{i}')
        else:
            raise ConvertError(f'{name}: 内部错误：{path} 处 nSHP 未经父节点处理')

    walk(0, 0, True, 'root')
    if stats['out'] != stats['expected']:
        raise ConvertError(
            f'{name}: 输出体素总数 {stats["out"]} ≠ 期望 {stats["expected"]}')
    return stats['out']


def frame_t(frames: list[dict]) -> tuple[int, int, int]:
    """取帧 `_t`；无帧或无 `_t` 视为零平移。"""
    if not frames or '_t' not in frames[0]:
        return (0, 0, 0)
    return tuple(int(v) for v in frames[0]['_t'].split())


def check_frames(out_frames, src_frames, is_root: bool, name: str, factor: int) -> None:
    """校验帧在细分前后语义一致：根帧原样；非根 `_t` ×factor、`_r` 与其余键原样。"""
    if is_root:
        if out_frames != src_frames:
            raise ConvertError(f'{name}: 根帧被改动')
        return
    if len(out_frames) != len(src_frames):
        raise ConvertError(f'{name}: 帧数量改变')
    for out, src in zip(out_frames, src_frames):
        if set(out) != set(src):
            raise ConvertError(f'{name}: 帧键集合改变 {sorted(src)} → {sorted(out)}')
        if '_t' in src:
            want = tuple(factor * int(v) for v in src['_t'].split())
            if frame_t([out]) != want:
                raise ConvertError(f'{name}: 帧平移未正确缩放 {out.get("_t")} ≠ {want}')
        if '_r' in src and out.get('_r') != src['_r']:
            raise ConvertError(f'{name}: 帧旋转被改动')


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def convert(src_path: Path, target_unit: float, staging_dir: Path,
            publish: bool) -> dict:
    """完整流程：读 → 校验 → 细分 → 写 staging → 验证 →（可选）发布。返回统计。"""
    name = src_path.name
    source = parse_vox(src_path.read_bytes(), name)
    meta_path = src_path.with_name(src_path.name + '.meta')
    source_unit = read_meta_unit(meta_path)
    check_units(source_unit, target_unit, name)

    source_voxels = sum(m.voxel_count for m in source.models)
    source_bytes = src_path.stat().st_size
    new_models, new_nodes = build_scene(source, FACTOR)

    staging_dir.mkdir(parents=True, exist_ok=True)
    staged_vox = staging_dir / src_path.name
    staged_meta = staging_dir / (src_path.name + '.meta')
    if staged_vox.exists() or staged_meta.exists():
        raise ConvertError(f'{staging_dir}: staging 产物已存在，拒绝覆盖（换新目录）')
    write_vox(new_models, new_nodes, source.raw_chunks, staged_vox)
    staged_meta.write_text(rewrite_meta_unit(meta_path.read_text(encoding='utf-8'),
                                             target_unit), encoding='utf-8')

    staged_bytes = staged_vox.read_bytes()
    out_voxels = verify_staged(staged_bytes, source, FACTOR, name)

    if publish:
        # 配对发布：源 VOX 先原子移入同目录备份（同文件系统），随后依次替换
        # VOX 与 meta；任一步失败把备份移回，保证资产与配置不成对损坏（方案 §8）。
        backup = src_path.with_name(src_path.name + '.subdiv-bak')
        os.replace(src_path, backup)
        try:
            os.replace(staged_vox, src_path)
            os.replace(staged_meta, meta_path)
        except OSError:
            os.replace(backup, src_path)
            raise
        # 发布成功后清理备份与空 staging 目录；失败时保留 staging 供取证。
        os.remove(backup)
        try:
            staging_dir.rmdir()
        except OSError:
            pass

    return {
        'source_voxels': source_voxels,
        'output_voxels': out_voxels,
        'source_bytes': source_bytes,
        'output_bytes': len(staged_bytes),
        'models': (len(source.models), len(new_models)),
        'published': publish,
    }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description='五倍保色保形细分 VOX 场景')
    parser.add_argument('source', type=Path, help='源 .vox 路径')
    parser.add_argument('--target-unit', type=float, default=TARGET_UNIT,
                        help=f'目标体素单位（米/格），默认 {TARGET_UNIT}')
    parser.add_argument('--staging', type=Path, default=None,
                        help='staging 目录；缺省时使用临时目录（仅与 --publish 搭配）')
    parser.add_argument('--publish', action='store_true',
                        help='验证通过后用 os.replace 原子替换源文件')
    args = parser.parse_args(argv)

    if not args.publish and args.staging is None:
        parser.error('未指定 --publish 时必须给出 --staging')
    if args.publish:
        staging = Path(tempfile.mkdtemp(prefix='subdivide-'))
    else:
        staging = args.staging

    tracemalloc.start()
    started = time.perf_counter()
    try:
        stats = convert(args.source, args.target_unit, staging, args.publish)
    except ConvertError as exc:
        print(f'转换失败：{exc}', file=sys.stderr)
        print(f'staging 保留在：{staging}', file=sys.stderr)
        return 1
    elapsed = time.perf_counter() - started
    _, peak = tracemalloc.get_traced_memory()
    tracemalloc.stop()

    where = '源文件（已发布）' if stats['published'] else f'{staging}'
    print(f'完成 {args.source.name} → {where}')
    print(f'  体素：{stats["source_voxels"]} → {stats["output_voxels"]}'
          f'（×{FACTOR ** 3}）')
    print(f'  模型数：{stats["models"][0]} → {stats["models"][1]}')
    print(f'  字节：{stats["source_bytes"]} → {stats["output_bytes"]}')
    print(f'  耗时 {elapsed:.2f} 秒，Python 堆峰值 {peak / 1024 / 1024:.1f} MiB')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
