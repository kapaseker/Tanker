"""校验全域分布、分块还原及 VOX 场景，不依赖第三方包。"""
from collections import Counter
import struct
import unittest
import generate_ground as ground


class GroundTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.heights, cls.colors = ground.generate_maps()
        cls.vox = ground.build_vox()

    def test_distribution_and_no_crossing_relief(self):
        self.assertEqual(len(self.heights), 632 * 632)
        counts = Counter(self.heights)
        self.assertEqual(set(counts), {3, 4, 5})
        for level, low, high in [(4, .45, .55), (3, .20, .30), (5, .20, .30)]:
            self.assertTrue(low <= counts[level] / len(self.heights) <= high)
        # 按四邻接检查所有非平坦区域，不能连通场地的相对两边。
        visited = bytearray(len(self.heights))
        size = ground.SIZE
        for start, level in enumerate(self.heights):
            if level == 4 or visited[start]:
                continue
            stack, edges = [start], set()
            visited[start] = 1
            while stack:
                cell = stack.pop()
                x, y = cell % size, cell // size
                if x == 0: edges.add('left')
                if x == size - 1: edges.add('right')
                if y == 0: edges.add('top')
                if y == size - 1: edges.add('bottom')
                for nx, ny in ((x-1, y), (x+1, y), (x, y-1), (x, y+1)):
                    if 0 <= nx < size and 0 <= ny < size:
                        neighbor = ny * size + nx
                        if not visited[neighbor] and self.heights[neighbor] != 4:
                            visited[neighbor] = 1
                            stack.append(neighbor)
            self.assertFalse({'left', 'right'} <= edges or {'top', 'bottom'} <= edges)

    def test_vox_tiles_reconstruct_the_global_map(self):
        # 直接解析序列化结果，防止生成器切块时丢列、重复地块或写错场景平移。
        offset, model, reconstructed, signatures, translations = 20, -1, bytearray(632*632), [], []
        while offset < len(self.vox):
            tag, length, children = struct.unpack_from('<4sII', self.vox, offset)
            data = self.vox[offset+12:offset+12+length]
            if tag == b'SIZE':
                model += 1
                self.assertEqual(struct.unpack('<III', data), (158, 158, 6))
            elif tag == b'XYZI':
                signatures.append(data)
                tx, ty = model % 4, model // 4
                for x, y, z, color in struct.iter_unpack('4B', data[4:]):
                    index = (ty*158+y)*632 + tx*158+x
                    reconstructed[index] = max(reconstructed[index], z+1)
                    self.assertEqual(color, self.colors[index] if z >= 2 else 1)
            elif tag == b'nTRN' and b'_t' in data:
                translations.append(data)
            offset += 12 + length + children
        self.assertEqual(model + 1, 16)
        self.assertEqual(len(set(signatures)), 16)
        self.assertEqual(reconstructed, self.heights)
        self.assertEqual(len(translations), 16)
        for y in (-237, -79, 79, 237):
            for x in (-237, -79, 79, 237):
                value = f'{x} {y} 0'.encode()
                self.assertTrue(any(struct.pack('<i', len(value)) + value in t for t in translations))

    def test_generated_asset_is_reproducible(self):
        self.assertEqual(self.vox, ground.build_vox())
        self.assertEqual(ground.OUTPUT.read_bytes(), self.vox)


if __name__ == '__main__':
    unittest.main()
