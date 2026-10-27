"""生成灰色梯形混凝土墩；0.1米/格，VOX X为厚度、Y为长度、Z为高度。"""
from pathlib import Path
import struct
from generate_ground import chunk, dictionary

OUTPUT = Path(__file__).resolve().parents[1] / 'assets' / 'obstacles' / 'concrete' / 'obstacle.vox'
# 底厚15格、顶厚9格、长80格、高80格（长与高各 8 米）；与 gameplay::obstacle 的凸碰撞体一致。
SIZE = (15, 80, 80)
PALETTE = ((137, 139, 137), (128, 130, 128), (146, 148, 146), (112, 114, 112))


def build_vox():
    """分层收窄实心墩体；固定坐标哈希形成少量斑点，接缝只改色不挖空。"""
    cells = []
    for z in range(SIZE[2]):
        inset = int(3 * z / (SIZE[2] - 1) + .5)
        for x in range(inset, SIZE[0] - inset):
            for y in range(SIZE[1]):
                noise = (x * 73856093 ^ y * 19349663 ^ z * 83492791) % 101
                color = 2 if noise < 5 else 3 if noise > 95 else 1
                # 接缝色带按约 30% 与 64% 高度放置，随总高缩放保持三段比例。
                if z in (24, 51):
                    color = 4
                cells.append(bytes((x, y, z, color)))
    children = chunk('SIZE', struct.pack('<III', *SIZE))
    children += chunk('XYZI', struct.pack('<I', len(cells)) + b''.join(cells))
    children += chunk('nTRN', struct.pack('<i', 0) + dictionary({})
                      + struct.pack('<iiii', 1, -1, -1, 1) + dictionary({}))
    children += chunk('nGRP', struct.pack('<i', 1) + dictionary({}) + struct.pack('<ii', 1, 2))
    children += chunk('nTRN', struct.pack('<i', 2) + dictionary({'_name': 'concrete'})
                      + struct.pack('<iiii', 3, -1, -1, 1) + dictionary({}))
    children += chunk('nSHP', struct.pack('<i', 3) + dictionary({})
                      + struct.pack('<ii', 1, 0) + dictionary({}))
    palette = b''.join(bytes((*rgb, 255)) for rgb in PALETTE)
    children += chunk('RGBA', palette + bytes((0, 0, 0, 255)) * (256 - len(PALETTE)))
    for index in range(1, len(PALETTE) + 1):
        children += chunk('MATL', struct.pack('<i', index)
                          + dictionary({'_type': '_diffuse', '_rough': '1'}))
    return b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children)


if __name__ == '__main__':
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_bytes(build_vox())
    print(f'已生成 {OUTPUT}')
