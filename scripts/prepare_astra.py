"""从 astraH 原 VOX 生成可活动整车；python3 scripts/prepare_astra.py [--check]。

不重建美术：车体、炮塔、炮管和轮组直接复制原模型字节，仅重设安装原点。
履带相位 0 保留原样，相位 1/2 沿原六段闭环搬移，保留左右独立控制。
"""
from __future__ import annotations

import argparse
from copy import deepcopy
from functools import lru_cache
import hashlib
import json
import math
import os
from pathlib import Path
import struct
import tempfile

from subdivide_vox import Model, Node, parse_vox, publish_pair, write_vox

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'tank_model/leovox_astraH/leopard_2a6.vox'
DEST = ROOT / 'assets/tank/player/leopard2_a6'
PIVOT = (192, 0, 67)
TURRET_PIVOT = (192, 0, 80)
BARREL_PIVOT = (278, 0, 104)
BODY_PARTS = {'hull', 'engine_deck', 'left_suspension', 'right_suspension'}
# 与原作者化脚本相同的闭环中心线；坐标为资产格。
TRACK_PATH = ((1, 21), (17, 2), (366, 2), (382, 21), (366, 45), (17, 45))
TRACK_SEGMENTS = []
for a, b in zip(TRACK_PATH, TRACK_PATH[1:] + TRACK_PATH[:1]):
    length = math.dist(a, b)
    TRACK_SEGMENTS.append((a, b, length, ((b[0]-a[0])/length, (b[1]-a[1])/length)))
TRACK_LENGTH = sum(s[2] for s in TRACK_SEGMENTS)


def read_source(path):
    """PACK 只声明模型数；去掉后复用已有严格静态场景解析器。"""
    data = path.read_bytes()
    assert data[:8] == b'VOX ' + struct.pack('<I', 150)
    kept = []
    pos = 20
    while pos < len(data):
        tag, size, children = struct.unpack_from('<4sII', data, pos)
        assert children == 0 and pos + 12 + size <= len(data)
        if tag != b'PACK':
            kept.append(data[pos:pos+12+size])
        pos += 12 + size
    assert pos == len(data)
    payload = b''.join(kept)
    return parse_vox(data[:16] + struct.pack('<I', len(payload)) + payload, str(path))


def groups(source):
    """返回完整场景根之下的命名装配组，不把分块误当零件。"""
    root = source.nodes[source.nodes[0].child]
    assert root.kind == 'grp'
    return {source.nodes[i].node_dict['_name']: i for i in root.children}


def group_cells(source, index, offset=(0, 0, 0)):
    """回读到原作者的整数格坐标；模型 SIZE 中心与 nTRN 平移共同决定位置。"""
    node = source.nodes[index]
    if node.kind == 'trn':
        assert len(node.frames) == 1 and '_r' not in node.frames[0]
        delta = tuple(map(int, node.frames[0].get('_t', '0 0 0').split()))
        yield from group_cells(source, node.child, tuple(a+b for a, b in zip(offset, delta)))
    elif node.kind == 'grp':
        for child in node.children:
            yield from group_cells(source, child, offset)
    else:
        model = source.models[node.models[0][0]]
        half = tuple(n//2 for n in model.size)
        for x, y, z, color in struct.iter_unpack('<BBBB', model.xyzi):
            yield (x+offset[0]-half[0], y+offset[1]-half[1], z+offset[2]-half[2], color)


class Assembly:
    """为一个运行时文件复制原模型/节点，只改指定装配组的局部原点。"""
    def __init__(self, source):
        self.source = source
        self.models = []
        self.nodes = [Node('trn', 0, {}), Node('grp', 1, {})]
        self.nodes[0].child = 1
        self.nodes[0].frames = [{'_t': '0 0 0'}]
        self.nodes[0].layer = 0

    def copy_node(self, index):
        old = self.source.nodes[index]
        node = deepcopy(old)
        node.node_id = len(self.nodes)
        self.nodes.append(node)
        if node.kind == 'trn':
            node.child = self.copy_node(old.child)
        elif node.kind == 'grp':
            node.children = [self.copy_node(i) for i in old.children]
        else:
            node.models = [(len(self.models), {})]
            self.models.append(self.source.models[old.models[0][0]])
        return node.node_id

    def copy_group(self, index, pivot, name=None):
        node = self.nodes[self.copy_node(index)]
        # 插件加载 #命名节点 时忽略该根的帧，偏移必须写进其子块。
        assert node.frames == [{'_t': '0 0 0'}]
        for child in self.nodes[node.child].children:
            block = self.nodes[child]
            delta = tuple(map(int, block.frames[0].get('_t', '0 0 0').split()))
            block.frames[0]['_t'] = ' '.join(str(a-b) for a, b in zip(delta, pivot))
        if name:
            node.node_dict['_name'] = name
        self.nodes[1].children.append(node.node_id)

    def add_cells(self, name, cells, pivot):
        # 坐标字节分块只负责存储，不改变几何；全部模型尺寸取偶数以保持中心为整数格。
        buckets = {}
        for x, y, z, color in cells:
            key = (x//256, y//256, z//256)
            buckets.setdefault(key, []).append((x, y, z, color))
        tr = Node('trn', len(self.nodes), {'_name': name})
        self.nodes.append(tr)
        group = Node('grp', len(self.nodes), {})
        self.nodes.append(group)
        tr.child = group.node_id
        tr.frames = [{'_t': '0 0 0'}]
        self.nodes[1].children.append(tr.node_id)
        for cells in buckets.values():
            low = tuple(min(c[i] for c in cells) for i in range(3))
            size = tuple((max(c[i] for c in cells)-low[i]+2)//2*2 for i in range(3))
            assert all(v <= 256 for v in size)
            raw = b''.join(bytes((x-low[0], y-low[1], z-low[2], c)) for x, y, z, c in cells)
            child = Node('trn', len(self.nodes), {})
            self.nodes.append(child)
            shape = Node('shp', len(self.nodes), {})
            self.nodes.append(shape)
            child.child = shape.node_id
            child.frames = [{'_t': ' '.join(str(low[i]+size[i]//2-pivot[i]) for i in range(3))}]
            shape.models = [(len(self.models), {})]
            self.models.append(Model(size, raw))
            group.children.append(child.node_id)

    def save(self, path):
        write_vox(self.models, self.nodes, self.source.raw_chunks, path)
        path.with_suffix('.vox.meta').write_bytes(SOURCE.with_suffix('.vox.meta').read_bytes())


@lru_cache(maxsize=None)
def move_track_cell(x, z, phase):
    """沿闭环前进，保留到中心线的法向偏移；上/下带自然反向，跨端部连续回绕。"""
    phase %= 3
    if not phase:
        return x, z
    best = None
    total = 0.0
    for a, b, length, tangent in TRACK_SEGMENTS:
        projection = max(0.0, min(length, (x-a[0])*tangent[0]+(z-a[1])*tangent[1]))
        px, pz = a[0]+projection*tangent[0], a[1]+projection*tangent[1]
        distance = (x-px)**2 + (z-pz)**2
        normal = (tangent[1], -tangent[0])
        candidate = (distance, total+projection, (x-px)*normal[0]+(z-pz)*normal[1])
        if best is None or distance < best[0]:
            best = candidate
        total += length
    # shortcut: 原作者各段板距略异，三相位按约 14cm 板距循环；需连续履带时改为逐板路径实例。
    # 作者 +X 向前：原路径下段沿 +X，因此前进相位沿路径负方向。
    travel = (best[1] - phase*7/3) % TRACK_LENGTH
    for a, b, length, tangent in TRACK_SEGMENTS:
        if travel <= length:
            normal = (tangent[1], -tangent[0])
            px = round(a[0]+travel*tangent[0]+best[2]*normal[0])
            pz = round(a[1]+travel*tangent[1]+best[2]*normal[1])
            return max(0, min(382, px)), max(0, pz)
        travel -= length
    raise AssertionError('闭环里程未落在任何线段')


def moved_track(source, index, phase):
    """重新体素化后的交叠格只保留一份，避免重复记录；颜色跟随履带板移动。"""
    cells = {}
    for x, y, z, color in group_cells(source, index):
        x, z = move_track_cell(x, z, phase)
        cells[x, y, z] = color
    return [(x, y, z, color) for (x, y, z), color in cells.items()]


def wheel_parts():
    """沿用运行时负宽轴侧为 l 的约定；原 astra 名称以实车自身左右区分。"""
    for side, label in [('l', 'right'), ('r', 'left')]:
        for i in range(7):
            yield f'wheel_{side}_{i}', f'{label}_roadwheel_{i+1}'
        yield f'wheel_{side}_7', f'{label}_sprocket'
        yield f'wheel_{side}_8', f'{label}_idler'


def bounds(cells):
    low = tuple(min(c[i] for c in cells) for i in range(3))
    high = tuple(max(c[i] for c in cells)+1 for i in range(3))
    return low, high


def digest(cells):
    """按世界格排序后比较位置和颜色，独立于 VOX 模型分块及节点顺序。"""
    ordered = sorted(cells)
    return hashlib.sha256(b''.join(struct.pack('<iiiB', *c) for c in ordered)).hexdigest()


def verify(source, output, assignments, wheel_info):
    """从实际写出的文件回读，核对默认姿态每个原装配组的有色体素完全一致。"""
    originals = [a[1] for a in assignments]
    assert len(originals) == len(set(originals)) and set(originals) == set(groups(source)), '装配组丢失或重复'
    count = 0
    for filename, original, target, pivot in assignments:
        delivered = output[filename]
        cells = list(group_cells(delivered, groups(delivered)[target]))
        restored = [(x+pivot[0], y+pivot[1], z+pivot[2], c) for x, y, z, c in cells]
        expected = list(group_cells(source, groups(source)[original]))
        assert digest(expected) == digest(restored), f'外观变化：{original}'
        count += len(expected)
    assert count == sum(model.voxel_count for model in source.models), f'原装配组丢失或重复：{count}'
    palette = [c for c in source.raw_chunks if c[0] in {'RGBA', 'MATL'}]
    assert all([c for c in out.raw_chunks if c[0] in {'RGBA', 'MATL'}] == palette for out in output.values())
    tracks = output['track']
    # 模拟插件的 #label 边界：根帧被忽略，因此命名根必须保持恒等且没有祖先名前缀。
    assert '_name' not in tracks.nodes[0].node_dict
    assert all(tracks.nodes[i].frames == [{'_t': '0 0 0'}] for i in groups(tracks).values())
    for side in ['l', 'r']:
        initial = list(group_cells(tracks, groups(tracks)[f'track_{side}_0']))
        assert bounds(initial)[0][2] == -PIVOT[2]
        for phase in [1, 2]:
            moving = list(group_cells(tracks, groups(tracks)[f'track_{side}_{phase}']))
            assert digest(initial) != digest(moving), '履带相位没有几何变化'
            assert len({c[:3] for c in moving}) == len(moving), '履带相位重复格'
            assert bounds(moving)[0][2] == -PIVOT[2], '运动时履带离地'
    return {'source_sha256': hashlib.sha256(SOURCE.read_bytes()).hexdigest(),
            'default_pose_voxels_preserved': count,
            'palette_preserved': True, 'independent_wheels': wheel_info,
            'track_phase_step_m': 0.14/3,
            'track_note': 'phase0原样；其他相位沿原中心线搬移并按2cm格重新量化'}


def publish_astra_pair(staged, target):
    """更新复用配对回滚；首次安装失败时撤下已发布 VOX，避免留下无 meta 资源。"""
    meta = target.with_name(target.name + '.meta')
    staged_meta = staged.with_name(staged.name + '.meta')
    if target.exists() != meta.exists():
        raise ValueError(f'目标已有不完整 VOX/meta 配对：{target}')
    if target.exists():
        publish_pair(staged, staged_meta, target)
    else:
        try:
            os.replace(staged, target)
            os.replace(staged_meta, meta)
        except OSError:
            target.unlink(missing_ok=True)
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='重生成到临时目录，比较所有交付文件')
    args = parser.parse_args()
    source = read_source(SOURCE)
    names = groups(source)
    wheel_names = dict(wheel_parts())
    moving = set(wheel_names.values()) | {'left_track', 'right_track', 'L55_gun'}
    turret_parts = set(names)-BODY_PARTS-moving
    assignments = []
    wheel_info = []
    if not args.check:
        DEST.mkdir(parents=True, exist_ok=True)
    # 发布临时文件必须与目标同卷；--check 仍只写系统临时目录。
    with tempfile.TemporaryDirectory(prefix='tanker-astra-', dir=None if args.check else DEST) as temp:
        staging = Path(temp)
        for filename, parts, pivot in [('body', sorted(BODY_PARTS), PIVOT),
                                       ('turret', sorted(turret_parts), TURRET_PIVOT),
                                       ('barrel', ['L55_gun'], BARREL_PIVOT)]:
            assembly = Assembly(source)
            for part in parts:
                assembly.copy_group(names[part], pivot)
                assignments.append((filename, part, part, pivot))
            assembly.save(staging/f'{filename}.vox')
        tracks = Assembly(source)
        for side, label in [('l', 'right'), ('r', 'left')]:
            name = f'{label}_track'
            for phase in range(3):
                target = f'track_{side}_{phase}'
                if phase == 0:
                    tracks.copy_group(names[name], PIVOT, target)
                    assignments.append(('track', name, target, PIVOT))
                else:
                    tracks.add_cells(target, moved_track(source, names[name], phase), PIVOT)
        for target, original in wheel_names.items():
            cells = list(group_cells(source, names[original]))
            low, high = bounds(cells)
            center = tuple((a+b)//2 for a, b in zip(low, high))
            assert all((a+b)%2 == 0 for a, b in zip(low, high)), original
            envelope = max(math.hypot(x+dx-center[0], z+dz-center[2])
                           for x, _, z, _ in cells for dx, dz in [(0,0),(1,0),(0,1),(1,1)])
            tracks.copy_group(names[original], center, target)
            assignments.append(('track', original, target, center))
            wheel_info.append({'name': target, 'center_cells': center,
                               'size_cells': tuple(b-a for a, b in zip(low, high)),
                               'rotation_envelope_m': envelope*0.02})
        tracks.save(staging/'track.vox')
        output = {n: read_source(staging/f'{n}.vox') for n in ['body', 'turret', 'barrel', 'track']}
        report = verify(source, output, assignments, wheel_info)
        if args.check:
            for path in staging.iterdir():
                assert path.read_bytes() == (DEST/path.name).read_bytes(), f'交付不一致：{path.name}'
        else:
            for path in sorted(staging.glob('*.vox')):
                publish_astra_pair(path, DEST/path.name)
        print(json.dumps({k:v for k,v in report.items() if k != 'independent_wheels'}, ensure_ascii=False))


if __name__ == '__main__':
    main()
