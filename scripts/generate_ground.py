"""用标准库生成 600×600 米场地的地形图（高度 + 土色）：python3 scripts/generate_ground.py。

输出两张紧凑地图（各 1200×1200 字节），由 Rust 侧编译成体素风格网格与高度场碰撞体：
- assets/ground_height.bin：高度值，10 为平地，0 / 20 对应 −1.0 / +1.0 米（每级 0.1 米）
- assets/ground_albedo.bin：调色板索引（0..4）

行列约定：行沿 Bevy X、列沿 Bevy Z；格 (row, col) 的世界中心为
x = (row + 0.5 − SIZE/2) × AUTHOR_RES，z = (col + 0.5 − SIZE/2) × AUTHOR_RES。

地形分布：少量大片区 + 若干小片区，坑洼与坡道混合；非平地覆盖 25%–45%，其余为绝对平地。
"""
import math
from pathlib import Path
import random

# 地图按 0.5 米采样作者化，渲染时由 Rust 量化成 0.1 米的体素台阶。
AUTHOR_RES = .5
SIDE_M = 600.
SIZE = int(SIDE_M / AUTHOR_RES)
HALF = SIZE / 2

# 起伏幅度与量化单位：±1.0 米 = 10 级，每级 0.1 米；平地值为 10。
VOXEL = .1
RELIEF_MAX = 1.
LEVEL_MAX = int(RELIEF_MAX / VOXEL)
FLAT = LEVEL_MAX

SEED = 20261018
SEED_TRIES = 24
# 覆盖率目标与硬上限（上限用于及时收手，避免落进重抽区间）。
COVER_TARGET = (.26, .34)
COVER_LIMIT = .38
COVER_RANGE = (.25, .45)
# 出生点保护半径（米）：玩家出生处必须绝对平坦。
SPAWN_PAD_M = 8.
# 片区尺度（米）、数量上限与调色板斑块数量；大片区刻意少而大，保证大片留白。
BIG_DIAMETER = (140., 260.)
SMALL_DIAMETER = (12., 32.)
BIG_REGIONS_MAX = 10
SMALL_REGIONS_MAX = 30
BIG_REGIONS_CHANCE = .6
COLOR_PATCHES = 2500
COLOR_DIAMETER = (6., 30.)

ASSETS = Path(__file__).resolve().parents[1] / 'assets'
HEIGHT_OUTPUT = ASSETS / 'ground_height.bin'
ALBEDO_OUTPUT = ASSETS / 'ground_albedo.bin'
# 与 Rust 侧 GROUND_PALETTE 同步（土质 5 色，索引 2 为底色）。
PALETTE = ((87, 65, 43), (102, 79, 52), (111, 85, 57), (119, 93, 63), (126, 99, 68))


def region_cells(rng, center, diameter, direction):
    """随机旋转的椭圆片区，边界用双向正弦扰动打散；尺寸单位为米。

    产出 (cell, r, tau, side)：r 为归一化半径（0 中心、1 边界），tau 为沿 direction
    归一化投影（0/1 为片区两端），side 为垂直方向的归一化距离（边缘收边用）。
    """
    cx, cy = center
    radius = diameter / 2 / AUTHOR_RES
    aspect, angle = rng.uniform(.6, 1), rng.uniform(0, math.tau)
    cos_a, sin_a = math.cos(angle), math.sin(angle)
    phase_u, phase_v = rng.uniform(0, math.tau), rng.uniform(0, math.tau)
    cos_d, sin_d = math.cos(direction), math.sin(direction)
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
            tau = .5 + .5*(dx*cos_d + dy*sin_d)/radius
            yield y*SIZE + x, min(1., r), min(1., max(0., tau)), min(1., abs(v))


def apply_region(values, rng, diameter, pit, nonflat):
    """把一个片区写进高度图；返回更新后的非平地格数。

    坑洼用余弦径向 falloff 从边缘的 0 平滑降到中心 −RELIEF_MAX；坡道沿随机方向
    做单向单调坡（落差 ≤ 2×RELIEF_MAX），只在垂直于坡向的两侧收边，使坡顶与坡底
    真正达到 ±RELIEF_MAX。片区重叠时取形变更强的一方，避免叠加出超深坑或超高山包。
    """
    center = (rng.uniform(0, SIZE), rng.uniform(0, SIZE))
    direction = rng.uniform(0, math.tau)
    added = 0
    for cell, r, tau, side in region_cells(rng, center, diameter, direction):
        falloff = .5 + .5*math.cos(math.pi*(r if pit else side))
        relief = -falloff if pit else (1 - 2*tau)*falloff
        level = max(-LEVEL_MAX, min(LEVEL_MAX, round(relief*RELIEF_MAX/VOXEL)))
        # 只接受更强形变，因此这里不会把非平地写回平地。
        if abs(level) <= abs(values[cell] - FLAT):
            continue
        if values[cell] == FLAT:
            added += 1
        values[cell] = FLAT + level
    return nonflat + added


def flat_pad(values, nonflat):
    """出生点保护：中心半径 SPAWN_PAD_M 内强制平地，返回更新后的非平地格数。"""
    radius = SPAWN_PAD_M / AUTHOR_RES
    low, high = math.floor(HALF - radius), math.ceil(HALF + radius)
    removed = 0
    for y in range(low, high):
        for x in range(low, high):
            if math.hypot(x + .5 - HALF, y + .5 - HALF) > radius:
                continue
            cell = y*SIZE + x
            if values[cell] != FLAT:
                values[cell] = FLAT
                removed += 1
    return nonflat - removed


def component_stats(values):
    """按四邻接扫描非平坦区域，返回 [(格数, (x0, x1, y0, y1), 触及的边)]。"""
    visited = bytearray(len(values))
    components = []
    for start, value in enumerate(values):
        if value == FLAT or visited[start]:
            continue
        stack, size = [start], 0
        x0 = y0 = SIZE
        x1 = y1 = -1
        edges = set()
        visited[start] = 1
        while stack:
            cell = stack.pop()
            x, y = cell % SIZE, cell // SIZE
            size += 1
            x0, x1 = min(x0, x), max(x1, x)
            y0, y1 = min(y0, y), max(y1, y)
            if x == 0:
                edges.add('left')
            if x == SIZE - 1:
                edges.add('right')
            if y == 0:
                edges.add('top')
            if y == SIZE - 1:
                edges.add('bottom')
            for nx, ny in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
                if 0 <= nx < SIZE and 0 <= ny < SIZE:
                    neighbor = ny*SIZE + nx
                    if not visited[neighbor] and values[neighbor] != FLAT:
                        visited[neighbor] = 1
                        stack.append(neighbor)
        components.append((size, (x0, x1, y0, y1), edges))
    return components


def build_colors():
    """固定种子撒独立土色斑块；比地形更宽，不勾起伏轮廓。"""
    colors = bytearray([2])*(SIZE*SIZE)
    rng = random.Random(SEED + 1)
    for _ in range(COLOR_PATCHES):
        color = rng.choice([1, 2, 3, 4])
        for cell, *_ in region_cells(rng, (rng.uniform(0, SIZE), rng.uniform(0, SIZE)),
                                     rng.uniform(*COLOR_DIAMETER), 0.):
            colors[cell] = color
    return colors


def generate_maps():
    """固定种子生成高度图与土色图；不满足覆盖率或连通性约束时换种子重抽。"""
    for attempt in range(SEED_TRIES):
        rng = random.Random(SEED + attempt)
        values = bytearray([FLAT])*(SIZE*SIZE)
        nonflat, target = 0, rng.uniform(*COVER_TARGET)
        big = small = 0
        # 大小片区交错铺开，凑够目标覆盖率即收手；上限用于兜底防呆。
        while (nonflat/len(values) < target
               and (big < BIG_REGIONS_MAX or small < SMALL_REGIONS_MAX)):
            if big < BIG_REGIONS_MAX and (small >= SMALL_REGIONS_MAX
                                          or rng.random() < BIG_REGIONS_CHANCE):
                diameter, big = rng.uniform(*BIG_DIAMETER), big + 1
            else:
                diameter, small = rng.uniform(*SMALL_DIAMETER), small + 1
            nonflat = apply_region(values, rng, diameter, rng.random() < .5, nonflat)
            if nonflat/len(values) >= COVER_LIMIT:
                break
        nonflat = flat_pad(values, nonflat)
        ratio = nonflat/len(values)
        if not COVER_RANGE[0] <= ratio <= COVER_RANGE[1]:
            continue
        components = component_stats(values)
        # 没有任何地形片区横穿场地的相对两边，保证地图留有绕行空间。
        if any({'left', 'right'} <= edges or {'top', 'bottom'} <= edges
               for _, _, edges in components):
            continue
        print(f'地形：非平地 {ratio:.1%}，片区 {len(components)} 个'
              f'（大 {big} / 小 {small}），种子偏移 {attempt}')
        return values, build_colors()
    raise RuntimeError(f'{SEED_TRIES} 次重抽仍未生成满足约束的地形')


def encode_height(values):
    """高度值本身就是字节级值（10 为平地），此函数只用于明确输出契约。"""
    return bytes(values)


def write_assets():
    """生成并写出两张地图，返回 (高度字节, 土色字节)。"""
    heights, colors = generate_maps()
    HEIGHT_OUTPUT.write_bytes(encode_height(heights))
    ALBEDO_OUTPUT.write_bytes(bytes(colors))
    print(f'已生成 {HEIGHT_OUTPUT.name} 与 {ALBEDO_OUTPUT.name}'
          f'（{SIZE}×{SIZE} @{AUTHOR_RES} 米/格，{SIDE_M:.0f} 米见方）')
    return heights, colors


if __name__ == '__main__':
    write_assets()
