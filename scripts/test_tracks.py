"""检查真实 player/enemy 履带包，防止把侧面整根竖条当作运动履带。"""
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import subdivide_vox as sv

ASSETS = Path(__file__).resolve().parents[1] / 'assets' / 'tank'


def read_models(path):
    """读取生成产物的各相位体素；测试实际交付的资产而非生成器辅助函数。"""
    data = path.read_bytes()
    models = []
    offset = 20
    while offset < len(data):
        tag, size, children = struct.unpack_from('<4sII', data, offset)
        if tag == b'XYZI':
            voxels = list(struct.iter_unpack('4B', data[offset+16:offset+12+size]))
            cells = {v[:3]: v[3] for v in voxels}
            if len(cells) != len(voxels):
                raise AssertionError('履带包包含重复体素')
            models.append(cells)
        offset += 12+size+children
    return models


# 各车型共享画布契约（参考格数），与 Rust 侧 CANVAS 一致；必须显式传给解析器，
# 避免对敌人包误套玩家画布。
PLAYER_CANVAS = (135, 36, 26)
ENEMY_CANVAS = (114, 34, 26)


def read_reference_models(path, canvas):
    """解析交付 .vox，按命名节点还原 {名称: {(参考格 x,y,z): 颜色}}。

    粗资产（0.1 米/格）模型坐标即参考格，直接取用；细分资产（0.02 米/格）
    用场景树的匿名分块平移还原全局细分格坐标：全局 = 块内 + 块起点 + 命名
    节点平移×5，块起点 = 分块平移 + (细分画布 − 块尺寸)/2，再整除 5 回参考格。
    canvas 为该包共享画布契约（参考格数），用于确定细分画布 = 5×canvas。
    """
    source = sv.parse_vox(path.read_bytes(), path.name)
    unit = sv.read_meta_unit(path.with_name(path.name + '.meta'))
    if unit == sv.TARGET_UNIT:
        factor = sv.FACTOR
    elif unit == 0.1:
        factor = 1
    else:
        raise AssertionError(f'{path.name}: 单位 {unit} 既非参考网格也非细分目标')
    root = source.nodes[0]
    if root.kind != 'trn' or source.nodes[root.child].kind != 'grp':
        raise AssertionError(f'{path.name}: 根场景结构不符契约')
    out = {}
    for node_id in source.nodes[root.child].children:
        node = source.nodes[node_id]
        if node.kind != 'trn' or '_name' not in node.node_dict:
            raise AssertionError(f'{path.name}: 顶层子节点应为命名 nTRN')
        name = node.node_dict['_name']
        t_named = sv.frame_t(node.frames)
        child = source.nodes[node.child]
        cells = {}
        if child.kind == 'shp':
            if factor != 1:
                raise AssertionError(f'{path.name}: {name} 细分后应展开为分块组')
            model = source.models[child.models[0][0]]
            for x, y, z, c in struct.iter_unpack('<BBBB', model.xyzi):
                cells[(x + t_named[0], y + t_named[1], z + t_named[2])] = c
        else:
            full = tuple(factor * s for s in canvas)
            for chunk_id in child.children:
                chunk = source.nodes[chunk_id]
                t_chunk = sv.frame_t(chunk.frames)
                model = source.nodes[chunk.child].models[0][0]
                model = source.models[model]
                # 轮子使用局部画布，细分后单模型仍小于 256 格。
                if name.startswith('wheel_'):
                    full = model.size
                # 转换器保证居中平移为整格；奇偶违例说明产物与契约不符。
                if any((f - s) % 2 for f, s in zip(full, model.size)):
                    raise AssertionError(f'{path.name}: {name} 分块 {model.size} 无法在画布 {full} 居中')
                origin = tuple(t + (f - s) // 2
                               for t, f, s in zip(t_chunk, full, model.size))
                for x, y, z, c in struct.iter_unpack('<BBBB', model.xyzi):
                    g = (x + origin[0] + t_named[0],
                         y + origin[1] + t_named[1],
                         z + origin[2] + t_named[2])
                    ref = tuple(v // factor for v in g)
                    if not all(0 <= r < s for r, s in zip(ref, canvas)):
                        raise AssertionError(f'{path.name}: {name} 还原格 {ref} 超出画布 {canvas}')
                    if cells.get(ref, c) != c:
                        raise AssertionError(f'{path.name}: {name} 分块降采样在 {ref} 颜色冲突')
                    cells[ref] = c
        out[name] = cells
    return out


class TrackLoopTests(unittest.TestCase):
    def test_both_chassis_roll_around_the_rim(self):
        for chassis, end, top, bevel in (('player', 74, 11, True), ('enemy', 65, 12, True)):
            with self.subTest(chassis=chassis):
                # VOX +X 是车头：前进时上段向 +X、前端向下、下段向 −X、后端向上。
                rim = ([(x, top) for x in range(end+1)]
                       + [(end, z) for z in range(top-1, 0, -1)]
                       + [(x, 0) for x in range(end-int(bevel), int(bevel)-1, -1)]
                       + [(0, z) for z in range(1, top)])
                if chassis == 'player':
                    rim = [(x+30, z) for x,z in rim]
                else:
                    rim = ([(x,12) for x in range(24,90)] + [(89,z) for z in range(11,1,-1)]
                           + [(88,1)] + [(x,0) for x in range(87,25,-1)]
                           + [(25,1)] + [(24,z) for z in range(2,12)])
                self.assertEqual(len(rim), len(set(rim)))
                self.assertEqual(len(rim) % 3, 0, '闭环接缝的齿距也必须连续')
                canvas = PLAYER_CANVAS if chassis == 'player' else ENEMY_CANVAS
                track = read_reference_models(ASSETS / 'tracks' / chassis / ('cleat' if chassis == 'player' else 'crawl') / 'track.vox', canvas)
                self.assertEqual(len(track), 20 if chassis == 'player' else 18)
                body = read_reference_models(ASSETS / 'bodies' / chassis / ('anvil' if chassis == 'player' else 'carapace') / 'body.vox', canvas)['body']
                sides = ((0,0,1),(1,35,34)) if chassis == 'player' else ((0,0,1),(1,33,32))
                for side, outer_y, face_y in sides:
                    phases = [track[f'track_{"lr"[side]}_{phase}'] for phase in range(3)]
                    stationary = None
                    for phase, cells in enumerate(phases):
                        self.assertFalse(cells.keys() & body.keys())
                        outer = {(x, z) for x, y, z in cells if y == outer_y}
                        expected = {point for s, point in enumerate(rim) if (s-phase) % 3 == 0}
                        self.assertEqual(outer, expected, f'{chassis} 侧 {side} 相位 {phase} 必须沿轮廓滚动')
                        # 两种底盘内侧连接结构不随履带相位平移或变色。
                        wheels = {(x, z): c for (x, y, z), c in cells.items()
                                  if y == face_y and (x, z) not in rim and z <= top}
                        if stationary is None:
                            stationary = wheels
                        self.assertEqual(wheels, stationary)
                        if chassis == 'enemy':
                            self.assertFalse(any((x,face_y,6) in cells for x in (44,57,70)), '敌人轮间必须透空')
                        else:
                            self.assertFalse(any((x, face_y, 6) in cells for x in (50, 62, 74, 86)), '玩家轮间必须透空')
                        next_positions = {rim[(s+1) % len(rim)] for s, point in enumerate(rim)
                                          if point in outer}
                        next_cells = phases[(phase+1) % 3]
                        self.assertEqual(next_positions, {(x, z) for x, y, z in next_cells if y == outer_y})


if __name__ == '__main__':
    unittest.main()
