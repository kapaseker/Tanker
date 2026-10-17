"""验证敌人 VOX 文件可复现、场景结构完整且尺寸与运行时碰撞体一致。"""
import struct
import unittest
import generate_enemy as enemy


class EnemyModelTests(unittest.TestCase):
    def test_vox_structure_bounds_palette_and_reproducibility(self):
        vox = enemy.build_vox()
        self.assertEqual(vox[:8], b'VOX ' + struct.pack('<I', 150))
        self.assertEqual(struct.unpack_from('<4sII', vox, 8), (b'MAIN', 0, len(vox)-20))
        chunks = {}
        offset = 20
        while offset < len(vox):
            tag, length, children = struct.unpack_from('<4sII', vox, offset)
            chunks.setdefault(tag, []).append(vox[offset+12:offset+12+length])
            self.assertEqual(children, 0)
            offset += 12 + length
        self.assertEqual(offset, len(vox))
        self.assertEqual(struct.unpack('<III', chunks[b'SIZE'][0]), (34, 26, 14))
        body = chunks[b'XYZI'][0]
        cells = list(struct.iter_unpack('4B', body[4:]))
        self.assertEqual(struct.unpack_from('<I', body)[0], len(cells))
        self.assertEqual(len({c[:3] for c in cells}), len(cells))
        for axis, size in enumerate((34, 26, 14)):
            self.assertEqual(min(c[axis] for c in cells), 0)
            self.assertEqual(max(c[axis] for c in cells), size-1)
        self.assertEqual(len(chunks[b'nTRN']), 2)
        self.assertIn(b'enemy_tank', chunks[b'nTRN'][1])
        self.assertIn(b'nGRP', chunks)
        self.assertIn(b'nSHP', chunks)
        palette = chunks[b'RGBA'][0]
        self.assertEqual(len(palette), 1024)
        red, green, blue, alpha = palette[:4]
        self.assertTrue(60 <= red <= 130 and red > green * 1.5 and red > blue * 1.5)
        self.assertEqual(alpha, 255)
        self.assertEqual(vox, enemy.build_vox())
        self.assertEqual(vox, enemy.OUTPUT.read_bytes())


if __name__ == '__main__':
    unittest.main()
