"""校验地块高度、接缝和确定性，不依赖第三方包。"""
import unittest
import generate_ground as ground


class GroundTests(unittest.TestCase):
    def test_height_range_and_matching_edges(self):
        heights = [[ground.height(x, y) for y in range(158)] for x in range(158)]
        self.assertEqual({h for row in heights for h in row}, {2, 3, 4, 5, 6})
        self.assertTrue(all(heights[0][i] == heights[-1][i] for i in range(158)))
        self.assertTrue(all(heights[i][0] == heights[i][-1] for i in range(158)))

    def test_generated_asset_is_reproducible(self):
        self.assertEqual(ground.build_vox(), ground.build_vox())
        self.assertEqual(ground.OUTPUT.read_bytes(), ground.build_vox())


if __name__ == '__main__':
    unittest.main()
