"""用标准库生成 632×632 格非周期土质场地：python3 scripts/generate_ground.py。"""
import math
from pathlib import Path
import random
import struct

# 单模型受 VOX 的 u8 坐标限制，全图先生成再切成 4×4 块。
TILE_SIZE = 158
TILES = 4
SIZE = TILE_SIZE * TILES
OUTPUT = Path(__file__).resolve().parents[1] / 'assets' / 'ground.vox'


def patch_cells(rng, diameter):
    """随机旋转的椭圆斑块，12 个随机半径打散边界；尺寸单位为体素格。"""
    cx, cy = rng.uniform(0, SIZE), rng.uniform(0, SIZE)
    radius = diameter / 2
    aspect, angle = rng.uniform(.55, 1), rng.uniform(0, math.tau)
    cosine, sine = math.cos(angle), math.sin(angle)
    outline = [rng.uniform(.65, 1) for _ in range(12)]
    for y in range(max(0, math.floor(cy-radius)), min(SIZE, math.ceil(cy+radius))):
        for x in range(max(0, math.floor(cx-radius)), min(SIZE, math.ceil(cx+radius))):
            dx, dy = x + .5 - cx, y + .5 - cy
            u, v = (dx*cosine + dy*sine)/radius, (-dx*sine + dy*cosine)/(radius*aspect)
            sector = (math.atan2(v, u) % math.tau) / math.tau * len(outline)
            index, fraction = int(sector), sector % 1
            edge = outline[index] * (1-fraction) + outline[(index+1) % len(outline)] * fraction
            if u*u + v*v <= edge*edge:
                yield y * SIZE + x


def generate_maps():
    """固定种子生成全域列高和独立土色；一半为平地，浅坑与凸起面积均衡。"""
    rng = random.Random(20261017)
    heights = bytearray([4]) * (SIZE*SIZE)
    counts = {3: 0, 5: 0}
    target = len(heights) // 2
    # 小、中、大斑块的数量比例；每格约 5 cm，直径范围对应 0.2～2 m。
    while sum(counts.values()) < target:
        low, high = rng.choices([(4, 10), (10, 20), (20, 40)], weights=[65, 30, 5])[0]
        level = 3 if counts[3] < counts[5] else 5
        for cell in patch_cells(rng, rng.uniform(low, high)):
            if heights[cell] == 4:
                heights[cell] = level
                counts[level] += 1
                if sum(counts.values()) >= target:
                    break
    # 颜色使用独立种子、位置和更宽的斑块，不以起伏轮廓勾边。
    colors = bytearray([3]) * len(heights)
    rng = random.Random(20261018)
    for _ in range(1600):
        color = rng.choice([2, 3, 4, 5])
        for cell in patch_cells(rng, rng.uniform(10, 60)):
            colors[cell] = color
    return heights, colors


def chunk(name, data=b'', children=b''):
    """写入 MagicaVoxel chunk 头与正文，长度均为字节数。"""
    return name.encode('ascii') + struct.pack('<II', len(data), len(children)) + data + children


def dictionary(values):
    """编码场景图属性字典，字符串长度按 UTF-8 字节数计算。"""
    result = struct.pack('<i', len(values))
    for key, value in values.items():
        for item in (key, value):
            encoded = item.encode('utf-8')
            result += struct.pack('<i', len(encoded)) + encoded
    return result


def build_vox():
    """将全图分为 16 个独立模型，打包为一个带定位场景图的 VOX 文件。"""
    heights, colors = generate_maps()
    children = b''
    for ty in range(TILES):
        for tx in range(TILES):
            voxels = bytearray()
            for y in range(TILE_SIZE):
                for x in range(TILE_SIZE):
                    cell = (ty*TILE_SIZE+y)*SIZE + tx*TILE_SIZE+x
                    for z in range(heights[cell]):
                        voxels.extend((x, y, z, colors[cell] if z >= 2 else 1))
            # 保留声明高度 6 与现有竖直偏移，列高 3/4/5 对应 −5/0/+5 cm。
            children += chunk('SIZE', struct.pack('<III', TILE_SIZE, TILE_SIZE, 6))
            children += chunk('XYZI', struct.pack('<I', len(voxels)//4) + voxels)
    children += chunk('nTRN', struct.pack('<i', 0) + dictionary({})
                      + struct.pack('<iiii', 1, -1, -1, 1) + dictionary({}))
    children += chunk('nGRP', struct.pack('<i', 1) + dictionary({})
                      + struct.pack('<i', TILES*TILES)
                      + struct.pack('<16i', *range(2, 34, 2)))
    for model in range(TILES*TILES):
        tx, ty = model % TILES, model // TILES
        node = 2 + model*2
        # 加载器把 VOX 平移 (x,y,z) 转为 Bevy (-x,z,y)；整体仍连续且居中。
        translation = f'{tx*TILE_SIZE-237} {ty*TILE_SIZE-237} 0'
        children += chunk('nTRN', struct.pack('<i', node) + dictionary({'_name': f'ground_{model}'})
                          + struct.pack('<iiii', node+1, -1, -1, 1) + dictionary({'_t': translation}))
        children += chunk('nSHP', struct.pack('<i', node+1) + dictionary({})
                          + struct.pack('<ii', 1, model) + dictionary({}))
    colors = [(87, 65, 43), (102, 79, 52), (111, 85, 57), (119, 93, 63), (126, 99, 68)]
    palette = b''.join(bytes((*rgb, 255)) for rgb in colors)
    palette += bytes((0, 0, 0, 255)) * (256 - len(colors))
    children += chunk('RGBA', palette)
    for index in range(1, 6):
        children += chunk('MATL', struct.pack('<i', index)
                          + dictionary({'_type': '_diffuse', '_rough': '1'}))
    return b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children)


if __name__ == '__main__':
    OUTPUT.write_bytes(build_vox())
    print(f'已生成 {OUTPUT}')
