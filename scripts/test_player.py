"""锁定玩家三包的米制尺寸、接缝与统一坐标契约。

参考格断言尺寸、占用与装配；资产格独立验证真实凹槽及配色，允许同一参考格内有细节。
"""
import math
import unittest
import struct
import subdivide_vox as sv
from test_tracks import read_reference_models, ASSETS, PLAYER_CANVAS, PLAYER_Z0

TRACK = ASSETS / 'tracks/player/cleat/track.vox'
BODY = ASSETS / 'bodies/player/anvil/body.vox'
BARREL = ASSETS / 'barrels/player/lance/barrel.vox'


class PlayerTests(unittest.TestCase):
    def test_delivered_grille_has_fine_geometry(self):
        source = sv.parse_vox(BODY.read_bytes(), 'body')
        body = next(n for n in source.nodes if n.node_dict.get('_name') == 'body')
        cells = {}
        for index in source.nodes[body.child].children:
            node = source.nodes[index]
            model = source.models[source.nodes[node.child].models[0][0]]
            origin = tuple(t + (f - s) // 2 for t, f, s in
                           zip(sv.frame_t(node.frames), (720, 190, 185), model.size))
            for x, y, z, c in struct.iter_unpack('<BBBB', model.xyzi):
                # 采样作者格 78/79 的凹槽与面板；加 PLAYER_Z0 折算到资产格。
                if 180 <= x + origin[0] < 235 and 77 + PLAYER_Z0 <= z + origin[2] < 81 + PLAYER_Z0:
                    cells[tuple(a + b for a, b in zip((x, y, z), origin))] = c
        # 同一个旧参考格中同时存在 0.02 米凹槽与面板，重复槽仍在共同坐标位置。
        for x in (180, 229):
            self.assertNotIn((x, 90, 79 + PLAYER_Z0), cells)
            self.assertEqual(cells[x, 90, 78 + PLAYER_Z0], 4)
            self.assertEqual(cells[x + 2, 90, 79 + PLAYER_Z0], 2)

    def test_independent_wheels_have_centered_sizes_and_rotation_clearance(self):
        track = read_reference_models(TRACK, PLAYER_CANVAS)
        self.assertEqual(len(track), 24, '六个履带相位与十八个独立轮节点')
        # 轮细节按资产格（0.02 米）验证，局部画布坐标。
        fine = read_reference_models(TRACK, PLAYER_CANVAS, asset_grid=True)
        for side in ('l', 'r'):
            for index in range(9):
                self.assertIn(f'wheel_{side}_{index}', fine, f'缺少轮节点 {side}_{index}')
        for side in ('l', 'r'):
            for index in range(9):
                cells = fine[f'wheel_{side}_{index}']
                end = index >= 7
                size, envelope = (25, 13.0) if end else (40, 20.5)
                tire_in, hub = (10.0, 5.0) if end else (15.0, 9.0)
                self.assertEqual((min(y for x, y, z in cells), max(y for x, y, z in cells)), (0, 19))
                self.assertEqual((min(x for x, y, z in cells), max(x for x, y, z in cells)), (0, size - 1))
                self.assertEqual((min(z for x, y, z in cells), max(z for x, y, z in cells)), (0, size - 1))
                self.assertTrue({1, 2, 3} <= set(cells.values()), '轮胎深灰、轮毂橄榄绿、中心盖暗橄榄')
                for x, y, z in cells:
                    # 检查整个格的四个角，不能只检查体素中心。
                    for a in (x, x + 1):
                        for b in (z, z + 1):
                            self.assertLessEqual((a - size / 2) ** 2 + (b - size / 2) ** 2, envelope ** 2)
                # 三条不对称辐条须在轮毂与轮胎之间可见（半径取带中线）。
                for angle in (0.0, 1.7, 4.0):
                    r = (tire_in + hub) / 2.0
                    x = round(size / 2 - 0.5 + r * math.cos(angle))
                    z = round(size / 2 - 0.5 + r * math.sin(angle))
                    self.assertEqual(cells.get((x, 0, z)), 1, f'角度 {angle} 的辐条不可见')

    def test_shared_canvas_and_palette(self):
        palettes = {}
        for path in (BODY, BARREL, TRACK):
            data = path.read_bytes()
            offset = 20
            while offset < len(data):
                tag, size, children = struct.unpack_from('<4sII', data, offset)
                if tag == b'RGBA':
                    palettes[path.name] = data[offset + 12:offset + 12 + size]
                offset += 12 + size + children
            self.assertIn(path.name, palettes, f'{path.name} 缺少 RGBA 调色板')
        self.assertEqual(palettes[BODY.name], palettes[BARREL.name])
        self.assertEqual(palettes[BODY.name], palettes[TRACK.name])

    def test_global_parts_fit_shared_canvas(self):
        # 车身与炮管共用 144×38×37 参考画布；细分资产还原后仍不得越界。
        for path in (BODY, BARREL):
            models = read_reference_models(path, PLAYER_CANVAS)
            self.assertEqual(list(models), [path.stem], '全局部件包应只有一个命名节点')
            for x, y, z in models[path.stem]:
                self.assertTrue(0 <= x < 144 and 0 <= y < 38 and 0 <= z < 37,
                                f'{path.name} 占用 {x},{y},{z} 超出共享画布')

    def test_refined_dimensions(self):
        body = read_reference_models(BODY, PLAYER_CANVAS)['body']
        ground = 5
        # 前楔：两侧颊甲为主橄榄，中线为近黑炮盾；颊甲顶低于主体屋顶。
        self.assertEqual(body[(93, 10, 25)], 1)
        self.assertEqual(body[(93, 18, 25)], 3)
        self.assertLess(max(z for x, y, z in body if x == 93),
                        max(z for x, y, z in body if x == 77))
        # 分段裙板（工单 04）：最外带存在，下缘高于 0.94 米顶齿（72 资产格）。
        self.assertTrue(any(y == 0 for x, y, z in body), '缺少分段裙板最外带')
        self.assertFalse(any(y <= 1 and z < ground + 10 for x, y, z in body))
        # 裙板下缘 1.04 米（77 资产格）：外带体素全部不低于参考格 15。
        self.assertTrue(all(z >= ground + 10 for x, y, z in body if y == 0))

    def test_three_packages_and_dimensions(self):
        body = read_reference_models(BODY, PLAYER_CANVAS)['body']
        barrel = read_reference_models(BARREL, PLAYER_CANVAS)['barrel']
        track = read_reference_models(TRACK, PLAYER_CANVAS)
        self.assertEqual(sv.read_meta_unit(TRACK.with_name(TRACK.name + '.meta')), 0.02)
        self.assertEqual(len(track), 24)
        self.assertFalse(body.keys() & barrel.keys())
        for name in ('track_l_0', 'track_r_0'):
            cells = track[name]
            self.assertFalse(body.keys() & cells.keys())
            self.assertFalse(barrel.keys() & cells.keys())
        combined = body | barrel | track['track_l_0'] | track['track_r_0']
        bounds = lambda cells: tuple((min(p[i] for p in cells), max(p[i] for p in cells) + 1) for i in range(3))
        self.assertEqual(bounds(combined)[2], (5, 36))
        self.assertEqual(bounds(body | track['track_l_0'] | track['track_r_0']), ((33, 111), (0, 38), (5, 36)))
        self.assertEqual(bounds(barrel)[0], (95, 144))
        self.assertTrue(any((94, y, z) in body and (95, y, z) in barrel for y in range(38) for z in range(37)))


if __name__ == '__main__':
    unittest.main()
