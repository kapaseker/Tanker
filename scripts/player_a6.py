"""豹 2A6 阶段轮廓的 0.02 米作者源；共同 VOX +X 向前、+Z 向上。"""
import ast
import hashlib
import itertools
import math
import re
import struct
from pathlib import Path
import subdivide_vox as sv

CONTRACT = (Path(__file__).resolve().parents[1] / 'src/tank/model/player.rs').read_text(encoding='utf-8')


def literal(name):
    """仅解析共享契约的字面量；格式漂移时失败，不执行 Rust 表达式。"""
    return ast.literal_eval(re.search(r'pub const '+name+r':[^=]+=(.*?);', CONTRACT).group(1).strip())


CANVAS = tuple(5 * n for n in literal('CANVAS'))
HULL = literal('VISUAL_HULL_X')
TURRET = literal('TURRET_X')
ROOF = literal('TURRET_TOP')
BARREL = tuple(round(5 * n) for n in literal('BARREL_X'))
BORE = round(5 * literal('BORE_HEIGHT'))
# 主体与楔形附加装甲的接缝位置（资产格）：楔形从前部 430 格开始叠加，
# 接缝 438 格保留为主体的前缘。
WEDGE_JOINT = 438
# 画布扩高的高度全部加在屋顶以上；但 VOX 插件以画布中心为模型原点，扩高后
# 中心比旧画布上移 25 资产格，作者几何必须整体上移同样的量，才能保持轮轴、
# 炮轴与离地高度。运行时用独立轴心 CANVAS_PIVOT（z=13.5 参考格）还原。
Z0 = (CANVAS[2] - 5 * 27) // 2
assert Z0 * 2 == CANVAS[2] - 5 * 27 and Z0 % 5 == 0, '新增高度应一半在屋顶以上'
# 独立轮安装契约：从 Rust 侧 wheel() 调用解析，end 决定局部画布与细节半径。
WHEEL_END = {name: end == 'true' for name, end in
             re.findall(r'wheel\("(\w+)", [\d.]+, [\d.]+, (true|false)\)', CONTRACT)}
# 履带闭环顶段高度，单位资产格；0.84 米，恰与负重轮轮顶相接。
TRACK_TOP = 42
# 三条不对称辐条角度（弧度），沿用粗源约定，避免低速转动混淆。
SPOKE_ANGLES = (0.0, 1.7, 4.0)


def box(cells, x, y, z, color, bevel=0):
    """填半开盒；最后写入者覆盖，0 用于凹槽和炮口挖空。z 以地面为 0 作者化，
    写入资产格时统一加 Z0。"""
    z = (z[0] + Z0, z[1] + Z0)
    if not all(0 <= lo < hi <= size for (lo, hi), size in zip((x, y, z), CANVAS)):
        raise sv.ConvertError(f'作者盒越出共同画布：{x},{y},{z}')
    row = bytes([color]) * (z[1] - z[0])
    for a in range(*x):
        for b in range(*y):
            if min(a-x[0], x[1]-1-a) + min(b-y[0], y[1]-1-b) < bevel:
                continue
            start = (a * CANVAS[1] + b) * CANVAS[2] + z[0]
            cells[start:start + len(row)] = row


def track_rim():
    """梯形闭环（资产格）：下段贴地、顶段 0.84 米与轮顶相接、两端斜坡内收 4 格。

    周长 840 = 齿距 15 × 56，保证接缝处齿图案连续。
    """
    rim = ([(x, TRACK_TOP) for x in range(HULL[0] + 4, HULL[1] - 5)]
           + [(HULL[1] - 1 - (z * 4) // 42, z) for z in range(TRACK_TOP, 0, -1)]
           + [(x, 0) for x in range(HULL[1] - 1, HULL[0] - 1, -1)]
           + [(HULL[0] + (z * 4) // 42, z) for z in range(1, TRACK_TOP)])
    assert len(rim) == len(set(rim)) == 840
    return rim


def wheel_geometry(name):
    """作者化独立轮细网格；返回（局部画布, 体素串）。轴心在画布中心（size/2），
    轮胎深灰（色 3）、轮毂橄榄绿（色 1）、中心盖暗橄榄（色 2）、辐条间透空。
    保留条件为整格最远角不超旋转包络，保证运行时 AABB 与净空契约一致。"""
    end = WHEEL_END[name]
    size = (25, 20, 25) if end else (40, 20, 40)
    # 画布为偶数或奇数时几何中心都在 size/2（资产格连续坐标），保证 AABB 居中。
    center = size[0] / 2.0
    envelope, tire_in, hub, cap = ((13.0, 10.0, 5.0, 2.0) if end
                                   else (20.5, 15.0, 9.0, 3.0))
    cells = bytearray(size[0] * size[1] * size[2])
    for x in range(size[0]):
        for z in range(size[2]):
            # 检查整个格的四个角，不能只查体素中心；否则旋转时越出包络。
            far = max(abs(x - center), abs(x + 1 - center)) ** 2 \
                + max(abs(z - center), abs(z + 1 - center)) ** 2
            if far > envelope ** 2:
                continue
            dx, dz = x + 0.5 - center, z + 0.5 - center
            r = (dx * dx + dz * dz) ** 0.5
            if r >= tire_in:
                color = 3  # 轮胎深灰
            elif r <= cap:
                color = 2  # 中心盖暗橄榄
            elif r <= hub:
                color = 1  # 轮毂橄榄绿
            else:
                color = 0
                for angle in SPOKE_ANGLES:
                    along = dx * math.cos(angle) + dz * math.sin(angle)
                    across = -dx * math.sin(angle) + dz * math.cos(angle)
                    if along > 0.0 and abs(across) < 1.6:
                        color = 1  # 辐条橄榄绿，其余透空
                        break
            if color:
                for y in range(size[1]):
                    cells[(x * size[1] + y) * size[2] + z] = color
    return size, cells


def geometry(name):
    """豹 2A6 车体与炮塔样板；主体、附加装甲、炮盾、后舱和车顶附件分层造型。

    配色约定（三包共享调色板）：色 1 主橄榄绿、色 2 暗橄榄分区/磨损、
    色 3 近黑灰（舱盖、炮盾、天线、灯）、色 4 履带深灰与炮口、色 5 浅灰凸齿。
    车顶附件最高点天线 151 格（3.02 米，照片估算），不再用旧 2.70 米画布压缩；
    高度与物理碰撞分开，见 player.rs 的 COLLISION_MAX。裙板外沿即冻结的
    3.76 米宽边界 Y=[1,189)。
    """
    cells = bytearray(CANVAS[0] * CANVAS[1] * CANVAS[2])
    if name == 'body':
        # 下车体与甲板：车首 82 格（1.64 米）内逐列降低顶面形成真实首上斜面，
        # 其余保持 1.60 米甲板；不用整块斜坡或色带代替首上/甲板分区。
        slope = 470
        box(cells, (HULL[0], slope), (40, 150), (22, 80), 1, 15)
        for x in range(slope, HULL[1]):
            top = 50 + (HULL[1] - x) * 30 // (HULL[1] - slope)
            box(cells, (x, x + 1), (40, 150), (22, top), 1)
            # 斜面上表面覆暗橄榄，随列阶梯下降，读取为装甲表面而非贴附色带。
            box(cells, (x, x + 1), (42, 148), (top - 1, top), 2)
        # 车首下缘暗色前板与牵引钩，给首上折线一个可见的收口。
        box(cells, (542, HULL[1]), (42, 148), (22, 58), 2)
        box(cells, (546, HULL[1]), (52, 138), (34, 42), 3)
        # 车侧装甲分区：三道竖向板缝与下缘少量暗橄榄磨损补丁。
        for x in (240, 310, 380):
            box(cells, (x, x + 2), (40, 44), (22, 80), 2)
            box(cells, (x, x + 2), (146, 150), (22, 80), 2)
        for x, y in ((184, 40), (430, 40), (300, 146)):
            box(cells, (x, x + 16), (y, y + 2), (22, 26), 2)
        # 驾驶员舱盖与两只前照灯：甲板/首上上的低凸附件，位置随斜面高度。
        box(cells, (432, 456), (86, 106), (80, 84), 3, 2)
        box(cells, (516, 524), (58, 68), (60, 66), 3, 2)
        box(cells, (516, 524), (122, 132), (60, 66), 3, 2)
        # 甲板前缘凸棱与后部发动机舱面板，使甲板有层次而非单一平面。
        box(cells, (458, 470), (44, 146), (80, 82), 2)
        box(cells, (176, 240), (46, 134), (80, 82), 3)
        # 发动机格栅：槽宽 0.04、深 0.02、节距 0.14 米，凹入甲板下 2 格，
        # 外沿深灰边框。
        box(cells, (178, 237), (48, 132), (78, 80), 3)
        box(cells, (180, 235), (50, 130), (78, 80), 2)
        for x in range(180, 235, 7):
            box(cells, (x, min(x + 2, 235)), (53, 127), (78, 79), 4)
            box(cells, (x, min(x + 2, 235)), (53, 127), (79, 80), 0)
        # 挡泥板横梁（1.60 米）与分段裙板：裙板自横梁外沿下垂到 1.04 米，
        # 每侧 6 段、段隙 5 格；下缘高于顶齿凸耳 47 格与轮组包络顶 41.5 格。
        for y in ((6, 36), (154, 184)):
            beam = (35, 40) if y[0] == 6 else (150, 155)
            box(cells, HULL, beam, (80, 85), 3)
            for x in range(HULL[0], HULL[1], 50):
                box(cells, (x, min(x + 45, HULL[1])), y, (80, 86), 3, 3)
        for i in range(6):
            x = HULL[0] + 3 + i * 64
            hi = min(x + 59, HULL[1])
            for y in ((1, 6), (184, 189)):
                box(cells, (x, hi), y, (54, 80), 1)
                box(cells, (x, hi), y, (52, 54), 2)
        # 炮塔后舱：独立厚度块，尾板、侧板和舱顶分别成面；与主体在 352 处分阶。
        box(cells, (TURRET[0], 352), (24, 166), (84, 125), 1, 10)
        box(cells, (TURRET[0], TURRET[0] + 5), (40, 150), (95, 121), 2)
        for y in ((24, 28), (162, 166)):
            box(cells, (250, 352), y, (86, 124), 2)
        box(cells, (246, 352), (26, 164), (124, 126), 2)
        # 炮塔座圈暗带；主体到 2.64 米屋顶，前部另加楔形附加装甲。
        box(cells, (250, 470), (20, 170), (80, 85), 2)
        box(cells, (352, 438), (25, 165), (84, ROOF), 1, 8)
        for y in ((25, 30), (160, 165)):
            box(cells, (352, 438), y, (86, ROOF - 2), 2)
        box(cells, (352, 356), (25, 165), (122, ROOF), 2)  # 后舱到主体的竖向接缝
        # 前楔附加装甲：左右两块独立颊甲，中央留出炮盾；颊甲比主体外扩，
        # 顶面自屋顶向前下斜，外缘立面单独成面，形成真实附加厚度。
        cheek_top = ROOF
        for x in range(WEDGE_JOINT, TURRET[1] - 1):
            if x % 6 == 0:
                cheek_top = max(ROOF - 10, cheek_top - 1)
            box(cells, (x, x + 1), (21, 76), (92, cheek_top), 1)
            box(cells, (x, x + 1), (114, 169), (92, cheek_top), 1)
            box(cells, (x, x + 1), (21, 76), (92, 96), 2)
            box(cells, (x, x + 1), (114, 169), (92, 96), 2)
        # 颊甲前脸与外侧立面：深灰收口，和主橄榄正面分开。
        box(cells, (TURRET[1] - 3, TURRET[1]), (21, 76), (92, cheek_top), 3)
        box(cells, (TURRET[1] - 3, TURRET[1]), (114, 169), (92, cheek_top), 3)
        box(cells, (WEDGE_JOINT, TURRET[1]), (21, 24), (96, cheek_top), 3)
        box(cells, (WEDGE_JOINT, TURRET[1]), (166, 169), (96, cheek_top), 3)
        # 炮盾：中央独立厚块，高于颊甲下缘并伸出前脸，炮管从其中心起步。
        box(cells, (438, TURRET[1] - 1), (76, 114), (BORE - 16, BORE + 16), 3, 3)
        box(cells, (440, TURRET[1] - 2), (78, 112), (BORE - 14, BORE + 14), 2)
        box(cells, (444, TURRET[1]), (80, 110), (BORE - 10, BORE + 10), 3)
        # 车顶：车长舱盖、潜望镜、装填手舱盖、炮长瞄具与简化机枪。
        box(cells, (294, 346), (44, 92), (132, 134), 2)
        box(cells, (298, 342), (48, 88), (134, 146), 1, 4)
        box(cells, (302, 338), (52, 84), (146, 148), 3, 3)
        for y in (56, 66, 76):
            box(cells, (340, 342), (y, y + 6), (138, 143), 4)
        box(cells, (350, 382), (40, 70), (132, 134), 2)
        box(cells, (354, 378), (44, 66), (134, 140), 3, 3)
        box(cells, (358, 374), (48, 62), (140, 141), 2)
        box(cells, (356, 392), (100, 134), (132, 136), 3, 4)
        box(cells, (372, 392), (104, 130), (136, 146), 3, 3)
        box(cells, (390, 392), (108, 126), (138, 144), 4)  # 观瞄镜片
        box(cells, (338, 352), (88, 100), (132, 137), 3)
        box(cells, (338, 340), (88, 100), (132, 137), 2)
        # 右侧车顶机枪：基座、受弹机、枪管与枪托各自有体积，低于天线顶。
        box(cells, (318, 330), (120, 134), (132, 138), 3)
        box(cells, (308, 342), (122, 132), (138, 146), 3, 2)
        box(cells, (342, 360), (124, 130), (141, 144), 3)
        box(cells, (308, 316), (122, 132), (138, 144), 2)
        # 车尾双天线：基座在发动机甲板，杆顶按照片估算到 3.02/2.84 米。
        box(cells, (198, 204), (40, 46), (82, 84), 3)
        box(cells, (200, 202), (42, 44), (84, 151), 3)
        box(cells, (256, 262), (140, 146), (85, 87), 3)
        box(cells, (258, 260), (142, 144), (87, 142), 3)
    elif name == 'barrel':
        # L55 风格长炮管：炮盾环、根部加粗段、细长管身、抽烟器与炮口箍；无制退器。
        box(cells, BARREL, (90, 100), (BORE-5, BORE+5), 2)
        box(cells, (BARREL[0], BARREL[0]+30), (85, 105), (BORE-10, BORE+10), 3, 3)
        box(cells, (505, 530), (88, 102), (BORE-7, BORE+7), 2)
        box(cells, (575, 605), (86, 104), (BORE-9, BORE+9), 1, 2)
        box(cells, (708, 713), (89, 101), (BORE-6, BORE+6), 3)
        box(cells, (BARREL[1]-5, BARREL[1]), (90, 100), (BORE-5, BORE+5), 4)
        box(cells, (BARREL[1]-5, BARREL[1]), (92, 98), (BORE-3, BORE+3), 0)
    else:
        side, phase = name.split('_')[1:]
        phase = int(phase)
        y, outer = ((5, 35), (1, 5)) if side == 'l' else ((155, 185), (185, 189))
        # 梯形闭环 840 格 = 齿距 15 × 56；每相位推进 5 格，仍为 0.1 米。
        # 内侧面板已删除，实现轮间透空；顶齿凸耳位于 0.84 米带上方 4 格。
        for i, (x, z) in enumerate(track_rim()):
            tooth = (i - phase*5) % 15 < 5
            box(cells, (x, x+1), y, (z, z+1), 5 if tooth else 4)
            if tooth:
                box(cells, (x, x+1), outer, (z, z+1), 5)
                if z == TRACK_TOP:
                    box(cells, (x, x+1), y, (TRACK_TOP+1, TRACK_TOP+5), 5)
    return cells


def author_package(path):
    """复用现有分块规划和配对发布；履带闭环与独立轮均为 0.02 米作者几何。"""
    source = sv.parse_vox(path.read_bytes(), str(path))
    models, nodes, expected = [], [], {}

    def node(kind, attrs=None):
        result = sv.Node(kind, len(nodes), attrs or {})
        nodes.append(result)
        return result

    root, group = node('trn'), node('grp')
    root.child, root.layer = group.node_id, -1
    for index in source.nodes[source.nodes[0].child].children:
        old = source.nodes[index]
        name = old.node_dict['_name']
        named, chunks = node('trn', {'_name': name}), node('grp')
        named.child, named.layer = chunks.node_id, -1
        group.children.append(named.node_id)
        pieces = []
        if name.startswith('wheel_'):
            # 独立轮不复制粗源细分结果，直接作者化 0.02 米细节；单块即整轮。
            size, wheel_cells = wheel_geometry(name)
            expected[name] = (size, hashlib.sha256(bytes(wheel_cells)).digest())
            data = bytearray()
            for x in range(size[0]):
                for y in range(size[1]):
                    for z in range(size[2]):
                        color = wheel_cells[(x*size[1]+y)*size[2]+z]
                        if color:
                            data.extend((x, y, z, color))
            pieces.append((sv.Model(size, bytes(data)), (0, 0, 0)))
        else:
            cells = geometry(name)
            expected[name] = (CANVAS, hashlib.sha256(cells).digest())
            for plan in itertools.product(*(sv.chunk_axis_plan(n) for n in CANVAS)):
                origin, size = tuple(p[0] for p in plan), tuple(p[1] for p in plan)
                data = bytearray()
                for x in range(size[0]):
                    for y in range(size[1]):
                        start = ((x+origin[0])*CANVAS[1]+y+origin[1])*CANVAS[2]+origin[2]
                        row = cells[start:start+size[2]]
                        if any(row):
                            for z, color in enumerate(row):
                                if color:
                                    data.extend((x, y, z, color))
                if data:
                    offset = tuple(o + (s-f)//2 for o, s, f in zip(origin, size, CANVAS))
                    pieces.append((sv.Model(size, bytes(data)), offset))
        for model, offset in pieces:
            trn, shape = node('trn'), node('shp')
            trn.child, trn.layer = shape.node_id, -1
            trn.frames = [{'_t': ' '.join(map(str, offset))}]
            shape.models = [(len(models), {})]
            chunks.children.append(trn.node_id)
            models.append(model)
    sv.write_vox(models, nodes, source.raw_chunks, path)
    verify_package(path, expected)


def verify_package(path, expected):
    """发布前独立回读：按场景变换重建占用和颜色，拒绝漏块、错移与重叠。"""
    source = sv.parse_vox(path.read_bytes(), str(path))
    root = source.nodes[0]
    names = []
    if root.frames:
        raise sv.ConvertError('作者包根变换必须为恒等')
    for index in source.nodes[root.child].children:
        named = source.nodes[index]
        name = named.node_dict.get('_name')
        if name not in expected or name in names or named.frames:
            raise sv.ConvertError(f'作者包命名根或原点异常：{name}')
        names.append(name)
        full, digest = expected[name]
        cells = bytearray(math.prod(full))
        for child in source.nodes[named.child].children:
            trn = source.nodes[child]
            if any(set(frame) - {'_t'} for frame in trn.frames):
                raise sv.ConvertError(f'分块含非平移变换：{name}')
            model = source.models[source.nodes[trn.child].models[0][0]]
            if any((f-s) % 2 for f, s in zip(full, model.size)):
                raise sv.ConvertError(f'分块画布无法整格居中：{name}')
            origin = tuple(t+(f-s)//2 for t, f, s in zip(sv.frame_t(trn.frames), full, model.size))
            for x, y, z, color in struct.iter_unpack('<BBBB', model.xyzi):
                x, y, z = x+origin[0], y+origin[1], z+origin[2]
                if not (0 <= x < full[0] and 0 <= y < full[1] and 0 <= z < full[2]):
                    raise sv.ConvertError(f'分块越出作者画布：{name}')
                position = (x*full[1]+y)*full[2]+z
                if cells[position]:
                    raise sv.ConvertError(f'分块体素重叠：{name}')
                cells[position] = color
        if hashlib.sha256(cells).digest() != digest:
            raise sv.ConvertError(f'作者几何回读不一致：{name}')
    if set(names) != set(expected):
        raise sv.ConvertError('作者包缺少命名根')
