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
        elif tag==b'MATL':
            index=body.int(); finish=body.dictionary()
            assert 1 <= index <= 255 and finish['_type']=='_metal'
            assert all(0 <= float(finish[key]) <= 1 for key in ('_rough','_metal'))
        elif tag==b'LAYR': body.int(); body.dictionary(); assert body.int()==-1
        else: raise AssertionError(f'未知块 {tag!r}')
        assert body.pos==len(body.data),tag
    assert len(models)==packed and palette is not None
    grid=np.zeros((812,320,390),np.uint8); owners=np.zeros_like(grid)
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
            coords=cells[:,:3].astype(int)+translation-size//2+(291,160,145)
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
    grid,owners,parts,data=read_vox(ROOT/'m41a1.vox')
    manifest=json.loads((ROOT/'manifest.json').read_text())
    assert hashlib.sha256(data).hexdigest()==manifest['sha256']
    assert np.count_nonzero(grid)==manifest['voxel_count']
    checks=[]
    def check(condition,label):
        assert condition,label
        checks.append(label)
    def bounds(name):
        cells=np.argwhere(owners==parts[name])
        return cells.min(0)-(0,160,0),cells.max(0)+1-(0,160,0)
    cells=np.argwhere(grid)
    low=cells.min(0)-(0,160,0); high=cells.max(0)+1-(0,160,0)
    check(np.array_equal(low,[0,-160,0]) and np.array_equal(high,[812,160,390]),'整车包络 8.12 × 3.20 × 3.90 m，接地 z=0')
    check(len(parts)==33,'33 个命名装配组，不把存储分块误计为部件')
    for side in ('left','right'):
        wheels=[name for name in parts if name.startswith(side+'_roadwheel_')]
        check(len(wheels)==5,f'{side}：五个负重轮')
        check(len([n for n in parts if n.startswith(side+'_return_roller_')])==3,f'{side}：三个托带轮')
        for name in wheels:
            lo,hi=bounds(name)
            check(hi[0]-lo[0]==65 and hi[2]-lo[2]==65,f'{name}：轮径 0.65 m')
        lo,_=bounds(side+'_sprocket'); front,_=bounds(side+'_idler')
        check(lo[0]<front[0],f'{side}：后主动轮、前诱导轮')
        track=owners==parts[side+'_track']
        check(np.any(track[:,:,0]),f'{side}：履带触地')
        labels,count=ndimage.label(track)
        check(count==1,f'{side}：履带六邻接闭环连续')
        # 负重轮顶 68.5 cm，上履带底约 111 cm；开阔轮间区不能被履带截穿。
        check(not np.any(track[100:480,:,71:90]),f'{side}：上履带没有截穿负重轮')
    check(not np.any(grid[782:790,147:173,186:196]),'制退器第一侧窗贯穿')
    check(not np.any(grid[796:804,147:173,186:196]),'制退器第二侧窗贯穿')
    check(not np.any(grid[780:812,153:167,186:196]),'炮口轴向空腔贯通')
    for cy in (-54,54):
        patch=grid[38:126,160+cy-41:160+cy+41,147]
        check(np.any(patch==0) and np.any(patch!=0),f'后甲板 {cy}：格栅网罩有真实空隙')
    check(bounds('commander_cupola')[1][1]<0,'车长塔位于右侧')
    check(bounds('loader_hatch')[0][1]>0,'装填手舱盖位于左侧')
    check(bounds('machine_gun')[1][2]==302,'机枪顶高 3.02 m')
    # 悬挂、履带与独立附件允许组间接触；记录连通性而不宣称可活动结构已验收。
    report={'result':'PASS','checks':checks,'sha256':manifest['sha256'],
            'voxel_count':manifest['voxel_count'],'named_parts':len(parts),
            'bounds_cells':[low.tolist(),high.tolist()]}
    (ROOT/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(report,ensure_ascii=False,indent=2))


if __name__=='__main__': main()
