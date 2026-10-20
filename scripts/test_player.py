"""锁定玩家三包的米制尺寸、接缝与统一坐标契约。"""
import unittest
import struct
from test_tracks import read_models, ASSETS

class PlayerTests(unittest.TestCase):
    def test_independent_wheels_have_centered_sizes_and_rotation_clearance(self):
        data = (ASSETS / 'tracks/player/lugged/default.vox').read_bytes()
        models = read_models(ASSETS / 'tracks/player/lugged/default.vox')
        self.assertEqual(len(models), 20, '六个履带相位与十四个独立轮节点')
        for side in ('l', 'r'):
            for index in range(7):
                self.assertIn(f'wheel_{side}_{index}'.encode(), data)
        for index, cells in enumerate(models[6:]):
            road = index % 7 < 5
            size, envelope = (8, 4.3) if road else (5, 2.8)
            self.assertEqual((min(y for x,y,z in cells), max(y for x,y,z in cells)), (0,3))
            self.assertEqual((min(x for x,y,z in cells),max(x for x,y,z in cells)), (0,size-1))
            self.assertTrue({1,4,6} <= set(cells.values()))
            if road:
                for x,z in [(6,3), (3,6), (2,2)]:
                    self.assertEqual(cells[x,0,z], 6, "三条不对称辐条均须可见")
            for x,y,z in cells:
                # 检查整个格的四个角，不能只检查体素中心。
                for a in (x, x+1):
                    for b in (z, z+1):
                        self.assertLessEqual((a-size/2)**2 + (b-size/2)**2, envelope**2)

    def test_shared_canvas_and_palette(self):
        palettes = []
        for path in ('player/default/body.vox', 'player/default/barrel.vox', 'tracks/player/lugged/default.vox'):
            data = (ASSETS / path).read_bytes()
            offset = 20
            sizes = []
            palette = None
            while offset < len(data):
                tag, size, children = struct.unpack_from('<4sII', data, offset)
                content = data[offset+12:offset+12+size]
                if tag == b'SIZE':
                    sizes.append(struct.unpack('<III', content))
                if tag == b'RGBA':
                    palette = content
                offset += 12+size+children
            self.assertTrue(sizes)
            self.assertTrue(all(size == (135,36,26) for size in sizes[:6]))
            if len(sizes) > 6:
                self.assertEqual(sizes[6:], [(8,4,8)]*5 + [(5,4,5)]*2 + [(8,4,8)]*5 + [(5,4,5)]*2)
            self.assertIsNotNone(palette)
            palettes.append(palette)
        self.assertEqual(palettes[0], palettes[1])
        self.assertEqual(palettes[0], palettes[2])

    def test_refined_dimensions(self):
        body, = read_models(ASSETS / 'player/default/body.vox')
        barrel, = read_models(ASSETS / 'player/default/barrel.vox')
        # 三个炮塔层面的最大宽度分别为 2.6、2.4、2.0 米。
        for z, expected in [(18,26),(21,24),(24,20)]:
            ys = [y for (x,y,h) in body if h == z and 49 <= x < 90]
            self.assertEqual(max(ys)-min(ys)+1, expected)
        for x, expected in [(100,2),(113,4),(92,4),(133,2)]:
            section = [(y,z) for (a,y,z) in barrel if a == x]
            self.assertEqual(max(y for y,z in section)-min(y for y,z in section)+1,expected)
            self.assertEqual(max(z for y,z in section)-min(z for y,z in section)+1,expected)
        self.assertFalse(any(z >= 16 and x < 49 for x,y,z in body))
        # 裙板面板之间必须露出实际空隙；支撑梁只放在内沿。
        for x in [42,52,62,72,82,92]:
            self.assertNotIn((x,1,15), body)
        # 七块裙板仍有分缝，抬高后的底部为 1.4 米。
        self.assertTrue(any(y == 1 and z == 14 for x,y,z in body))
        self.assertFalse(any(y == 1 and z < 14 for x,y,z in body))

    def test_three_packages_and_dimensions(self):
        body, = read_models(ASSETS / 'player/default/body.vox')
        barrel, = read_models(ASSETS / 'player/default/barrel.vox')
        tracks = read_models(ASSETS / 'tracks/player/lugged/default.vox')
        self.assertEqual(len(tracks), 20)
        self.assertFalse(body.keys() & barrel.keys())
        for cells in tracks[:6]:
            self.assertFalse(body.keys() & cells.keys())
            self.assertFalse(barrel.keys() & cells.keys())
        combined = body | barrel | tracks[0] | tracks[3]
        bounds = lambda cells: tuple((min(p[i] for p in cells), max(p[i] for p in cells)+1) for i in range(3))
        self.assertEqual(bounds(combined), ((30, 135), (0, 36), (0, 26)))
        self.assertEqual(bounds(body | tracks[0] | tracks[3]), ((30, 105), (0, 36), (0, 26)))
        self.assertEqual(bounds(barrel)[0], (90, 135))
        self.assertTrue(any((89,y,z) in body and (90,y,z) in barrel for y in range(36) for z in range(26)))

if __name__ == '__main__':
    unittest.main()
