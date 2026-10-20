"""锁定玩家三包的米制尺寸、接缝与统一坐标契约。"""
import unittest
import struct
from test_tracks import read_models, ASSETS

class PlayerTests(unittest.TestCase):
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
            self.assertTrue(all(size == (135,36,26) for size in sizes))
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
            self.assertNotIn((x,1,12), body)
        tracks = read_models(ASSETS / 'tracks/player/lugged/default.vox')
        for cells in (tracks[0], tracks[3]):
            face = 1 if (40,1,4) in cells else 34
            for x in [40,49,58,67,76,85,94]:
                self.assertEqual(cells[(x,face,4)],3)
            for x in [33,101]:
                self.assertEqual(cells[(x,face,5)],3)

    def test_three_packages_and_dimensions(self):
        body, = read_models(ASSETS / 'player/default/body.vox')
        barrel, = read_models(ASSETS / 'player/default/barrel.vox')
        tracks = read_models(ASSETS / 'tracks/player/lugged/default.vox')
        self.assertEqual(len(tracks), 6)
        self.assertFalse(body.keys() & barrel.keys())
        for cells in tracks:
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
