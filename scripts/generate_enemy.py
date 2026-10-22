"""确定性生成暗红敌人三包；尺寸和轮位读取 src/enemy_model.rs。"""
from pathlib import Path
import ast
import math
import re
import struct
from generate_ground import chunk, dictionary

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'assets' / 'tank'
BODY_OUTPUT = ASSETS / 'bodies/enemy/carapace/body.vox'
BARREL_OUTPUT = ASSETS / 'barrels/enemy/spit/barrel.vox'
TRACKS_OUTPUT = ASSETS / 'tracks/enemy/crawl/track.vox'
OUTPUTS = (BODY_OUTPUT, BARREL_OUTPUT, TRACKS_OUTPUT)
CONTRACT = (ROOT / 'src/enemy_model.rs').read_text()


def literal(name):
    """只解析声明中的字面量；格式漂移时报错，不执行 Rust 或任意代码。"""
    return ast.literal_eval(re.search(r'pub const '+name+r':[^=]+=(.*?);', CONTRACT).group(1).strip())


CANVAS = literal('CANVAS')
HULL_X = literal('HULL_X')
BARREL_X = literal('BARREL_X')
BORE_HEIGHT = literal('BORE_HEIGHT')
# 轮类型参数从同一契约读取；只接受 WheelSpec 当前的两种字面量分支。
wheel_fn = CONTRACT[CONTRACT.index('const fn wheel'):]


def wheel_value(field, end):
    """读取端轮/负重轮对应字段；不解释或执行其它 Rust 表达式。"""
    branches = re.search(field+r': if end \{ (.*?) \} else \{ (.*?) \}', wheel_fn)
    return ast.literal_eval(branches.group(1 if end else 2))


heights = tuple(map(float, re.search(
    r'center: \[x, y, if end \{ (.*?) \} else \{ (.*?) \}', wheel_fn).groups()))
WHEELS = []
for name, x, y, kind in re.findall(
        r'wheel\("(wheel_[lr]_\d+)", ([\d.]+), ([\d.]+), (true|false)\)', CONTRACT):
    end = kind == 'true'
    center = (float(x), float(y), heights[0 if end else 1])
    WHEELS.append((name, center, wheel_value('size', end)[0],
                   wheel_value('radius', end), wheel_value('envelope', end)))
if len(WHEELS) != 12:
    raise ValueError('敌人轮契约必须包含左右各6轮')


def box(cells, x0, x1, y0, y1, z0, z1, color, bevel=0):
    """半开区间填充，削平面四角；后写细节覆盖底色。"""
    for x in range(x0,x1):
        for y in range(y0,y1):
            if min(x-x0,x1-1-x)+min(y-y0,y1-1-y) >= bevel:
                for z in range(z0,z1):
                    cells[x,y,z] = color


def body_cells():
    """倾斜首上、短尾舱和阶梯炮塔；侧裙抬高，保留轮间透空。"""
    cells = {}
    box(cells,25,89,7,27,5,12,3,2)
    for z in range(12,16):
        box(cells,26,90-(z-12)*2,7,27,z,z+1,1 if z<15 else 2,3)
    box(cells,27,39,9,25,15,16,3,1)
    for x in range(28,38,2):
        box(cells,x,x+1,11,23,15,16,4)
    # 炮塔尾舱低一格，前楔逐层收窄，区别于玩家的长炮塔。
    box(cells,43,76,6,28,16,20,1,4)
    box(cells,45,76,7,27,20,23,1,4)
    box(cells,47,74,9,25,23,25,2,3)
    box(cells,50,57,12,20,25,26,3,2)
    box(cells,72,76,14,20,18,22,3,0)
    for y in (1,31):
        for x in range(27,85,12):
            box(cells,x,x+10,y,y+2,15,17,1,0)
            # 短横梁连接侧裙与甲板，位于凸齿上方，不侵入轮组净空。
            box(cells,x+3,x+5,3 if y==1 else 27,7 if y==1 else 31,14,16,3)
        box(cells,26,88,5 if y==1 else 28,6 if y==1 else 29,14,16,3,0)
    return cells


def barrel_cells():
    """炮管不参与碰撞；炮盾连接面和炮轴来自契约。"""
    cells = {}
    a,b = BARREL_X
    h = int(BORE_HEIGHT)
    box(cells,a,b,16,18,h-1,h+1,2)
    box(cells,a,a+5,15,19,h-2,h+2,3)
    box(cells,a+19,a+25,15,19,h-2,h+2,1,1)
    box(cells,b-2,b,16,18,h-1,h+1,4)
    return cells


def track_loop():
    """底部两角各削两格，150点闭环整除齿距3，前进沿顶部向+X。"""
    a,b = HULL_X
    return ([(x,12) for x in range(a,b)] + [(b-1,z) for z in range(11,1,-1)]
            + [(b-2,1)] + [(x,0) for x in range(b-3,a+1,-1)]
            + [(a+1,1)] + [(a,z) for z in range(2,12)])


def wheel_cells(spec):
    """局部画布居中；仅保留所有顶点均位于旋转包络内的格。"""
    name,center,size,radius,envelope = spec
    cells = {}
    for x in range(size):
        for z in range(size):
            if math.hypot(max(abs(x-size/2),abs(x+1-size/2)), max(abs(z-size/2),abs(z+1-size/2))) > envelope:
                continue
            dx,dz = x+0.5-size/2,z+0.5-size/2
            spoke = any(dx*math.cos(a)+dz*math.sin(a)>0 and abs(-dx*math.sin(a)+dz*math.cos(a))<0.55 for a in (0,1.7,4.0))
            color = 4 if math.hypot(dx,dz)>radius-1.2 else (5 if spoke else 1)
            for y in range(4):
                cells[x,y,z] = color
    return cells


def build_vox(models):
    """车身、炮管及相位共享画布；轮节点使用按轴心居中的局部画布。"""
    children = b''
    for name, cells in models:
        voxels = b''.join(bytes((*position, color)) for position, color in sorted(cells.items()))
        size = next(((w[2], 4, w[2]) for w in WHEELS if w[0] == name), CANVAS)
        if any(not all(0 <= p[i] < size[i] for i in range(3)) for p in cells):
            raise ValueError(f'{name} 存在超出画布的体素')
        children += chunk('SIZE', struct.pack('<III', *size))
        children += chunk('XYZI', struct.pack('<I', len(cells)) + voxels)
    children += chunk('nTRN', struct.pack('<i', 0) + dictionary({})
                      + struct.pack('<iiii', 1, -1, -1, 1) + dictionary({}))
    children += chunk('nGRP', struct.pack('<i', 1) + dictionary({})
                      + struct.pack('<i', len(models))
                      + b''.join(struct.pack('<i', 2+2*i) for i in range(len(models))))
    for i, (name, cells) in enumerate(models):
        node = 2+2*i
        children += chunk('nTRN', struct.pack('<i', node) + dictionary({'_name': name})
                          + struct.pack('<iiii', node+1, -1, -1, 1) + dictionary({}))
        children += chunk('nSHP', struct.pack('<i', node+1) + dictionary({})
                          + struct.pack('<ii', 1, i) + dictionary({}))
    colors = [(100, 42, 38), (121, 55, 46), (75, 31, 29), (36, 39, 37), (66, 60, 53)]
    palette = b''.join(bytes((*rgb, 255)) for rgb in colors)
    children += chunk('RGBA', palette + bytes((0, 0, 0, 255)) * (256-len(colors)))
    for index in range(1, len(colors)+1):
        children += chunk('MATL', struct.pack('<i', index)
                          + dictionary({'_type': '_diffuse', '_rough': '1'}))
    return b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children)


def build_assets():
    """输出车身、炮管和六相位加十二轮；不读取旧产物。"""
    rim = track_loop()
    assert len(rim) == len(set(rim)) and len(rim)%3 == 0
    models = []
    for side,y0,y1,outer,inner in (('l',1,7,0,6),('r',27,33,33,27)):
        for phase in range(3):
            cells = {}
            box(cells,26,88,inner,inner+1,6,8,4)
            for i,(x,z) in enumerate(rim):
                tooth = (i-phase)%3 == 0
                for y in range(y0,y1):
                    cells[x,y,z] = 5 if tooth else 4
                    if tooth and z == 12:
                        cells[x,y,13] = 5
                if tooth:
                    cells[x,outer,z] = 5
            models.append((f'track_{side}_{phase}',cells))
    models += [(w[0],wheel_cells(w)) for w in WHEELS]
    return build_vox([('body',body_cells())]), build_vox([('barrel',barrel_cells())]), build_vox(models)


if __name__ == '__main__':
    for path,data in zip(OUTPUTS,build_assets()):
        path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(data)
        print(f'已生成 {path}')
