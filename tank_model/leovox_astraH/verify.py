"""从磁盘独立解析标准 VOX，检查场景装配、量纲和关键外观结构。"""
from pathlib import Path
import hashlib
import json
import struct

import numpy as np
from scipy import ndimage

ROOT=Path(__file__).resolve().parent


class Reader:
    """有边界检查的块读取器；残缺或长度不一致的文件直接失败。"""
    def __init__(self,data):
        self.data=data; self.pos=0

    def take(self,n):
        assert n>=0 and self.pos+n<=len(self.data),'截断文件'
        data=self.data[self.pos:self.pos+n]; self.pos+=n
        return data

    def int(self):
        return struct.unpack('<i',self.take(4))[0]

    def dictionary(self):
        count=self.int(); assert 0<=count<100
        return {self.take(self.int()).decode(): self.take(self.int()).decode() for _ in range(count)}


def read_vox(path):
    """只接受本交付使用的静态平移场景；不默默忽略未知旋转或动画。"""
    data=path.read_bytes(); reader=Reader(data)
    assert reader.take(4)==b'VOX ' and reader.int()==150
    assert reader.take(4)==b'MAIN' and reader.int()==0
    assert reader.int()==len(data)-20
    models=[]; nodes={}; palette=None; packed=None
    while reader.pos<len(data):
        tag=reader.take(4); n=reader.int(); children=reader.int()
        assert children==0
        body=Reader(reader.take(n))
        if tag==b'PACK': packed=body.int()
        elif tag==b'SIZE': size=np.array([body.int() for _ in range(3)])
        elif tag==b'XYZI':
            count=body.int(); cells=np.frombuffer(body.take(count*4),np.uint8).reshape(-1,4).copy()
            assert np.all(cells[:,:3]<size) and np.all(cells[:,3]>0)
            assert len(np.unique(cells[:,:3],axis=0))==count,'局部体素坐标重复'
            models.append((size.copy(),cells))
        elif tag==b'nTRN':
            node=body.int(); attrs=body.dictionary(); child=body.int()
            assert node==len(nodes),'节点必须按 ID 递增存储以兼容 dot_vox'
            assert body.int()==-1; layer=body.int(); assert layer==0
            assert body.int()==1
            frame=body.dictionary(); assert '_r' not in frame
            offset=np.array(list(map(int,frame['_t'].split())))
            assert node not in nodes
            nodes[node]=('transform',attrs['_name'],child,offset)
        elif tag==b'nGRP':
            node=body.int(); body.dictionary(); count=body.int()
            assert node==len(nodes),'节点必须按 ID 递增存储以兼容 dot_vox'
            assert node not in nodes
            nodes[node]=('group',[body.int() for _ in range(count)])
        elif tag==b'nSHP':
            node=body.int(); body.dictionary(); assert body.int()==1
            assert node==len(nodes),'节点必须按 ID 递增存储以兼容 dot_vox'
            model=body.int(); body.dictionary()
            assert node not in nodes
            nodes[node]=('shape',model)
        elif tag==b'RGBA': palette=np.frombuffer(body.take(1024),np.uint8).reshape(256,4)
        elif tag==b'LAYR': body.int(); body.dictionary(); assert body.int()==-1
        else: raise AssertionError(f'未知块 {tag!r}')
        assert body.pos==len(body.data),tag
    assert len(models)==packed and palette is not None
    grid=np.zeros((549,188,224),np.uint8); owners=np.zeros_like(grid)
    parts={}; seen=set(); visited=set()
    def walk(node,translation,names):
        assert node not in visited,'场景环或重复引用'; visited.add(node)
        entry=nodes[node]
        if entry[0]=='transform':
            walk(entry[2],translation+entry[3],names+[entry[1]])
        elif entry[0]=='group':
            for child in entry[1]: walk(child,translation,names)
        else:
            model=entry[1]; assert model not in seen; seen.add(model)
            size,cells=models[model]
            coords=cells[:,:3].astype(int)+translation-size//2+(0,94,0)
            assert np.all(coords>=0) and np.all(coords<grid.shape)
            idx=tuple(coords.T)
            assert np.all(grid[idx]==0),'场景分组重叠'
            name=names[1]
            if name not in parts: parts[name]=len(parts)+1
            owners[idx]=parts[name]; grid[idx]=cells[:,3]
    walk(0,np.zeros(3,int),[])
    assert len(seen)==len(models) and len(visited)==len(nodes)
    assert np.max(grid)<=19
    return grid,owners,parts,data


def main():
    grid,owners,parts,data=read_vox(ROOT/'leopard_2a6.vox')
    manifest=json.loads((ROOT/'manifest.json').read_text(encoding='utf-8'))
    assert hashlib.sha256(data).hexdigest()==manifest['sha256']
    assert np.count_nonzero(grid)==manifest['voxel_count']
    checks=[]
    def check(condition,label):
        assert condition,label
        checks.append(label)
    def bounds(name):
        xyz=np.argwhere(owners==parts[name])
        return xyz.min(0)-(0,94,0),xyz.max(0)+1-(0,94,0)
    xyz=np.argwhere(grid)
    low=xyz.min(0)-(0,94,0); high=xyz.max(0)+1-(0,94,0)
    check(np.array_equal(low,[0,-94,0]) and np.array_equal(high,[549,94,216]),'整车包络 10.98 × 3.76 × 4.32 m，地面 z=0')
    lo,hi=bounds('hull')
    check(lo[0]==0 and hi[0]==383,'非炮管车体长度 7.66 m')
    lo,hi=bounds('L55_gun')
    check(hi[0]==549,'炮口端面 x=10.98 m')
    lo,hi=bounds('turret')
    check(lo[0]==76 and hi[0]==306 and lo[1]==-75 and hi[1]==75,'炮塔含楔甲长 4.60 m、主体宽 3.00 m')
    check(np.all(grid[541:549,93:95,103:105]==0),'炮口内腔贯通，非黑色实心端面')
    check(np.count_nonzero(grid[180:200,35:60,131])>200,'炮塔主体屋顶在 z=132 边界')
    check(np.count_nonzero(grid[20:70,30:155,79])>3000,'后甲板基准高 1.60 m')
    for side in ['left','right']:
        wheels=[p for p in parts if p.startswith(side+'_roadwheel_')]
        check(len(wheels)==7,f'{side}: 七个独立负重轮')
        for name in wheels:
            lo,hi=bounds(name)
            check(hi[0]-lo[0]==40 and hi[1]-lo[1]==20 and lo[2]==1 and hi[2]==41,f'{name}: 直径 0.80 m、宽 0.40 m、轴高 0.42 m')
        lo,_=bounds(side+'_sprocket'); front,_=bounds(side+'_idler')
        check(lo[0]<front[0],f'{side}: 后主动轮、前诱导轮')
        check(np.any(owners[:, :, 0]==parts[side+'_track']),f'{side}: 履带触地')
        check(np.any(owners[0:25,:,:]==parts[side+'_track']) and np.any(owners[365:,:,:]==parts[side+'_track']),f'{side}: 履带完整覆盖首尾')
    for name in ['commander_hatch','loader_hatch']:
        # 舱盖把手属于附件，本体 134 层必须存在。
        check(np.count_nonzero(owners[:,:,134]==parts[name])>500,f'{name}: 舱盖本体顶高 2.70 m')
    for cy in [-37,37]:
        check(grid[40,94+cy,83]>0,f'风扇中心 {cy}: 轮毂存在')
        patch=grid[15:65,94+cy-25:94+cy+25,83]
        check(np.count_nonzero(patch==0)>900,f'风扇 {cy}: 网罩存在真实空隙')
    for side in [-1,1]:
        for z in [104,117]:
            for x in [107,119,131,143]:
                check(np.any(owners[x:x+3,94+side*82:94+side*82+1,z+2:z+6]==parts[('right' if side<0 else 'left')+'_smoke_launchers']),f'烟幕筒: side={side}, x={x}, z={z}')
    # 六邻接检测给出零散几何证据，不把装配节点数误作连通体数。
    labels,count=ndimage.label(grid!=0)
    sizes=np.bincount(labels.ravel())[1:]
    check(count==1,'六邻接几何连续，没有悬空碎块')
    report={'result':'PASS','checks':checks,'sha256':manifest['sha256'],
            'voxel_count':int(np.count_nonzero(grid)),'named_parts':len(parts),
            'connected_components':count,'component_sizes_desc':sorted(map(int,sizes),reverse=True)[:20],
            'bounds_cells':[list(map(int,low)),list(map(int,high))]}
    (ROOT/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False,indent=2))


if __name__=='__main__': main()
