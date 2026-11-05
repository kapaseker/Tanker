"""M41A1 外观作者化（基础几何和 VOX 编码沿用 astraH）：坐标格 1 cm，+X 车首、+Y 左、+Z 上。

仅依赖 numpy；不读取或修改已有坦克资产。所有区间均为左闭右开。
"""
from pathlib import Path
import hashlib
import json
import math
import struct

import numpy as np

OUT = Path(__file__).resolve().parent
UNIT = 0.01
# 包含完整长炮和天线的统一作者画布；Y 下标相对车中线偏移 160。
SHAPE = (812, 320, 390)
GRID = np.zeros(SHAPE, np.uint8)
OWNER = np.zeros(SHAPE, np.uint8)
PARTS = []
CURRENT = 0
OLIVE, DARK_OLIVE, LIGHT_OLIVE, EDGE = 1, 2, 3, 4
RUBBER, STEEL, METAL, BLACK, GLASS, GLINT = 5, 6, 7, 8, 9, 10
RED, AMBER, LAMP, DUST, BROWN = 11, 12, 13, 14, 15
PALETTE = [
    (48, 48, 41), (36, 37, 32), (57, 56, 48), (91, 86, 71),
    (32, 32, 30), (36, 37, 34), (64, 62, 53), (21, 23, 22),
    (37, 48, 49), (104, 119, 119), (126, 38, 29), (169, 108, 43),
    (211, 214, 199), (110, 101, 81), (70, 62, 48),
    (46, 46, 39), (50, 50, 43), (55, 54, 46), (59, 58, 50),
]


def part(name):
    """命名装配分组，导出时保留为独立场景节点。"""
    global CURRENT
    PARTS.append(name)
    CURRENT = len(PARTS)


def region(lo, hi):
    """物理坐标区间映射到数组，越界直接报错，防止静默截断零件。"""
    a = np.array(lo, dtype=int) + (0, 160, 0)
    b = np.array(hi, dtype=int) + (0, 160, 0)
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
    lo = np.maximum(np.floor(np.minimum(a,b)-radius).astype(int), (0,-160,0))
    hi = np.minimum(np.ceil(np.maximum(a,b)+radius).astype(int), (812,160,390))
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


def bolts(axis, center, start, end, radius, count=8):
    """轮辋/舱盖螺栓沿真实圆周排列，1 cm 最小格仍保留间距。"""
    for i in range(count):
        a = i * math.tau / count
        c = (center[0]+radius*math.cos(a), center[1]+radius*math.sin(a))
        cylinder(axis,c,start,end,1.5,METAL,sides=6)


def hull():
    """车体斜甲、翼子板、工具箱及灯具牵引件。"""
    part('hull')
    # 船形腹甲离地 45 cm；首上从甲板向车首下降，侧翼不盖住轮组。
    polygon([(0,74),(12,131),(40,145),(425,145),(582,104),(582,74),(544,45),(34,45)],-108,108,OLIVE,axis=1)
    for z in range(45,114):
        w=min(112,85+(z-45)//2)
        box((35,-w,z),(548,w,z+1),DARK_OLIVE)
    for side in (-1,1):
        ya,yb=sorted((side*106,side*160))
        polygon([(0,109),(17,127),(442,127),(565,106),(582,90),(582,98),(568,113),(442,134),(16,134),(0,118)],ya,yb,OLIVE,axis=1)
        # 独立窄工具箱沿翼子板排列，没有现代主战坦克侧裙。
        for a,b in [(28,119),(143,337),(361,436)]:
            yl,yh=sorted((side*122,side*153))
            box((a,yl,134),(b,yh,149),DARK_OLIVE)
            box((a+2,yl+1,136),(b-2,yh-1,151),OLIVE)
            box((a+2,yl+1,150),(b-2,yh-1,152),LIGHT_OLIVE)
            for x in (a+10,b-13):
                box((x,yl-1,138),(x+3,yh+1,148),METAL)
                handle(x,side*137,152,8)
        # 前灯的保护框有真实空隙，灯面朝前。
        cy=side*92
        box((523,cy-10,116),(539,cy+10,127),DARK_OLIVE)
        cylinder(0,(cy,132),527,541,9,STEEL)
        cylinder(0,(cy,132),540,542,7,LAMP)
        cylinder(0,(cy-1,134),541,543,3,GLINT)
        for y in (cy-13,cy+12):
            box((519,y,120),(544,y+2,148),STEEL)
        box((519,cy-13,146),(544,cy+14,148),METAL)
        for x in (521,542):
            box((x,cy-13,122),(x+2,cy+14,124),STEEL)
        # 首尾牵引环，沿车体方向挤出的环孔不以黑色色块代替。
        for x,z in ((1,74),(570,76)):
            cylinder(0,(side*75,z),x,x+10,8,METAL,inner=4.5)
            box((x+3,side*75-5,z+5),(x+9,side*75+5,z+15),OLIVE)
        cy=side*98
        box((0,cy-15,102),(7,cy+15,124),DARK_OLIVE)
        for dy,col in ((side*7,RED),(-side*7,METAL)):
            cylinder(0,(cy+dy,113),0,5,6,STEEL)
            cylinder(0,(cy+dy,113),0,2,4,col)
    # 首上维修盖贴合斜面，边缘与铰链保持几何高度差。
    for x in range(446,506):
        z=math.floor(145-(x-425)*41/157)
        box((x,-43,z),(x+1,43,z+2),DARK_OLIVE if x in (446,505) else LIGHT_OLIVE)
        for y in (-44,43): box((x,y,z),(x+1,y+2,z+3),METAL)
    for y in (-32,0,32):
        box((446,y-2,140),(453,y+2,145),METAL)
    for x in (404,419):
        for y in (-85,85): handle(x,y,146,9)
    box((0,-7,82),(12,7,96),OLIVE)
    cylinder(0,(0,80),0,8,7,METAL,inner=3)


def wheel(cx,cz,side,radius,name,teeth=False):
    """双排轮胎之间留导向槽；外轮盘凹入胎面并保留轮毂和螺栓。"""
    part(name)
    # 奇数格直径必须以半格为圆心，避免中心采样丢掉两端体素。
    if radius == 32.5:
        cx += 0.5
        cz += 1.5
    for low,high in ((111,128),(133,150)):
        a,b=sorted((side*low,side*high))
        cylinder(1,(cx,cz),a,b,radius,STEEL if teeth else RUBBER)
        # 凹轮辋和胎唇：最外层只保留窄圆环。
        fa,fb=sorted((side*(high-2),side*high))
        cylinder(1,(cx,cz),fa,fb,radius-4,0)
        fa,fb=sorted((side*(high-5),side*(high-2)))
        cylinder(1,(cx,cz),fa,fb,radius-6,OLIVE)
        cylinder(1,(cx,cz),fa,fb,radius-7,METAL,inner=radius-9)
    a,b=sorted((side*108,side*152))
    cylinder(1,(cx,cz),a,b,7,OLIVE,sides=8)
    a,b=sorted((side*148,side*153))
    bolts(1,(cx,cz),a,b,12,8)
    if teeth:
        for i in range(14):
            t=i*math.tau/14
            x,z=cx+(radius+1)*math.cos(t),cz+(radius+1)*math.sin(t)
            for y in (115,146):
                a,b=sorted((side*(y-3),side*(y+3)))
                box((int(x)-3,a,int(z)-3),(int(x)+4,b,int(z)+4),METAL)


def running_gear():
    """每侧五轮、三托带轮、双端轮及同一闭环履带。"""
    for side,label in ((-1,'right'),(1,'left')):
        part(label+'_suspension')
        for cx in (111,201,291,381,471):
            beam((cx-18,side*105,61),(cx,side*126,36),6,DARK_OLIVE)
        for i,cx in enumerate((111,201,291,381,471),1):
            wheel(cx,36,side,32.5,f'{label}_roadwheel_{i}')
        wheel(41,76,side,26,label+'_sprocket',True)
        wheel(541,76,side,26,label+'_idler')
        for i,cx in enumerate((161,291,421),1):
            wheel(cx,100,side,12,f'{label}_return_roller_{i}')
        part(label+'_track')
        # 闭环中心线下缘位于地面，轮顶与上带分离；板块按弧长均匀铺排。
        points=[(41,113),(541,113),(578,77),(543,23),(504,3),(79,3),(40,23),(4,77)]
        segments=[]
        for a,b in zip(points,points[1:]+points[:1]):
            d=np.array(b,float)-a; length=np.linalg.norm(d)
            segments.append((np.array(a,float),d/length,length))
        # 连续内带按整段铺设，端点加圆角，避免分节板在折角处只对角接触。
        ya,yb=sorted((side*107,side*157))
        for a,d,length in segments:
            n=np.array([-d[1],d[0]])
            polygon([tuple(a+d*u+n*v) for u,v in ((0,-2),(length,-2),(length,2),(0,2))],ya,yb,STEEL,axis=1)
            cylinder(1,tuple(a),ya,yb,2,STEEL)
        perimeter=sum(s[2] for s in segments)
        count=round(perimeter/12)
        for i in range(count):
            distance=i*perimeter/count
            for a,d,length in segments:
                if distance < length: break
                distance-=length
            x,z=a+d*distance
            # 每节履带独立胶垫/横向钢板/铰链，连续内带保证闭环。
            n=np.array([-d[1],d[0]])
            def patch(along, across, outward, thickness, color):
                points2=[tuple(np.array([x,z])+d*u+n*v) for u,v in [(-along/2,outward), (along/2,outward),(along/2,outward+thickness),(-along/2,outward+thickness)]]
                points2=[(min(582,max(0,px)),max(0,pz)) for px,pz in points2]
                ya,yb=sorted((side*(132-across/2),side*(132+across/2)))
                polygon(points2,math.floor(ya),math.ceil(yb),color,axis=1)
            patch(13,49,-2,2,STEEL)
            patch(9,53,0,3,METAL)
            patch(7,37,1,2,RUBBER)
            patch(4,4,-7,6,STEEL)


def engine_deck():
    """双矩形通风网罩和维护附件，保留开孔深度。"""
    part('engine_deck')
    # 双矩形通风口下方是暗腔和百叶，上方有真正镂空的横纵网罩。
    for cy in (-54,54):
        box((35,cy-43,135),(128,cy+43,146),0)
        box((35,cy-43,134),(128,cy+43,136),BLACK)
        for x in range(38,126,9): box((x,cy-41,136),(x+4,cy+41,139),STEEL)
        for x in (33,128): box((x,cy-46,144),(x+3,cy+46,149),METAL)
        for y in (cy-46,cy+43): box((33,y,144),(131,y+3,149),METAL)
        for x in range(38,128,5): box((x,cy-43,145),(x+1,cy+43,147),STEEL)
        for y in range(cy-41,cy+43,5): box((36,y,147),(128,y+1,148),METAL)
    for x,y in ((17,-90),(17,73),(133,-91),(133,74)):
        handle(x,y,145,14,1)
    for y in (-91,91):
        beam((20,y,150),(132,y,150),2.0,DARK_OLIVE)
    for x in range(22,136,23):
        for y in (-102,102): cylinder(2,(x,y),145,147,2,METAL,sides=6)


def turret():
    """圆角炮塔、尾箱、防盾及带镂空制退器的长炮。"""
    part('turret')
    cylinder(2,(303,0),145,153,108,STEEL,sides=64)
    cylinder(2,(303,0),153,160,112,OLIVE,sides=64)
    # 铸造炮塔以不对称椭圆截面逐层收分，尾舱较平，前肩圆润。
    for z in range(158,235):
        t=(z-158)/77
        width=111-16*t + 3*math.sin(t*math.pi)
        rear=143+12*t
        front=445-17*t
        center=(rear+front)/2
        rx=(front-rear)/2
        masked((int(rear),-math.ceil(width),z),(math.ceil(front),math.ceil(width),z+1),
               lambda x,y,h: ((x-center)/rx)**4+(y/width)**2<=1,
               LIGHT_OLIVE if z>232 else OLIVE)
    # 尾部储物箱及绑带，侧面窄扶手与焊接安装座。
    polygon([(137,-62),(137,62),(151,76),(171,76),(171,-76),(151,-76)],177,222,OLIVE)
    for y in (-52,52):
        box((136,y-3,180),(174,y+3,225),METAL)
    for side in (-1,1):
        for x in (186,240,304,362):
            y=side*(103 if x<360 else 99)
            beam((x,y,192),(x+17,y,192),1.5,METAL)
            for xx in (x,x+17): beam((xx,y-side*3,188),(xx,y,196),2,OLIVE)
        for x in (195,343):
            y=side*80
            handle(x,y,235,15)
    # 曲面侧盖依附炮塔实际截面，轮廓有凹缝和扣件，不把扶手埋进铸造曲面。
    def flank(x,z):
        t=(z-158)/77
        rear,front=143+12*t,445-17*t
        q=(x-(rear+front)/2)/((front-rear)/2)
        return (111-16*t+3*math.sin(t*math.pi))*math.sqrt(max(0,1-q**4))
    for side in (-1,1):
        for z in range(180,225):
            inset=max(0,7-min(z-180,224-z))
            for x in range(183+inset,386-inset):
                edge=math.ceil(flank(x+0.5,z+0.5))
                ya,yb=sorted((side*(edge-2),side*(edge+1)))
                border=z in (180,224) or x in (183+inset,385-inset)
                box((x,ya,z),(x+1,yb,z+1),DARK_OLIVE if border else OLIVE)
        for x in (199,242,292,341,373):
            for z in (185,220):
                cy=side*(math.ceil(flank(x,z))+1)
                ya,yb=sorted((cy-side,cy+side*3))
                box((x-2,ya,z-3),(x+3,yb,z+4),METAL)
        for x in (225,317):
            cy=side*(math.ceil(flank(x,205))+4)
            for xx in (x,x+17):
                beam((xx,cy-side*6,205),(xx,cy,205),2,OLIVE)
            beam((x,cy,205),(x+17,cy,205),1.5,METAL)
    # 防盾是饱满的横向圆鼓，前端逐级收口。
    part('gun_76')
    cylinder(0,(0,191),411,439,37,DARK_OLIVE,sides=24)
    cylinder(0,(0,191),430,451,33,OLIVE,sides=24)
    cylinder(0,(0,191),448,463,19,OLIVE,sides=20)
    cylinder(0,(0,191),460,779,7,METAL,sides=16)
    for a,b,r in ((462,468,9),(553,580,11),(551,555,12),(579,584,12),(772,781,10)):
        cylinder(0,(0,191),a,b,r,OLIVE,sides=16)
    # 双室制退器：两个贯穿侧窗和轴向空腔，保留实物般的深色内壁。
    polygon([(777,-12),(805,-12),(812,-9),(812,9),(805,12),(777,12)],181,201,OLIVE)
    box((780,-7,186),(812,7,196),0)
    for a,b in ((782,790),(796,804)):
        box((a,-13,186),(b,13,196),0)
    for x in (777,791,806):
        box((x,-12,200),(x+3,12,202),METAL)


def roof_fittings():
    """右车长塔、左舱盖、机枪、固定光学件和双天线。"""
    part('commander_cupola')
    cx,cy=265,-49
    cylinder(2,(cx,cy),234,243,39,DARK_OLIVE,sides=16)
    cylinder(2,(cx,cy),240,248,38,OLIVE,sides=16)
    cylinder(2,(cx,cy),248,264,34,OLIVE,sides=12)
    # 十二个潜望窗固定朝径向，顶部帽檐压住玻璃形成遮光层。
    for i in range(12):
        a=i*math.tau/12
        c=np.array([cx,cy]); n=np.array([math.cos(a),math.sin(a)]); t=np.array([-n[1],n[0]])
        pts=[tuple(c+n*r+t*u) for r,u in ((31,-7),(35,-7),(35,7),(31,7))]
        polygon(pts,250,259,GLASS)
    cylinder(2,(cx,cy),262,267,36,METAL,sides=12)
    cylinder(2,(cx,cy),267,271,27,OLIVE,sides=24)
    cylinder(2,(cx,cy),271,276,19,DARK_OLIVE,sides=16)
    cylinder(2,(cx,cy),276,280,20,OLIVE,sides=16)
    bolts(2,(cx,cy),267,269,30,12)
    handle(cx-7,cy,280,14)
    part('loader_hatch')
    cylinder(2,(302,50),234,238,31,DARK_OLIVE,sides=24)
    cylinder(2,(302,50),238,242,29,OLIVE,sides=24)
    bolts(2,(302,50),242,244,25,12)
    handle(296,49,242,17)
    box((275,30,236),(281,69,245),METAL)
    part('roof_fittings')
    for x,y in ((374,-44),(389,55),(326,-18)):
        box((x-8,y-8,234),(x+8,y+8,244),DARK_OLIVE)
        box((x-6,y-7,244),(x+7,y+7,247),OLIVE)
        box((x+7,y-5,239),(x+9,y+5,244),GLASS)
    for x,y in ((352,22),(214,-70),(214,63),(353,-65)):
        handle(x,y,235,15)
    part('machine_gun')
    cylinder(2,(302,9),234,274,5,STEEL,sides=12)
    cylinder(2,(302,9),269,276,8,OLIVE,sides=12)
    box((282,3,275),(317,15,290),STEEL)
    box((283,-8,277),(301,3,291),OLIVE)
    box((275,5,278),(284,13,285),BROWN)
    cylinder(0,(9,284),317,370,3,STEEL,sides=12)
    cylinder(0,(9,284),319,339,5,METAL,sides=12)
    # 枪管散热孔用穿孔代替贴黑斑，枪口仍是环状。
    for x in (322,328,334): cylinder(1,(x,284),3,15,2,0)
    cylinder(0,(9,284),367,373,4,STEEL,inner=2,sides=12)
    box((312,8,287),(315,10,302),STEEL)
    box((287,6,290),(292,12,296),METAL)
    part('antennas')
    for cy in (-29,29):
        cylinder(2,(182,cy),231,240,8,OLIVE,sides=12)
        cylinder(2,(182,cy),240,255,5,STEEL,sides=12)
        cylinder(2,(182,cy),255,277,3,METAL)
        box((181,cy-1,277),(183,cy+1,310),DARK_OLIVE)
        box((182,cy,310),(183,cy+1,390),METAL)


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
    # 相邻两格共用磨损种子，形成可读的小缺口而非单点噪声。
    chips=((x//2)*73856093 ^ (y//2)*19349663 ^ (z//2)*83492791) % 101
    armor=(GRID==OLIVE)|(GRID==LIGHT_OLIVE)|((GRID>=16)&(GRID<=19))
    edges=(neighbors<=4)&armor
    GRID[edges & (chips<2)]=EDGE
    GRID[(GRID==STEEL)&(neighbors<=4)&(chips<5)]=DUST


def hollow_interior():
    """保留两格厚外壳和装配边界，避免单格壳在履带台阶处断开。"""
    interior = OWNER != 0
    for axis in range(3):
        for step in (-1, 1):
            interior &= OWNER == np.roll(OWNER, step, axis)
    interior[0,:,:] = interior[-1,:,:] = False
    interior[:,0,:] = interior[:,-1,:] = False
    interior[:,:,0] = interior[:,:,-1] = False
    deep = interior.copy()
    for axis in range(3):
        for step in (-1, 1):
            deep &= np.roll(interior, step, axis)
    GRID[deep] = 0
    OWNER[deep] = 0


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
            offset=base+size//2-(291,160,145)
            chunks += [transform(tr,f'{name}_tile_{model_id}',shape,tuple(map(int,offset))),chunk('nSHP',i32(shape)+dictionary({})+i32(1,model_id)+dictionary({}))]
            model_id+=1
        chunks += [transform(group_transform,name,group),chunk('nGRP',i32(group)+dictionary({})+i32(len(child_nodes))+i32(*child_nodes))]
        records.append({'name':name,'voxels':len(coords),'bounds_cells':[list(map(int,lo-(0,160,0))),list(map(int,hi-(0,160,0)))],'tiles':len(child_nodes)})
    root=transform(0,'m41a1',1)+chunk('nGRP',i32(1)+dictionary({})+i32(len(part_nodes))+i32(*part_nodes))
    colors=b''.join(bytes((*c,255)) for c in PALETTE)+bytes(4*(256-len(PALETTE)))
    layer=chunk('LAYR',i32(0)+dictionary({'_name':'M41A1 / 10mm'})+i32(-1))
    # dot_vox 按读取次序建立节点数组，按节点 ID 排序兼容它的索引语义。
    chunks.sort(key=lambda raw:struct.unpack_from('<i',raw,12)[0])
    # Bevy 的 VOX 加载器为漫反射槽保留默认粗糙度；涂漆与裸钢分槽。
    finishes = {
        OLIVE: (0.95, 0.0), DARK_OLIVE: (0.95, 0.0), LIGHT_OLIVE: (0.95, 0.0),
        16: (0.95, 0.0), 17: (0.95, 0.0), 18: (0.95, 0.0), 19: (0.95, 0.0),
        EDGE: (0.88, 0.05), STEEL: (0.90, 0.08), METAL: (0.85, 0.18),
        GLASS: (0.24, 0.12), GLINT: (0.20, 0.12),
    }
    materials = b''.join(
        chunk('MATL', i32(index) + dictionary({
            '_type': '_metal', '_rough': str(roughness), '_metal': str(metalness),
        }))
        for index, (roughness, metalness) in finishes.items()
    )
    data=b'VOX '+i32(150)+chunk('MAIN',children=chunk('PACK',i32(model_id))+b''.join(models)+root+b''.join(chunks)+layer+chunk('RGBA',colors)+materials)
    path=OUT/'m41a1.vox'; path.write_bytes(data)
    (OUT/'m41a1.vox.meta').write_text('''(
    meta_format_version: "1.0",
    asset: Load(
        loader: "bevy_vox_scene::load::VoxSceneLoader",
        settings: (
            voxel_size: 0.01,
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
    manifest={'voxel_size_m':UNIT,'axis':'+X front, +Y left, +Z up; origin hull center at deck height 1.45m',
              'part_bounds_origin':'rear center at ground, before scene pivot shift',
              'dimensions_m':{'hull_length':5.82,'overall_length':8.12,'width':3.20,'deck_height':1.45,'turret_roof':2.35,'gun_axis':1.91,'machine_gun_top':3.02,'antenna_top':3.90},
              'parts':records,'models':model_id,'voxel_count':int(np.count_nonzero(GRID)),
              'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)}
    (OUT/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({k:v for k,v in manifest.items() if k!='parts'},ensure_ascii=False,indent=2))


def main():
    """按装配覆盖关系生成，再减去不可见内部并导出。"""
    for f in (hull,running_gear,engine_deck,turret,roof_fittings,finish_paint,hollow_interior,export):
        print(f.__name__,flush=True)
        f()


if __name__ == '__main__':
    main()
