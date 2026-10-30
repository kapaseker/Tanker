"""Leopard 2A6 proportioned reference tank. Final cells are 12.5 mm cubes.

Run: python build_tank.py. Only numpy is required. All geometry is real voxels.
"""
from pathlib import Path
import json
import math
import struct
import time

import numpy as np

OUT = Path(__file__).resolve().parent
LO = np.array([-228, -120, -4])
SHAPE = (660, 240, 310)
V = np.zeros(SHAPE, dtype=np.uint8)
PART = np.zeros(SHAPE, dtype=np.uint8)
GROUPS = {1: "Hull_and_skirts", 2: "Track_left", 3: "Track_right",
          4: "Turret", 5: "Main_gun", 6: "Roof_equipment", 7: "Antennas"}
current_part = 1

# Sixteen close shades per material keep highlights separate from geometry.
BASES = [(106, 111, 74), (122, 125, 85), (78, 85, 58), (149, 143, 98),
         (32, 35, 32), (63, 66, 59), (102, 103, 87), (156, 148, 111),
         (19, 30, 29), (52, 76, 77), (211, 214, 173), (108, 51, 35),
         (89, 91, 66), (53, 59, 45), (177, 169, 124)]
OLIVE, DECK, DEEP, EDGE, RUBBER, STEEL, METAL, BRASS, BLACK, GLASS, LAMP, RED, DUST, SHADOW, WEAR = range(15)
PALETTE = np.zeros((256, 4), dtype=np.uint8)
for family, base in enumerate(BASES):
    for shade in range(16):
        PALETTE[1 + family * 16 + shade] = [*[max(0, min(255, round(c * (.79 + shade * .028)))) for c in base], 255]


def region(bounds):
    a = np.maximum(np.floor(bounds[0]).astype(int), LO)
    b = np.minimum(np.ceil(bounds[1]).astype(int) + 1, LO + SHAPE)
    assert np.all(b > a), (a, b)
    sl = tuple(slice(int(a[i] - LO[i]), int(b[i] - LO[i])) for i in range(3))
    xyz = np.ogrid[tuple(slice(int(a[i]), int(b[i])) for i in range(3))]
    return sl, xyz


def paint(bounds, material, test=None, texture=True):
    sl, (x, y, z) = region(bounds)
    a = V[sl]
    mask = np.ones(a.shape, bool) if test is None else np.broadcast_to(test(x, y, z), a.shape)
    if material is None:
        a[mask] = 0
        PART[sl][mask] = 0
        return
    if texture:
        # Coherent paint mottling, not pre-baked illumination.
        h = ((x // 3 * 73856093) ^ (y // 3 * 19349663) ^ (z // 3 * 83492791)) & 255
        fine = ((x * 31337 ^ y * 6971 ^ z * 1013) & 63)
        shade = np.clip(7 + (h % 5) - 2 + (fine == 0) * 2 - (fine == 1) * 2, 1, 14)
        colors = np.broadcast_to(1 + material * 16 + shade, a.shape).astype(np.uint8)
        a[mask] = colors[mask]
    else:
        a[mask] = 1 + material * 16 + 8
    PART[sl][mask] = current_part


def box(a, b, material, texture=True):
    paint((np.minimum(a, b), np.maximum(a, b)), material, texture=texture)


def cylinder(center, radius, low, high, material, axis=2, inner=0):
    axes = [i for i in range(3) if i != axis]
    a, b = [0., 0., 0.], [0., 0., 0.]
    a[axis], b[axis] = low, high
    for j, dim in enumerate(axes):
        a[dim], b[dim] = center[j] - radius, center[j] + radius
    def test(x, y, z):
        xyz = (x, y, z)
        # Compensate the authoring grid so final road wheels remain circular.
        aspect = .819 if axis == 1 and current_part in (2, 3) else 1.
        r2 = ((xyz[axes[0]] - center[0]) / aspect) ** 2 + (xyz[axes[1]] - center[1]) ** 2
        return (r2 <= radius ** 2) & (r2 >= inner ** 2)
    paint((a, b), material, test)


def prism(points, low, high, material):
    p = np.asarray(points)
    def test(x, y, z):
        inside = np.zeros(np.broadcast_shapes(x.shape, y.shape), dtype=bool)
        for a, b in zip(p, np.roll(p, -1, axis=0)):
            if a[1] != b[1]:
                inside ^= ((a[1] > y) != (b[1] > y)) & (x < (b[0]-a[0])*(y-a[1])/(b[1]-a[1]) + a[0])
        return inside
    paint(((p[:, 0].min(), p[:, 1].min(), low), (p[:, 0].max(), p[:, 1].max(), high)), material, test)


def rod(a, b, radius, material):
    a, b = np.array(a, float), np.array(b, float)
    d = b - a
    def test(x, y, z):
        xyz = (x-a[0], y-a[1], z-a[2])
        t = np.clip(sum(xyz[i]*d[i] for i in range(3)) / np.dot(d, d), 0, 1)
        return sum((xyz[i]-t*d[i])**2 for i in range(3)) <= radius**2
    paint((np.minimum(a, b)-radius, np.maximum(a, b)+radius), material, test)


def bolt(x, y, z, axis=2, material=METAL, radius=1.5):
    xyz = [x, y, z]
    axes = [i for i in range(3) if i != axis]
    cylinder([xyz[i] for i in axes], radius, xyz[axis]-1, xyz[axis]+1, material, axis)


def tracks(side):
    global current_part
    current_part = 2 if side > 0 else 3
    ya, yb = sorted((side * 77, side * 111))
    # Continuous track carcass under individual curved-path shoes.
    paint(((-219, ya, 0), (219, yb, 81)), RUBBER,
          lambda x,y,z: (np.maximum(np.abs(x)-178, 0)**2 + (z-40)**2 <= 40**2)
          & (np.maximum(np.abs(x)-178, 0)**2 + (z-40)**2 >= 33**2))
    perimeter = 712 + 2*math.pi*37
    count = round(perimeter / 8.8)
    for i in range(count):
        t = i * perimeter / count
        if t < 356:
            cx, cz, tx, tz = -178+t, 77, 1, 0
        elif t < 356+math.pi*37:
            angle = math.pi/2-(t-356)/37
            cx, cz = 178+37*math.cos(angle), 40+37*math.sin(angle)
            tx, tz = math.sin(angle), -math.cos(angle)
        elif t < 712+math.pi*37:
            cx, cz, tx, tz = 178-(t-356-math.pi*37), 3, -1, 0
        else:
            angle = -math.pi/2-(t-712-math.pi*37)/37
            cx, cz = -178+37*math.cos(angle), 40+37*math.sin(angle)
            tx, tz = math.sin(angle), -math.cos(angle)
        nx, nz = -tz, tx
        bounds = ((cx-9, ya-1, max(0,cz-9)), (cx+9,yb+1,cz+9))
        def shoe(x,y,z):
            along, out = (x-cx)*tx+(z-cz)*tz, (x-cx)*nx+(z-cz)*nz
            return (np.abs(along)<3.8)&(np.abs(out)<3)
        paint(bounds, STEEL, shoe)
        # Two rubber pads, steel hinge bars and exposed edge cleats.
        for pya, pyb in ((ya+5,ya+15),(ya+19,yb-5)):
            paint(((cx-9,pya,max(0,cz-9)),(cx+9,pyb,cz+9)), RUBBER,
                  lambda x,y,z: (np.abs((x-cx)*tx+(z-cz)*tz)<2.7)&(np.abs((x-cx)*nx+(z-cz)*nz-3)<1.2))
        for yy in (ya, yb-2):
            paint(((cx-9,yy,max(0,cz-9)),(cx+9,yy+2,cz+9)), METAL,
                  lambda x,y,z: (np.abs((x-cx)*tx+(z-cz)*tz)<3.6)&(np.abs((x-cx)*nx+(z-cz)*nz-2)<1.5))
        # Inner guide teeth.
        rod((cx-nx*5,side*94,cz-nz*5),(cx-nx*9,side*94,cz-nz*9),2,STEEL)
    # Axles and torsion arms remain visible in the wheel gaps.
    for x in (-148,-100,-52,-4,44,92,140):
        cylinder((x,32), 8, *sorted((side*65,side*91)), SHADOW, axis=1)
        rod((x-14,side*79,57),(x,side*89,32),4,STEEL)
        cylinder((x,32),29,ya+2,yb-2,RUBBER,axis=1)
        outer = side*109
        cylinder((x,32),25,*sorted((side*82,outer)),DEEP,axis=1)
        cylinder((x,32),23,*sorted((outer,outer+side)),EDGE,axis=1,inner=21)
        cylinder((x,32),20,*sorted((outer,outer+side)),OLIVE,axis=1)
        cylinder((x,32),14,*sorted((outer+side,outer+2*side)),DEEP,axis=1)
        cylinder((x,32),8,*sorted((outer,outer+4*side)),EDGE,axis=1)
        cylinder((x,32),5,*sorted((outer+4*side,outer+5*side)),METAL,axis=1)
        for j in range(8):
            a = j*math.tau/8
            bolt(x+9*math.cos(a),outer+3*side,32+11*math.sin(a),1,BRASS,1)
    for x in (-182,182):
        z = 42 if x>0 else 44
        outer = side*109
        cylinder((x,z),25,ya+1,yb-1,RUBBER,axis=1)
        cylinder((x,z),21,*sorted((side*82,outer)),OLIVE,axis=1)
        cylinder((x,z),18,*sorted((outer,outer+side)),EDGE,axis=1,inner=15)
        cylinder((x,z),12,*sorted((outer,outer+2*side)),SHADOW,axis=1)
        cylinder((x,z),7,*sorted((outer,outer+4*side)),METAL,axis=1)
        for j in range(12):
            a=j*math.tau/12
            xx,zz=x+18*math.cos(a),z+22*math.sin(a)
            box((xx-2,ya+2,zz-2),(xx+2,yb-2,zz+2),METAL)
            if j%2 == 0:
                bolt(x+9.8*math.cos(a),outer+2*side,z+12*math.sin(a),1,EDGE,1.3)


def hull():
    global current_part
    current_part = 1
    # Lower tub, angled bow and substantial top hull, with beveled corners.
    prism([(-202,-68),(-192,-80),(171,-80),(202,-60),(202,60),(171,80),(-192,80),(-202,68)],31,77,DEEP)
    paint(((-209,-94,66),(216,94,112)),OLIVE,
          lambda x,y,z: (x <= 236-(z-66)*1.9)&(x >= -209+(z-66)*.09)&(np.abs(y)<=94-(z-90).clip(0)*.19))
    paint(((179,-76,30),(218,76,69)),DEEP,
          lambda x,y,z:(x<=180+(z-30)*.96)&(np.abs(y)<76))
    # Broad upper glacis panels with small joints, not oversized brickwork.
    for ya,yb in ((-76,-27),(-25,25),(27,76)):
        paint(((146,ya,74),(217,yb,113)),DECK,
              lambda x,y,z:(np.abs(z-(113-(x-146)/1.9))<.8)&(x<217))
    for x in (101,127,154):
        box((x,-74,112),(x+1,74,113),SHADOW)
    for x in (108,135,161):
        for y in (-72,-25,25,72):
            bolt(x,y,113,material=EDGE,radius=1)
    # Subtle plate patchwork on front deck.
    for xa,xb in ((55,99),(102,125),(130,151),(154,164)):
        for ya,yb in ((-74,-27),(-24,24),(27,74)):
            box((xa,ya,112),(xb,yb,113),DECK)
    box((134,-18,113),(155,19,115),OLIVE)
    for y in (-22,22):
        box((140,y,114),(153,y+2,117),DEEP)
    # Driver's raised hatch and separate armored optic slots.
    prism([(84,-46),(110,-46),(117,-38),(117,-15),(84,-15)],113,117,OLIVE)
    box((107,-42,117),(119,-19,120),DEEP)
    for y in (-39,-31,-23):
        box((119,y,117),(120,y+5,119),GLASS)
    box((90,-35,118),(99,-33,120),EDGE)
    # Rear engine deck: louvers, dark cooling cavities and raised ribs.
    box((-207,-77,111),(-112,77,114),DECK)
    for yy in (-49,30):
        box((-199,yy-19,114),(-132,yy+20,115),SHADOW)
        for x in range(-197,-131,5):
            box((x,yy-18,115),(x+2,yy+19,117),DEEP)
            box((x,yy-18,117),(x,yy+19,117),EDGE)
        for y in (yy-20,yy+20):
            box((-201,y,115),(-130,y+1,118),OLIVE)
    for x in (-195,-157,-121):
        for y in (-72,68):
            bolt(x,y,116,material=METAL)
    box((-126,-73,114),(-112,72,117),OLIVE)
    for y in range(-65,66,9):
        box((-124,y,117),(-114,y+3,118),SHADOW)
    # Side fenders and ten individually edged skirt modules.
    for side in (-1,1):
        ya,yb=sorted((side*80,side*114))
        box((-211,ya,91),(194,yb,101),DEEP)
        box((-207,ya,102),(184,yb-1,106),DECK)
        for i,x in enumerate(range(-207,180,43)):
            outer=side*115
            top=102 if i<7 else 98
            box((x,outer-2,55),(x+41,outer+2,top),OLIVE if i%3 else DUST)
            box((x+2,outer-side*3,top),(x+39,outer+side,top+2),EDGE)
            box((x+1,outer+side*2,76),(x+39,outer+side*2,76),DEEP)
            for xx in (x+4,x+37):
                bolt(xx,outer+side*3,top-5,1,EDGE,1.3)
                bolt(xx,outer+side*3,59,1,METAL,1)
            box((x+13,outer-side*2,top+1),(x+24,outer+side*3,top+4),OLIVE)
            for xx in (x+14,x+23):
                box((xx,outer+side*3,top-1),(xx+1,outer+side*4,top+2),METAL)
        # Upper flank bins stepped in height.
        for x in (-207,-158,-109,-60,-11,38):
            ya,yb=sorted((side*82,side*107))
            box((x,ya,107),(x+46,yb,119 if x<-80 else 113),OLIVE)
            box((x+2,ya,119 if x<-80 else 113),(x+44,yb,120 if x<-80 else 114),DECK)
            for xx in (x+4,x+40):
                bolt(xx,side*108,113,1,EDGE,1)
        # Sloped front mudguards sitting over the curved tracks.
        ya,yb=sorted((side*80,side*116))
        paint(((181,ya,76),(224,yb,100)),OLIVE,
              lambda x,y,z:(z>=93-(x-185)*.40)&(z<=100-(x-185)*.40))
        paint(((181,ya,76),(224,yb,101)),EDGE,
              lambda x,y,z: (np.abs(z-(100-(x-185)*.40))<.7)&((y==ya)|(y==yb)))
        # Short rear flexible mud flaps.
        box((-218,ya,47),(-214,yb,91),RUBBER)
        for y in range(ya+2,yb,7):
            box((-219,y,49),(-218,y+1,88),STEEL)
        # Headlights: armored housings, inner dark recess and 2x2 lenses.
        cy=side*66
        box((178,cy-13,88),(201,cy+13,110),OLIVE)
        box((201,cy-10,97),(204,cy+10,108),BLACK)
        box((204,cy-7,99),(205,cy+3,106),LAMP,False)
        box((205,cy-2,99),(205,cy-2,106),METAL)
        box((205,cy-7,102),(205,cy+3,102),METAL)
        box((201,cy+6,98),(205,cy+10,107),STEEL)
        box((205,cy+7,100),(205,cy+9,105),GLASS)
        for yy in (cy-12,cy+12):
            rod((189,yy,97),(203,yy,112),1,EDGE)
        rod((203,cy-12,112),(203,cy+12,112),1,EDGE)
        # Front towing lugs with a visible central aperture.
        cy=side*53
        box((201,cy-6,46),(213,cy+6,64),OLIVE)
        cylinder((cy,52),6,210,218,METAL,axis=0,inner=3)
        bolt(216,cy,64,0,EDGE,2)
        rod((186,cy,35),(215,cy,64),3,OLIVE)
        cylinder((208,56),3,*sorted((cy-9,cy+9)),METAL,axis=1)
        for yy in (cy-8,cy+8):
            box((204,yy-2,53),(212,yy+2,59),EDGE)
        # Rear lamps and exhaust grilles.
        cy=side*67
        box((-212,cy-8,83),(-208,cy+8,96),SHADOW)
        box((-214,cy-6,88),(-212,cy-1,93),RED)
        box((-214,cy+2,88),(-212,cy+6,93),LAMP)
    box((-211,-56,76),(-209,56,102),SHADOW)
    for z in range(78,102,5):
        box((-214,-54,z),(-211,54,z+2),DEEP)
    for y in (-46,46):
        cylinder((y,67),7,-216,-207,METAL,axis=0,inner=3)
    # Rear towing cable, jack and engine-access handles.
    rod((-206,-76,122),(-206,70,122),2,STEEL)
    rod((-206,-76,122),(-174,-76,122),2,STEEL)
    for y in (-65,-18,28,65):
        box((-207,y,113),(-203,y+3,124),OLIVE)
    box((-188,-9,115),(-145,8,120),DEEP)
    cylinder((0,121),3,-193,-138,METAL,axis=0)


def turret():
    global current_part
    current_part=4
    cylinder((-23,0),77,113,118,SHADOW)
    cylinder((-23,0),74,119,123,STEEL)
    cylinder((-23,0),72,123,125,OLIVE)
    # Main armored shell, tapering inward toward the roof.
    footprint=[(-139,-60),(-125,-79),(-26,-84),(48,-78),(111,-51),(116,-23),
               (116,23),(111,51),(48,78),(-26,84),(-125,79),(-139,60)]
    prism(footprint,126,174,OLIVE)
    # Cut the underside back toward the turret ring, exposing the rounded base.
    paint(((-140,-86,126),(117,86,136)),None,
          lambda x,y,z:(np.abs(y)>70+(z-126)*1.5)|(x>100+(z-126)*2.2))
    # Chamfered roof and rear bustle.
    roof=[(-134,-57),(-121,-73),(-26,-77),(47,-71),(101,-47),(108,-22),
          (108,22),(101,47),(47,71),(-26,77),(-121,73),(-134,57)]
    prism(roof,174,183,DECK)
    for side in (-1,1):
        ya,yb=sorted((side*25,side*84))
        # The front wedges are deep, solid armor with an exposed sloping face.
        paint(((15,ya,127),(138,yb,181)),OLIVE,
              lambda x,y,z:(x<=138-(z-130)*.57)&(x<=112+(z-126)*2.8)
              &(np.abs(y)<=85-(z-130)*.16)&(np.abs(y)<=70+(z-126)*1.3)&(np.abs(y)>=25))
        # A front plane separated into two broad plates by an actual joint.
        for a,b in ((25,50),(54,83)):
            low,high=sorted((side*a,side*b))
            paint(((99,low,132),(139,high,181)),DECK,
                  lambda x,y,z:(np.abs(x-(138-(z-130)*.57))<1.5)&(x<=112+(z-126)*2.8)
                  &(np.abs(y)<=85-(z-130)*.16)&(np.abs(y)<=70+(z-126)*1.3))
        # Deep, narrow joints between the two cheek cassettes.
        low,high=sorted((side*51,side*53))
        paint(((105,low,136),(139,high,181)),None,
              lambda x,y,z:x>134-(z-130)*.57)
        # Wear around the bottom and outer edge of the cheek, not a thick outline.
        paint(((102,ya,132),(139,yb,180)),EDGE,
              lambda x,y,z:(np.abs(x-(138-(z-130)*.57))<1.6)&(x<=112+(z-126)*2.8)
              &((z<135)|(np.abs(y-(side*(84-(z-130)*.16)))<1)))
        for z in (143,157,173):
            xx=138-(z-130)*.57
            for yy in (side*30,side*72):
                bolt(xx,yy,z,0,EDGE,1)
        # Long turret side armor cassettes with crisp panel divisions.
        for x in (-119,-83,-47,-11):
            ya,yb=sorted((side*78,side*85))
            box((x,ya,131),(x+33,yb,174),OLIVE)
            box((x+1,ya,174),(x+32,yb,176),EDGE)
            for xx in (x+4,x+29):
                bolt(xx,side*86,169,1,EDGE,1)
                bolt(xx,side*86,136,1,DEEP,1)
        # Recognizable arrays of applique blocks / smoke-launcher armor.
        for xx,cols,rows in ((-114,3,3),(-29,3,3)):
            for col in range(cols):
                for row in range(rows):
                    pitch=11 if xx < -50 else 14
                    size=8 if xx < -50 else 11
                    x=xx+col*pitch
                    z=(139 if xx < -50 else 133)+row*pitch
                    outer=side*((90 if xx < -50 else 94)+(col%2)*2)
                    ya,yb=sorted((side*84,outer))
                    box((x,ya,z),(x+size,yb,z+size),SHADOW)
                    box((x+1,outer-side,z+1),(x+size-1,outer+side*2,z+size-1),DEEP if xx< -50 else OLIVE)
                    box((x+1,outer+side*2,z+size-1),(x+size-1,outer+side*2,z+size),EDGE)
                    bolt(x+size/2,outer+side*3,z+size/2,1,METAL,1)
        # Rear bustle stowage boxes, basket frame and gridded outside face.
        ya,yb=sorted((side*52,side*80))
        box((-153,ya,141),(-137,yb,175),DEEP)
        for z in (143,153,163,175):
            rod((-155,ya,z),(-155,yb,z),1,EDGE)
        for y in range(ya,yb+1,9):
            rod((-155,y,142),(-155,y,176),1,OLIVE)
        # Turret lifting eyes.
        cylinder((-122,side*63),4,183,186,EDGE,inner=2)
        box((-124,side*63-1,182),(-120,side*63+1,184),OLIVE)
    # Rear rack rails extend around the bustle, with several storage cases.
    for y in (-66,66):
        rod((-147,y,179),(-100,y,187),1.4,EDGE)
        rod((-147,y,164),(-147,y,179),1.5,DEEP)
    rod((-148,-66,179),(-148,66,179),1.5,EDGE)
    for y in (-45,0,45):
        box((-147,y-16,176),(-125,y+16,185),OLIVE)
        box((-147,y-16,184),(-125,y+16,186),DECK)
        box((-140,y-3,186),(-130,y+3,188),DEEP)
    # Roof joints, maintenance plates and bolt rows.
    box((-12,-67,184),(-11,62,184),SHADOW)
    box((41,-59,184),(42,57,184),SHADOW)
    for xa,xb,ya,yb in ((-10,37,-66,-18),(-8,36,6,65),(44,86,-48,-19),(45,86,20,47)):
        box((xa,ya,183),(xb,yb,184),DECK)
        for x in (xa+3,xb-3):
            for y in (ya+3,yb-3):
                bolt(x,y,185,material=EDGE,radius=1)
    box((6,-53,184),(30,-37,188),OLIVE)
    box((9,-50,188),(27,-40,189),DECK)
    box((13,-44,190),(24,-43,191),DEEP)
    for x in (48,75):
        box((x,-54,185),(x+8,-48,187),EDGE)
    # Main gun opening and mantlet trunnion protection.
    box((102,-25,136),(123,25,173),SHADOW)
    box((107,-23,138),(126,23,170),DEEP)
    for y in (-25,21):
        box((105,y,141),(119,y+4,170),OLIVE)
    # Secondary sight beside the main mantlet, with a recessed glass aperture.
    box((94,47,164),(113,65,182),OLIVE)
    box((113,50,168),(115,62,179),BLACK)
    box((115,53,170),(116,60,177),GLASS)
    box((110,47,181),(116,65,184),EDGE)


def main_gun():
    global current_part
    current_part=5
    center=(0,154)
    # Octagonal, stepped thermal jacket and dark mantlet collar.
    def barrel(xa,xb,r,mat,inside=0):
        aspect = (300/239)/(60/72)
        paint(((xa,-r,154-r*aspect),(xb,r,154+r*aspect)),mat,
              lambda x,y,z:(np.maximum(np.abs(y),np.abs(z-154)/aspect)<=r)&(np.abs(y)+np.abs(z-154)/aspect<=r*1.43)
              & ((y*y+((z-154)/aspect)**2>=inside**2) if inside else True))
    barrel(116,137,20,OLIVE)
    barrel(138,142,21,EDGE)
    barrel(143,159,18,DECK)
    barrel(160,164,19,DEEP)
    barrel(165,176,17,OLIVE)
    barrel(177,207,20,STEEL)
    barrel(181,201,20,DUST)
    barrel(208,213,19,BLACK)
    barrel(214,222,18,STEEL)
    barrel(223,230,15,DEEP)
    barrel(231,399,10,OLIVE)
    # Distinct jacket sections, seam lines and narrow clamp straps.
    for x in (241,268,295,322,349,377):
        barrel(x,x+1,10,SHADOW)
        barrel(x+2,x+3,11,DECK)
    box((232,-7,167),(397,-6,168),EDGE)
    box((232,-10,151),(398,-10,152),DEEP)
    barrel(399,402,12,STEEL)
    barrel(403,416,13,DECK,inside=8)
    barrel(417,420,12,EDGE,inside=8)
    # A real recessed, open bore. Dark inner lining remains visible from front.
    barrel(401,416,9,BLACK,inside=7)
    box((398,-6,148),(400,6,160),BLACK)
    for x in (132,155,174):
        box((x,-12,179),(x+2,12,182),EDGE)


def equipment():
    global current_part
    current_part=6
    # Commander cupola: layered circular foundation, periscopes and armored lid.
    cx,cy=-62,-26
    cylinder((cx,cy),45,184,188,OLIVE)
    cylinder((cx,cy),42,189,196,SHADOW)
    cylinder((cx,cy),43,197,200,DECK)
    cylinder((cx,cy),38,200,202,OLIVE)
    for j in range(12):
        a=j*math.tau/12
        x,y=cx+41*math.cos(a),cy+41*math.sin(a)
        box((x-5,y-4,190),(x+5,y+4,196),OLIVE)
        if abs(math.cos(a))>.6:
            box((x+math.copysign(5,math.cos(a)),y-3,191),(x+math.copysign(5,math.cos(a)),y+3,195),GLASS)
        else:
            box((x-3,y+math.copysign(4,math.sin(a)),191),(x+3,y+math.copysign(4,math.sin(a)),195),GLASS)
    # Tall square observation head, rounded / chamfered at all four corners.
    cylinder((cx,cy),25,202,205,STEEL)
    points=[(cx-22,cy-16),(cx-16,cy-22),(cx+16,cy-22),(cx+22,cy-16),
            (cx+22,cy+16),(cx+16,cy+22),(cx-16,cy+22),(cx-22,cy+16)]
    prism(points,205,237,OLIVE)
    prism([(cx+(x-cx)*.95,cy+(y-cy)*.95) for x,y in points],237,240,DECK)
    # Deep black main optic and differently sized adjacent optical windows.
    box((cx+22,cy-15,209),(cx+23,cy+15,234),SHADOW)
    box((cx+23,cy-12,211),(cx+24,cy+12,231),BLACK)
    box((cx+24,cy-9,214),(cx+24,cy+6,229),GLASS)
    box((cx+24,cy+8,214),(cx+24,cy+11,224),BLACK)
    box((cx+24,cy-9,214),(cx+24,cy-8,228),DEEP)
    box((cx+20,cy-16,235),(cx+25,cy+16,237),EDGE)
    box((cx-12,cy-24,212),(cx+4,cy-22,229),SHADOW)
    box((cx-9,cy-25,215),(cx+1,cy-24,226),BLACK)
    for z in (211,231):
        box((cx-21,cy-23,z),(cx-18,cy-20,z+3),EDGE)
    box((cx-14,cy-13,240),(cx+12,cy+13,243),DEEP)
    box((cx-11,cy-10,243),(cx+9,cy+10,244),DECK)
    for x in (cx-14,cx+11):
        bolt(x,cy,245,material=EDGE,radius=1)
    # Loader's hatch, pintle and compact roof machine gun to far side.
    cx,cy=-28,43
    cylinder((cx,cy),22,184,187,SHADOW)
    cylinder((cx,cy),20,188,191,OLIVE)
    cylinder((cx,cy),18,191,192,DECK)
    rod((cx-9,cy-7,194),(cx+6,cy-7,194),1.4,EDGE)
    cylinder((4,43),18,185,189,DEEP)
    cylinder((4,43),16,190,193,EDGE)
    cylinder((4,43),12,194,199,OLIVE)
    box((-4,36,199),(10,50,207),DEEP)
    box((-9,31,202),(15,54,221),OLIVE)
    box((-8,31,220),(15,54,223),DECK)
    box((14,37,208),(49,48,218),STEEL)
    box((17,36,217),(46,49,219),DEEP)
    box((49,39,211),(67,45,215),METAL)
    cylinder((42,213),2,66,97,STEEL,axis=0)
    cylinder((42,213),3,96,100,DEEP,axis=0,inner=1)
    for x in (57,65,73,81):
        cylinder((42,213),3,x,x+2,STEEL,axis=0)
    box((35,40,219),(39,44,224),BLACK)
    box((86,41,215),(88,43,220),STEEL)
    box((7,25,206),(22,35,218),DEEP)
    for x in range(9,22,4):
        box((x,24,207),(x+1,25,217),EDGE)
    box((-10,31,207),(-8,37,215),BLACK)
    # Two aerials with heavy bases, thin whips and a small pale tip.
    current_part=7
    for x,y,top in ((-112,-61,297),(-10,63,269)):
        cylinder((x,y),9,183,187,SHADOW)
        cylinder((x,y),7,188,194,DECK)
        box((x-7,y-7,194),(x+7,y+7,210),OLIVE)
        bolt(x+8,y,202,0,STEEL,2)
        cylinder((x,y),3,210,221,DEEP)
        rod((x,y,221),(x,y,top-2),.85,STEEL)
        rod((x,y,221),(x,y,237),1.2,DEEP)
        box((x,y,top-1),(x,y,top),EDGE)
    # Small roof fasteners and tie-down handles.
    current_part=6
    for x,y in ((-8,-69),(29,61),(78,-39),(-102,15),(60,34)):
        box((x-4,y-2,183),(x+4,y+2,187),OLIVE)
        rod((x-3,y,188),(x+3,y,188),1,EDGE)


def surface_detail():
    """Fine panel scores and a restrained scattering of exposed metal chips."""
    # Work in x slices so detailing does not allocate multiple full-size grids.
    for i in range(V.shape[0]):
        slab=V[i]
        y,z=np.ogrid[LO[1]:LO[1]+SHAPE[1],LO[2]:LO[2]+SHAPE[2]]
        x=i+LO[0]
        family=(slab.astype(np.int16)-1)//16
        green=(slab>0)&np.isin(family,[OLIVE,DECK,DEEP,DUST])
        # Only affect exposed surfaces; leave holes, optics and tracks intact.
        edges=np.zeros(slab.shape,np.uint8)
        if i>0: edges += V[i-1]==0
        if i+1<V.shape[0]: edges += V[i+1]==0
        edges[1:] += slab[:-1]==0
        edges[:-1] += slab[1:]==0
        edges[:,1:] += slab[:,:-1]==0
        edges[:,:-1] += slab[:,1:]==0
        exposed=edges>0
        hashv=(x*73856093 ^ y*19349663 ^ z*83492791)&65535
        score=green&exposed&(((x%13==0)&(z%14==0))|((y%13==0)&(z%14==0)))
        slab[score]=np.maximum(1,slab[score]-3)
        chips=green&exposed&((hashv<150)|((edges>=2)&(hashv<5500)))
        slab[chips]=1+EDGE*16+6+(hashv[chips]%5).astype(np.uint8)


def calibrate_dimensions():
    """Map authored landmarks onto a cubic metric grid, retaining color/part IDs."""
    global V, PART, LO, SHAPE
    # Cell boundaries: length 10.975 m, hull 7.700 m, width 3.750 m,
    # sight roof 3.000 m, antenna tip 3.650 m.
    source = [([-228, -219, 225, 421, 432], [-317, -304, 312, 574, 589]),
              ([-120, -119, 120], [-151, -150, 150]),
              ([-4, 0, 81, 112, 184, 205, 247, 298, 306],
               [-5, 0, 92, 124, 184, 198, 240, 292, 300])]
    indices = []
    low = []
    for axis, (authored, metric) in enumerate(source):
        low.append(metric[0])
        centers = np.arange(metric[0], metric[-1]) + .5
        index = np.floor(np.interp(centers, metric, authored)).astype(int)-LO[axis]
        assert np.all((index >= 0) & (index < V.shape[axis]))
        indices.append(index)
    selection = np.ix_(*indices)
    V, PART = V[selection], PART[selection]
    LO = np.array(low)
    SHAPE = V.shape
    cells = np.argwhere(V)
    assert tuple(cells.max(0)-cells.min(0)+1) == (878, 300, 292)
    body = np.argwhere(PART == 1)
    assert body[:, 0].max()-body[:, 0].min()+1 == 616
    solid = np.argwhere((PART > 0) & (PART != 7))
    assert solid[:, 2].max()+LO[2]+1 == 240
    assert abs(58*.819*(616/444)-58*(92/81)) < 1


def chunk(kind,data=b"",children=b""):
    return kind.encode()+struct.pack('<ii',len(data),len(children))+data+children


def dictionary(d):
    data=struct.pack('<i',len(d))
    for k,v in d.items():
        for s in (k,str(v)):
            s=s.encode('utf8')
            data+=struct.pack('<i',len(s))+s
    return data


def export_vox():
    pieces=[]
    for group in GROUPS:
        coords=np.argwhere(PART==group)
        low,high=coords.min(0),coords.max(0)+1
        for x in range(int(low[0]),int(high[0]),240):
            for y in range(int(low[1]),int(high[1]),240):
                for z in range(int(low[2]),int(high[2]),240):
                    start=np.array([x,y,z])
                    stop=np.minimum(start+240,high)
                    sl=tuple(slice(int(a),int(b)) for a,b in zip(start,stop))
                    c=np.argwhere(PART[sl]==group)
                    if not len(c): continue
                    tightlow,tighthigh=c.min(0),c.max(0)+1
                    c=c-tightlow
                    dims=tighthigh-tightlow
                    origin=start+tightlow+LO
                    values=V[tuple((c+start+tightlow).T)]
                    records=np.column_stack((c,values)).astype(np.uint8)
                    pieces.append((group,dims,origin,records))
    data=chunk('PACK',struct.pack('<i',len(pieces)))
    for group,dims,origin,records in pieces:
        data+=chunk('SIZE',struct.pack('<iii',*dims))
        data+=chunk('XYZI',struct.pack('<i',len(records))+records.tobytes())
    # Standard scene graph. Model-local coordinates are centered by SIZE // 2.
    data+=chunk('nTRN',struct.pack('<i',0)+dictionary({'_name':'Reference MBT'})+struct.pack('<iiii',1,-1,-1,1)+dictionary({'_r':'4'}))
    data+=chunk('nGRP',struct.pack('<i',1)+dictionary({})+struct.pack('<i',len(GROUPS))+struct.pack('<'+'i'*len(GROUPS),*[2+2*i for i in range(len(GROUPS))]))
    next_node=2+2*len(GROUPS)
    entries=[]
    for i,(group,name) in enumerate(GROUPS.items()):
        ids=[]
        for model_id,(g,dims,origin,records) in enumerate(pieces):
            if g!=group: continue
            transform,shape=next_node,next_node+1
            next_node+=2
            ids.append(transform)
            pos=origin+dims//2
            entries.append(chunk('nTRN',struct.pack('<i',transform)+dictionary({'_name':f'{name}_{len(ids):02}'})+struct.pack('<iiii',shape,-1,-1,1)+dictionary({'_r':'4','_t':' '.join(map(str,pos))})))
            entries.append(chunk('nSHP',struct.pack('<i',shape)+dictionary({})+struct.pack('<ii',1,model_id)+dictionary({})))
        data+=chunk('nTRN',struct.pack('<i',2+2*i)+dictionary({'_name':name})+struct.pack('<iiii',3+2*i,-1,-1,1)+dictionary({'_r':'4'}))
        data+=chunk('nGRP',struct.pack('<i',3+2*i)+dictionary({})+struct.pack('<i',len(ids))+struct.pack('<'+'i'*len(ids),*ids))
    data+=b''.join(entries)
    # RGBA entry zero is the color for voxel index 1, not the empty color.
    data+=chunk('RGBA',np.vstack((PALETTE[1:],PALETTE[:1])).tobytes())
    for family in range(len(BASES)):
        for shade in range(16):
            index=1+family*16+shade
            attrs={'_type':'_diffuse','_rough':'0.85'}
            if family in (STEEL,METAL,BRASS): attrs={'_type':'_metal','_metal':'0.35','_rough':'0.65'}
            data+=chunk('MATL',struct.pack('<i',index)+dictionary(attrs))
    path=OUT/'leopard2a6.vox'
    path.write_bytes(b'VOX '+struct.pack('<i',150)+chunk('MAIN',children=data))
    return path,len(pieces)


def validate_vox(path):
    """Read the exported file and traverse its graph, then compare every cell."""
    raw=path.read_bytes()
    assert raw[:8]==b'VOX '+struct.pack('<i',150)
    assert raw[8:12]==b'MAIN'
    assert struct.unpack_from('<ii',raw,12)==(0,len(raw)-20)
    def read_dict(data,pos):
        n,=struct.unpack_from('<i',data,pos); pos+=4
        d={}
        for _ in range(n):
            pair=[]
            for j in range(2):
                length,=struct.unpack_from('<i',data,pos);pos+=4
                pair.append(data[pos:pos+length].decode());pos+=length
            d[pair[0]]=pair[1]
        return d,pos
    models=[];nodes={};pos=20;dims=None;palette=None
    while pos<len(raw):
        kind=raw[pos:pos+4].decode()
        n,c=struct.unpack_from('<ii',raw,pos+4)
        assert c==0
        d=raw[pos+12:pos+12+n];pos+=12+n
        if kind=='SIZE':
            dims=np.array(struct.unpack('<iii',d));assert np.all((dims>0)&(dims<=256))
        elif kind=='XYZI':
            count,=struct.unpack_from('<i',d)
            assert len(d)==4+count*4
            a=np.frombuffer(d[4:],np.uint8).reshape(-1,4).copy()
            assert np.all(a[:,:3]<dims) and np.all(a[:,3]>0)
            assert len(np.unique(a[:,:3],axis=0))==len(a)
            models.append((dims,a))
        elif kind in ('nTRN','nGRP','nSHP'):
            node,=struct.unpack_from('<i',d);attrs,p=read_dict(d,4)
            assert node not in nodes
            if kind=='nTRN':
                child,reserved,layer,frames=struct.unpack_from('<iiii',d,p);p+=16
                assert frames==1
                frame,p=read_dict(d,p)
                assert frame.get('_r','4')=='4'
                nodes[node]=('transform',[child],np.array(list(map(int,frame.get('_t','0 0 0').split()))))
            elif kind=='nGRP':
                count,=struct.unpack_from('<i',d,p);p+=4
                children=list(struct.unpack_from('<'+'i'*count,d,p));p+=4*count
                nodes[node]=('group',children,None)
            else:
                count,model=struct.unpack_from('<ii',d,p);p+=8
                assert count==1
                _,p=read_dict(d,p)
                nodes[node]=('shape',[],model)
            assert p==len(d)
        elif kind=='RGBA':
            rgba=np.frombuffer(d,np.uint8).reshape(256,4)
            palette=np.vstack((rgba[-1:],rgba[:-1]))
    assert pos==len(raw) and np.array_equal(palette,PALETTE)
    rebuilt=np.zeros_like(V);used=[];visited=set()
    def walk(node,translation):
        assert node not in visited
        visited.add(node)
        kind,children,value=nodes[node]
        if kind=='transform':translation=translation+value
        if kind=='shape':
            used.append(value)
            size,cells=models[value]
            xyz=cells[:,:3].astype(int)-size//2+translation-LO
            assert np.all(xyz>=0) and np.all(xyz<SHAPE)
            indices=tuple(xyz.T)
            assert not np.any(rebuilt[indices])
            rebuilt[indices]=cells[:,3]
        for child in children: walk(child,translation)
    walk(0,np.zeros(3,int))
    assert len(visited)==len(nodes) and sorted(used)==list(range(len(models)))
    assert np.array_equal(rebuilt,V), 'Export changed voxel positions or colors'
    # Preview renderer consumes this round-tripped volume, never separate geometry.
    with (OUT/'render_volume.bin').open('wb') as f:
        f.write(struct.pack('<6i',*SHAPE,*LO));f.write(PALETTE.tobytes());f.write(rebuilt.tobytes())
    coords=np.argwhere(rebuilt)
    bounds=[(coords.min(0)+LO).tolist(),(coords.max(0)+LO).tolist()]
    stats={'file':path.name,'vox_version':150,'voxels':int(np.count_nonzero(rebuilt)),
           'bounds_inclusive_xyz':bounds,'dimensions_xyz':(coords.max(0)-coords.min(0)+1).tolist(),
           'models':len(models),'scene_nodes':len(nodes),'groups':list(GROUPS.values()),
           'palette_colors_used':len(np.unique(rebuilt))-1,
           'validation':'PASS: chunk bounds, unique cells, palette, scene references, full voxel round trip',
           'axes':'+X forward, +Y left, +Z up; origin at hull center / ground',
           'meters_per_voxel':0.0125,
           'dimensions_meters':((coords.max(0)-coords.min(0)+1)*.0125).tolist(),
           'hull_length_meters':7.7,'height_without_antennas_meters':3.0}
    (OUT/'validation.json').write_text(json.dumps(stats,indent=2),encoding='utf8')
    print(json.dumps(stats,indent=2))


if __name__=='__main__':
    started=time.time()
    for side in (-1,1): tracks(side)
    print('Running gear done',flush=True)
    hull();print('Hull done',flush=True)
    turret();main_gun();equipment();surface_detail()
    calibrate_dimensions()
    path,models=export_vox()
    validate_vox(path)
    print(f'Built in {time.time()-started:.1f}s; {path.stat().st_size/1048576:.1f} MiB',flush=True)
