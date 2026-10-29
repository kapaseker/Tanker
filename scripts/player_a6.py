"""豹 2A6 阶段轮廓的 0.02 米作者源；共同 VOX +X 向前、+Z 向上。"""
import ast
import hashlib
import itertools
import math
import re
import struct
from pathlib import Path
import subdivide_vox as sv

CONTRACT = (Path(__file__).resolve().parents[1] / 'src/tank/model/player.rs').read_text(encoding='utf-8')


def literal(name):
    """仅解析共享契约的字面量；格式漂移时失败，不执行 Rust 表达式。"""
    return ast.literal_eval(re.search(r'pub const '+name+r':[^=]+=(.*?);', CONTRACT).group(1).strip())


CANVAS = tuple(5 * n for n in literal('CANVAS'))
HULL = literal('VISUAL_HULL_X')
TURRET = literal('TURRET_X')
ROOF = literal('TURRET_TOP')
BARREL = tuple(round(5 * n) for n in literal('BARREL_X'))
BORE = round(5 * literal('BORE_HEIGHT'))


def box(cells, x, y, z, color, bevel=0):
    """填半开盒；最后写入者覆盖，0 用于凹槽和炮口挖空。"""
    if not all(0 <= lo < hi <= size for (lo, hi), size in zip((x, y, z), CANVAS)):
        raise sv.ConvertError(f'作者盒越出共同画布：{x},{y},{z}')
    row = bytes([color]) * (z[1] - z[0])
    for a in range(*x):
        for b in range(*y):
            if min(a-x[0], x[1]-1-a) + min(b-y[0], y[1]-1-b) < bevel:
                continue
            start = (a * CANVAS[1] + b) * CANVAS[2] + z[0]
            cells[start:start + len(row)] = row


def geometry(name):
    """直接表达真实尺度主体；五轮与附件细节是 02 的阶段状态。"""
    cells = bytearray(CANVAS[0] * CANVAS[1] * CANVAS[2])
    if name == 'body':
        box(cells, HULL, (40, 150), (22, 56), 1, 15)
        # 首上向后收；不把装备总高填成整块装甲。
        for z in range(56, 80):
            box(cells, (HULL[0], HULL[1] - 2 * (z-56)), (35, 155), (z, z+1), 2 if z == 79 else 1, 12)
        for y in ((6, 36), (154, 184)):
            beam = (35, 40) if y[0] == 6 else (150, 155)
            box(cells, HULL, beam, (80, 85), 3)
            for x in range(HULL[0], HULL[1], 50):
                box(cells, (x, min(x+45, HULL[1])), y, (80, 86), 3, 3)
        # 炮塔后舱与前楔仍属于 body，不引入转塔。
        box(cells, (TURRET[0], 390), (20, 170), (80, 110), 1, 10)
        box(cells, (TURRET[0]+5, 390), (25, 165), (110, ROOF), 2, 8)
        for x in range(390, TURRET[1]):
            half = 75 - (x-390) * 45 // (TURRET[1]-390-1)
            top = ROOF - max(0, x-420) // 4
            box(cells, (x, x+1), (95-half, 95+half), (80, top), 1)
        box(cells, (TURRET[1]-20, TURRET[1]), (75, 115), (BORE-15, BORE+15), 3, 3)
        box(cells, (305, 345), (55, 95), (ROOF, 135), 3, 6)
        box(cells, (360, 390), (105, 135), (ROOF, 135), 3, 5)
        # 槽宽 0.04、深 0.02、节距 0.14 米，保留细网格作者能力。
        box(cells, (180, 235), (50, 130), (78, 80), 2)
        for x in range(180, 235, 7):
            box(cells, (x, min(x+2, 235)), (53, 127), (78, 79), 4)
            box(cells, (x, min(x+2, 235)), (53, 127), (79, 80), 0)
    elif name == 'barrel':
        box(cells, BARREL, (90, 100), (BORE-5, BORE+5), 2)
        box(cells, (BARREL[0], BARREL[0]+30), (85, 105), (BORE-10, BORE+10), 3, 3)
        box(cells, (575, 605), (86, 104), (BORE-9, BORE+9), 1, 2)
        box(cells, (BARREL[1]-5, BARREL[1]), (90, 100), (BORE-5, BORE+5), 4)
        box(cells, (BARREL[1]-5, BARREL[1]), (92, 98), (BORE-3, BORE+3), 0)
    else:
        side, phase = name.split('_')[1:]
        phase = int(phase)
        y, outer, inner = ((5, 35), (1, 5), (35, 40)) if side == 'l' else ((155, 185), (185, 189), (150, 155))
        # 闭环 900 格；齿距 15 格，每相位推进 5 格，仍为 0.1 米。
        rim = ([(x, 68) for x in range(HULL[0], HULL[1])]
               + [(HULL[1]-1, z) for z in range(67, 0, -1)]
               + [(x, 0) for x in range(HULL[1]-1, HULL[0]-1, -1)]
               + [(HULL[0], z) for z in range(1, 68)])
        assert len(rim) == len(set(rim)) == 900
        box(cells, (HULL[0]+5, HULL[1]-5), inner, (25, 35), 4)
        for i, (x, z) in enumerate(rim):
            tooth = (i - phase*5) % 15 < 5
            box(cells, (x, x+1), y, (z, z+1), 5 if tooth else 4)
            if tooth:
                box(cells, (x, x+1), outer, (z, z+1), 5)
                if z == 68:
                    box(cells, (x, x+1), y, (69, 73), 5)
    return cells


def author_package(path):
    """复用现有分块规划和配对发布；独立轮保留 Rust 粗源细分结果。"""
    source = sv.parse_vox(path.read_bytes(), str(path))
    models, nodes, expected = [], [], {}

    def node(kind, attrs=None):
        result = sv.Node(kind, len(nodes), attrs or {})
        nodes.append(result)
        return result

    root, group = node('trn'), node('grp')
    root.child, root.layer = group.node_id, -1
    for index in source.nodes[source.nodes[0].child].children:
        old = source.nodes[index]
        name = old.node_dict['_name']
        named, chunks = node('trn', {'_name': name}), node('grp')
        named.child, named.layer = chunks.node_id, -1
        group.children.append(named.node_id)
        pieces = []
        if name.startswith('wheel_'):
            for child in source.nodes[old.child].children:
                trn = source.nodes[child]
                model = source.models[source.nodes[trn.child].models[0][0]]
                pieces.append((model, sv.frame_t(trn.frames)))
            # 轮子小于单块上限，局部画布原点就是轮轴画布的最小角。
            if len(pieces) != 1 or any(pieces[0][1]):
                raise sv.ConvertError(f'独立轮局部画布异常：{name}')
            model = pieces[0][0]
            wheel_cells = bytearray(math.prod(model.size))
            for x, y, z, color in struct.iter_unpack('<BBBB', model.xyzi):
                wheel_cells[(x*model.size[1]+y)*model.size[2]+z] = color
            expected[name] = (model.size, hashlib.sha256(wheel_cells).digest())
        else:
            cells = geometry(name)
            expected[name] = (CANVAS, hashlib.sha256(cells).digest())
            for plan in itertools.product(*(sv.chunk_axis_plan(n) for n in CANVAS)):
                origin, size = tuple(p[0] for p in plan), tuple(p[1] for p in plan)
                data = bytearray()
                for x in range(size[0]):
                    for y in range(size[1]):
                        start = ((x+origin[0])*CANVAS[1]+y+origin[1])*CANVAS[2]+origin[2]
                        row = cells[start:start+size[2]]
                        if any(row):
                            for z, color in enumerate(row):
                                if color:
                                    data.extend((x, y, z, color))
                if data:
                    offset = tuple(o + (s-f)//2 for o, s, f in zip(origin, size, CANVAS))
                    pieces.append((sv.Model(size, bytes(data)), offset))
        for model, offset in pieces:
            trn, shape = node('trn'), node('shp')
            trn.child, trn.layer = shape.node_id, -1
            trn.frames = [{'_t': ' '.join(map(str, offset))}]
            shape.models = [(len(models), {})]
            chunks.children.append(trn.node_id)
            models.append(model)
    sv.write_vox(models, nodes, source.raw_chunks, path)
    verify_package(path, expected)


def verify_package(path, expected):
    """发布前独立回读：按场景变换重建占用和颜色，拒绝漏块、错移与重叠。"""
    source = sv.parse_vox(path.read_bytes(), str(path))
    root = source.nodes[0]
    names = []
    if root.frames:
        raise sv.ConvertError('作者包根变换必须为恒等')
    for index in source.nodes[root.child].children:
        named = source.nodes[index]
        name = named.node_dict.get('_name')
        if name not in expected or name in names or named.frames:
            raise sv.ConvertError(f'作者包命名根或原点异常：{name}')
        names.append(name)
        full, digest = expected[name]
        cells = bytearray(math.prod(full))
        for child in source.nodes[named.child].children:
            trn = source.nodes[child]
            if any(set(frame) - {'_t'} for frame in trn.frames):
                raise sv.ConvertError(f'分块含非平移变换：{name}')
            model = source.models[source.nodes[trn.child].models[0][0]]
            if any((f-s) % 2 for f, s in zip(full, model.size)):
                raise sv.ConvertError(f'分块画布无法整格居中：{name}')
            origin = tuple(t+(f-s)//2 for t, f, s in zip(sv.frame_t(trn.frames), full, model.size))
            for x, y, z, color in struct.iter_unpack('<BBBB', model.xyzi):
                x, y, z = x+origin[0], y+origin[1], z+origin[2]
                if not (0 <= x < full[0] and 0 <= y < full[1] and 0 <= z < full[2]):
                    raise sv.ConvertError(f'分块越出作者画布：{name}')
                position = (x*full[1]+y)*full[2]+z
                if cells[position]:
                    raise sv.ConvertError(f'分块体素重叠：{name}')
                cells[position] = color
        if hashlib.sha256(cells).digest() != digest:
            raise sv.ConvertError(f'作者几何回读不一致：{name}')
    if set(names) != set(expected):
        raise sv.ConvertError('作者包缺少命名根')
