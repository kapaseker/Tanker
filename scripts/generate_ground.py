"""用标准库生成 600×600 米场地的地形图（高度 + 土色）：python3 scripts/generate_ground.py。

输出两张紧凑地图（高度图 1200×1200 字节，土色图含文件自带调色板），由 Rust 侧编译成体素风格网格与体素台阶碰撞体：
- assets/terrain/ground/ground.ht：高度值，全图固定为 10（Y=0 米）
- assets/terrain/ground/ground.abd：调色板 + 索引（[数量 u8][数量×3 RGB][1200×1200 索引]，索引指向文件自带调色板）

行列约定：行沿 Bevy X、列沿 Bevy Z；格 (row, col) 的世界中心为
x = (row + 0.5 − SIZE/2) × AUTHOR_RES，z = (col + 0.5 − SIZE/2) × AUTHOR_RES。

全图纯平；花色沿用原来的独立固定种子，不随高度改变。
"""
import math
from pathlib import Path
import random
import struct


def chunk(tag, content=b'', children=b''):
    """组装一个 VOX 分块：4 字节标记 + 内容长度 + 子块长度 + 内容 + 子块。"""
    return tag.encode() + struct.pack('<II', len(content), len(children)) + content + children


def dictionary(entries):
    """编码 VOX 节点字典：数量 u32，每项为（键长 + 键 + 值长 + 值）。"""
    data = struct.pack('<I', len(entries))
    for key, value in entries.items():
        key_bytes = key.encode()
        value_bytes = value.encode()
        data += struct.pack('<I', len(key_bytes)) + key_bytes
        data += struct.pack('<I', len(value_bytes)) + value_bytes
    return data

# 地图花色按 0.5 米采样，地面高度统一为零。
AUTHOR_RES = .5
SIDE_M = 600.
SIZE = int(SIDE_M / AUTHOR_RES)
# 保留高度文件的既有编码：10 表示 Y=0 米。
FLAT = 10
SEED = 20261018
# 土色斑块数量与尺度保持不变，确保花色字节一致。
COLOR_PATCHES = 2500
COLOR_DIAMETER = (6., 30.)

ASSETS = Path(__file__).resolve().parents[1] / 'assets'
# 地形资产根目录：每个子目录是一个地形（<地形名>/ground.ht + ground.abd）。
TERRAIN_DIR = ASSETS / 'terrain' / 'ground'
HEIGHT_OUTPUT = TERRAIN_DIR / 'ground.ht'
ALBEDO_OUTPUT = TERRAIN_DIR / 'ground.abd'
# 土质 5 色（sRGB），索引 2 为底色；调色板随 .abd 文件写出，Rust 侧从文件读取。
PALETTE = ((87, 65, 43), (102, 79, 52), (111, 85, 57), (119, 93, 63), (126, 99, 68))


def region_cells(rng, center, diameter):
    """产出随机旋转椭圆内的格子索引，正弦扰动打散边界；尺寸单位米。"""
    cx, cy = center
    radius = diameter / 2 / AUTHOR_RES
    aspect, angle = rng.uniform(.6, 1), rng.uniform(0, math.tau)
    cos_a, sin_a = math.cos(angle), math.sin(angle)
    phase_u, phase_v = rng.uniform(0, math.tau), rng.uniform(0, math.tau)
    x0, x1 = max(0, math.floor(cx - radius)), min(SIZE, math.ceil(cx + radius))
    y0, y1 = max(0, math.floor(cy - radius)), min(SIZE, math.ceil(cy + radius))
    for y in range(y0, y1):
        for x in range(x0, x1):
            dx, dy = x + .5 - cx, y + .5 - cy
            u, v = (dx*cos_a + dy*sin_a)/radius, (-dx*sin_a + dy*cos_a)/(radius*aspect)
            r2 = u*u + v*v
            if r2 > 1.44:  # 边界扰动上限 1.2 的平方，先粗筛外侧
                continue
            r = math.sqrt(r2)
            if r > 1 + .2*math.sin(2.7*u + phase_u)*math.cos(3.1*v + phase_v):
                continue
            yield y*SIZE + x


def build_colors():
    """固定种子撒独立土色斑块，保留铺平之前的形状、随机序列和调色板。"""
    colors = bytearray([2])*(SIZE*SIZE)
    rng = random.Random(SEED + 1)
    for _ in range(COLOR_PATCHES):
        color = rng.choice([1, 2, 3, 4])
        for cell in region_cells(rng, (rng.uniform(0, SIZE), rng.uniform(0, SIZE)),
                                     rng.uniform(*COLOR_DIAMETER)):
            colors[cell] = color
    return colors


def generate_maps():
    """生成全图 Y=0 的高度图，保留原有独立种子花色。"""
    return bytearray([FLAT]) * (SIZE * SIZE), build_colors()


def encode_height(values):
    """高度值本身就是字节级值（10 为平地），此函数只用于明确输出契约。"""
    return bytes(values)


def write_assets():
    """生成并写出两张地图，返回 (高度字节, 土色字节)。

    土色图布局：[调色板数量 u8][数量×3 字节 RGB][1200×1200 索引字节]。
    """
    heights, colors = generate_maps()
    TERRAIN_DIR.mkdir(parents=True, exist_ok=True)
    HEIGHT_OUTPUT.write_bytes(encode_height(heights))
    albedo = bytes([len(PALETTE)]) + b''.join(bytes(rgb) for rgb in PALETTE) + bytes(colors)
    ALBEDO_OUTPUT.write_bytes(albedo)
    print(f'已生成 {HEIGHT_OUTPUT.name} 与 {ALBEDO_OUTPUT.name}'
          f'（{SIZE}×{SIZE} @{AUTHOR_RES} 米/格，{SIDE_M:.0f} 米见方，调色板 {len(PALETTE)} 色）')
    return heights, colors


if __name__ == '__main__':
    write_assets()
