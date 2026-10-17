"""生成独立暗红靶车，34×26×14 格；运行：python3 scripts/generate_enemy.py。"""
from pathlib import Path
import struct
from generate_ground import chunk, dictionary

OUTPUT = Path(__file__).resolve().parents[1] / 'assets' / 'enemy_tank.vox'


def build_vox():
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
        box(0, 26, y, y+4, 5, 5, 3, bevel=1)
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

    voxels = b''.join(bytes((*position, color)) for position, color in sorted(cells.items()))
    children = chunk('SIZE', struct.pack('<III', 34, 26, 14))
    children += chunk('XYZI', struct.pack('<I', len(cells)) + voxels)
    children += chunk('nTRN', struct.pack('<i', 0) + dictionary({})
                      + struct.pack('<iiii', 1, -1, -1, 1) + dictionary({}))
    children += chunk('nGRP', struct.pack('<i', 1) + dictionary({}) + struct.pack('<ii', 1, 2))
    children += chunk('nTRN', struct.pack('<i', 2) + dictionary({'_name': 'enemy_tank'})
                      + struct.pack('<iiii', 3, -1, -1, 1) + dictionary({}))
    children += chunk('nSHP', struct.pack('<i', 3) + dictionary({}) + struct.pack('<ii', 1, 0)
                      + dictionary({}))
    colors = [(100, 42, 38), (121, 55, 46), (75, 31, 29), (36, 39, 37), (66, 60, 53)]
    palette = b''.join(bytes((*rgb, 255)) for rgb in colors)
    children += chunk('RGBA', palette + bytes((0, 0, 0, 255)) * (256-len(colors)))
    for index in range(1, len(colors)+1):
        children += chunk('MATL', struct.pack('<i', index)
                          + dictionary({'_type': '_diffuse', '_rough': '1'}))
    return b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children)


if __name__ == '__main__':
    OUTPUT.write_bytes(build_vox())
    print(f'已生成 {OUTPUT}')
