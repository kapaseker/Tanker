"""生成暗红敌人车体与 lugged 履带包；运行：python scripts/generate_enemy.py。"""
from pathlib import Path
import struct
from generate_ground import chunk, dictionary

ASSETS = Path(__file__).resolve().parents[1] / 'assets' / 'tank'
BODY_OUTPUT = ASSETS / 'enemy' / 'default' / 'body.vox'
TRACKS_OUTPUT = ASSETS / 'tracks' / 'enemy' / 'lugged' / 'default.vox'


def build_cells():
    """VOX 的 +X 对应车头（Bevy −X），Z 为高度，每格由加载器映射为 0.1 米。"""
    cells = {}

    def box(x0, x1, y0, y1, z0, z1, color, bevel=0):
        # 削去平面四角，区别于主坦克的直角车身；后写部件覆盖接缝体素。
        for x in range(x0, x1+1):
            for y in range(y0, y1+1):
                if min(x-x0, x1-x) + min(y-y0, y1-y) < bevel:
                    continue
                for z in range(z0, z1+1):
                    cells[x, y, z] = color

    # 双侧深灰履带、侧面轮组和红色挡泥板。
    for y in (0, 21):
        box(0, 26, y, y+4, 0, 4, 4, bevel=1)
        for x in (3, 8, 13, 18, 23):
            box(x-1, x+1, y, y+4, 1, 3, 5)
        # 挡泥板抬到 z=7（车体侧壁顶面之上），留出 z=5、6 的空隙露出履带上沿。
        box(0, 26, y, y+4, 7, 7, 3, bevel=1)
    box(1, 26, 4, 21, 3, 6, 1, bevel=2)
    box(3, 25, 5, 20, 7, 7, 2, bevel=3)
    # 矮宽棱角炮塔、偏长炮管与顶盖；炮口保持暗色，主体使用哑光暗红。
    box(8, 23, 7, 18, 8, 10, 1, bevel=3)
    box(10, 21, 8, 17, 11, 11, 2, bevel=2)
    box(21, 33, 11, 13, 9, 10, 5)
    box(31, 33, 10, 14, 9, 11, 3, bevel=1)
    box(33, 33, 11, 13, 10, 10, 4)
    box(13, 18, 10, 15, 12, 13, 3, bevel=1)
    # 车尾散热格栅，打破大片纯色。
    for x in range(3, 8, 2):
        box(x, x, 8, 17, 8, 8, 4)

    return cells


def build_vox(models):
    """所有模型共用 34×26×14 全局坐标，加载后车体与履带自动居中对齐。"""
    children = b''
    for name, cells in models:
        voxels = b''.join(bytes((*position, color)) for position, color in sorted(cells.items()))
        children += chunk('SIZE', struct.pack('<III', 34, 26, 14))
        children += chunk('XYZI', struct.pack('<I', len(cells)) + voxels)
    children += chunk('nTRN', struct.pack('<i', 0) + dictionary({})
                      + struct.pack('<iiii', 1, -1, -1, 1) + dictionary({}))
    children += chunk('nGRP', struct.pack('<i', 1) + dictionary({})
                      + struct.pack('<i', len(models))
                      + b''.join(struct.pack('<i', 2+2*i) for i in range(len(models))))
    for i, (name, cells) in enumerate(models):
        node = 2+2*i
        children += chunk('nTRN', struct.pack('<i', node) + dictionary({'_name': name})
                          + struct.pack('<iiii', node+1, -1, -1, 1) + dictionary({}))
        children += chunk('nSHP', struct.pack('<i', node+1) + dictionary({})
                          + struct.pack('<ii', 1, i) + dictionary({}))
    colors = [(100, 42, 38), (121, 55, 46), (75, 31, 29), (36, 39, 37), (66, 60, 53)]
    palette = b''.join(bytes((*rgb, 255)) for rgb in colors)
    children += chunk('RGBA', palette + bytes((0, 0, 0, 255)) * (256-len(colors)))
    for index in range(1, len(colors)+1):
        children += chunk('MATL', struct.pack('<i', index)
                          + dictionary({'_type': '_diffuse', '_rough': '1'}))
    return b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children)


def build_assets():
    """保留红色车身及挡泥板，将灰色履带拆成左右各三相位。"""
    original = build_cells()
    tracks = {p: c for p, c in original.items()
              if p[2] < 5 and (p[1] <= 4 or p[1] >= 21) and c in (4, 5)}
    body = {p: c for p, c in original.items() if p not in tracks}
    # 60 个轮廓点构成闭环：上段向车头 +X、前端向下、下段向后、后端向上。
    # 长度整除齿距 3，避免三个相位循环时接缝突然跳齿。
    rim = ([(x, 4) for x in range(27)]
           + [(26, z) for z in range(3, 0, -1)]
           + [(x, 0) for x in range(26, -1, -1)]
           + [(0, z) for z in range(1, 4)])
    models = []
    for side, face in (('l', 0), ('r', 25)):
        for phase in range(3):
            # 外侧一格仅放轮廓上的凸齿；其内侧保留原有静止轮组，不再整面变色。
            cells = {p: c for p, c in tracks.items()
                     if (p[1] <= 4) == (side == 'l') and p[1] != face}
            core_y = range(1, 5) if side == 'l' else range(21, 25)
            for s, (x, z) in enumerate(rim):
                tooth = (s-phase) % 3 == 0
                for y in core_y:
                    # 挡泥板与内侧车身保持原样，不能被履带轮廓覆盖。
                    if (x, y, z) not in body:
                        cells[x, y, z] = 5 if tooth else 4
                if tooth:
                    cells[x, face, z] = 5
            models.append((f'track_{side}_{phase}', cells))
    return build_vox([('body', body)]), build_vox(models)


if __name__ == '__main__':
    for path, data in zip((BODY_OUTPUT, TRACKS_OUTPUT), build_assets()):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        print(f'已生成 {path}')
