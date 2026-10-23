"""生成 12 帧逐帧爆炸动画 explosion.vox；运行：python scripts/generate_explosion.py。

一个 nSHP 列出 12 个 model（MagicaVoxel 动画格式），bevy_vox_scene 检测到
多模型后自动挂 FlipbookPlayer 逐帧播放。几何为逐帧外扩的空心球壳：
亮黄起爆 → 橙红 → 灰烟稀疏消散。
"""
from pathlib import Path
import struct
from generate_ground import chunk, dictionary

OUTPUT = Path(__file__).resolve().parents[1] / 'assets' / 'effects' / 'explosion.vox'

# 帧数与单帧时长（1/12 秒）须与 src/gameplay/combat/explosion.rs 的 EXPLOSION_FRAME_RATE 对齐；
# 实际存活时长以加载后 FlipbookFrame 的数量推导，避免两处漂移。
FRAMES = 12
# 作者化基准边长40格（4米）；运行时 EXPLOSION_SCALE=2 放大至8米，适配新车体。
SIZE = 40
# 球壳半径从起爆火球扩到最大烟团，单位：格。
SHELL_START = 4.0
SHELL_END = 20.0
# 球壳厚度，单位：格；保持空心以减少体素量。
SHELL_THICKNESS = 3.0
# 立方体几何中心；VOX 网格原点在角落，对称球心取 (SIZE-1)/2。
CENTER = (SIZE - 1) / 2
# 后 1/3 帧开始稀疏化（烟尘消散感）；阈值与确定性抖动共同保证可复现。
THIN_FROM = 0.66

# 关键色：起爆亮黄 → 橙 → 暗红 → 灰烟；每帧在相邻关键色间线性插值。
STOPS = [(255, 230, 90), (255, 150, 40), (190, 60, 25), (80, 75, 72)]


def frame_color(index):
    """第 index 帧的调色板主色，索引即调色板号（1 起）。"""
    t = index / (FRAMES - 1) * (len(STOPS) - 1)
    low = int(t)
    high = min(low + 1, len(STOPS) - 1)
    mix = t - low
    return tuple(round(a + (b - a) * mix) for a, b in zip(STOPS[low], STOPS[high]))


def frame_voxels(index):
    """第 index 帧的体素列表 [(x, y, z, 调色板号)]，按坐标排序保证可复现。"""
    t = index / (FRAMES - 1)
    radius = SHELL_START + t * (SHELL_END - SHELL_START)
    voxels = []
    for x in range(SIZE):
        for y in range(SIZE):
            for z in range(SIZE):
                distance = ((x - CENTER) ** 2 + (y - CENTER) ** 2 + (z - CENTER) ** 2) ** 0.5
                if abs(distance - radius) > SHELL_THICKNESS / 2:
                    continue
                # 尾段按确定性哈希丢弃约 2/3 体素，避免整团烟同时消失。
                if t > THIN_FROM and (x + 2 * y + 3 * z) % 3:
                    continue
                voxels.append((x, y, z, index + 1))
    return sorted(voxels)


def build_vox():
    """组装含 12 帧模型的动画 VOX；mesh 默认以模型中心为原点，无需场景平移。"""
    colors = [frame_color(i) for i in range(FRAMES)]
    children = b''
    for index in range(FRAMES):
        voxels = frame_voxels(index)
        data = b''.join(bytes(v) for v in voxels)
        children += chunk('SIZE', struct.pack('<III', SIZE, SIZE, SIZE))
        children += chunk('XYZI', struct.pack('<I', len(voxels)) + data)
    # 场景结构与 generate_enemy.py 同构：root → group → 具名 transform → shape。
    children += chunk('nTRN', struct.pack('<i', 0) + dictionary({})
                      + struct.pack('<iiii', 1, -1, -1, 1) + dictionary({}))
    children += chunk('nGRP', struct.pack('<i', 1) + dictionary({}) + struct.pack('<ii', 1, 2))
    children += chunk('nTRN', struct.pack('<i', 2) + dictionary({'_name': 'explosion'})
                      + struct.pack('<iiii', 3, -1, -1, 1) + dictionary({}))
    # nSHP 布局（dot_vox 解析顺序）：id + 字典 + 模型数 + (模型 id + 字典) × N。
    shape_models = b''.join(struct.pack('<i', i) + dictionary({}) for i in range(FRAMES))
    children += chunk('nSHP', struct.pack('<i', 3) + dictionary({})
                      + struct.pack('<i', FRAMES) + shape_models)
    palette = b''.join(bytes((*rgb, 255)) for rgb in colors)
    children += chunk('RGBA', palette + bytes((0, 0, 0, 255)) * (256-len(colors)))
    for index in range(1, len(colors)+1):
        children += chunk('MATL', struct.pack('<i', index)
                          + dictionary({'_type': '_diffuse', '_rough': '1'}))
    return b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children)


if __name__ == '__main__':
    OUTPUT.write_bytes(build_vox())
    print(f'已生成 {OUTPUT}')
