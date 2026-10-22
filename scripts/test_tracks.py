"""检查真实 player/enemy 履带包，防止把侧面整根竖条当作运动履带。"""
from pathlib import Path
import struct
import unittest

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
                models = read_models(ASSETS / 'tracks' / chassis / ('cleat' if chassis == 'player' else 'crawl') / 'track.vox')
                self.assertEqual(len(models), 20 if chassis == 'player' else 18)
                body, = read_models(ASSETS / 'bodies' / chassis / ('anvil' if chassis == 'player' else 'carapace') / 'body.vox')
                sides = ((0,0,1),(1,35,34)) if chassis == 'player' else ((0,0,1),(1,33,32))
                for side, outer_y, face_y in sides:
                    phases = models[side*3:side*3+3]
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
