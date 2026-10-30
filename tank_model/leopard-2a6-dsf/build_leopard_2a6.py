"""豹 2A6 精细体素作者源（1 厘米/格，独立工作区）。

坐标：VOX +X 车首、+Y 车体左侧→右侧、+Z 向上，地面 Z=0。
外形尺寸取自 docs/leopard-a6-dimensions.md：
  车体长 7.66、全长 10.98、宽 3.76、炮塔顶 2.64、炮轴 2.08、甲板 1.60 米。
外观依据 docs/references/leopard-2a6-style-v2-2026-10-30 的 01/03/04/05 图：
  楔形炮塔、分段裙板、七负重轮 + 前后端轮、双风扇发动机甲板、尾部宽格栅、
  两侧各八联烟幕发射器、车顶指挥塔/装填手舱盖/周视镜/机枪/双天线。
体素语言照参考图：每块装甲板带 1～2 厘米暗橄榄包边，做出砖块式的硬边观感。

尺寸文档把“最高点 2.70 米”限定为舱盖/观瞄/机枪，天线一栏注明是旧画布裁切值
而非实测；本模型车顶附件压到 2.70，双天线按参考图比例单独做到 3.50 米，
画布高度取 3.80 米。车首取浅首上斜面 + 竖直首前板的组合：首前板顶 1.34 米
才放得下参考图 03 里的前灯与牵引钩，若沿用 1.04 米会把灯组顶出装甲面。

运行：python -X utf8 models/leopard-2a6/build_leopard_2a6.py [--render]
输出：leopard_2a6.vox / .vox.meta / report.txt / preview/*.png
"""
from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
# 复用工程既有 VOX 写出与分块规划，避免重复实现 MagicaVoxel 二进制格式。
sys.path.insert(0, str(HERE.parents[1] / 'scripts'))
import subdivide_vox as sv  # noqa: E402

# ---------------------------------------------------------------------------
# 画布与调色板
# ---------------------------------------------------------------------------
CANVAS = (1440, 380, 380)     # 14.40 × 3.80 × 3.80 米
OUT_PATH = HERE / 'leopard_2a6.vox'

PALETTE = [
    (0, 0, 0, 0),                 # 0 空
    (152, 160, 104, 255),         # 1 主橄榄（参考图的沙橄榄基色）
    (86, 94, 58, 255),            # 2 暗橄榄（包边、面板缝）
    (26, 28, 24, 255),            # 3 近黑（轮胎、枪管根部、舱口）
    (58, 60, 52, 255),            # 4 履带橡胶
    (168, 170, 152, 255),         # 5 浅灰钢（螺栓、齿圈）
    (214, 150, 70, 255),          # 6 琥珀灯片
    (198, 62, 48, 255),           # 7 红灯片
    (44, 56, 66, 255),            # 8 玻璃/观瞄窗
    (128, 134, 84, 255),          # 9 交替面板橄榄
    (178, 170, 128, 255),         # 10 沙色（磨损、车灯外壳）
    (124, 126, 114, 255),         # 11 履带齿钢
    (72, 80, 50, 255),            # 12 深绿面板
    (214, 212, 198, 255),         # 13 灯片（参考图车辆大灯的浅色玻璃）
]
(OLIVE, OLIVE_D, BLACK, RUBBER, STEEL, AMBER, RED, GLASS, OLIVE_M, SAND,
 TRACK_STEEL, OLIVE_DD, LAMP) = range(1, 14)

CX, CY, CZ = CANVAS
grid = np.zeros(CANVAS, dtype=np.uint8)


# ---------------------------------------------------------------------------
# 基础体素操作
# ---------------------------------------------------------------------------
def box(x, y, z, color, bevel=0, rim=0):
    """填半开盒；z 直接为画布高度（地面 0）。

    rim>0 时在顶面外缘留 1 层暗橄榄包边，做出参考图那种“亮板面 + 深色轮廓”
    的砖块体素观感。盒子任一方向过薄时自动跳过。
    """
    (x0, x1), (y0, y1), (z0, z1) = x, y, z
    if not (0 <= x0 < x1 <= CX and 0 <= y0 < y1 <= CY and 0 <= z0 < z1 <= CZ):
        raise ValueError(f'盒越界：{x},{y},{z}')
    view = grid[x0:x1, y0:y1, z0:z1]
    view[...] = color
    if bevel:   # 只切 X/Y 平面四角，用于灯罩、灯箱一类圆角小件
        ai = np.arange(x1 - x0)[:, None]
        bj = np.arange(y1 - y0)[None, :]
        corners = np.minimum(ai, x1 - x0 - 1 - ai) + np.minimum(bj, y1 - y0 - 1 - bj)
        view[corners < bevel] = OLIVE_D
    if rim and min(x1 - x0, y1 - y0, z1 - z0) > 2 * rim + 1:
        # 包边只画顶面外缘一圈。整个外壳都涂暗时，渲染出的表面全是暗色，
        # 板件会变成一个“洞”，参考图的亮面 + 深色轮廓反而消失。
        view[:rim, :, -1] = OLIVE_D
        view[-rim:, :, -1] = OLIVE_D
        view[:, :rim, -1] = OLIVE_D
        view[:, -rim:, -1] = OLIVE_D


def disc_xy(cx, cy, r, z, color):
    """XY 平面竖圆柱：风扇、舱盖。"""
    xs = np.arange(cx - r, cx + r + 1)
    ys = np.arange(cy - r, cy + r + 1)
    m = (xs - cx)[:, None] ** 2 + (ys - cy)[None, :] ** 2 <= r * r
    v = grid[cx - r:cx + r + 1, cy - r:cy + r + 1, z[0]:z[1]]
    v[np.broadcast_to(m[:, :, None], v.shape)] = color


def ring_xy(cx, cy, r_out, r_in, z, color):
    """XY 平面圆环：风扇护罩、座圈、指挥塔。"""
    xs = np.arange(cx - r_out, cx + r_out + 1)
    ys = np.arange(cy - r_out, cy + r_out + 1)
    d2 = (xs - cx)[:, None] ** 2 + (ys - cy)[None, :] ** 2
    m = (d2 <= r_out * r_out) & (d2 >= r_in * r_in)
    v = grid[cx - r_out:cx + r_out + 1, cy - r_out:cy + r_out + 1, z[0]:z[1]]
    v[np.broadcast_to(m[:, :, None], v.shape)] = color


def plate(x, y, z, color=OLIVE, edge=2):
    """带包边板件：先铺暗色底，再内缩 edge 层铺主色。

    用于裙板、装甲块这类厚度只有几厘米的板：`box(rim=1)` 会把 6 厘米厚板的
    整个外表面涂暗，读不出板面，这里只在板缘留一圈暗色轮廓。
    """
    box(x, y, z, OLIVE_D)
    box((x[0] + edge, x[1] - edge), (y[0], y[1]), (z[0] + edge, z[1] - edge), color)


def seam_rect(x0, x1, y0, y1, z, w=2, color=OLIVE_D):
    """在水平顶面画一圈宽 w 的矩形板缝（单层体素）。"""
    box((x0, x1), (y0, y0 + w), (z, z + 1), color)
    box((x0, x1), (y1 - w, y1), (z, z + 1), color)
    box((x0, x0 + w), (y0, y1), (z, z + 1), color)
    box((x1 - w, x1), (y0, y1), (z, z + 1), color)


def disc_xz(cx, cz, y, r, color, r_in=None):
    """XZ 平面横圆柱（负重轮）：r_in 给定时只画圆环。"""
    xs = np.arange(cx - r, cx + r + 1)
    zs = np.arange(cz - r, cz + r + 1)
    d2 = (xs - cx)[:, None] ** 2 + (zs - cz)[None, :] ** 2
    m = d2 <= r * r
    if r_in is not None:
        m &= d2 >= r_in * r_in
    v = grid[cx - r:cx + r + 1, y[0]:y[1], cz - r:cz + r + 1]
    v[np.broadcast_to(m[:, None, :], v.shape)] = color


def cyl_x(x, cy, cz, r, color):
    """沿 X 轴圆柱：炮管段、前灯透镜。"""
    ys = np.arange(cy - r, cy + r + 1)
    zs = np.arange(cz - r, cz + r + 1)
    m = (ys - cy)[:, None] ** 2 + (zs - cz)[None, :] ** 2 <= r * r
    v = grid[x[0]:x[1], cy - r:cy + r + 1, cz - r:cz + r + 1]
    v[np.broadcast_to(m[None, :, :], v.shape)] = color


def dot(x, y, z, color, size=2):
    """小细节（螺栓、镜片、窗孔）：以格点为中心铺 size 见方。"""
    box((x - size // 2, x - size // 2 + size), (y - size // 2, y - size // 2 + size),
        (z - size // 2, z - size // 2 + size), color)


def stepped(x, x0, x1, top0, top1, step=8):
    """把连续斜面量化成 step 厘米台阶：1 厘米体素直接铺斜面会出现细碎锯齿。"""
    if x <= x0:
        return top0
    if x >= x1:
        return top1
    rise = (top0 - top1) * (x - x0) // (x1 - x0)
    return top0 - ((rise + step - 1) // step) * step


# ---------------------------------------------------------------------------
# 关键尺寸（厘米）
# ---------------------------------------------------------------------------
HULL_X = (338, 1104)          # 车尾装甲后缘 → 车首前缘 = 7.66 米
GUN_X = (950, 1436)           # 炮口端面；全长 10.98 米
WIDTH = (2, 378)              # 3.76 米
DECK_Z = 160                  # 甲板 1.60 米
ROOF = 264                    # 炮塔屋顶 2.64 米
TOP_LIMIT = 270               # 舱盖/观瞄/机枪上限 2.70 米
ANTENNA_TOP = 350             # 天线顶 3.50 米：参考图比例估算，不受 2.70 附件上限约束
BORE_Z = 208                  # 炮轴 2.08 米
NOSE_Z = 134                  # 首前板顶：浅首上斜面收头，正好容纳前灯与牵引钩
BELLY_Z = 56                  # 车底

HULL_Y = (74, 306)            # 两履带之间的车体
FENDER_Y = ((8, 74), (306, 372))
SKIRT_Y = ((2, 8), (372, 378))
TRACK_Y = ((8, 68), (312, 372))
WHEEL_Y = ((18, 58), (322, 362))

ROAD_WHEEL_X = (445, 534, 623, 712, 801, 890, 979)
SPROCKET_X = 368              # 主动轮在后（尺寸表末段角色修正：后主动、前诱导）
IDLER_X = 1070                # 诱导轮在前
WHEEL_Z = 44                  # 轴高 0.42 米上取整；轮胎底面 4 格骑在下段履带上
ROAD_WHEEL_R = 40             # 负重轮直径 0.80 米
END_WHEEL_R = 25              # 端轮直径 0.50 米
TRACK_TOP_Z = 84              # 履带闭环顶段下表面；顶面 0.92、凸耳顶 0.96 米
TRACK_THICK = 8

TURRET_REAR = 500             # 尾舱后缘
BUSTLE_END = 660              # 尾舱 → 主体扩张起点
TURRET_MAIN = (702, 902)      # 炮塔主体（平面最宽段，3.00 米）
TURRET_FRONT = 950            # 含前楔、不含炮管，自车尾 4.50 米
BUSTLE_TOP = 250              # 尾舱顶：比屋顶低 14 厘米，形成参考图的落差分
TURRET_C = 190                # 炮塔横向中心（车宽 2..378 的中点）
FRONT_CHAMFER = 520           # 尾舱后端倒角结束处


def hull_top(x):
    """车体顶部高度：平甲板 → 浅首上斜面 → 竖直首前板。"""
    if x < 1024:
        return DECK_Z
    return stepped(x, 1024, HULL_X[1], DECK_Z, NOSE_Z)


# ---------------------------------------------------------------------------
# 车体
# ---------------------------------------------------------------------------
def build_hull():
    # 下车体：逐列取顶形成首上斜面；底边收口压暗。
    for x in range(HULL_X[0], HULL_X[1]):
        box((x, x + 1), HULL_Y, (BELLY_Z, hull_top(x)), OLIVE)
    box(HULL_X, HULL_Y, (BELLY_Z, BELLY_Z + 4), OLIVE_D)
    # 车尾立面保持主色：整面压暗会让尾部读成一整块黑墙，格栅细节全丢失。
    box((HULL_X[0], HULL_X[0] + 3), HULL_Y, (BELLY_Z, 94), OLIVE)
    box((HULL_X[0], HULL_X[0] + 3), HULL_Y, (94, 150), OLIVE_D)

    # 甲板与翼子板：甲板覆盖履带之间的车体，翼子板低 8 厘米压在履带上。
    for x in range(HULL_X[0], HULL_X[1]):
        top = hull_top(x)
        box((x, x + 1), HULL_Y, (top - 10, top), OLIVE)
    box((HULL_X[0], HULL_X[1]), HULL_Y, (DECK_Z - 10, DECK_Z), OLIVE)
    for (y0, y1) in FENDER_Y:
        edge = y0 if y0 < 100 else y1 - 2      # 翼子板外侧压条贴车体最外缘
        for x in range(HULL_X[0], HULL_X[1]):
            top = hull_top(x)
            box((x, x + 1), (y0, y1), (top - 10, top - 2), OLIVE)
            # 外缘 2 厘米压条随首上斜面同步下降，铆钉列压在压条上。
            box((x, x + 1), (edge, edge + 2), (top - 10, top - 2), OLIVE_D)
        for x in range(356, 1098, 30):
            top = hull_top(x)
            dot(x, edge + 1, top - 7, STEEL, 3)

    # 甲板分块缝：横缝三处、纵缝两条，压成 1 格深的面板缝。
    for x in (400, 960):
        box((x, x + 2), HULL_Y, (DECK_Z - 1, DECK_Z), OLIVE_D)
    for y in (86, 294):
        box((HULL_X[0], 1004), (y, y + 2), (DECK_Z - 1, DECK_Z), OLIVE_D)
    # 炮塔前方的大板件：带暗色包边的甲板分区，参考图前甲板的做法。
    seam_rect(970, 1020, 80, 300, DECK_Z - 1, w=2)
    # 首上斜面两侧压暗边，读出斜面与平甲板的转接。
    for x in range(1024, HULL_X[1]):
        top = hull_top(x)
        for y in (HULL_Y[0] + 2, HULL_Y[1] - 4):
            box((x, x + 1), (y, y + 2), (top - 1, top), OLIVE_D)

    # 前甲板检修口：炮塔前方的一块带包边盖板，避开驾驶员舱盖。
    box((966, 1018), (150, 290), (DECK_Z, DECK_Z + 4), OLIVE_M, rim=1)

    # 驾驶员舱盖（车体左侧前甲板）：低凸圆盖 + 盖顶压环 + 前方观察窗。
    disc_xy(986, 104, 26, (DECK_Z, DECK_Z + 3), OLIVE_D)
    disc_xy(986, 104, 21, (DECK_Z + 3, DECK_Z + 5), OLIVE)
    disc_xy(986, 104, 7, (DECK_Z + 5, DECK_Z + 6), OLIVE_DD)   # 盖顶压环，不挖黑孔
    box((954, 968), (94, 114), (DECK_Z, DECK_Z + 6), OLIVE_D)
    box((968, 970), (98, 110), (DECK_Z + 2, DECK_Z + 5), GLASS)

    # 前灯：装在竖直首前板上的圆形大灯（外侧）+ 防空灯盒（内侧），对齐参考图 03。
    # 灯组整体压在前板顶（1.34 米）以下，避免高出装甲面形成悬空感。
    for yc in (48, 312):
        sign = 1 if yc < 100 else -1
        box((1076, HULL_X[1]), (yc - 16, yc + 16), (98, 130), OLIVE_D, rim=1)
        cyl_x((1096, HULL_X[1]), yc, 116, 13, OLIVE_D)     # 圆灯座外圈
        cyl_x((1099, HULL_X[1]), yc, 116, 10, SAND)        # 暖色灯罩
        cyl_x((1101, HULL_X[1]), yc, 116, 4, LAMP)         # 高亮灯芯
        # 防空灯盒：紧贴大灯内侧，暗壳 + 竖条玻璃。
        b0, b1 = sorted((yc + sign * 20, yc + sign * 40))
        g0, g1 = sorted((yc + sign * 26, yc + sign * 31))
        box((1084, HULL_X[1]), (b0, b1), (100, 126), OLIVE_D, rim=1)
        box((1100, HULL_X[1]), (g0, g1), (104, 122), GLASS)

    # 车首牵引钩两处：首前板下缘的小钩体 + 钢色销轴。
    for yc in (116, 264):
        box((1090, HULL_X[1]), (yc - 10, yc + 10), (62, 88), OLIVE_D)
        box((1100, HULL_X[1]), (yc - 5, yc + 5), (70, 80), OLIVE_DD)
        box((1102, HULL_X[1]), (yc - 2, yc + 2), (72, 78), STEEL)

    # 侧裙板：7 段，上缘贴翼子板下沿（1.50 米），下缘 1.04 米。
    # 每段都是带 2 厘米暗色包边的独立板，段间再留暗缝，复刻参考图的裙板分块。
    for (sy0, sy1) in SKIRT_Y:
        for i in range(7):
            x0 = 348 + i * 106
            x1 = min(x0 + 102, 1098)
            plate((x0, x1), (sy0, sy1), (104, DECK_Z - 10), edge=2)
            box((x1, x1 + 4), (sy0, sy1), (104, DECK_Z - 10), OLIVE_D)   # 段间暗缝
            # 安装块：参考图每段裙板上下缘各有一排灰色钢制卡扣。
            for bx in (x0 + 12, x0 + 46, x1 - 20):
                for zz in (108, DECK_Z - 20):
                    box((bx, bx + 8), (sy0, sy1), (zz, zz + 6), STEEL)

    # 发动机甲板：整片带包边甲板 + 两具圆风扇（黑腔 + 径向格栅 + 轮毂）。
    box((344, 486), (20, 360), (DECK_Z - 2, DECK_Z + 2), OLIVE_M, rim=1)
    for cy in (126, 254):
        ring_xy(420, cy, 50, 42, (DECK_Z + 2, DECK_Z + 5), OLIVE_D)   # 风扇护罩
        disc_xy(420, cy, 42, (DECK_Z + 2, DECK_Z + 4), BLACK)         # 罩内黑腔
        for ang in range(0, 360, 15):
            a = math.radians(ang)
            for t in range(9, 41):
                px = int(round(420 + t * math.cos(a)))
                py = int(round(cy + t * math.sin(a)))
                grid[px, py, DECK_Z + 3] = TRACK_STEEL                # 径向格栅
        ring_xy(420, cy, 10, 4, (DECK_Z + 3, DECK_Z + 5), OLIVE_D)    # 轮毂
        disc_xy(420, cy, 4, (DECK_Z + 4, DECK_Z + 5), STEEL)
    # 风扇与车尾之间的横向排气百叶。
    box((342, 368), (76, 304), (DECK_Z, DECK_Z + 2), BLACK)
    for y in range(80, 302, 10):
        box((342, 368), (y, y + 6), (DECK_Z + 1, DECK_Z + 2), OLIVE_D)

    # 尾部宽格栅：暗色百叶 + 五分区竖肋 + 顶沿压边（百叶压在最外 2 厘米面上）。
    box((338, 340), (80, 300), (96, 146), BLACK)
    for z in range(98, 144, 8):
        box((338, 340), (84, 296), (z, z + 5), OLIVE_D)
    for i in range(1, 5):
        y = 74 + i * (306 - 74) // 5
        box((338, 341), (y - 3, y + 3), (96, 146), OLIVE)
    box((338, 342), (74, 306), (146, 150), OLIVE)

    # 尾灯组：外侧琥珀/红色横条透镜 + 内侧黑色圆灯，左右镜像。
    # 黑色圆灯必须画在尾端面（x=338）上：画在灯箱内部会被灯箱表面挡住。
    for mirror in (False, True):
        y0 = 30 if not mirror else 312
        box((338, 350), (y0, y0 + 42), (112, 148), OLIVE_D, bevel=2, rim=1)
        lens = (y0 + 5, y0 + 15) if not mirror else (y0 + 30, y0 + 40)
        box((338, 340), lens, (124, 141), AMBER)
        red = (y0 + 17, y0 + 27) if not mirror else (y0 + 18, y0 + 28)
        box((338, 340), red, (124, 141), RED)
        y_round = y0 + 36 if not mirror else y0 + 8
        cyl_x((338, 341), y_round, 132, 6, BLACK)
        cyl_x((338, 341), y_round, 132, 2, STEEL)

    # 后牵引钩两处 + 中央拖钩：暗色钩体 + 钢色销轴，不做整块黑面。
    for yc in (120, 260):
        box((338, 350), (yc - 12, yc + 12), (58, 88), OLIVE_D)
        box((338, 342), (yc - 5, yc + 5), (66, 80), OLIVE_DD)
        box((338, 340), (yc - 2, yc + 2), (70, 76), STEEL)
    box((338, 352), (180, 200), (60, 86), OLIVE_D)
    box((338, 342), (184, 196), (66, 80), OLIVE_DD)
    box((338, 340), (187, 193), (70, 76), STEEL)


# ---------------------------------------------------------------------------
# 行走装置
# ---------------------------------------------------------------------------
def track_rim():
    """梯形闭环外轮廓：下段贴地、顶段 0.84 米、两端 10 厘米内收。"""
    return ([(x, 0) for x in range(360, 1081)]
            + [(1080 - (z * 10) // TRACK_TOP_Z, z) for z in range(1, TRACK_TOP_Z)]
            + [(x, TRACK_TOP_Z) for x in range(1070, 369, -1)]
            + [(360 + (z * 10) // TRACK_TOP_Z, z) for z in range(TRACK_TOP_Z - 1, 0, -1)])


def build_tracks():
    rim = track_rim()
    for (ty0, ty1) in TRACK_Y:
        for i, (x, z) in enumerate(rim):
            thick = 4 if z == 0 else TRACK_THICK   # 下段只剩 4 厘米贴地接触面
            cleat = i % 6 < 3                       # 3 格胶块 + 3 格齿块，形成交替齿列
            for k in range(thick):
                c = (RUBBER if z == 0 and k < 2 else
                     (OLIVE_D if cleat else RUBBER))
                box((x, x + 1), (ty0, ty1), (z + k, z + k + 1), c)
            if cleat:
                # 齿块在两侧外缘凸 2 厘米，侧视能读出抓地块与沟槽。
                box((x, x + 1), (ty0, ty0 + 6), (z, z + thick), OLIVE_D)
                box((x, x + 1), (ty1 - 6, ty1), (z, z + thick), OLIVE_D)
            if z == TRACK_TOP_Z and i % 15 < 2:
                # 顶段导向凸耳，顶面 0.96 米，与裙板下缘留 8 厘米净隙。
                box((x, x + 1), (ty0 + 6, ty1 - 6),
                    (TRACK_TOP_Z + TRACK_THICK, TRACK_TOP_Z + TRACK_THICK + 4), STEEL)


def wheel_face(wx, cz, ys, r):
    """在 y 切片 ys 上画单侧轮面：胎圈、包边环、八条辐板、轮毂与螺栓圈。

    细节必须画在最外 2 厘米的 y 层上，画在轮体内部会被外层胎圈完全遮住
    （上一版轮面全黑就是这个原因）。
    """
    disc_xz(wx, cz, ys, r, RUBBER)          # 胎圈
    disc_xz(wx, cz, ys, r - 3, OLIVE_D)     # 轮缘包边
    disc_xz(wx, cz, ys, r - 6, OLIVE)       # 轮面
    # 同心环 + 轮毂：参考图轮面是几道同心圈，不加辐条避免“风车”观感。
    if r >= 30:
        disc_xz(wx, cz, ys, r - 11, OLIVE_D, r_in=r - 14)
    disc_xz(wx, cz, ys, 11, OLIVE_D)        # 轮毂
    disc_xz(wx, cz, ys, 4, BLACK)           # 轮毂芯
    yc = (ys[0] + ys[1]) // 2
    for k in range(6):                      # 六颗钢色螺栓（略凸出轮面）
        a = math.radians(k * 60 + 30)
        dot(int(round(wx + 17 * math.cos(a))), yc,
            int(round(cz + 17 * math.sin(a))), STEEL, 3)


def build_wheels():
    """负重轮七枚 + 前后端轮：两侧轮面都带同心环与螺栓。"""
    for (wy0, wy1) in WHEEL_Y:
        for idx, wx in enumerate(list(ROAD_WHEEL_X) + [IDLER_X, SPROCKET_X]):
            end = idx >= len(ROAD_WHEEL_X)
            r = END_WHEEL_R if end else ROAD_WHEEL_R
            disc_xz(wx, WHEEL_Z, (wy0, wy1), r, RUBBER)      # 轮体
            wheel_face(wx, WHEEL_Z, (wy0, wy0 + 2), r)
            wheel_face(wx, WHEEL_Z, (wy1 - 2, wy1), r)
            if end and wx == SPROCKET_X:
                # 主动轮（后）：钢齿圈伸出胎面，与光滑的诱导轮区分。
                for k in range(10):
                    a = math.pi / 5 * k
                    for t in range(r - 3, r + 5):
                        px = int(round(wx + t * math.cos(a)))
                        pz = int(round(WHEEL_Z + t * math.sin(a)))
                        box((px, px + 2), (wy0, wy1), (pz, pz + 2), STEEL)


# ---------------------------------------------------------------------------
# 炮塔与火炮
# ---------------------------------------------------------------------------
def turret_half_plan(x):
    """塔身平面半宽（厘米）：按站位硬跳变，不做连续收分。

    尾舱 2.36 米 → 过渡 → 主体 3.00 米 → 前楔收到 1.24 米。
    连续斜壁在 1 厘米体素上会退化成每格一台阶的竖条纹，这里把尾舱后角、
    尾舱到主体的过渡、前楔都做成 10～20 厘米宽的大台阶，读作块状倒角。
    """
    if x < 510:
        return 118
    if x < FRONT_CHAMFER:      # 尾舱后角第二级台阶
        return 130
    if x < BUSTLE_END:
        return 138
    if x < TURRET_MAIN[0]:     # 尾舱 → 主体过渡
        return 144
    if x < 910:
        return 150
    # 前楔：每 9～10 厘米收 8 厘米半宽，平面斜度约 45°，收成 2.12 米宽的前脸。
    # 台阶再大就会读成一排竖直鳍片，再小又会退化成瓦楞条纹。
    if x < 911:
        return 142
    if x < 921:
        return 134
    if x < 930:
        return 126
    if x < 940:
        return 118
    return 110


def turret_top(x):
    """塔顶轮廓：尾舱低顶 → 主体平顶 → 前楔顶面降一级。

    台阶全部做成硬边：宽 40 厘米级的缓坡在 1 厘米体素上会积出十几条明暗
    条纹，读起来像瓦楞，而 10 厘米级的硬落差只留一条干净的暗面。
    """
    if x < TURRET_MAIN[0]:
        return BUSTLE_TOP
    if x < TURRET_MAIN[1]:
        return ROOF
    return ROOF - 24


def turret_floor(x):
    """塔底轮廓：主体落在座圈顶，前楔底面按 8 厘米台阶抬高，让塔前脸成楔形。"""
    if x < TURRET_MAIN[1]:
        return DECK_Z + 6
    return stepped(x, TURRET_MAIN[1], TURRET_FRONT, DECK_Z + 6, DECK_Z + 32)


def turret_half(x, z):
    """给定平面位置与高度的半宽（厘米）。

    塔壁全高竖直：1 厘米体素上任何平缓收分都会积成一条条明暗竖纹，
    给定高度的收分在这里没有可读的收益，屋顶边缘改用 2 厘米暗色压条。
    """
    return max(6, turret_half_plan(x))


def build_turret():
    # 座圈暗带：略宽于塔底，形成可见环座阴影。
    box((484, 964), (TURRET_C - 152, TURRET_C + 152), (DECK_Z, DECK_Z + 6), OLIVE_D)

    # 塔体：逐列按平面半宽与高度实心填充；尾舱台阶、前楔、屋顶落差都由
    # turret_half_plan / turret_top 一处决定，避免分块拼接处出现错位。
    for x in range(TURRET_REAR, TURRET_FRONT):
        top = turret_top(x)
        for z in range(turret_floor(x), top):
            hw = turret_half(x, z)
            box((x, x + 1), (TURRET_C - hw, TURRET_C + hw), (z, z + 1), OLIVE)

    # 侧壁横向板缝：只压塔侧，不绕到正面，避免正视出现横条纹。
    for zb in (196, 226):
        for x in range(FRONT_CHAMFER, TURRET_FRONT - 30):
            hw = turret_half(x, zb)
            box((x, x + 1), (TURRET_C - hw, TURRET_C - hw + 2), (zb, zb + 1), OLIVE_D)
            box((x, x + 1), (TURRET_C + hw - 2, TURRET_C + hw), (zb, zb + 1), OLIVE_D)
    # 侧壁竖向板缝：把塔侧分成四块大板，对应参考图块状装甲面的分缝。
    for xb in (600, 690, 780, 870):
        for z in range(turret_floor(xb), turret_top(xb)):
            hw = turret_half(xb, z)
            box((xb, xb + 2), (TURRET_C - hw, TURRET_C - hw + 2), (z, z + 1), OLIVE_D)
            box((xb, xb + 2), (TURRET_C + hw - 2, TURRET_C + hw), (z, z + 1), OLIVE_D)
    # 侧壁铆钉列：贴在塔底最宽段的侧面上。
    for x in range(540, 900, 26):
        hw = turret_half(x, 176)
        dot(x, TURRET_C - hw - 1, 176, STEEL, 3)
        dot(x, TURRET_C + hw + 1, 176, STEEL, 3)
    # 屋顶周边压暗线 + 纵向板缝。
    for x in range(TURRET_MAIN[0], TURRET_MAIN[1]):
        hw = turret_half(x, ROOF - 1)
        box((x, x + 1), (TURRET_C - hw, TURRET_C - hw + 2), (ROOF - 1, ROOF), OLIVE_D)
        box((x, x + 1), (TURRET_C + hw - 2, TURRET_C + hw), (ROOF - 1, ROOF), OLIVE_D)
    for x in (748, 812):
        box((x, x + 2), (TURRET_C - 116, TURRET_C + 116), (ROOF - 1, ROOF), OLIVE_D)

    # 尾部储物箱：尾面中央一个宽箱（暗框 + 亮面 + 中间横缝），读作物资箱。
    box((494, TURRET_REAR), (128, 252), (200, 236), OLIVE_D)
    box((494, 496), (132, 248), (204, 233), OLIVE_M)
    box((494, 496), (132, 248), (217, 219), OLIVE_D)

    # 车长塔（车体右侧，y>190）：座圈 + 低盖 + 六面周视窗，顶端压在 2.70 米内。
    CUP_Y = 268
    ring_xy(744, CUP_Y, 31, 25, (ROOF, ROOF + 4), OLIVE_D)
    disc_xy(744, CUP_Y, 25, (ROOF + 4, ROOF + 5), OLIVE)
    ring_xy(744, CUP_Y, 20, 14, (ROOF + 5, ROOF + 6), OLIVE_D)
    disc_xy(744, CUP_Y, 14, (ROOF + 5, ROOF + 6), OLIVE_M)
    dot(744, CUP_Y, ROOF + 3, OLIVE_DD, 6)
    for k in range(6):
        a = math.radians(k * 60 + 30)
        dot(int(round(744 + 27 * math.cos(a))), int(round(CUP_Y + 27 * math.sin(a))),
            ROOF + 2, GLASS, 5)
    box((708, 720), (CUP_Y - 8, CUP_Y + 8), (ROOF + 3, ROOF + 4), OLIVE_D)  # 舱盖铰链

    # 装填手舱盖（车体左侧）：圆盖 + 座圈。
    box((730, 742), (104, 124), (ROOF + 3, ROOF + 4), OLIVE_D)
    ring_xy(762, 112, 27, 21, (ROOF, ROOF + 4), OLIVE_D)
    disc_xy(762, 112, 21, (ROOF + 4, ROOF + 5), OLIVE)
    disc_xy(762, 112, 7, (ROOF + 5, ROOF + 6), OLIVE_DD)

    # 车长周视镜 PERI：紧贴车长塔后方的短基座 + 方形观察窗，顶端 2.70 米。
    box((706, 736), (252, 288), (ROOF, ROOF + 2), OLIVE_D)
    box((708, 734), (256, 284), (ROOF + 2, TOP_LIMIT), OLIVE, rim=1)
    box((732, 734), (260, 280), (ROOF + 3, TOP_LIMIT - 1), GLASS)

    # 车长/炮长 EMES 瞄准镜（车体右侧前部，炮塔前楔顶面）：小型方盒，不居中。
    # 只放在前楔顶（240 厘米）上，越过 910 会落到收窄段外侧。
    box((904, 928), (272, 294), (ROOF - 24, ROOF - 16), OLIVE_D, rim=1)
    box((926, 928), (278, 290), (ROOF - 22, ROOF - 18), GLASS)

    # 装填手 MG3（车体左侧，紧邻装填手舱盖）：枪座 + 机匣 + 枪管 + 弹链盒。
    box((786, 810), (40, 64), (ROOF, ROOF + 2), OLIVE_D)      # 枪座
    box((810, 832), (44, 60), (ROOF + 2, ROOF + 4), OLIVE_D)  # 机匣
    box((832, 878), (50, 58), (ROOF + 4, TOP_LIMIT - 1), BLACK)  # 枪管
    box((790, 812), (64, 82), (ROOF, ROOF + 3), OLIVE_M)      # 弹链盒

    # 两侧烟幕发射器：尾面后角向后伸出的两排四管，管口朝后（正后视可见 2×4 圆口）。
    # 管径 7 厘米、间距 9 厘米，再密就会互相重叠糊成一团。
    for side in (1, -1):
        yc = 96 if side > 0 else 284                       # 左 / 右管束中心
        box((496, TURRET_REAR), (yc - 18, yc + 18), (176, 218), OLIVE)   # 安装板
        for dz in (188, 203):
            for k in range(4):
                yt = yc - 13 + 9 * k
                cyl_x((472, 496), yt, dz, 3, OLIVE_D)       # 发射管
                cyl_x((472, 474), yt, dz, 3, BLACK)         # 管口

    # 双天线：尾舱顶后角，杆细且高，与车顶附件形成明显高差。
    for yc in (93, 287):
        box((524, 540), (yc - 9, yc + 9), (BUSTLE_TOP, BUSTLE_TOP + 8), BLACK, rim=1)
        box((530, 534), (yc - 2, yc + 2), (BUSTLE_TOP + 8, ANTENNA_TOP), BLACK)
    # 车顶杂项：火控箱、吊耳、GPS 盒与边角小件。
    box((706, 730), (80, 106), (ROOF, ROOF + 5), OLIVE_D, rim=1)
    box((830, 858), (78, 102), (ROOF, ROOF + 4), OLIVE_D, rim=1)
    for (x, y) in ((752, 292), (700, 300), (866, 96)):
        box((x, x + 10), (y - 4, y + 4), (ROOF, ROOF + 3), OLIVE_D)

    # L55 炮管：粗炮根套筒、细管身、抽烟器、炮口箍，无制退器。
    box((944, 960), (146, 234), (184, 242), OLIVE, rim=1)   # 炮盾
    cyl_x((960, 1018), 190, BORE_Z, 20, OLIVE)              # 炮根套筒
    box((1018, 1024), (168, 212), (BORE_Z - 20, BORE_Z + 20), OLIVE_D)
    cyl_x((1024, 1150), 190, BORE_Z, 10, OLIVE)
    box((1150, 1156), (176, 204), (BORE_Z - 14, BORE_Z + 14), OLIVE_D)
    cyl_x((1156, 1246), 190, BORE_Z, 18, OLIVE)             # 抽烟器
    box((1246, 1252), (174, 206), (BORE_Z - 18, BORE_Z + 18), OLIVE_D)
    cyl_x((1252, 1414), 190, BORE_Z, 10, OLIVE)
    cyl_x((1414, 1426), 190, BORE_Z, 14, OLIVE_D)           # 炮口箍
    cyl_x((1426, 1436), 190, BORE_Z, 10, OLIVE)
    cyl_x((1428, 1436), 190, BORE_Z, 5, BLACK)              # 炮口内孔


# ---------------------------------------------------------------------------
# 导出与校验
# ---------------------------------------------------------------------------
def exposed_mask():
    """表面体素：任一六邻域为空即保留；实心内部不写入，控制文件体积。

    画布外按空气处理（补一圈 0），否则贴地 z=0 层与画布边界面会被漏掉。
    """
    padded = np.pad(grid, 1)
    air = padded == 0
    mask = np.zeros(padded.shape, dtype=bool)
    mask[1:-1, 1:-1, 1:-1] = (padded[1:-1, 1:-1, 1:-1] > 0) & (
        air[:-2, 1:-1, 1:-1] | air[2:, 1:-1, 1:-1]
        | air[1:-1, :-2, 1:-1] | air[1:-1, 2:, 1:-1]
        | air[1:-1, 1:-1, :-2] | air[1:-1, 1:-1, 2:])
    del air, padded
    return mask[1:-1, 1:-1, 1:-1]


def export_vox(mask, path):
    """≤256 分块写出场景：命名根 leopard_2a6 下每块一个居中平移节点。"""
    plans = [sv.chunk_axis_plan(n) for n in CANVAS]
    models, nodes = [], []

    def node(kind, attrs=None):
        result = sv.Node(kind, len(nodes), attrs or {})
        nodes.append(result)
        return result

    root, group = node('trn'), node('grp')
    root.child, root.layer = group.node_id, -1
    named, chunks = node('trn', {'_name': 'leopard_2a6'}), node('grp')
    named.child, named.layer = chunks.node_id, -1
    group.children.append(named.node_id)

    for ix, iy, iz in np.ndindex(len(plans[0]), len(plans[1]), len(plans[2])):
        origin = (plans[0][ix][0], plans[1][iy][0], plans[2][iz][0])
        size = (plans[0][ix][1], plans[1][iy][1], plans[2][iz][1])
        sub = mask[origin[0]:origin[0] + size[0],
                   origin[1]:origin[1] + size[1],
                   origin[2]:origin[2] + size[2]]
        xs, ys, zs = np.nonzero(sub)
        if len(xs) == 0:
            continue
        rec = np.empty((len(xs), 4), dtype=np.uint8)
        rec[:, 0], rec[:, 1], rec[:, 2] = xs, ys, zs
        rec[:, 3] = grid[origin[0] + xs, origin[1] + ys, origin[2] + zs]
        models.append(sv.Model(size, rec.tobytes()))
        trn = node('trn')
        trn.frames = [{'_t': ' '.join(str(o + (s - f) // 2)
                                     for o, s, f in zip(origin, size, CANVAS))}]
        shp = node('shp')
        shp.models = [(len(models) - 1, {})]
        trn.child, trn.layer = shp.node_id, -1
        chunks.children.append(trn.node_id)

    palette = (b''.join(bytes(c) for c in PALETTE)
               + bytes([0, 0, 0, 0]) * (256 - len(PALETTE)))
    sv.write_vox(models, nodes, [('RGBA', palette)], path)


def verify(mask):
    """独立回读 vox 并核对体素数、颜色与外形三轴边界。"""
    source = sv.parse_vox(OUT_PATH.read_bytes(), OUT_PATH.name)
    total = sum(m.voxel_count for m in source.models)
    xs, ys, zs, colors = [], [], [], set()

    def walk(index, acc):
        node = source.nodes[index]
        if node.kind == 'trn':
            walk(node.child, tuple(a + b for a, b in zip(acc, sv.frame_t(node.frames))))
        elif node.kind == 'grp':
            for child in node.children:
                walk(child, acc)
        else:
            for mid, _ in node.models:
                model = source.models[mid]
                # 画布坐标 = 场景平移 − 居中差 + 局部位坐标。
                off = tuple(a - (s - f) // 2 for a, s, f in zip(acc, model.size, CANVAS))
                rec = np.frombuffer(model.xyzi, dtype=np.uint8).reshape(-1, 4)
                xs.append(rec[:, 0].astype(int) + off[0])
                ys.append(rec[:, 1].astype(int) + off[1])
                zs.append(rec[:, 2].astype(int) + off[2])
                colors.update(int(c) for c in np.unique(rec[:, 3]))

    walk(0, (0, 0, 0))
    xs, ys, zs = np.concatenate(xs), np.concatenate(ys), np.concatenate(zs)
    bbox = (int(xs.min()), int(xs.max()), int(ys.min()), int(ys.max()),
            int(zs.min()), int(zs.max()))
    if total != int(mask.sum()):
        raise SystemExit(f'回读体素数 {total} ≠ 表面体素数 {int(mask.sum())}')
    if not colors <= set(range(1, len(PALETTE))):
        raise SystemExit(f'非法颜色索引：{sorted(colors - set(range(1, len(PALETTE))))}')
    # 外形契约（端体素含端面）：车尾 338、炮口 1435、宽 2..377、地面 0、天线顶 299。
    assert bbox[0] == HULL_X[0], f'车尾后缘 {bbox[0]} ≠ {HULL_X[0]}'
    assert bbox[1] == GUN_X[1] - 1, f'炮口端面 {bbox[1]} ≠ {GUN_X[1] - 1}'
    assert bbox[2] == WIDTH[0] and bbox[3] == WIDTH[1] - 1, f'宽度 {bbox[2]}..{bbox[3]}'
    assert bbox[4] == 0, f'地面 Z {bbox[4]} ≠ 0'
    assert bbox[5] == ANTENNA_TOP - 1, f'天线顶 {bbox[5]} ≠ {ANTENNA_TOP - 1}'
    return dict(total=total, bbox=bbox, colors=sorted(colors))


# ---------------------------------------------------------------------------
# 正交预览渲染（numpy 深度缓冲，无需 GPU）
# ---------------------------------------------------------------------------
SHADE = {(1, 0, 0): 0.90, (0, 1, 0): 0.74, (0, 0, 1): 1.0,
         (-1, 0, 0): 0.80, (0, -1, 0): 0.64, (0, 0, -1): 0.5}
FACE_ORDER = [(0, 0, 1), (1, 0, 0), (0, 1, 0), (0, 0, -1), (-1, 0, 0), (0, -1, 0)]
FACE_AXES = {(1, 0, 0): (1, 2), (-1, 0, 0): (1, 2), (0, 1, 0): (0, 2),
             (0, -1, 0): (0, 2), (0, 0, 1): (0, 1), (0, 0, -1): (0, 1)}


def render_view(coords, colors, direction, path, scale=2.0, background=0x9a):
    from PIL import Image

    d = np.asarray(direction, dtype=float)
    d /= np.linalg.norm(d)
    xs, ys, zs = coords
    pos = np.stack([xs, ys, zs], axis=1).astype(np.float64)

    if abs(d[0]) < 1e-6 and abs(d[1]) < 1e-6:
        right, up = np.array([0.0, 1.0, 0.0]), np.array([1.0, 0.0, 0.0])   # 俯视：车首朝上
    else:
        right = np.cross(np.array([0.0, 0.0, 1.0]), d)
        right /= np.linalg.norm(right)
        up = np.cross(d, right)

    su, sv = pos @ right, pos @ up
    u0, vmax = su.min(), sv.max()
    w = int((su.max() - u0) * scale) + 8
    h = int((vmax - sv.min()) * scale) + 8

    facing = [n for n in FACE_ORDER if n[0] * d[0] + n[1] * d[1] + n[2] * d[2] > 0.02]
    shown = np.zeros(len(colors), dtype=bool)
    normal_of = np.zeros((len(colors), 3), dtype=np.int8)
    for normal in facing:
        nx = np.clip(xs + normal[0], 0, CX - 1)
        ny = np.clip(ys + normal[1], 0, CY - 1)
        nz = np.clip(zs + normal[2], 0, CZ - 1)
        take = (grid[nx, ny, nz] == 0) & ~shown
        normal_of[take] = normal
        shown |= take
    if not shown.any():
        raise SystemExit('该视角没有可见表面')

    idx = np.nonzero(shown)[0]
    depth = pos @ d
    key = np.full((h, w), -1.0)
    zbuf = np.full((h, w), -1.0e9)
    for normal in facing:
        sel = idx[(normal_of[idx] == normal).all(axis=1)]
        if len(sel) == 0:
            continue
        ax, ay = FACE_AXES[normal]
        du, dv = abs(right[ax]) + abs(right[ay]), abs(up[ax]) + abs(up[ay])
        px = ((su[sel] - u0) * scale).astype(np.int32)
        py = ((vmax - sv[sel]) * scale).astype(np.int32)
        # 组合键：深度为主，体素索引打破共面平局，避免补缝处互相覆盖。
        vox_key = (depth[sel] - depth.min()) * 6.0e6 + sel
        for dx in range(int(np.ceil(du * scale)) + 1):
            for dy in range(int(np.ceil(dv * scale)) + 1):
                np.maximum.at(key, (py + dy, px + dx), vox_key)
                np.maximum.at(zbuf, (py + dy, px + dx), depth[sel])

    img = np.full((h, w, 3), background, dtype=np.uint8)
    # 调色板按颜色索引直接成表：把逐体素配色再按索引取色会把索引当成行号，
    # 拿到完全无关的颜色（上一版预览整体偏暗就是这个原因）。
    rgb = np.array([c[:3] for c in PALETTE], dtype=np.float64)
    for normal in facing:
        sel = idx[(normal_of[idx] == normal).all(axis=1)]
        if len(sel) == 0:
            continue
        ax, ay = FACE_AXES[normal]
        du, dv = abs(right[ax]) + abs(right[ay]), abs(up[ax]) + abs(up[ay])
        px = ((su[sel] - u0) * scale).astype(np.int32)
        py = ((vmax - sv[sel]) * scale).astype(np.int32)
        vox_key = (depth[sel] - depth.min()) * 6.0e6 + sel
        col = np.clip(rgb[colors[sel]] * SHADE[normal], 0, 255).astype(np.uint8)
        for dx in range(int(np.ceil(du * scale)) + 1):
            for dy in range(int(np.ceil(dv * scale)) + 1):
                hit = key[py + dy, px + dx] == vox_key
                img[py[hit] + dy, px[hit] + dx] = col[hit]

    # 深度断差描边：6 厘米以上台阶压暗，便于核对结构边界。
    solid = zbuf > -1e8
    edge = ((np.abs(np.diff(zbuf, axis=1))[:-1, :] > 6)
            | (np.abs(np.diff(zbuf, axis=0))[:, :-1] > 6))
    edge &= solid[:-1, :-1] | solid[1:, :-1] | solid[:-1, 1:] | solid[1:, 1:]
    img[:-1, :-1][edge] = (img[:-1, :-1][edge].astype(np.int16) * 0.55).astype(np.uint8)
    Image.fromarray(img).save(path)


def render_all(mask):
    xs, ys, zs = np.nonzero(mask)
    colors = grid[xs, ys, zs]
    out = HERE / 'preview'
    out.mkdir(exist_ok=True)
    views = [
        ('01-front-right', (1.0, 1.0, 0.85), 2.0),
        ('02-rear', (-1.0, 0.0, 0.15), 2.0),
        ('03-front', (1.0, 0.0, 0.06), 2.0),
        ('04-top', (0.0, 0.0, 1.0), 2.0),
        ('05-front-left', (1.0, -1.0, 0.85), 2.0),
        ('06-rear-left', (-1.0, -1.0, 0.85), 2.0),
        ('07-rear-right', (-1.0, 1.0, 0.85), 2.0),
        # 纯侧视：核对车体/炮塔侧轮廓、裙板与负重轮排布。
        ('08-side-left', (0.0, 1.0, 0.02), 3.0),
        ('09-side-right', (0.0, -1.0, 0.02), 3.0),
    ]
    for name, direction, scale in views:
        render_view((xs, ys, zs), colors, direction, out / f'{name}.png', scale=scale)
        print(f'  渲染 {name}.png')


def main(argv):
    parser = argparse.ArgumentParser(description='豹 2A6 1 厘米体素作者源')
    parser.add_argument('--render', action='store_true', help='生成多视角预览图')
    args = parser.parse_args(argv)

    build_hull()
    build_tracks()
    build_wheels()
    build_turret()

    mask = exposed_mask()
    export_vox(mask, OUT_PATH)
    (HERE / 'leopard_2a6.vox.meta').write_text(
        '(meta_format_version: "1.0", voxel_size: 0.01)\n', encoding='utf-8')
    stats = verify(mask)

    report = [
        '豹 2A6 精细体素模型（1 厘米/格）',
        f'画布 {CANVAS[0]}×{CANVAS[1]}×{CANVAS[2]} 格'
        f'（{CANVAS[0]/100:.2f}×{CANVAS[1]/100:.2f}×{CANVAS[2]/100:.2f} 米，地面 Z=0）',
        f'表面体素 {stats["total"]}，颜色索引 {stats["colors"]}',
        f'包围盒 X {stats["bbox"][0]}..{stats["bbox"][1]}，Y {stats["bbox"][2]}..{stats["bbox"][3]}，'
        f'Z {stats["bbox"][4]}..{stats["bbox"][5]}（厘米）',
        f'车体长 {HULL_X[1]-HULL_X[0]} 厘米 / 全长 {stats["bbox"][1]+1-HULL_X[0]} 厘米 / '
        f'宽 {WIDTH[1]-WIDTH[0]} 厘米 / 炮塔顶 {ROOF} 厘米 / 炮轴 {BORE_Z} 厘米',
        f'车顶附件上限 {TOP_LIMIT} 厘米（舱盖、观瞄、机枪），天线顶 {stats["bbox"][5]+1} 厘米',
        f'文件 {OUT_PATH.name} {OUT_PATH.stat().st_size/1024/1024:.1f} MiB',
    ]
    (HERE / 'report.txt').write_text('\n'.join(report) + '\n', encoding='utf-8')
    print('\n'.join(report))

    if args.render:
        render_all(mask)
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
