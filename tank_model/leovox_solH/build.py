"""按指定概念图从零作者化 Leopard 2A6；所有几何坐标为厘米，+X 前、+Y 左、+Z 上。"""
from __future__ import annotations

import hashlib
import json
import math
import struct
from pathlib import Path

import numpy as np
from scipy.spatial import ConvexHull

ROOT = Path(__file__).resolve().parent
SIZE = (1097, 376, 438)
# 材质色没有烘焙方向光；磨损只用于接缝与零件边缘。
COLORS = [
    (0, 0, 0), (87, 98, 55), (98, 107, 62), (79, 90, 49),
    (119, 125, 78), (69, 79, 45), (51, 59, 39), (30, 33, 30),
    (58, 59, 52), (85, 83, 70), (139, 136, 96), (18, 23, 23),
    (38, 58, 61), (94, 119, 119), (133, 40, 29), (179, 112, 30),
    (191, 192, 164), (66, 72, 47), (126, 131, 83), (38, 41, 36),
]
OLIVE, PALE, SHADE, EDGE, DARK, RECESS = range(1, 7)
RUBBER, STEEL, METAL, WORN, BLACK, GLASS, GLINT, RED, AMBER, LAMP, MOSS, LIGHT, TRACK = range(7, 20)


class Model:
    """厘米整数网格与机械部件归属；顺序叠加实体，导出前保留两格外壳。"""
    def __init__(self, size=SIZE):
        self.color = np.zeros(size, dtype=np.uint8)
        self.part = np.zeros(size, dtype=np.uint8)
        self.names = {}
        self.current = 0
        self.features = {}

    def group(self, name):
        if name not in self.names:
            self.names[name] = len(self.names) + 1
        self.current = self.names[name]

    def stamp(self, lo, hi, color, mask=None):
        lo, hi = np.array(lo, dtype=int), np.array(hi, dtype=int)
        assert np.all(lo >= 0) and np.all(hi <= self.color.shape) and np.all(hi > lo), (lo, hi)
        region = tuple(slice(a, b) for a, b in zip(lo, hi))
        if mask is None:
            self.color[region] = color
            self.part[region] = self.current if color else 0
        else:
            self.color[region][mask] = color
            self.part[region][mask] = self.current if color else 0

    def box(self, x0, x1, y0, y1, z0, z1, color):
        self.stamp((x0, y0, z0), (x1, y1, z1), color)

    def cylinder(self, axis, center, radius, start, end, color, inner=0):
        """轴向端点半开；圆截面按格心采样，内半径可生成真正的环和孔。"""
        other = [i for i in range(3) if i != axis]
        lo = np.zeros(3, int)
        hi = np.zeros(3, int)
        lo[axis], hi[axis] = start, end
        for i, c in zip(other, center):
            lo[i], hi[i] = math.floor(c - radius), math.ceil(c + radius)
        grids = np.ogrid[tuple(slice(a, b) for a, b in zip(lo, hi))]
        rr = sum((grids[i] + 0.5 - c) ** 2 for i, c in zip(other, center))
        mask = np.broadcast_to((rr < radius ** 2) & (rr >= inner ** 2), hi - lo)
        self.stamp(lo, hi, color, mask)

    def convex(self, vertices, color):
        """凸多面体用于倾斜装甲；直接厘米采样，不把粗模型细分成伪高精度。"""
        points = np.array(vertices, dtype=float)
        lo = np.floor(points.min(axis=0)).astype(int)
        hi = np.ceil(points.max(axis=0)).astype(int)
        grids = np.ogrid[tuple(slice(a, b) for a, b in zip(lo, hi))]
        inside = np.ones(hi - lo, dtype=bool)
        for a, b, c, d in ConvexHull(points).equations:
            inside &= (a * (grids[0] + .5) + b * (grids[1] + .5) + c * (grids[2] + .5) + d) <= 1e-6
        self.stamp(lo, hi, color, inside)

    def prism(self, polygon_xz, y0, y1, color):
        self.convex([(x, y, z) for y in (y0, y1) for x, z in polygon_xz], color)

    def rod(self, a, b, radius, color):
        """带球端的体素杆用于保护架、牵引索及天线；坐标与半径均为厘米。"""
        a, b = np.array(a, float), np.array(b, float)
        lo = np.maximum(0, np.floor(np.minimum(a, b) - radius).astype(int))
        hi = np.minimum(self.color.shape, np.ceil(np.maximum(a, b) + radius).astype(int))
        g = np.ogrid[tuple(slice(l, h) for l, h in zip(lo, hi))]
        v = b - a
        t = np.clip(sum((g[i] + .5 - a[i]) * v[i] for i in range(3)) / np.dot(v, v), 0, 1)
        distance = sum((g[i] + .5 - a[i] - t * v[i]) ** 2 for i in range(3))
        self.stamp(lo, hi, color, distance <= radius ** 2)

    def shell(self):
        """去掉不可见实心填充；不同机械部件交界也保留外壳，方便以后独立转动。"""
        inside = self.part != 0
        for axis in range(3):
            for delta in (-2, -1, 1, 2):
                inside &= self.part == np.roll(self.part, delta, axis)
            edge = [slice(None)] * 3
            for band in (slice(0, 2), slice(-2, None)):
                edge[axis] = band
                inside[tuple(edge)] = False
        self.color[inside] = 0
        self.part[inside] = 0


def bolt(m, x, y, z, axis=2, color=METAL):
    """3 cm 螺栓头；用在结构连接处，不作均匀噪声。"""
    coords = [x, y, z]
    m.cylinder(axis, [coords[i] for i in range(3) if i != axis], 1.6, coords[axis], coords[axis] + 2, color)


def armor_plate(m, outline, color=OLIVE):
    """斜装甲面上的 3 cm 独立板，边缘窄倒角与内侧色面随同一平面变化。"""
    outline = np.array(outline, float)
    m.convex([tuple(p + (0, 0, dz)) for dz in (-1.0, 2.0) for p in outline], EDGE)
    center = outline.mean(axis=0)
    inset = center + (outline - center) * .955
    m.convex([tuple(p + (0, 0, dz)) for dz in (1.0, 2.5) for p in inset], color)


def hull(m):
    """低甲板车体、前上/下装甲、分段裙板；车尾 0、车首 766，甲板顶 160。"""
    m.group("hull")
    m.prism([(0, 60), (30, 44), (690, 44), (766, 92), (766, 109), (550, 160), (0, 160)], 64, 312, OLIVE)
    # 车体下部内收，履带与负重轮腔保持通透。
    m.prism([(12, 56), (685, 46), (746, 88), (12, 113)], 77, 299, SHADE)
    m.box(8, 550, 59, 317, 149, 160, PALE)
    # 发动机检修板；细缝是几何台阶，避免铺满虚构表面砖线。
    for a, b in ((10, 64), (67, 139), (142, 187)):
        for y0, y1 in ((70, 180), (196, 306)):
            m.box(a, b, y0, y1, 159, 161, OLIVE if a == 67 else SHADE)
    # 前上装甲按三列四段分板，轮廓连续，接缝只凹一厘米。
    for a, b in ((552, 600), (602, 653), (655, 708), (710, 758)):
        za, zb = 160 - (a - 550) * 51 / 216, 160 - (b - 550) * 51 / 216
        for c, d in ((68, 153), (155, 221), (223, 308)):
            armor_plate(m, [(a, c, za), (b, c, zb), (b, d, zb), (a, d, za)], PALE if c == 155 else OLIVE)
    for a in (600, 653, 708):
        za = 160 - (a - 550) * 51 / 216
        m.prism([(a, za - 1), (a + 2, za - 1.5), (a + 2, za + 1), (a, za + 1.5)], 67, 309, DARK)
    # 两条外侧挡泥板与独立裙板，宽度与履带外沿同为 376 cm。
    for side in (0, 1):
        outer = 0 if side == 0 else 368
        # 全宽水平翼子板把裙板与主车体接合，不能只留下悬空的外侧薄边。
        fender0, fender1 = (0, 66) if side == 0 else (310, 376)
        m.box(8, 715, fender0, fender1, 149, 160, OLIVE)
        m.box(12, 720, outer, outer + 8, 149, 158, DARK)
        for i, (a, b) in enumerate(((14, 93), (96, 181), (184, 269), (272, 357), (360, 445), (448, 533), (536, 621), (624, 714))):
            m.box(a, b, outer, outer + 8, 104, 147, OLIVE if i % 3 else SHADE)
            m.box(a, b, outer, outer + 8, 145, 148, EDGE)
            m.box(a + 6, a + 16, outer, outer + 8, 145, 155, STEEL)
            for x in (a + 6, b - 7):
                bolt(m, x, 0 if side == 0 else 374, 112, axis=1)
                bolt(m, x, 0 if side == 0 else 374, 139, axis=1)
        # 前挡泥板沿鼻部折下，后挡泥板遮住上履带而不封死轮腔。
        m.prism([(714, 158), (740, 145), (766, 118), (766, 111), (714, 148)], fender0, fender1, PALE)
        m.box(0, 12, outer, outer + 8, 97, 146, OLIVE)
        m.box(0, 9, outer + 1, outer + 7, 66, 98, RUBBER)
        for x in range(18, 715, 31):
            y = 15 if side == 0 else 353
            m.box(x, x + 20, y, y + 8, 159, 162, OLIVE)
            bolt(m, x + 4, y + 4, 162)
        # 侧工具箱、细长牵引索和固定卡箍在甲板外侧。
        y = 32 if side == 0 else 324
        for x in (22, 106, 192):
            m.box(x, x + 70, y, y + 20, 153, 164, DARK)
            m.box(x + 2, x + 68, y + 1, y + 19, 164, 166, OLIVE)
            m.box(x + 32, x + 38, y, y + 21, 165, 168, STEEL)
        cable_y = 62 if side == 0 else 314
        m.rod((45, cable_y, 166), (493, cable_y, 166), 1.6, METAL)
        for x in (84, 207, 330, 453):
            m.box(x, x + 5, cable_y - 4, cable_y + 4, 158, 169, DARK)
    # 两个真正的圆形凹槽风扇：同心保护圈、径向筋条和深色网格。
    for cy in (116, 260):
        m.cylinder(2, (91, cy), 54, 157, 164, STEEL)
        m.cylinder(2, (91, cy), 49, 160, 166, BLACK)
        m.cylinder(2, (91, cy), 53, 164, 167, METAL, inner=49)
        for r in (15, 28, 40):
            m.cylinder(2, (91, cy), r + 1, 165, 167, STEEL, inner=r - 1)
        for angle in np.linspace(0, math.tau, 16, endpoint=False):
            m.rod((91 + 7 * math.cos(angle), cy + 7 * math.sin(angle), 166),
                  (91 + 49 * math.cos(angle), cy + 49 * math.sin(angle), 166), .7, METAL)
        m.cylinder(2, (91, cy), 7, 165, 169, STEEL)
        for x in (38, 144):
            for y in (cy - 47, cy + 47):
                bolt(m, x, y, 163)
    m.features["engine_fans"] = {"centers_cm": [[91, 116, 164], [91, 260, 164]], "diameter_cm": 106, "count": 2}
    # 后横向百叶面统一五区，不在炮塔尾部复制发动机格栅。
    m.box(0, 6, 66, 310, 91, 146, STEEL)
    m.box(0, 3, 70, 306, 94, 143, BLACK)
    for z in range(96, 143, 6):
        m.box(0, 5, 70, 306, z, z + 2, METAL)
    for y in (66, 114, 162, 210, 258, 306):
        m.box(0, 7, y, y + 4, 91, 146, OLIVE)
        bolt(m, 0, y + 2, 98, axis=0)
        bolt(m, 0, y + 2, 137, axis=0)
    # 灯未点亮；外琥珀、内红，再靠中线是暗圆灯。左右严格对应。
    for side in (0, 1):
        a, b = (7, 61) if side == 0 else (315, 369)
        m.box(0, 12, a, b, 135, 156, DARK)
        m.box(0, 3, a + 3, b - 3, 138, 154, BLACK)
        amber_y, red_y, round_y = (10, 21, 46) if side == 0 else (356, 341, 330)
        m.box(0, 2, amber_y, amber_y + 10, 141, 151, AMBER)
        m.box(0, 2, red_y, red_y + 14, 141, 151, RED)
        m.cylinder(0, (round_y, 146), 8, 0, 4, STEEL)
        m.cylinder(0, (round_y, 146), 5, 0, 2, TRACK)
    # 闭合牵引环采用中空实体，保留两个尾环、中央钩和两个首环。
    for x0, x1, z in ((0, 12, 70), (750, 766, 89)):
        for y in (97, 279):
            m.box(x0, x1, y - 12, y + 12, z - 14, z + 16, DARK)
            m.cylinder(0, (y, z), 12, x0, x1, OLIVE, inner=7)
            m.box(x0, x1, y - 12, y + 12, z + 8, z + 15, PALE)
            m.cylinder(0, (y, z), 7, x0, x1, 0)
    m.box(0, 15, 177, 199, 53, 84, STEEL)
    m.cylinder(1, (6, 68), 6, 176, 200, METAL)
    # 圆形前灯与独立保护架，禁止因视角改变形状。
    for y in (48, 328):
        m.box(733, 750, y - 14, y + 14, 119, 130, DARK)
        m.cylinder(0, (y, 137), 10, 736, 753, STEEL)
        m.cylinder(0, (y, 137), 7, 751, 754, LAMP)
        m.cylinder(0, (y, 137), 4, 753, 754, GLINT)
        for yy in (y - 14, y + 14):
            m.rod((735, yy, 121), (756, yy, 151), 1.6, OLIVE)
        m.rod((756, y - 14, 151), (756, y + 14, 151), 1.6, OLIVE)
    # 驾驶员舱盖依附车首斜甲，不增加虚构中央高塔。
    m.prism([(543, 160), (579, 151), (579, 154), (543, 163)], 202, 255, DARK)
    m.prism([(546, 162), (575, 155), (575, 158), (546, 165)], 206, 251, PALE)
    m.box(572, 582, 216, 239, 158, 165, DARK)
    m.box(580, 583, 219, 236, 160, 163, GLASS)


def running_gear(m):
    """两条闭合履带与独立轮组；七个 80 cm 负重轮，前诱导、后主动。"""
    road_x = [107, 196, 285, 374, 463, 552, 641]
    # 六边形轨迹保留真实端轮的斜包络，不用长方形履带框。
    loop = np.array([(30, 2), (736, 2), (763, 42), (731, 92), (35, 92), (3, 42)], float)
    for side, y0, y1, face, wheel0, wheel1 in (("right", 0, 66, 22, 23, 63), ("left", 310, 376, 354, 313, 353)):
        m.group(f"track_{side}")
        m.prism([(0, 42), (28, 0), (738, 0), (766, 42), (733, 94), (33, 94)], y0, y1, TRACK)
        # 内侧整条贯通，外侧履带保持连续连接。
        m.prism([(7, 42), (32, 2), (734, 2), (758, 42), (728, 85), (38, 85)], y0, y1, 0)
        # 沿闭合折线按等弧长放置 56 对橡胶垫与端连接耳。
        segments = np.roll(loop, -1, axis=0) - loop
        lengths = np.linalg.norm(segments, axis=1)
        cumulative = np.r_[0, np.cumsum(lengths)]
        for i in range(56):
            distance = (i + .5) * cumulative[-1] / 56
            idx = min(np.searchsorted(cumulative, distance) - 1, 5)
            tangent = segments[idx] / lengths[idx]
            p = loop[idx] + tangent * (distance - cumulative[idx])
            normal = np.array((tangent[1], -tangent[0]))
            # 每条板体 23 cm，节距约 30 cm；下段厚度不侵入 42 cm 轴高的轮胎。
            outward = 2 if p[1] < 4 else 4
            polygon = [p + tangent * a + normal * b for a, b in ((-11.5, -.8), (11.5, -.8), (11.5, outward), (-11.5, outward))]
            # 转角的宽板切向延伸也必须留在 0～96 cm 的履带包络内。
            polygon = [(max(0, min(766, x)), max(0, min(96, z))) for x, z in polygon]
            for ya, yb in ((y0 + 6, y0 + 29), (y0 + 37, y1 - 6)):
                m.prism(polygon, ya, yb, RUBBER)
            for ya, yb in ((y0, y0 + 5), (y1 - 5, y1)):
                m.prism(polygon, ya, yb, METAL if i % 4 == 0 else STEEL)
        # 每个轮胎有双轮胎、凹入轮辐盘、凸轮毂、可辨的螺栓圆。
        for i, x in enumerate(road_x):
            m.group(f"roadwheel_{side}_{i + 1:02d}")
            m.cylinder(1, (x, 42), 40, wheel0, wheel1, RUBBER)
            m.cylinder(1, (x, 42), 33, wheel0 - 1, wheel1 + 1, OLIVE)
            front = (wheel0 - 2, wheel0 + 2) if side == "right" else (wheel1 - 2, wheel1 + 2)
            m.cylinder(1, (x, 42), 31, *front, PALE, inner=27)
            m.cylinder(1, (x, 42), 26, *front, DARK)
            m.cylinder(1, (x, 42), 24, *front, OLIVE, inner=21)
            hub = (wheel0 - 5, wheel0 + 2) if side == "right" else (wheel1 - 2, wheel1 + 5)
            m.cylinder(1, (x, 42), 10, *hub, PALE)
            m.cylinder(1, (x, 42), 5, hub[0] if side == "right" else hub[1] - 2, hub[0] + 2 if side == "right" else hub[1], STEEL)
            for angle in np.linspace(0, math.tau, 8, endpoint=False):
                xx, zz = round(x + 17 * math.cos(angle)), round(42 + 17 * math.sin(angle))
                bolt(m, xx, front[0] if side == "right" else front[1] - 2, zz, axis=1, color=METAL)
        for role, x in (("sprocket", 30), ("idler", 732)):
            m.group(f"{role}_{side}")
            m.cylinder(1, (x, 42), 25, wheel0, wheel1, STEEL)
            m.cylinder(1, (x, 42), 22, wheel0 - 2, wheel1 + 2, PALE, inner=16)
            m.cylinder(1, (x, 42), 15, wheel0 - 1, wheel1 + 1, DARK)
            m.cylinder(1, (x, 42), 9, wheel0 - 4, wheel1 + 4, OLIVE)
            for angle in np.linspace(0, math.tau, 12 if role == "sprocket" else 6, endpoint=False):
                if role == "sprocket":
                    xx, zz = round(x + 24 * math.cos(angle)), round(42 + 24 * math.sin(angle))
                    m.box(xx - 2, xx + 3, wheel0, wheel1, zz - 2, zz + 3, METAL)
                else:
                    xx, zz = round(x + 13 * math.cos(angle)), round(42 + 13 * math.sin(angle))
                    m.cylinder(1, (xx, zz), 3, wheel0 - 2, wheel1 + 2, BLACK)
    m.features["suspension"] = {"roadwheel_centers_x_cm": road_x, "axis_z_cm": 42, "roadwheel_diameter_cm": 80,
                                "wheel_width_cm": 40, "front_idler_x_cm": 732, "rear_sprocket_x_cm": 30,
                                "endwheel_diameter_cm": 50, "links_per_track": 56}


def turret(m):
    """4.60×3.00 m 楔形炮塔，屋顶 2.64 m；附属设备不撑高主体装甲。"""
    m.group("turret")
    m.cylinder(2, (365, 188), 129, 161, 173, STEEL)
    m.cylinder(2, (365, 188), 125, 173, 178, DARK)
    # 后舱和两侧立面由收底的上下多边形定义。
    lower = [(152, 90), (176, 58), (427, 50), (547, 126), (547, 250), (427, 326), (176, 318), (152, 286)]
    upper = [(158, 92), (178, 50), (400, 38), (448, 146), (448, 230), (400, 338), (178, 326), (158, 284)]
    m.convex([(x, y, 178) for x, y in lower] + [(x, y, 264) for x, y in upper], OLIVE)
    # 固定的 V 形前楔；尖端居中偏前，顶面向后上倾，底面向后下倾。
    for side in (0, 1):
        def yy(y):
            return y if side == 0 else 376 - y
        m.convex([(420, yy(y), z) for y in (38, 158) for z in (184, 264)] +
                 [(500, yy(y), z) for y in (54, 158) for z in (180, 245.67)] +
                 [(612, yy(y), z) for y in (92, 158) for z in (212, 220)], SHADE)
        # 正面两片上斜装甲与一片下斜装甲，所有侧面共享同一空间接缝。
        for y_back0, y_back1, y_tip0, y_tip1 in ((44, 101, 94, 114), (105, 156, 118, 156)):
            armor_plate(m, [(438, yy(y_back0), 259.875), (603, yy(y_tip0), 222.0625),
                            (603, yy(y_tip1), 222.0625), (438, yy(y_back1), 259.875)], OLIVE)
        armor_plate(m, [(519, yy(59), 186), (608, yy(94), 213),
                        (608, yy(156), 213), (519, yy(156), 186)], DARK)
        # 侧面三块装甲盒，朝外面板及合页层次一致。
        rail0, rail1 = (40, 64) if side == 0 else (312, 336)
        for z in (196, 244):
            m.box(182, 398, rail0, rail1, z, z + 5, DARK)
        for i, (a, b) in enumerate(((182, 251), (254, 324), (327, 398))):
            y0, y1 = (40, 49) if side == 0 else (327, 336)
            m.box(a, b, y0, y1, 190, 250, SHADE if i == 0 else OLIVE)
            m.box(a, b, y0, y1, 249, 252, EDGE)
            for x in (a + 5, b - 7):
                bolt(m, x, y0 if side == 0 else y1 - 2, 242, axis=1)
        # 烟幕装置：每侧两个四联排，管轴朝外并稍向上/前；八个独立管口。
        y0, y1 = (30, 46) if side == 0 else (330, 346)
        m.box(203, 302, y0, y1, 201, 246, DARK)
        for z in (211, 234):
            for x in (213, 237, 261, 285):
                out = 14 if side == 0 else 362
                mount = 39 if side == 0 else 337
                m.rod((x - 3, mount, z - 3), (x + 4, out, z + 3), 6, STEEL)
                m.cylinder(1, (x + 4, z + 3), 5, out - 2, out + 2, METAL)
                m.cylinder(1, (x + 4, z + 3), 3, out - 2, out + 2, BLACK)
    # 顶板在各边缘内收；稀疏的检修缝、把手及螺栓沿结构布置。
    for a in (250, 290, 338, 397):
        m.box(a, a + 2, 78, 298, 262, 265, 0)
        m.box(a, a + 2, 78, 298, 261, 262, DARK)
    for y in (171, 205):
        m.box(183, 426, y, y + 2, 262, 265, 0)
        m.box(183, 426, y, y + 2, 261, 262, DARK)
    for a, b in ((184, 250), (253, 290), (341, 397), (400, 429)):
        for y0, y1 in ((82, 170), (174, 204), (208, 294)):
            m.box(a, b, y0, y1, 262, 264, OLIVE if a % 2 else PALE)
    # 把手、铰链与周向紧固件强化装配关系，屋顶中心仍保持 264 cm。
    for x, y in ((212, 188), (366, 145), (366, 244), (407, 187)):
        m.box(x - 8, x - 5, y - 5, y + 5, 264, 267, DARK)
        m.box(x + 5, x + 8, y - 5, y + 5, 264, 267, DARK)
        m.rod((x - 6, y, 267), (x + 6, y, 267), 1, METAL)
    for x in range(185, 415, 38):
        for y in (67, 309):
            bolt(m, x, y, 263, color=DARK)
    for cy in (110, 258):
        for angle in np.linspace(0, math.tau, 6, endpoint=False):
            x, y = 315 + 39 * math.cos(angle), cy + 39 * math.sin(angle)
            m.box(round(x) - 3, round(x) + 3, round(y) - 3, round(y) + 3, 264, 268, DARK)
            # 舱盖周圈的小镜块固定朝外，不随相机转向。
            m.box(round(x) - 2, round(x) + 2, round(y) - 2, round(y) + 2, 267, 269, GLASS)
    # 炮塔后部是封闭储物装甲，有三分区绑带和开口浅篮，无发动机百叶。
    for a, b in ((85, 143), (146, 230), (233, 291)):
        m.box(152, 161, a, b, 199, 251, DARK)
        m.box(152, 157, a + 2, b - 2, 202, 249, OLIVE)
        m.box(152, 162, (a + b) // 2 - 2, (a + b) // 2 + 2, 198, 257, STEEL)
        m.box(154, 177, a, b, 250, 253, SHADE)
        m.box(155, 159, a, b, 253, 263, OLIVE)
        for y in (a, b - 3):
            m.box(155, 177, y, y + 3, 252, 263, OLIVE)
    # 两个闭合低舱盖；车长右侧、装填手左侧。
    for cy in (110, 258):
        m.cylinder(2, (315, cy), 35, 262, 266, DARK)
        m.cylinder(2, (315, cy), 32, 266, 270, PALE)
        m.cylinder(2, (315, cy), 27, 268, 270, OLIVE)
        for angle in np.linspace(0, math.tau, 10, endpoint=False):
            x, y = 315 + 30 * math.cos(angle), cy + 30 * math.sin(angle)
            m.cylinder(2, (x, y), 1.3, 268, 270, METAL)
        m.box(300, 309, cy - 35, cy - 26, 266, 270, STEEL)
        m.box(299, 305, cy + 25, cy + 34, 266, 270, STEEL)
        m.rod((304, cy - 9, 269), (304, cy + 9, 269), 1, DARK)
    # PERI 在右舱盖后方；小底座 + 紧凑矩形罩，单一向前暗玻璃窗。
    m.cylinder(2, (268, 110), 15, 264, 274, DARK)
    m.cylinder(2, (268, 110), 12, 274, 279, METAL)
    m.box(252, 285, 94, 126, 277, 301, DARK)
    m.box(253, 286, 96, 124, 280, 303, OLIVE)
    m.box(283, 287, 100, 120, 282, 299, BLACK)
    m.box(286, 287, 102, 118, 284, 297, GLASS)
    m.box(286, 287, 102, 107, 291, 297, GLINT)
    m.box(250, 289, 93, 127, 300, 303, PALE)
    # EMES 固定在炮塔前右，另一个小型辅助镜留在炮盾旁。
    m.box(432, 469, 98, 135, 254, 276, DARK)
    m.box(433, 471, 99, 134, 274, 279, PALE)
    m.box(468, 472, 103, 130, 259, 273, BLACK)
    for ya, yb in ((105, 116), (119, 128)):
        m.box(471, 472, ya, yb, 262, 271, GLASS)
        m.box(471, 472, ya, ya + 2, 267, 271, GLINT)
    m.box(517, 535, 222, 240, 237, 252, DARK)
    m.box(534, 536, 226, 237, 240, 249, GLASS)
    # 装填手机枪位于左舱盖旁，固定向前；外形保持 MG3 的简洁长枪管。
    m.cylinder(2, (302, 299), 7, 260, 268, STEEL)
    m.cylinder(2, (302, 299), 3, 268, 284, METAL)
    m.box(286, 328, 294, 303, 282, 292, STEEL)
    m.box(295, 323, 291, 306, 288, 296, TRACK)
    m.box(276, 290, 296, 302, 282, 289, RUBBER)
    m.box(312, 322, 283, 294, 280, 291, DARK)
    m.cylinder(0, (299, 288), 3.5, 327, 363, STEEL)
    m.cylinder(0, (299, 288), 2, 363, 385, METAL)
    m.cylinder(0, (299, 288), 2.6, 382, 386, TRACK, inner=1)
    for x in (337, 346, 355):
        m.box(x, x + 3, 295, 303, 287, 290, BLACK)
    m.box(314, 319, 298, 300, 295, 299, METAL)
    # 后部双天线的薄杆和不同粗细绝缘底座，完整表现而非旧画布高度裁切。
    for cy in (65, 311):
        m.cylinder(2, (191, cy), 10, 262, 268, DARK)
        m.cylinder(2, (191, cy), 7, 268, 280, OLIVE)
        m.cylinder(2, (191, cy), 4, 280, 291, RUBBER)
        m.cylinder(2, (191, cy), 2, 291, 328, STEEL)
        m.cylinder(2, (191, cy), 1, 328, 438, MOSS)
        for z in (339, 380, 421):
            m.cylinder(2, (191, cy), 1, z, z + 3, WORN)
    m.features["turret"] = {"length_cm": 460, "width_cm": 300, "roof_z_cm": 264, "hatches_z_cm": 270,
                            "commander_side": "right", "peri_top_z_cm": 303, "antenna_top_z_cm": 438,
                            "smoke_tubes_per_side": 8, "antenna_height_is_estimate": True}


def gun(m):
    """L55 外露段：炮盾、伸缩连接、抽烟器、隔热套和贯通 120 mm 炮膛；无炮口制退器。"""
    m.group("gun")
    m.cylinder(0, (188, 208), 24, 566, 606, DARK)
    m.cylinder(0, (188, 208), 22, 587, 618, PALE)
    m.cylinder(0, (188, 208), 18, 608, 634, OLIVE)
    for a, b, r in ((630, 637, 14), (637, 644, 12), (644, 651, 11)):
        m.cylinder(0, (188, 208), r, a, b, TRACK)
    m.cylinder(0, (188, 208), 10, 649, 1085, SHADE)
    # 炮管隔热套采用分节低反差色面；中段抽烟器只出现一次。
    for a, b in ((651, 698), (763, 843), (848, 928), (933, 1013), (1018, 1081)):
        m.cylinder(0, (188, 208), 9, a, b, PALE)
        m.cylinder(0, (188, 208), 10, b, b + 4, STEEL)
        m.box(a + 3, b - 3, 186, 190, 216, 218, OLIVE)
    m.cylinder(0, (188, 208), 14, 701, 757, METAL)
    m.cylinder(0, (188, 208), 15, 701, 707, TRACK)
    m.cylinder(0, (188, 208), 15, 751, 758, TRACK)
    m.cylinder(0, (188, 208), 13, 708, 750, MOSS)
    m.cylinder(0, (188, 208), 9, 1084, 1097, OLIVE)
    m.cylinder(0, (188, 208), 8, 1094, 1097, METAL)
    # 深孔连至炮盾内；端面空格代表真实孔洞。
    m.cylinder(0, (188, 208), 6, 619, 1097, 0)
    m.features["gun"] = {"axis_cm": [None, 188, 208], "muzzle_x_cm": 1097, "bore_diameter_cm": 12,
                         "fume_extractor_x_cm": [701, 758], "muzzle_brake": False}


def finish(m):
    """结构优先的低反差分块色变化；不用满表面噪点或画上假的体素网格。"""
    for x0 in range(0, SIZE[0], 32):
        x1 = min(x0 + 32, SIZE[0])
        c = m.color[x0:x1]
        x, y, z = np.ogrid[x0:x1, :SIZE[1], :SIZE[2]]
        pattern = ((x // 13) * 17 + (y // 11) * 7 + (z // 9) * 13) % 31
        c[(c == OLIVE) & (pattern == 0)] = PALE
        c[(c == OLIVE) & (pattern == 1)] = SHADE
    m.shell()


def build():
    m = Model()
    for fn in (hull, running_gear, turret, gun, finish):
        print(fn.__name__, flush=True)
        fn(m)
    report = write_vox(m, ROOT / "leopard_2a6.vox")
    report["dimensions_m"] = {"hull_length": 7.66, "gun_forward_length": 10.97, "width": 3.76,
                               "deck_height": 1.60, "turret_roof_height": 2.64, "hatch_height": 2.70,
                               "complete_height_with_antennas": 4.38}
    (ROOT / "model_report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"完成：{report['scene_chunks']} 叶块，{sum(p['voxels'] for p in report['parts']):,} 体素，{report['file_bytes']:,} 字节", flush=True)


def chunk(kind, content):
    return kind + struct.pack("<II", len(content), 0) + content


def dictionary(values):
    out = struct.pack("<i", len(values))
    for key, value in values.items():
        for item in (key, str(value)):
            encoded = item.encode("utf-8")
            out += struct.pack("<i", len(encoded)) + encoded
    return out


def transform(node, name, child, translation):
    return chunk(b"nTRN", struct.pack("<i", node) + dictionary({"_name": name}) +
                 struct.pack("<iiii", child, -1, 0, 1) + dictionary({"_t": " ".join(map(str, translation))}))


def write_vox(m, path):
    """整车不受 256 格限制；标准 XYZI 的字节坐标只用于无损的场景叶块。"""
    blocks, stats = [], []
    for name, part in m.names.items():
        occupied = np.argwhere(m.part == part)
        assert len(occupied), name
        lower, upper = occupied.min(axis=0), occupied.max(axis=0) + 1
        stats.append({"name": name, "voxels": len(occupied), "bounds_cm": [lower.tolist(), upper.tolist()]})
        for x in range(int(lower[0]) // 256 * 256, int(upper[0]), 256):
            for y in range(int(lower[1]) // 256 * 256, int(upper[1]), 256):
                for z in range(int(lower[2]) // 256 * 256, int(upper[2]), 256):
                    origin = np.array((x, y, z))
                    select = np.all((occupied >= origin) & (occupied < origin + 256), axis=1)
                    xyz = occupied[select]
                    if not len(xyz):
                        continue
                    origin = xyz.min(axis=0)
                    # 偶数画布保证标准整数 pivot 与加载器的半尺寸网格原点完全一致。
                    size = ((xyz.max(axis=0) - origin + 2) // 2) * 2
                    local = np.zeros((len(xyz), 4), np.uint8)
                    local[:, :3] = xyz - origin
                    local[:, 3] = m.color[tuple(xyz.T)]
                    blocks.append((f"{name}/{x}_{y}_{z}", origin, size, local))
    chunks = [chunk(b"PACK", struct.pack("<I", len(blocks)))]
    for _, _, size, cells in blocks:
        chunks += [chunk(b"SIZE", struct.pack("<III", *size)),
                   chunk(b"XYZI", struct.pack("<I", len(cells)) + cells.tobytes())]
    chunks += [transform(0, "Leopard_2A6_solH", 1, (0, 0, 0)),
               chunk(b"nGRP", struct.pack("<i", 1) + dictionary({}) + struct.pack("<i", len(blocks)) +
                     b"".join(struct.pack("<i", 2 + i * 2) for i in range(len(blocks))))]
    for i, (name, origin, size, _) in enumerate(blocks):
        node = 2 + i * 2
        chunks += [transform(node, name, node + 1, origin + size // 2),
                   chunk(b"nSHP", struct.pack("<i", node + 1) + dictionary({}) +
                         struct.pack("<ii", 1, i) + dictionary({}))]
    rgba = [(*c, 255) for c in COLORS[1:]]
    rgba.extend([(0, 0, 0, 255)] * (256 - len(rgba)))
    chunks.append(chunk(b"RGBA", bytes(c for color in rgba for c in color)))
    for index in range(1, len(COLORS)):
        rough = .94 if index in (RUBBER, TRACK, BLACK) else (.34 if index in (GLASS, GLINT) else .78)
        chunks.append(chunk(b"MATL", struct.pack("<i", index) + dictionary({"_type": "_diffuse", "_rough": rough})))
    body = b"".join(chunks)
    data = b"VOX " + struct.pack("<I", 150) + b"MAIN" + struct.pack("<II", 0, len(body)) + body
    path.write_bytes(data)
    return {"voxel_size_m": .01, "axis": "+X forward, +Y vehicle left, +Z up", "sha256": hashlib.sha256(data).hexdigest(),
            "file_bytes": len(data), "scene_chunks": len(blocks), "parts": stats, "features": m.features}


def self_check():
    """先验证厘米格心、环孔、斜面和跨 256 格拼接，不涉及现有模型。"""
    m = Model((520, 20, 20))
    m.group("test")
    m.box(0, 520, 0, 20, 0, 20, OLIVE)
    m.cylinder(0, (10, 10), 5, 10, 510, STEEL, inner=3)
    assert m.color[260, 10, 10] == OLIVE and m.color[260, 14, 10] == STEEL
    m.shell()
    assert m.color[260, 10, 10] == 0 and m.color[260, 0, 10] == OLIVE


if __name__ == "__main__":
    self_check()
    build()
