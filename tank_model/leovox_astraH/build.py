"""独立豹 2A6 外观作者化：坐标格 2 cm，+X 车首、+Y 左、+Z 上。

仅依赖 numpy；不读取工程内任何已有坦克模型。所有区间均为左闭右开。
"""
from pathlib import Path
import hashlib
import json
import math
import struct

import numpy as np

OUT = Path(__file__).resolve().parent
UNIT = 0.02
# 包含完整长炮和天线的统一作者画布；Y 下标相对车中线偏移 94。
SHAPE = (549, 188, 224)
GRID = np.zeros(SHAPE, np.uint8)
OWNER = np.zeros(SHAPE, np.uint8)
PARTS = []
CURRENT = 0
OLIVE, DARK_OLIVE, LIGHT_OLIVE, EDGE = 1, 2, 3, 4
RUBBER, STEEL, METAL, BLACK, GLASS, GLINT = 5, 6, 7, 8, 9, 10
RED, AMBER, LAMP, DUST, BROWN = 11, 12, 13, 14, 15
PALETTE = [
    (112, 119, 72), (89, 98, 58), (125, 130, 80), (150, 144, 94),
    (36, 38, 35), (57, 59, 53), (91, 89, 75), (22, 26, 24),
    (39, 59, 62), (111, 143, 145), (139, 39, 28), (198, 126, 33),
    (220, 221, 197), (117, 107, 82), (83, 77, 60),
    (109, 116, 69), (115, 122, 75), (122, 127, 77), (128, 133, 83),
]


def part(name):
    """命名装配分组，导出时保留为独立场景节点。"""
    global CURRENT
    PARTS.append(name)
    CURRENT = len(PARTS)


def region(lo, hi):
    """物理坐标区间映射到数组，越界直接报错，防止静默截断零件。"""
    a = np.array(lo, dtype=int) + (0, 94, 0)
    b = np.array(hi, dtype=int) + (0, 94, 0)
    assert np.all(a >= 0) and np.all(b <= SHAPE), (lo, hi)
    assert np.all(b > a), (lo, hi)
    return tuple(slice(int(l), int(h)) for l, h in zip(a, b))


def box(lo, hi, color):
    sl = region(lo, hi)
    GRID[sl] = color
    OWNER[sl] = CURRENT if color else 0


def masked(lo, hi, predicate, color):
    """在体素中心采样几何；同一格只有一个最终材质和所属部件。"""
    sl = region(lo, hi)
    xyz = np.ogrid[tuple(slice(a, b) for a, b in zip(lo, hi))]
    mask = np.broadcast_to(predicate(*(v + 0.5 for v in xyz)), GRID[sl].shape)
    GRID[sl][mask] = color
    OWNER[sl][mask] = CURRENT if color else 0


def cylinder(axis, c, start, end, radius, color, inner=0, sides=0):
    """轴向圆柱/环；c 为另两轴的中心，sides 可指定真实棱面。"""
    others = [i for i in range(3) if i != axis]
    lo, hi = [0]*3, [0]*3
    lo[axis], hi[axis] = start, end
    for i, v in zip(others, c):
        lo[i], hi[i] = math.floor(v-radius), math.ceil(v+radius)

    def inside(*xyz):
        a, b = [xyz[i]-v for i, v in zip(others, c)]
        r = radius
        if sides:
            angle = np.arctan2(b, a)
            r = radius * math.cos(math.pi/sides) / np.cos((angle+math.pi/sides) % (2*math.pi/sides)-math.pi/sides)
        d = a*a+b*b
        return (d <= r*r) & (d >= inner*inner)
    masked(lo, hi, inside, color)


def beam(a, b, radius, color):
    """任意方向平端圆杆，用于烟幕筒、扶手、牵引索与悬挂。"""
    a, b = np.array(a, float), np.array(b, float)
    d = b-a
    length = np.linalg.norm(d)
    d /= length
    lo = np.maximum(np.floor(np.minimum(a,b)-radius).astype(int), (0,-94,0))
    hi = np.minimum(np.ceil(np.maximum(a,b)+radius).astype(int), (549,94,224))
    def inside(x,y,z):
        v = [x-a[0],y-a[1],z-a[2]]
        t = sum(v[i]*d[i] for i in range(3))
        q = sum(v[i]*v[i] for i in range(3))-t*t
        return (t >= 0) & (t < length) & (q <= radius*radius)
    masked(lo,hi,inside,color)


def polygon(points, low, high, color, axis=2):
    """二维凸/凹多边形挤出，使用奇偶规则测试体素中心。"""
    others = [i for i in range(3) if i != axis]
    lo, hi = [0]*3, [0]*3
    lo[axis], hi[axis] = low, high
    for j,i in enumerate(others):
        lo[i] = math.floor(min(p[j] for p in points))
        hi[i] = math.ceil(max(p[j] for p in points))
    def inside(*xyz):
        u,v = xyz[others[0]],xyz[others[1]]
        mask = np.zeros(np.broadcast_shapes(u.shape,v.shape), bool)
        for p,q in zip(points,points[1:]+points[:1]):
            if p[1] != q[1]:
                mask ^= ((p[1] > v) != (q[1] > v)) & (u < (q[0]-p[0])*(v-p[1])/(q[1]-p[1])+p[0])
        return mask
    masked(lo,hi,inside,color)


def handle(x,y,z, length=9, along=0):
    """离面一格的 U 形扶手，留出可读空隙。"""
    a = np.array([x,y,z],float)
    b = a.copy(); b[along] += length
    beam(a,a+(0,0,3),1,STEEL)
    beam(b,b+(0,0,3),1,STEEL)
    beam(a+(0,0,3),b+(0,0,3),1,STEEL)


def hull():
    part('hull')
    # 船形下车体：倾斜侧壁与首尾下装甲，底部保持离地。
    for z in range(22,66):
        w = min(69, 51+(z-22)//2)
        x0 = max(3, 19-(z-22)//2)
        x1 = min(383, 359+(z-22)//2)
        box((x0,-w,z),(x1,w,z+1),DARK_OLIVE)
    profile=[(0,55),(0,80),(291,80),(383,59),(383,43),(364,27),(17,27)]
    polygon(profile,-77,77,OLIVE,axis=1)
    # 后甲板与车首斜面，面板缝为真实浅凹槽。
    for x in range(291,383):
        z = math.floor(80-(x-291)*21/92)
        box((x,-77,z-3),(x+1,77,z),LIGHT_OLIVE)
    for x in [294,320,347,373]:
        z = math.floor(80-(x-291)*21/92)
        box((x,-69,z-2),(x+1,69,z),DARK_OLIVE)
    for y in [-49,0,49]:
        for x in range(292,379):
            z = math.floor(80-(x-291)*21/92)
            box((x,y,z-1),(x+1,y+1,z),DARK_OLIVE)
    for side in [-1,1]:
        yl,yh = sorted([side*74,side*94])
        polygon([(0,70),(0,80),(299,80),(383,61),(383,55),(296,74)],yl,yh,OLIVE,axis=1)
        # 挡泥板底沿、侧面工具箱和九段有独立铰链的裙甲。
        for j,(a,b) in enumerate(zip([3,44,87,130,173,216,259,302,343],[43,86,129,172,215,258,301,342,380])):
            ya,yb=sorted([side*88,side*94])
            top = min(77,math.floor(80-max(0,b-291)*21/92))
            bottom = 49 if j < 7 else 46
            box((a,ya,bottom),(b,yb,top),LIGHT_OLIVE if j%3==1 else OLIVE)
            # 重装甲前段多一层阶梯折边。
            if j >= 6:
                box((a+2,ya,bottom),(b-2,yb,bottom+2),DARK_OLIVE)
            for x in [a+3,b-5]:
                box((x,ya,top-5),(x+3,yb,top+1),STEEL)
                y = side*93
                box((x+1,min(y,y+side),top-3),(x+2,max(y,y+side),top-2),METAL)
            ya,yb=sorted([side*78,side*88])
            if b<295:
                box((a+2,ya,78),(b-2,yb,82),OLIVE)
                box((a+5,ya+2,82),(a+9,yb-2,83),STEEL)
        # 前圆灯、保护框、外侧示廓灯，前后挂点不超出车体长度。
        cy=side*66
        box((364,cy-8,60),(376,cy+8,74),DARK_OLIVE)
        cylinder(0,(cy,67),373,378,6,STEEL)
        cylinder(0,(cy,67),377,379,4.5,LAMP)
        cylinder(0,(cy-1,68),378,379,2,GLINT)
        for y in [cy-8,cy+7]:
            box((362,y,61),(379,y+1,77),STEEL)
        box((362,cy-8,76),(379,cy+8,77),STEEL)
        box((375,cy-side*13-2,64),(379,cy-side*13+2,69),BLACK)
        box((378,cy-side*13-1,65),(379,cy-side*13+1,68),GLASS)
        # 前牵引耳：竖向空心环和横向销轴。
        cy=side*48
        box((375,cy-5,35),(382,cy+5,48),OLIVE)
        box((377,cy-2,38),(383,cy+2,44),0)
        cylinder(1,(378,47),cy-7,cy+7,2.5,METAL)
    # 右前驾驶员舱盖和三联潜望镜。
    box((287,-46,79),(316,-20,82),DARK_OLIVE)
    box((289,-44,81),(314,-22,83),OLIVE)
    for y in [-42,-34,-26]:
        box((311,y,82),(319,y+6,86),OLIVE)
        box((318,y+1,83),(319,y+5,85),GLASS)
    handle(293,-40,83,12,1)
    # 上甲板检修舱、螺栓和边缘焊缝。
    for x in [272,300,330,355]:
        z=80 if x<291 else math.floor(80-(x-291)*21/92)
        for y in [-58,54]:
            box((x,y,z),(x+5,y+4,z+2),STEEL)
            box((x+1,y+1,z+2),(x+3,y+3,z+3),METAL)


def running_gear():
    # 单独保留每个轮组，左右分别作者化；轴心严格沿用 0.42 m 口径。
    for side,label in [(-1,'right'),(1,'left')]:
        ya,yb=sorted([side*67,side*87])
        outer = yb if side>0 else ya
        for i,cx in enumerate([58,103,147,192,236,281,325]):
            part(f'{label}_roadwheel_{i+1}')
            # 双胎中央窄缝、层叠轮辋、八颗紧固件。
            cylinder(1,(cx,21),ya,yb,20,RUBBER)
            cylinder(1,(cx,21),(ya+yb)//2-1,(ya+yb)//2+1,20,BLACK,inner=18)
            for face,recess in [(ya,ya+2),(yb-2,yb-4)]:
                # 轮辋凹入两格，轮缘、轮毂和紧固件保持外凸，避免平面彩色圆片。
                cylinder(1,(cx,21),face,face+2,16.8,0)
                cylinder(1,(cx,21),recess,recess+2,16.8,DARK_OLIVE,sides=16)
                cylinder(1,(cx,21),recess,recess+2,13.5,OLIVE)
                cylinder(1,(cx,21),face,face+2,16.8,LIGHT_OLIVE,inner=15.2,sides=16)
                for k in range(8):
                    t=2*math.pi*k/8
                    cylinder(1,(cx+10.2*math.cos(t),21+10.2*math.sin(t)),face,face+2,1.2,METAL)
                    beam((cx+5*math.cos(t),face+1,21+5*math.sin(t)),(cx+13*math.cos(t),face+1,21+13*math.sin(t)),1.2,OLIVE)
                cylinder(1,(cx,21),face,face+2,6,LIGHT_OLIVE,sides=8)
                cylinder(1,(cx,21),face,face+2,3.2,STEEL,sides=8)
                cylinder(1,(cx,21),face,face+2,1.5,METAL)
        for role,cx in [('sprocket',18),('idler',365)]:
            part(f'{label}_{role}')
            cylinder(1,(cx,21),ya,yb,12.5,STEEL,sides=16)
            face=outer-2 if side>0 else outer
            cylinder(1,(cx,21),face,face+2,10.5,OLIVE,sides=12)
            cylinder(1,(cx,21),face,face+2,8.5,BLACK)
            for k in range(10):
                t=k*2*math.pi/10
                p=(cx+6*math.cos(t),21+6*math.sin(t))
                cylinder(1,p,face,face+2,2.5,OLIVE)
                if role=='sprocket':
                    p=(cx+12.3*math.cos(t),21+12.3*math.sin(t))
                    cylinder(1,p,face,face+2,2.2,METAL)
            cylinder(1,(cx,21),face,face+2,4.5,LIGHT_OLIVE,sides=8)
            cylinder(1,(cx,21),face,face+2,1.8,STEEL)
        part(f'{label}_suspension')
        for cx in [58,103,147,192,236,281,325]:
            beam((cx-12,side*65,34),(cx,side*68,21),3,STEEL)
        for cx in [76,166,256,339]:
            cylinder(1,(cx,42),ya+3,yb-3,5,RUBBER)
        part(f'{label}_track')
        ya,yb=sorted([side*64,side*94])
        # 连续内带承力，单独构造每块履带板和两片橡胶垫；端部随路径转向。
        path=np.array([(1,21),(17,2),(366,2),(382,21),(366,45),(17,45)],float)
        for a,b in zip(path,np.roll(path,-1,axis=0)):
            d=b-a; length=np.linalg.norm(d); tangent=d/length
            normal=np.array([tangent[1],-tangent[0]])
            # 轴高 0.42 m 与轮径 0.80 m 仅余 2 cm，底履带按尺寸文档用单格接触层。
            if a[1]==b[1]==2:
                box((int(a[0]),ya,0),(int(b[0]),yb,1),STEEL)
                for x in range(int(a[0]),int(b[0])-5,7):
                    box((x,ya,0),(x+5,yb,1),METAL)
                    for y in [ya+3,ya+17]:
                        box((x+1,y,0),(x+5,y+10,1),RUBBER)
                continue
            points=[tuple(a-normal*1),tuple(b-normal*1),tuple(b+normal*1),tuple(a+normal*1)]
            polygon(points,ya+1,yb-1,STEEL,axis=1)
            count=math.ceil(length/7)
            for i in range(count):
                c=a+d*(i+0.5)/count
                # 预留齿距间隙；板体包络不超车体前后端。
                def tread(depth, width, ca,cb,color):
                    ps=[c-tangent*width+normal*(depth-1),c+tangent*width+normal*(depth-1),c+tangent*width+normal*depth,c-tangent*width+normal*depth]
                    ps=[(float(np.clip(p[0],0,383)),float(max(0,p[1]))) for p in ps]
                    polygon(ps,ca,cb,color,axis=1)
                tread(1.4,2.9,ya,yb,METAL)
                tread(2,2.5,ya+3,ya+13,RUBBER)
                tread(2,2.5,ya+17,yb-3,RUBBER)
                tread(-1.2,1.5,ya+13,ya+17,STEEL)


def engine_deck():
    part('engine_deck')
    # 两个真实镂空间隙的同心圆格栅，下方有斜向叶片与黑色风道。
    for cy in [-37,37]:
        box((8,cy-31,78),(72,cy+31,81),DARK_OLIVE)
        box((10,cy-29,80),(70,cy+29,82),OLIVE)
        cylinder(2,(40,cy),79,83,28,BLACK)
        for k in range(8):
            t=k*math.pi/4
            beam((40+8*math.cos(t),cy+8*math.sin(t),80.5),(40+24*math.cos(t+0.35),cy+24*math.sin(t+0.35),80.5),2.5,STEEL)
        for r in [6,12,18,24,28]:
            cylinder(2,(40,cy),82,84,r,METAL,inner=r-1.1)
        for k in range(12):
            t=k*math.pi/6
            beam((40,cy,83),(40+27*math.cos(t),cy+27*math.sin(t),83),0.65,STEEL)
        cylinder(2,(40,cy),82,85,3.5,METAL,sides=8)
        for x in [11,66]:
            for y in [cy-28,cy+26]:
                box((x,y,82),(x+3,y+3,83),METAL)
    box((15,-7,81),(67,7,83),OLIVE)
    handle(24,-3,83,13,0)
    # 主风扇两侧的辅助进风口与工具，不增加主风扇数量。
    for side in [-1,1]:
        ya,yb=sorted([side*71,side*83])
        box((16,ya,82),(62,yb,84),STEEL)
        for x in range(18,61,3):
            box((x,ya+1,84),(x+1,yb-1,85),METAL)
        beam((75,side*80,84),(197,side*80,84),1.3,BROWN)
        box((75,min(side*84,side*76),83),(84,max(side*84,side*76),86),STEEL)
        for x in [93,154,193]:
            box((x, min(side*83,side*77),83),(x+2,max(side*83,side*77),86),STEEL)
    # 五分区后百叶：整条深色底面，结构框不变成炮塔尾格栅。
    box((0,-69,51),(4,69,77),BLACK)
    for z in range(53,76,3):
        box((0,-68,z),(3,68,z+1),METAL)
    for y in [-69,-42,-14,14,42,68]:
        box((0,y,51),(4,y+1,78),LIGHT_OLIVE)
    box((0,-69,77),(4,69,79),EDGE)
    for side in [-1,1]:
        cy=side*81
        box((0,cy-10,60),(6,cy+10,76),DARK_OLIVE)
        cylinder(0,(side*75,68),0,3,4,STEEL)
        cylinder(0,(side*75,68),0,1,2.5,METAL)
        yl,yh=sorted([side*80,side*90])
        box((0,yl,65),(2,yh,72),BLACK)
        yl,yh=sorted([side*81,side*85])
        box((0,yl,66),(1,yh,71),RED)
        yl,yh=sorted([side*85,side*89])
        box((0,yl,66),(1,yh,71),AMBER)
        # 后牵引环和中央拖钩，形成三个位置明确的挂点。
        cy=side*49
        box((0,cy-6,31),(7,cy+6,47),LIGHT_OLIVE)
        box((0,cy-3,34),(5,cy+3,42),0)
        cylinder(1,(4,46),cy-7,cy+7,2,METAL)
    box((0,-7,31),(6,7,49),STEEL)
    box((0,-4,28),(4,4,42),METAL)
    box((0,-2,32),(3,2,37),BLACK)


def turret():
    part('turret')
    cylinder(2,(192,0),80,85,69,BLACK)
    cylinder(2,(192,0),84,87,67,METAL)
    # 炮塔基础盒体的上下倒角与前端收束。
    for z in range(87,132):
        inset=max(0,(94-z)//2,(z-124)//2)
        points=[(76+inset,-57),(86+inset,-69+inset),(213,-69+inset),(262,-48),(283,-17),(283,17),(262,48),(213,69-inset),(86+inset,69-inset),(76+inset,57)]
        polygon(points,z,z+1,OLIVE)
    # 左右楔甲在水平和垂直方向都倾斜，前缘形成真实箭头截面。
    for side in [-1,1]:
        for z in range(89,131):
            tip=306-max(0,z-106)*1.08-max(0,106-z)*0.95
            points=[(215,side*70),(232,side*74),(tip-30,side*66),(tip,side*17),(265,side*17)]
            polygon(points,z,z+1,LIGHT_OLIVE if z>106 else DARK_OLIVE)
        # 楔甲上层两个模块的分隔缝，保留装甲厚度。
        for z in range(109,130):
            tip=306-(z-106)*1.08
            y=side*44
            x=int(tip-0.60*(44-17))
            box((x-1,min(y,y+side),z),(x+2,max(y,y+side),z+1),DARK_OLIVE)
        # 侧面装甲板和四块固定挂座。
        ya,yb=sorted([side*69,side*75])
        for a,b in [(91,126),(129,170),(173,211)]:
            box((a,ya,93),(b,yb,128),OLIVE)
            box((a+1,ya,128),(b-1,yb,130),LIGHT_OLIVE)
        for x in [201,215]:
            for z in [104,118]:
                ya,yb=sorted([side*70,side*75])
                box((x,ya,z),(x+11,yb,z+10),DARK_OLIVE)
                y=side*74
                box((x+4,min(y,y+side),z+4),(x+7,max(y,y+side),z+7),METAL)
    # 车顶嵌板，分缝浅一格；主体屋顶仍严格在 132 格。
    for x in [99,139,184,221,249]:
        box((x,-57,131),(x+1,57,132),DARK_OLIVE)
    for y in [-48,-18,18,48]:
        box((100,y,131),(240,y+1,132),DARK_OLIVE)
    for x in [104,146,188,227]:
        for y in [-54,52]:
            box((x,y,132),(x+2,y+2,133),METAL)
    # 炮塔尾为三只实心储物箱，不复制动力舱百叶。
    for ya,yb in [(-59,-29),(-27,27),(29,59)]:
        box((76,ya,96),(84,yb,127),DARK_OLIVE)
        box((76,ya+1,98),(79,yb-1,125),OLIVE)
        box((77,ya+1,125),(85,yb-1,129),LIGHT_OLIVE)
        box((76,(ya+yb)//2-2,113),(78,(ya+yb)//2+2,119),STEEL)
    # 尾部两条牵引索：固定在后半炮塔两侧，不漂浮。
    for side in [-1,1]:
        beam((88,side*64,131),(149,side*64,131),1,STEEL)
        beam((88,side*64,131),(83,side*49,131),1,STEEL)
        for x in [95,138]:
            box((x, min(side*66,side*62),130),(x+2,max(side*66,side*62),133),DARK_OLIVE)


def armament():
    part('L55_gun')
    # 防盾、耳轴护罩、热护套、抽烟器、卡箍与无遮罩空心炮口。
    cylinder(0,(0,104),278,301,13,STEEL,sides=8)
    cylinder(0,(0,104),283,309,11,OLIVE,sides=8)
    cylinder(0,(0,104),309,328,7.5,LIGHT_OLIVE,sides=8)
    cylinder(0,(0,104),328,355,9.5,STEEL,sides=8)
    cylinder(0,(0,104),331,351,10,OLIVE,sides=8)
    cylinder(0,(0,104),355,543,5.5,LIGHT_OLIVE,sides=8)
    for x in [310,326,354,394,437,480,527]:
        cylinder(0,(0,104),x,x+2,6.4 if x>354 else 8,STEEL,sides=8)
        box((x,-2,109),(x+2,2,111),METAL)
    cylinder(0,(0,104),540,549,6.3,OLIVE,sides=8)
    cylinder(0,(0,104),541,549,3.2,0)
    cylinder(0,(0,104),540,541,3.2,BLACK)
    # 炮口校准镜支架保持小尺寸。
    box((540,-2,110),(545,2,112),DARK_OLIVE)
    box((540,-1,111),(542,1,112),GLASS)


def roof_fittings():
    # 面向车首，车长在右（负 Y），装填手及机枪在左（正 Y）。
    for side,label in [(-1,'commander'),(1,'loader')]:
        part(label+'_hatch')
        cy=side*32
        cylinder(2,(155,cy),131,133,19,STEEL,sides=16)
        cylinder(2,(155,cy),133,135,17.5,OLIVE,sides=16)
        cylinder(2,(155,cy),134,135,14,LIGHT_OLIVE,sides=16)
        for k in range(8):
            t=k*math.pi/4
            x,y=155+15*math.cos(t),cy+15*math.sin(t)
            cylinder(2,(x,y),134,135,1,METAL)
        box((137,cy-5,133),(143,cy+5,135),DARK_OLIVE)
        # 低矮舱盖本体到 2.70 m；握把作为附件单独计高。
        handle(151,cy-4,135,8,1)
        for y in [cy-11,cy-3,cy+5]:
            box((174,y,132),(180,y+6,136),DARK_OLIVE)
            box((179,y+1,133),(180,y+5,135),GLASS)
    part('sights')
    # 紧凑 PERI，高度 3.02 m，玻璃固定朝前；不采用失败候选的中央高塔。
    cylinder(2,(182,-34),131,137,8,STEEL,sides=12)
    box((176,-41,136),(190,-27,150),OLIVE)
    box((189,-39,139),(191,-29,148),BLACK)
    box((190,-38,140),(191,-30,147),GLASS)
    box((190,-37,144),(191,-33,146),GLINT)
    box((175,-42,149),(192,-26,151),LIGHT_OLIVE)
    # 前方主瞄准镜低矮箱体，两个真实凹入的光学窗口。
    box((236,-43,130),(251,-20,139),OLIVE)
    box((249,-41,132),(252,-22,137),BLACK)
    box((250,-40,133),(251,-33,136),GLASS)
    box((250,-31,133),(251,-24,136),GLASS)
    box((235,-44,138),(253,-19,140),LIGHT_OLIVE)
    box((248,33,129),(257,42,136),DARK_OLIVE)
    box((256,35,131),(257,40,134),GLASS)
    part('loader_machine_gun')
    cylinder(2,(168,37),135,145,2.5,STEEL)
    box((164,33,144),(180,39,150),STEEL)
    box((158,34,146),(167,38,149),BROWN)
    box((172,27,144),(180,33,150),DARK_OLIVE)
    cylinder(0,(36,147),180,198,1.5,METAL)
    cylinder(0,(36,147),196,200,2,STEEL)
    box((184,35,147),(186,37,152),STEEL)
    box((166,34,150),(169,38,152),STEEL)
    part('antennas')
    for side in [-1,1]:
        cy=side*58
        box((99,cy-5,131),(111,cy+5,136),OLIVE)
        cylinder(2,(105,cy),135,144,3.5,STEEL,sides=8)
        cylinder(2,(105,cy),144,157,2,METAL)
        # 双天线完整建模到 4.32 m，格宽 2 cm 是可表达的最小杆径。
        box((104,cy-1,155),(106,cy+1,178),DARK_OLIVE)
        box((105,cy,178),(106,cy+1,216),DARK_OLIVE)
        box((105,cy,215),(106,cy+1,216),EDGE)
    for side,label in [(-1,'right'),(1,'left')]:
        part(label+'_smoke_launchers')
        ya,yb=sorted([side*70,side*77])
        box((101,ya,98),(150,yb,123),DARK_OLIVE)
        # 两排各四筒，以同一侧向外上方固定发射朝向，所有视角几何一致。
        for z in [104,117]:
            for x in [107,119,131,143]:
                a=np.array([x,side*74,z],float)
                b=np.array([x+1,side*84,z+4],float)
                beam(a,b,4,STEEL)
                beam(b-(b-a)*0.17,b,3.1,METAL)
                beam(b-(b-a)*0.04,b+(b-a)*0.03,1.7,BLACK)
    part('roof_hardware')
    for x,y in [(108,-38),(108,30),(212,-57),(212,48)]:
        handle(x,y,132,8,1)
    for x,y in [(122,0),(210,6)]:
        box((x-7,y-6,132),(x+7,y+6,134),DARK_OLIVE)
        box((x-6,y-5,134),(x+6,y+5,135),OLIVE)


def finish_paint():
    """轻微色差与少量棱边磨损；避免全表面随机斑点吞没结构。"""
    occupied=GRID!=0
    neighbors=np.zeros(SHAPE,np.uint8)
    for axis in range(3):
        for sign in [-1,1]:
            neighbors += np.roll(occupied,sign,axis)
    x,y,z=np.indices(SHAPE,sparse=True)
    coarse=((x//13)*73856093 ^ (y//11)*19349663 ^ (z//9)*83492791) % 23
    mask=(GRID==OLIVE)&(neighbors<6)
    GRID[mask & (coarse==0)]=16
    GRID[mask & (coarse==1)]=17
    mask=(GRID==LIGHT_OLIVE)&(neighbors<6)
    GRID[mask & (coarse==0)]=18
    GRID[mask & (coarse==1)]=19
    hashed=(x*73856093 ^ y*19349663 ^ z*83492791)%101
    edges=(neighbors<=4)&((GRID==OLIVE)|(GRID==LIGHT_OLIVE))
    GRID[edges & (hashed<5)]=EDGE
    GRID[(GRID==STEEL)&(neighbors<=4)&(hashed<8)]=DUST


def i32(*values):
    return struct.pack('<'+'i'*len(values), *values)


def dictionary(items):
    data=i32(len(items))
    for key,value in items.items():
        for s in [key,str(value)]:
            raw=s.encode('utf-8'); data+=i32(len(raw))+raw
    return data


def chunk(kind, payload=b'', children=b''):
    return kind.encode('ascii')+i32(len(payload),len(children))+payload+children


def transform(node,name,child,translation=(0,0,0)):
    return chunk('nTRN',i32(node)+dictionary({'_name':name})+i32(child,-1,0,1)+dictionary({'_t':' '.join(map(str,translation))}))


def export():
    """单一标准 VOX 场景；自动存储分块只处理字节坐标编码，不缩小模型。"""
    chunks=[]; models=[]; records=[]; node=2; model_id=0
    part_nodes=[]
    for part_id,name in enumerate(PARTS,1):
        coords=np.argwhere(OWNER==part_id)
        assert len(coords), name
        lo=coords.min(0); hi=coords.max(0)+1
        group_transform,group=node,node+1; node+=2
        part_nodes.append(group_transform)
        child_nodes=[]
        for tile in np.unique(coords//256,axis=0):
            sel=coords[np.all(coords//256==tile,axis=1)]
            base=sel.min(0); extent=sel.max(0)-base+1
            size=(extent+1)//2*2  # 偶数包围盒保证标准场景中心落在整数格。
            local=sel-base
            data=np.column_stack((local,GRID[tuple(sel.T)])).astype(np.uint8).tobytes()
            models += [chunk('SIZE',i32(*map(int,size))),chunk('XYZI',i32(len(sel))+data)]
            tr,shape=node,node+1; node+=2; child_nodes.append(tr)
            offset=base+size//2-(0,94,0)
            chunks += [transform(tr,f'{name}_tile_{model_id}',shape,tuple(map(int,offset))),chunk('nSHP',i32(shape)+dictionary({})+i32(1,model_id)+dictionary({}))]
            model_id+=1
        chunks += [transform(group_transform,name,group),chunk('nGRP',i32(group)+dictionary({})+i32(len(child_nodes))+i32(*child_nodes))]
        records.append({'name':name,'voxels':len(coords),'bounds_cells':[list(map(int,lo-(0,94,0))),list(map(int,hi-(0,94,0)))],'tiles':len(child_nodes)})
    root=transform(0,'leopard_2a6_astraH',1)+chunk('nGRP',i32(1)+dictionary({})+i32(len(part_nodes))+i32(*part_nodes))
    colors=b''.join(bytes((*c,255)) for c in PALETTE)+bytes(4*(256-len(PALETTE)))
    layer=chunk('LAYR',i32(0)+dictionary({'_name':'Leopard 2A6 / 20mm'})+i32(-1))
    # dot_vox 按读取次序建立节点数组，按节点 ID 排序兼容它的索引语义。
    chunks.sort(key=lambda raw:struct.unpack_from('<i',raw,12)[0])
    data=b'VOX '+i32(150)+chunk('MAIN',children=chunk('PACK',i32(model_id))+b''.join(models)+root+b''.join(chunks)+layer+chunk('RGBA',colors))
    path=OUT/'leopard_2a6.vox'; path.write_bytes(data)
    (OUT/'leopard_2a6.vox.meta').write_text('''(
    meta_format_version: "1.0",
    asset: Load(
        loader: "bevy_vox_scene::load::VoxSceneLoader",
        settings: (
            voxel_size: 0.02,
            mesh_outer_faces: true,
            mesh_offset: ((0.5, 0.5, 0.5)),
            emission_strength: 0.0,
            uses_srgb: true,
            diffuse_roughness: 0.88,
            retain_mesh_data: true,
        ),
    ),
)
''',encoding='utf-8')
    manifest={'voxel_size_m':UNIT,'axis':'+X front, +Y left, +Z up; origin rear center at ground',
              'dimensions_m':{'hull_length':7.66,'overall_length':10.98,'width':3.76,'deck_height':1.60,'turret_roof':2.64,'hatch_body_top':2.70,'gun_axis':2.08,'antenna_top':4.32},
              'parts':records,'models':model_id,'voxel_count':int(np.count_nonzero(GRID)),
              'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)}
    (OUT/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({k:v for k,v in manifest.items() if k!='parts'},ensure_ascii=False,indent=2))


def main():
    for f in [hull,running_gear,engine_deck,turret,armament,roof_fittings,finish_paint,export]:
        print(f.__name__,flush=True); f()


if __name__=='__main__':
    main()
