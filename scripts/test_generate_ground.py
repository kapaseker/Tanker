"""校验 600 米场地的分布、出生区、连通性与资产输出，不依赖第三方包。"""
import unittest
import generate_ground as ground


class GroundTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.heights, cls.colors = ground.generate_maps()
        cls.components = ground.component_stats(cls.heights)
        ground.write_assets()

    def test_map_dimensions_and_byte_ranges(self):
        self.assertEqual(len(self.heights), ground.SIZE * ground.SIZE)
        self.assertEqual(len(self.colors), len(self.heights))
        self.assertTrue(0 <= min(self.heights) and max(self.heights) <= 2*ground.LEVEL_MAX)
        self.assertTrue(set(self.colors) <= {0, 1, 2, 3, 4})
        self.assertGreater(len(set(self.colors)), 1)

    def test_terrain_is_grouped_and_covers_a_quarter_to_a_half(self):
        flat = self.heights.count(ground.FLAT)
        ratio = 1 - flat/len(self.heights)
        self.assertTrue(ground.COVER_RANGE[0] <= ratio <= ground.COVER_RANGE[1])
        self.assertGreaterEqual(flat/len(self.heights), .5)
        # 至少一个连续片区横跨 40 米以上，保证地形成片而不是碎点。
        widest = max(min(x1 - x0, y1 - y0) for _, (x0, x1, y0, y1), _ in self.components)
        self.assertGreaterEqual(widest, 40/ground.AUTHOR_RES)

    def test_spawn_pad_is_flat(self):
        radius = ground.SPAWN_PAD_M/ground.AUTHOR_RES
        half = ground.HALF
        for y in range(int(half - radius), int(half + radius) + 1):
            for x in range(int(half - radius), int(half + radius) + 1):
                if (x + .5 - half)**2 + (y + .5 - half)**2 <= radius**2:
                    self.assertEqual(self.heights[y*ground.SIZE + x], ground.FLAT)

    def test_relief_never_crosses_the_field(self):
        # 按四邻接检查：任何非平坦区域都不能同时触及场地相对的两边。
        for _, _, edges in self.components:
            self.assertFalse({'left', 'right'} <= edges or {'top', 'bottom'} <= edges)

    def test_generated_assets_are_reproducible(self):
        heights, colors = ground.generate_maps()
        self.assertEqual(heights, self.heights)
        self.assertEqual(colors, self.colors)
        self.assertEqual(ground.HEIGHT_OUTPUT.read_bytes(), ground.encode_height(self.heights))
        self.assertEqual(ground.ALBEDO_OUTPUT.read_bytes(), bytes(self.colors))


if __name__ == '__main__':
    unittest.main()
