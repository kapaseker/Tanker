"""用标准库生成 158×158 格土质地块：python3 scripts/generate_ground.py。"""
import math
from pathlib import Path
import random
import struct

SIZE = 158
OUTPUT = Path(__file__).resolve().parents[1] / 'assets' / 'ground.vox'
# 固定种子产生周期波的相位，让高度和色块可以复现。
PHASES = [random.Random(20261016 + i).uniform(0, math.tau) for i in range(4)]


def height(x, y):
    """返回列高 2～6 格；对边使用同一高度，重复铺设时不出现高度裂缝。"""
    a, b = math.tau * x / (SIZE - 1), math.tau * y / (SIZE - 1)
    wave = (math.sin(2 * a + PHASES[0]) * math.cos(2 * b + PHASES[1])
            + 0.4 * math.sin(3 * a + 2 * b + PHASES[2])) / 1.4
    return 4 + round(2 * wave)


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
    """生成带最小场景图及哑光土色调色板的完整 VOX 文件。"""
    voxels = bytearray()
    for x in range(SIZE):
        for y in range(SIZE):
            # 连续色斑避免逐格随机噪点；对边颜色也周期匹配。
            a, b = math.tau * x / (SIZE - 1), math.tau * y / (SIZE - 1)
            shade = round(2 + math.sin(3 * a + PHASES[3]) * math.cos(3 * b))
            for z in range(height(x, y)):
                voxels.extend((x, y, z, 1 + shade if z >= 2 else 1))
    children = chunk('SIZE', struct.pack('<III', SIZE, SIZE, 6))
    children += chunk('XYZI', struct.pack('<I', len(voxels) // 4) + voxels)
    # 场景图是 bevy_vox_scene 加载所必需的；根 -> 组 -> 模型变换 -> 模型。
    children += chunk('nTRN', struct.pack('<i', 0) + dictionary({})
                      + struct.pack('<iiii', 1, -1, -1, 1) + dictionary({}))
    children += chunk('nGRP', struct.pack('<i', 1) + dictionary({}) + struct.pack('<ii', 1, 2))
    children += chunk('nTRN', struct.pack('<i', 2) + dictionary({'_name': 'ground'})
                      + struct.pack('<iiii', 3, -1, -1, 1) + dictionary({}))
    children += chunk('nSHP', struct.pack('<i', 3) + dictionary({})
                      + struct.pack('<ii', 1, 0) + dictionary({}))
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
