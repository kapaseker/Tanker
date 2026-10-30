"""Reference-inspired tank. X forward, Y left, Z up; run with Python + numpy/Pillow."""
from pathlib import Path
import struct, json, math
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

OUT = Path(__file__).resolve().parent
V = np.zeros((480, 180, 208), np.uint8)
PALETTE = [(0, 0, 0, 0)]
BASE = [(115,111,71), (133,126,83), (91,92,61), (46,46,39),
        (64,60,48), (145,132,87), (29,37,34), (164,174,152),
        (234,220,163), (82,77,58), (110,101,70), (37,30,25)]
for rgb in BASE:
    for d in (-5,-2,0,2,5): PALETTE.append(tuple(max(0,min(255,c+d)) for c in rgb)+(255,))
PALETTE += [(0,0,0,255)]*(256-len(PALETTE))

def paint(x,y,z,mask,mat):
    region=V[x,y,z]
    xx,yy,zz=np.indices(region.shape)
    # Stable small surface variation; broader blocks keep the armor readable.
    noise=((xx+x.start)//3*73856093)^((yy+y.start)//3*19349663)^((zz+z.start)//3*83492791)
    shade=(noise^(noise>>13))%5
    region[mask]=(1+mat*5+shade[mask]).astype(np.uint8) if mat>=0 else 0

def box(a,b,mat=0):
    a=np.maximum(a,0).astype(int); b=np.minimum(b,V.shape).astype(int)
    if np.any(b<=a): return
    ss=tuple(slice(int(i),int(j)) for i,j in zip(a,b))
    paint(*ss,np.ones(tuple(b-a),bool),mat)

def cyl(axis,center,r,lo,hi,mat=0,inner=0):
    axes=[i for i in range(3) if i!=axis]
    a=[0]*3;b=[0]*3;a[axis]=lo;b[axis]=hi
    for i,c in zip(axes,center): a[i]=int(c-r-1);b[i]=int(c+r+2)
    a=np.maximum(a,0);b=np.minimum(b,V.shape)
    ss=tuple(slice(int(i),int(j)) for i,j in zip(a,b))
    grid=np.ogrid[tuple(slice(int(i),int(j)) for i,j in zip(a,b))]
    d=sum((grid[i]-c)**2 for i,c in zip(axes,center))
    mask=np.broadcast_to((d<=r*r)&(d>=inner*inner),tuple(b-a))
    paint(*ss,mask,mat)

def prism(poly,z0,z1,mat=0):
    from matplotlib.path import Path as Poly
    x0,y0=np.floor(np.min(poly,axis=0)).astype(int);x1,y1=np.ceil(np.max(poly,axis=0)).astype(int)+1
    xx,yy=np.meshgrid(np.arange(x0,x1),np.arange(y0,y1),indexing='ij')
    mask=Poly(poly).contains_points(np.c_[xx.ravel()+.01,yy.ravel()+.01]).reshape(xx.shape)
    paint(slice(x0,x1),slice(y0,y1),slice(z0,z1),np.broadcast_to(mask[:,:,None],(*mask.shape,z1-z0)),mat)

def build():
    # Track loops follow a stadium profile, with separate shoes and rubber pads.
    for y0,y1 in [(13,37),(143,167)]:
        for x in range(19,306):
            for z in range(5,66):
                d=math.hypot(max(47-x,0,x-277),z-35)
                if 25<=d<=30: box((x,y0,z),(x+1,y1,z+1),3)
        path=[]
        for x in np.arange(47,278,7): path += [(x,5,0),(x,65,0)]
        for cx,sgn in [(47,-1),(277,1)]:
            for t in np.linspace(-math.pi/2,math.pi/2,14):
                path.append((cx+sgn*30*math.cos(t),35+30*math.sin(t),t))
        for x,z,t in path:
            box((x-2,y0-1,z-2),(x+3,y1+1,z+2),4)
            box((x-1,y0+4,z-2),(x+2,y1-4,z+3),3)
            for yy in (y0,y1-2): box((x-1,yy,z),(x+1,yy+2,z+2),10)
        for x in range(63,265,33):
            cyl(1,(x,29),19,y0+2,y1-2,3)
            cyl(1,(x,29),16,y0,y1,0)
            cyl(1,(x,29),12,y0-1,y1+1,2)
            cyl(1,(x,29),10,y0-2,y1+2,0)
            cyl(1,(x,29),5,y0-3,y1+3,4)
            cyl(1,(x,29),3,y0-4,y1+4,1)
            for t in np.linspace(0,2*math.pi,9)[:-1]:
                for yy in (y0-2,y1+1):
                    cyl(1,(x+8*math.cos(t),29+8*math.sin(t)),1,yy,yy+1,5)
        for x in (41,285):
            cyl(1,(x,37),20,y0+1,y1-1,4)
            cyl(1,(x,37),15,y0,y1,0)
            cyl(1,(x,37),10,y0-1,y1+1,3)
            cyl(1,(x,37),6,y0-2,y1+2,5)
    # Lower hull, sloped nose and broad upper deck.
    box((34,36,34),(285,144,72),2)
    for z in range(39,87):
        front=int(286+(z-39)*.42) if z<66 else int(297-(z-66)*.85)
        box((27,31,z),(front,149,z+1),0)
    box((25,14,64),(282,166,80),0)
    for z in range(80,92): box((32,23,z),(int(283-(z-80)*1.7),157,z+1),0)
    # Rear engine deck: inset black grilles with fine olive slats.
    for yy in (39,96):
        box((32,yy,91),(99,yy+45,93),3)
        for xx in range(35,98,4): box((xx,yy+2,93),(xx+2,yy+43,95),4)
        for xx in (32,97): box((xx,yy,93),(xx+2,yy+45,96),1)
    for yy in (18,149):
        box((26,yy,82),(91,yy+13,94),0)
        for xx in range(29,88,14):
            box((xx,yy,94),(xx+11,yy+13,95),1)
            box((xx+4,yy+4,95),(xx+8,yy+8,97),4)
    # Individual skirt panels, hinges and rail; gaps expose a dark backing.
    for yy in (10,164):
        box((28,yy,43),(278,yy+6,79),3)
        for i,xx in enumerate(range(29,277,31)):
            box((xx,yy-1,45),(xx+29,yy+7,76),0 if i%3 else 1)
            box((xx,yy-2,73),(xx+29,yy+8,77),1)
            for hx in (xx+2,xx+23):
                box((hx,yy-2,72),(hx+4,yy+8,79),4)
                box((hx+1,yy-3,76),(hx+3,yy+9,78),5)
    # Front glacis armor panels and front fenders.
    for xx in range(207,279,23):
        for yy in (37,91):
            z=int(92-max(0,xx-245)*.35)
            box((xx,yy,z),(xx+21,yy+51,z+2),0)
            for yb in (yy+3,yy+46): box((xx+2,yb,z+2),(xx+4,yb+2,z+3),5)
    for yy in (11,142):
        for z in range(65,85):
            x=302-(z-65)
            box((x-14,yy,z),(x,yy+27,z+1),1 if z>81 else 0)
        box((267,yy+7,85),(279,yy+20,94),3)
        box((278,yy+9,87),(281,yy+18,92),8)
        box((264,yy+5,94),(281,yy+22,96),0)
    for yy in (51,126):
        for z in range(41,73,8): box((291+(z-40)//5,yy,z),(297+(z-40)//5,yy+4,z+6),4)
    # Turret bearing, angular armored body and two massive wedge cheeks.
    cyl(2,(158,90),65,92,97,3)
    cyl(2,(158,90),62,97,101,4)
    for z in range(100,147):
        shrink=max(0,z-125)*.38
        poly=[(80,34+shrink),(165,29+shrink),(204,39+shrink),(225,65),(225,115),(204,141-shrink),(165,151-shrink),(80,146-shrink),(71,130),(71,50)]
        prism(poly,z,z+1,0)
    # Armor wedges slope backward toward roof, with lower undercuts.
    for z in range(103,146):
        front=239-(z-116)*.65 if z>=116 else 230+(z-103)*.7
        for ya,yb in [(35,78),(102,145)]:
            prism([(177,ya),(front-6,ya+5),(front,yb-3),(183,yb)],z,z+1,1 if z in (116,144,145) else 0)
    # Roof plates with narrow joints.
    for xa,xb in [(83,119),(121,160),(162,193),(195,215)]:
        for ya,yb in [(48,86),(89,130)]:
            box((xa,ya,147),(xb,yb,149),0)
            for xx in (xa+2,xb-3): box((xx,ya+2,149),(xx+2,ya+4,150),5)
    # Side applique blocks and smoke launchers.
    for side in (-1,1):
        for xx in (106,127,151):
            for zz in (111,126):
                yy=29 if side<0 else 146
                box((xx,yy,zz),(xx+18,yy+7,zz+13),2)
                box((xx+1,yy-1,zz+1),(xx+16,yy+8,zz+12),0)
        yy=24 if side<0 else 149
        for xx in (78,86,94,102):
            for zz in (117,130):
                box((xx,yy,zz),(xx+6,yy+7,zz+9),4)
                box((xx+1,yy-1,zz+5),(xx+5,yy+8,zz+9),3)
    # Bustle stowage baskets, external ribs, spare track links.
    for yy in (35,109):
        box((56,yy,110),(76,yy+36,140),2)
        for zz in range(111,141,6): box((54,yy,zz),(58,yy+36,zz+2),4)
        for yb in range(yy,yy+37,9): box((53,yb,110),(59,yb+2,142),1)
    # Main gun: mantlet, recoil sleeve, thermal shroud rings and open dark muzzle.
    cyl(0,(90,128),17,214,240,2)
    cyl(0,(90,128),14,232,254,0)
    cyl(0,(90,128),12,252,270,4)
    cyl(0,(90,128),10,269,285,3)
    cyl(0,(90,128),7,282,448,0)
    for xx in (278,286,315,346,378,410,439): cyl(0,(90,128),8,xx,xx+3,5 if xx==439 else 4)
    cyl(0,(90,128),9,445,456,1)
    cyl(0,(90,128),6,451,457,-1)
    cyl(0,(90,128),5,450,451,11)
    # Commander's circular cupola, dark viewing ring, armored sight tower.
    cyl(2,(132,87),31,149,154,1)
    cyl(2,(132,87),28,154,161,3)
    for t in np.arange(0,2*math.pi,math.pi/4):
        x=132+26*math.cos(t);y=87+26*math.sin(t)
        box((x-3,y-3,154),(x+4,y+4,161),0)
    cyl(2,(132,87),29,161,164,1)
    cyl(2,(130,87),17,164,168,2)
    box((119,75,168),(143,99,191),0)
    box((140,78,171),(145,96,188),3)
    box((144,81,175),(146,93,185),6)
    box((120,76,191),(141,98,194),1)
    box((125,81,194),(137,94,196),2)
    for zz in (173,184): box((117,79,zz),(120,94,zz+3),4)
    # Loader hatch and machine gun.
    cyl(2,(168,121),17,149,153,1)
    cyl(2,(168,121),14,153,156,2)
    box((163,116,156),(170,124,168),4)
    box((162,114,165),(186,123,174),2)
    box((160,105,164),(170,114,176),0)
    cyl(0,(119,170),2.4,183,209,3)
    cyl(0,(119,170),3,207,210,4)
    box((174,115,174),(179,122,177),4)
    # Antenna bases and tapered whips, roof grab handles and periscopes.
    for x,y,h in [(94,51,206),(159,131,193)]:
        cyl(2,(x,y),6,149,153,4)
        box((x-4,y-4,153),(x+5,y+5,163),0)
        box((x-1,y-1,163),(x+2,y+2,181),4)
        box((x,y,181),(x+1,y+1,h),5)
    for x,y in [(178,62),(196,112),(110,120),(207,82)]:
        box((x,y,149),(x+15,y+7,152),2)
        box((x+2,y,152),(x+13,y+2,154),6)
    for x,y in [(95,67),(157,57),(189,130),(226,38),(246,136)]:
        box((x,y,150 if x<210 else 92),(x+2,y+7,155 if x<210 else 96),4)
        box((x+10,y,150 if x<210 else 92),(x+12,y+7,155 if x<210 else 96),4)
        box((x,y,154 if x<210 else 95),(x+12,y+2,156 if x<210 else 97),1)
    # Rear lamps, towing hardware and exhaust grille.
    box((24,55,57),(28,125,77),3)
    for yy in range(57,124,5): box((22,yy,59),(25,yy+2,75),4)
    for yy in (42,134): box((24,yy,71),(27,yy+6,76),10)

def chunk(tag,body=b'',children=b''): return tag+struct.pack('<II',len(body),len(children))+body+children
def ints(*v): return struct.pack('<'+'i'*len(v),*v)
def dic(d):
    b=ints(len(d))
    for k,v in d.items():
        for s in (k,v):
            t=str(s).encode(); b+=ints(len(t))+t
    return b

def write_vox(path,grid,multi):
    models=[]
    for x in range(0,grid.shape[0],240):
        for y in range(0,grid.shape[1],240):
            for z in range(0,grid.shape[2],240):
                g=grid[x:x+240,y:y+240,z:z+240]
                xyz=np.argwhere(g)
                if len(xyz): models.append((g.shape,xyz,g[tuple(xyz.T)],(x,y,z)))
    parts=[]
    if multi: parts.append(chunk(b'PACK',ints(len(models))))
    for shape,xyz,colors,origin in models:
        parts += [chunk(b'SIZE',ints(*shape)),chunk(b'XYZI',ints(len(xyz))+np.c_[xyz,colors].astype(np.uint8).tobytes())]
    if multi:
        parts.append(chunk(b'nTRN',ints(0)+dic({'_name':'Reference tank'})+ints(1,-1,0,1)+dic({})))
        parts.append(chunk(b'nGRP',ints(1)+dic({})+ints(len(models))+ints(*[2+i*2 for i in range(len(models))])))
        for i,(shape,xyz,c,origin) in enumerate(models):
            # VOX shapes are centered on their SIZE box by the scene graph.
            t=[origin[j]+shape[j]//2 for j in range(3)]
            parts.append(chunk(b'nTRN',ints(2+i*2)+dic({'_name':f'tank_section_{i}'})+ints(3+i*2,-1,0,1)+dic({'_t':' '.join(map(str,t))})))
            parts.append(chunk(b'nSHP',ints(3+i*2)+dic({})+ints(1,i)+dic({})))
        parts.append(chunk(b'LAYR',ints(0)+dic({'_name':'Tank'})+ints(-1)))
    parts.append(chunk(b'RGBA',bytes(v for c in PALETTE[1:]+[PALETTE[0]] for v in c)))
    path.write_bytes(b'VOX '+ints(150)+chunk(b'MAIN',children=b''.join(parts)))
    return len(models)

def validate(path):
    data=path.read_bytes(); assert data[:4]==b'VOX '
    assert struct.unpack_from('<I',data,16)[0]==len(data)-20
    pos=20;count=0;models=0
    while pos<len(data):
        tag=data[pos:pos+4];size,children=struct.unpack_from('<II',data,pos+4);body=pos+12
        assert body+size+children<=len(data)
        if tag==b'SIZE': shape=struct.unpack_from('<iii',data,body); models+=1
        if tag==b'XYZI':
            n=struct.unpack_from('<I',data,body)[0]; assert size==4+4*n
            a=np.frombuffer(data,dtype=np.uint8,count=n*4,offset=body+4).reshape(-1,4)
            assert np.all(a[:,:3]<shape) and np.all(a[:,3]>0)
            assert len(np.unique(a[:,:3],axis=0))==n
            count+=n
        pos=body+size+children
    assert pos==len(data)
    return {'voxels':count,'models':models,'bytes':len(data)}

def render(grid,path,rear=False):
    if rear: grid=grid[::-1,::-1,:]
    occupied=grid>0; records=[]
    directions=[(0,1,.79),(1,-1,.58),(2,1,1.13)]
    # Visible voxel faces, depth-sorted orthographically: the preview is actual voxel geometry.
    for axis,sign,light in directions:
        neighbor=np.roll(occupied,-sign,axis=axis)
        idx=[slice(None)]*3;idx[axis]=-1 if sign==1 else 0;neighbor[tuple(idx)]=False
        xyz=np.argwhere(occupied & ~neighbor)
        colors=grid[tuple(xyz.T)]
        for p,c in zip(xyz,colors): records.append((float(.6175*p[0]-.855*p[1]+.5405*p[2]),p,int(c),axis,sign,light))
    records.sort(key=lambda r:r[0])
    W,H=1800,1200
    points=np.argwhere(occupied)
    px=.90*points[:,0]+.65*points[:,1];py=.25*points[:,0]-.42*points[:,1]-.95*points[:,2]
    scale=min(1560/(px.max()-px.min()),960/(py.max()-py.min()))
    ox=(W-scale*(px.max()+px.min()))/2;oy=(H-scale*(py.max()+py.min()))/2
    def project(p):
        x,y,z=p;return (ox+scale*(.90*x+.65*y),oy+scale*(.25*x-.42*y-.95*z))
    im=Image.new('RGB',(W,H),(34,37,37)); shadow=Image.new('RGBA',(W,H));sd=ImageDraw.Draw(shadow)
    sd.polygon([project(p) for p in [(13,4,0),(315,4,0),(315,177,0),(13,177,0)]],fill=(0,0,0,150))
    im=Image.alpha_composite(im.convert('RGBA'),shadow.filter(ImageFilter.GaussianBlur(25))).convert('RGB')
    draw=ImageDraw.Draw(im)
    for depth,p,c,axis,sign,light in records:
        axes=[i for i in range(3) if i!=axis]; base=p.astype(float);base[axis]+=int(sign>0)
        corners=[]
        for u,v in [(0,0),(1,0),(1,1),(0,1)]:
            q=base.copy();q[axes[0]]+=u;q[axes[1]]+=v;corners.append(project(q))
        col=np.array(PALETTE[c][:3],float)
        # Local occlusion in the air immediately outside the face.
        q=p.copy();q[axis]+=sign;ao=0
        for other in axes:
            for s in (-3,3):
                a=q.copy();a[other]+=s
                if np.all(a>=0) and np.all(a<grid.shape) and occupied[tuple(a)]: ao+=1
        shade=light*(1-.075*ao)
        col=np.clip(col*shade,0,255).astype(int)
        draw.polygon(corners,fill=tuple(col))
    im.save(path)

if __name__=='__main__':
    build()
    write_vox(OUT/'reference_tank.vox',V,True)
    # Majority occupied sample preserves thin details better than striding.
    small=np.zeros(tuple(s//2 for s in V.shape),np.uint8)
    for dx in range(2):
        for dy in range(2):
            for dz in range(2):
                a=V[dx::2,dy::2,dz::2];small[a>0]=a[a>0]
    write_vox(OUT/'reference_tank_single.vox',small,False)
    report={p.name:validate(p) for p in OUT.glob('*.vox')}
    assert report['reference_tank.vox']['voxels']==int(np.count_nonzero(V))
    np.savez_compressed(OUT/'working_grid.npz',voxels=V,palette=PALETTE)
    (OUT/'validation.json').write_text(json.dumps(report,indent=2))
    print(json.dumps(report),flush=True)
    render(V,OUT/'preview_front.png')
    render(V,OUT/'preview_rear.png',True)
    print('Previews complete.',flush=True)
