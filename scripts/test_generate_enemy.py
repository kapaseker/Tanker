"""锁定敌人三包、4+2轮组及玩家同高碰撞契约。"""
import math
import struct
import unittest
import generate_enemy as enemy
from test_tracks import read_models


class EnemyModelTests(unittest.TestCase):
    def test_packages_are_reproducible_and_share_palette(self):
        packages = enemy.build_assets()
        self.assertEqual(packages, enemy.build_assets())
        palettes = []
        for path, data in zip(enemy.OUTPUTS, packages):
            self.assertEqual(data, path.read_bytes())
            self.assertNotIn(b'_hidden', data)
            offset = 20
            while offset < len(data):
                tag, length, children = struct.unpack_from('<4sII', data, offset)
                if tag == b'RGBA':
                    palettes.append(data[offset+12:offset+12+length])
                offset += 12+length+children
            self.assertEqual(offset, len(data))
        self.assertEqual(len(palettes), 3)
        self.assertEqual(palettes, [palettes[0]]*3)
        self.assertEqual(palettes[0][:20], bytes((100,42,38,255,121,55,46,255,75,31,29,255,36,39,37,255,66,60,53,255)))

    def test_bounds_seams_and_rotating_wheel_clearance(self):
        body, = read_models(enemy.BODY_OUTPUT)
        barrel, = read_models(enemy.BARREL_OUTPUT)
        tracks = read_models(enemy.TRACKS_OUTPUT)
        self.assertEqual(len(tracks), 18)
        bounds = lambda cells: tuple((min(p[i] for p in cells),max(p[i] for p in cells)+1) for i in range(3))
        self.assertFalse(body.keys() & barrel.keys())
        self.assertEqual(bounds(barrel)[0], enemy.BARREL_X)
        self.assertTrue(any((enemy.BARREL_X[0]-1,y,z) in body and (enemy.BARREL_X[0],y,z) in barrel for y in range(34) for z in range(26)))
        for left in tracks[:3]:
            for right in tracks[3:6]:
                self.assertFalse(body.keys() & left.keys())
                self.assertFalse(body.keys() & right.keys())
                self.assertEqual(bounds(body | left | right), ((24,90),(0,34),(0,26)))
                self.assertEqual(bounds(body | barrel | left | right), ((24,114),(0,34),(0,26)))
        for side in range(2):
            wheels = enemy.WHEELS[side*6:side*6+6]
            for i,(_,a,_,_,ar) in enumerate(wheels):
                for _,b,_,_,br in wheels[:i]:
                    self.assertGreater(math.hypot(a[0]-b[0],a[2]-b[2]), ar+br)
        for spec, cells in zip(enemy.WHEELS, tracks[6:]):
            name, center, size, radius, envelope = spec
            self.assertIn(name.encode(), enemy.TRACKS_OUTPUT.read_bytes())
            for axis, extent in enumerate((envelope, 2, envelope)):
                self.assertGreaterEqual(center[axis]-extent, (24,0,0)[axis])
                self.assertLessEqual(center[axis]+extent, (90,34,26)[axis])
            self.assertEqual(bounds(cells), ((0,size),(0,4),(0,size)))
            self.assertTrue({1,4,5} <= set(cells.values()))
            # 检查体素四角的完整旋转包络，不能只检查格中心。
            for x,y,z in cells:
                for a in (x,x+1):
                    for b in (z,z+1):
                        self.assertLessEqual(math.hypot(a-size/2,b-size/2), envelope)
            self.assertGreater(center[2]-envelope, 1)
            self.assertLess(center[2]+envelope, 12)
            for phase in tracks[:6]:
                for x,y,z in body.keys() | phase.keys():
                    if abs(y+0.5-center[1]) < 2:
                        dx = max(x-center[0], center[0]-(x+1),0)
                        dz = max(z-center[2], center[2]-(z+1),0)
                        self.assertGreater(math.hypot(dx,dz), envelope, name)


if __name__ == '__main__':
    unittest.main()
