"""锁定玩家三包的米制尺寸、接缝与统一坐标契约。

体素占用统一经 read_reference_models 还原到参考网格（0.1 米/格）：粗资产直接
取用，细分资产（body/barrel/track，0.02 米/格）按场景树分块平移还原后降采样，断言
与迁移前逐字一致，证明细分不改变米制契约。
"""
import unittest
import struct
import subdivide_vox as sv
from test_tracks import read_reference_models, ASSETS, PLAYER_CANVAS

TRACK = ASSETS / 'tracks/player/cleat/track.vox'
BODY = ASSETS / 'bodies/player/anvil/body.vox'
BARREL = ASSETS / 'barrels/player/lance/barrel.vox'


class PlayerTests(unittest.TestCase):
    def test_independent_wheels_have_centered_sizes_and_rotation_clearance(self):
        track = read_reference_models(TRACK, PLAYER_CANVAS)
        self.assertEqual(len(track), 20, '六个履带相位与十四个独立轮节点')
        for side in ('l', 'r'):
            for index in range(7):
                self.assertIn(f'wheel_{side}_{index}', track, f'缺少轮节点 {side}_{index}')
        for side in ('l', 'r'):
            for index in range(7):
                cells = track[f'wheel_{side}_{index}']
                road = index < 5
                size, envelope = (8, 4.3) if road else (5, 2.8)
                self.assertEqual((min(y for x, y, z in cells), max(y for x, y, z in cells)), (0, 3))
                self.assertEqual((min(x for x, y, z in cells), max(x for x, y, z in cells)), (0, size - 1))
                self.assertTrue({1, 4, 6} <= set(cells.values()))
                if road:
                    for x, z in [(6, 3), (3, 6), (2, 2)]:
                        self.assertEqual(cells[x, 0, z], 6, "三条不对称辐条均须可见")
                for x, y, z in cells:
                    # 检查整个格的四个角，不能只检查体素中心。
                    for a in (x, x + 1):
                        for b in (z, z + 1):
                            self.assertLessEqual((a - size / 2) ** 2 + (b - size / 2) ** 2, envelope ** 2)

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
                self.assertTrue(0 <= x < 135 and 0 <= y < 36 and 0 <= z < 26,
                                f'{path.name} 占用 {x},{y},{z} 超出共享画布')

    def test_refined_dimensions(self):
        body = read_reference_models(BODY, PLAYER_CANVAS)['body']
        barrel = read_reference_models(BARREL, PLAYER_CANVAS)['barrel']
        # 三个炮塔层面的最大宽度分别为 2.6、2.4、2.0 米。
        for z, expected in [(18, 26), (21, 24), (24, 20)]:
            ys = [y for (x, y, h) in body if h == z and 49 <= x < 90]
            self.assertEqual(max(ys) - min(ys) + 1, expected)
        for x, expected in [(100, 2), (113, 4), (92, 4), (133, 2)]:
            section = [(y, z) for (a, y, z) in barrel if a == x]
            self.assertEqual(max(y for y, z in section) - min(y for y, z in section) + 1, expected)
            self.assertEqual(max(z for y, z in section) - min(z for y, z in section) + 1, expected)
        self.assertFalse(any(z >= 16 and x < 49 for x, y, z in body))
        # 裙板面板之间必须露出实际空隙；支撑梁只放在内沿。
        for x in [42, 52, 62, 72, 82, 92]:
            self.assertNotIn((x, 1, 15), body)
        # 七块裙板仍有分缝，抬高后的底部为 1.4 米。
        self.assertTrue(any(y == 1 and z == 14 for x, y, z in body))
        self.assertFalse(any(y == 1 and z < 14 for x, y, z in body))

    def test_three_packages_and_dimensions(self):
        body = read_reference_models(BODY, PLAYER_CANVAS)['body']
        barrel = read_reference_models(BARREL, PLAYER_CANVAS)['barrel']
        track = read_reference_models(TRACK, PLAYER_CANVAS)
        self.assertEqual(sv.read_meta_unit(TRACK.with_name(TRACK.name + '.meta')), 0.02)
        self.assertEqual(len(track), 20)
        self.assertFalse(body.keys() & barrel.keys())
        for name in ('track_l_0', 'track_r_0'):
            cells = track[name]
            self.assertFalse(body.keys() & cells.keys())
            self.assertFalse(barrel.keys() & cells.keys())
        combined = body | barrel | track['track_l_0'] | track['track_r_0']
        bounds = lambda cells: tuple((min(p[i] for p in cells), max(p[i] for p in cells) + 1) for i in range(3))
        self.assertEqual(bounds(combined), ((30, 135), (0, 36), (0, 26)))
        self.assertEqual(bounds(body | track['track_l_0'] | track['track_r_0']), ((30, 105), (0, 36), (0, 26)))
        self.assertEqual(bounds(barrel)[0], (90, 135))
        self.assertTrue(any((89, y, z) in body and (90, y, z) in barrel for y in range(36) for z in range(26)))


if __name__ == '__main__':
    unittest.main()
