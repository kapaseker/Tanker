"""锁定玩家三包的米制尺寸、接缝与统一坐标契约。

参考格断言尺寸、占用与装配；资产格独立验证真实凹槽及配色，允许同一参考格内有细节。
"""
import math
import unittest
import struct
import subdivide_vox as sv
from test_tracks import read_reference_models, ASSETS, PLAYER_CANVAS

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
                           zip(sv.frame_t(node.frames), (720, 190, 135), model.size))
            for x, y, z, c in struct.iter_unpack('<BBBB', model.xyzi):
                if 180 <= x + origin[0] < 235 and 77 <= z + origin[2] < 80:
                    cells[tuple(a + b for a, b in zip((x, y, z), origin))] = c
        # 同一个旧参考格中同时存在 0.02 米凹槽与面板，重复槽仍在共同坐标位置。
        for x in (180, 229):
            self.assertNotIn((x, 90, 79), cells)
            self.assertEqual(cells[x, 90, 78], 4)
            self.assertEqual(cells[x + 2, 90, 79], 2)

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
        # 车身与炮管共用 135×36×26 参考画布；细分资产还原后仍不得越界。
        for path in (BODY, BARREL):
            models = read_reference_models(path, PLAYER_CANVAS)
            self.assertEqual(list(models), [path.stem], '全局部件包应只有一个命名节点')
            for x, y, z in models[path.stem]:
                self.assertTrue(0 <= x < 144 and 0 <= y < 38 and 0 <= z < 27,
                                f'{path.name} 占用 {x},{y},{z} 超出共享画布')

    def test_refined_dimensions(self):
        body = read_reference_models(BODY, PLAYER_CANVAS)['body']
        # 首上斜面不是直立车首；炮塔前楔向炮盾收窄。
        self.assertIn((108, 15, 11), body)
        self.assertNotIn((108, 15, 15), body)
        middle = [y for x, y, z in body if x == 77 and z == 18]
        nose = [y for x, y, z in body if x == 93 and z == 18]
        self.assertGreater(max(middle)-min(middle), max(nose)-min(nose))
        # 临时裙板始终高于 0.94 米顶齿，轮间透空。
        self.assertFalse(any(y == 1 and z < 16 for x, y, z in body))

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
        self.assertEqual(bounds(combined), ((33, 144), (0, 38), (0, 27)))
        self.assertEqual(bounds(body | track['track_l_0'] | track['track_r_0']), ((33, 111), (0, 38), (0, 27)))
        self.assertEqual(bounds(barrel)[0], (95, 144))
        self.assertTrue(any((94, y, z) in body and (95, y, z) in barrel for y in range(38) for z in range(27)))


if __name__ == '__main__':
    unittest.main()
