"""验证爆炸 VOX 可复现、含 12 帧模型且帧几何随时间外扩。"""
import struct
import unittest
import generate_explosion as explosion


class ExplosionModelTests(unittest.TestCase):
    def test_vox_structure_frames_and_reproducibility(self):
        vox = explosion.build_vox()
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
        # 12 帧 = 12 组 SIZE/XYZI，各帧体素均在立方体内且非空。
        self.assertEqual(len(chunks[b'XYZI']), explosion.FRAMES)
        counts = []
        for body in chunks[b'XYZI']:
            count = struct.unpack_from('<I', body)[0]
            cells = list(struct.iter_unpack('4B', body[4:]))
            self.assertEqual(count, len(cells))
            self.assertTrue(cells)
            for cell in cells:
                self.assertTrue(all(0 <= axis < explosion.SIZE for axis in cell[:3]))
            counts.append(count)
        # 前 8 帧未稀疏化，外扩球壳表面积应严格递增；尾段稀疏化后允许回落。
        for earlier, later in zip(counts[:7], counts[1:8]):
            self.assertLess(earlier, later)
        # 恰好 1 个 nSHP，模型数等于帧数。
        self.assertEqual(len(chunks[b'nSHP']), 1)
        shape = chunks[b'nSHP'][0]
        # nSHP 内容 = 节点 id(4B) + 空字典(4B) + 模型数。
        model_count = struct.unpack_from('<i', shape, 8)[0]
        self.assertEqual(model_count, explosion.FRAMES)
        self.assertIn(b'nTRN', chunks)
        self.assertIn(b'nGRP', chunks)
        palette = chunks[b'RGBA'][0]
        self.assertEqual(len(palette), 1024)
        self.assertEqual(len(chunks[b'MATL']), explosion.FRAMES)
        self.assertEqual(vox, explosion.build_vox())
        self.assertEqual(vox, explosion.OUTPUT.read_bytes())


if __name__ == '__main__':
    unittest.main()
