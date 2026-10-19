"""验证敌人 VOX 文件可复现、场景结构完整且尺寸与运行时碰撞体一致。"""
import struct
import hashlib
import unittest
import generate_enemy as enemy


class EnemyModelTests(unittest.TestCase):
    def test_split_lugged_assets_keep_bounds_and_body_clearance(self):
        body, tracks = enemy.build_assets()

        def models(vox):
            result = []
            offset = 20
            while offset < len(vox):
                tag, length, children = struct.unpack_from('<4sII', vox, offset)
                data = vox[offset+12:offset+12+length]
                if tag == b'SIZE':
                    self.assertEqual(struct.unpack('<III', data), (34, 26, 14))
                if tag == b'XYZI':
                    cells = dict((c[:3], c[3]) for c in struct.iter_unpack('4B', data[4:]))
                    self.assertEqual(len(cells), struct.unpack_from('<I', data)[0])
                    result.append(cells)
                offset += 12 + length + children
            return result

        body_cells, = models(body)
        # 锁定拆分前的非履带体素，防止车身、炮塔、炮管或挡泥板随生成器漂移。
        self.assertEqual(hashlib.sha256(b''.join(bytes((*p, c)) for p, c in
                                                sorted(body_cells.items()))).hexdigest(),
                         'b7bf4bf9cb502e77ac856f5a9c38d369c7ce18cc0db67983df85b75aa1e3b8c9')
        variants = models(tracks)
        self.assertEqual(len(variants), 6)
        self.assertIn(b'body', body)
        self.assertNotIn(b'_hidden', tracks)
        for side in range(2):
            for phase in range(3):
                variant = variants[side*3+phase]
                self.assertIn(f'track_{"lr"[side]}_{phase}'.encode(), tracks)
                self.assertFalse(body_cells.keys() & variant.keys())
                self.assertTrue(all(z < 5 for x, y, z in variant))
                self.assertTrue(any(y == (0 if side == 0 else 25) for x, y, z in variant))
                self.assertNotEqual(variant, variants[side*3+(phase+1) % 3])
        # 挡泥板和车身保持原体素，所有相位组合都应保持整车尺寸。
        original = enemy.build_cells()
        self.assertEqual(body_cells, {p: c for p, c in original.items()
                                     if not (p[2] < 5 and (p[1] <= 4 or p[1] >= 21)
                                             and c in (4, 5))})
        for left in variants[:3]:
            for right in variants[3:]:
                combined = body_cells.keys() | left.keys() | right.keys()
                for axis, size in enumerate((34, 26, 14)):
                    self.assertEqual(min(p[axis] for p in combined), 0)
                    self.assertEqual(max(p[axis] for p in combined), size-1)
        self.assertEqual(body, enemy.BODY_OUTPUT.read_bytes())
        self.assertEqual(tracks, enemy.TRACKS_OUTPUT.read_bytes())
        self.assertEqual((body, tracks), enemy.build_assets())

    def test_vox_structure_bounds_palette_and_reproducibility(self):
        vox, _ = enemy.build_assets()
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
        for x, y, z, color in cells:
            self.assertTrue(0 <= x < 34 and 0 <= y < 26 and 0 <= z < 14)
        self.assertEqual(len(chunks[b'nTRN']), 2)
        self.assertIn(b'body', chunks[b'nTRN'][1])
        self.assertIn(b'nGRP', chunks)
        self.assertIn(b'nSHP', chunks)
        palette = chunks[b'RGBA'][0]
        self.assertEqual(len(palette), 1024)
        red, green, blue, alpha = palette[:4]
        self.assertTrue(60 <= red <= 130 and red > green * 1.5 and red > blue * 1.5)
        self.assertEqual(alpha, 255)
        self.assertEqual(vox, enemy.build_assets()[0])
        self.assertEqual(vox, enemy.BODY_OUTPUT.read_bytes())


if __name__ == '__main__':
    unittest.main()
