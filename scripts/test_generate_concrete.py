"""校验混凝土墩的梯形尺寸、VOX 场景图与确定性。"""
import struct
import unittest
import generate_concrete as concrete


class ConcreteTests(unittest.TestCase):
    def test_trapezoid_vox_matches_asset(self):
        vox = concrete.build_vox()
        self.assertEqual(vox[:8], b'VOX ' + struct.pack('<I', 150))
        self.assertEqual(struct.unpack_from('<4sII', vox, 8), (b'MAIN', 0, len(vox) - 20))
        chunks = {}
        offset = 20
        while offset < len(vox):
            tag, size, children = struct.unpack_from('<4sII', vox, offset)
            self.assertEqual(children, 0)
            chunks.setdefault(tag, []).append(vox[offset + 12:offset + 12 + size])
            offset += 12 + size
        self.assertEqual(offset, len(vox))
        self.assertEqual(struct.unpack('<III', chunks[b'SIZE'][0]), (15, 80, 80))
        count = struct.unpack_from('<I', chunks[b'XYZI'][0])[0]
        cells = list(struct.iter_unpack('4B', chunks[b'XYZI'][0][4:]))
        self.assertEqual(count, len(cells))
        self.assertEqual(count, len({v[:3] for v in cells}))
        for z in range(80):
            layer = [v for v in cells if v[2] == z]
            low, high = min(v[0] for v in layer), max(v[0] for v in layer) + 1
            expected_low = int(3 * z / 79 + .5)
            self.assertEqual((low, high), (expected_low, 15 - expected_low))
            self.assertEqual(len(layer), (high - low) * 80)
            # 体素台阶外轮廓与连续斜面偏差不能超过一格。
            for surface_z in (z, z + 1):
                self.assertLessEqual(abs(low - 3 * surface_z / 80), 1)
        self.assertTrue(all(0 <= y < 80 and 1 <= c <= 4 for _, y, _, c in cells))
        self.assertEqual(len(chunks[b'RGBA'][0]), 1024)
        self.assertEqual(len(chunks[b'MATL']), 4)
        self.assertEqual(len(chunks[b'nSHP']), 1)
        self.assertEqual(struct.unpack_from('<i', chunks[b'nSHP'][0], 8)[0], 1)
        self.assertIn(b'nTRN', chunks)
        self.assertIn(b'nGRP', chunks)
        self.assertEqual(vox, concrete.build_vox())
        self.assertEqual(vox, concrete.OUTPUT.read_bytes())


if __name__ == '__main__':
    unittest.main()
