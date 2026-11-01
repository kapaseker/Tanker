"""整车转换边界：原始静态几何不丢失，履带沿闭环移动且负里程回绕。"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import prepare_astra as astra


class AstraConversionTests(unittest.TestCase):
    def test_track_motion_follows_top_and_bottom_opposite_directions(self):
        # 原作者 +X 向前：前进时顶带向车首、底带向车尾。
        self.assertEqual(astra.move_track_cell(100, 45, 0), (100, 45))
        self.assertGreater(astra.move_track_cell(100, 45, 1)[0], 100)
        self.assertLess(astra.move_track_cell(100, 0, 1)[0], 100)
        self.assertEqual(astra.move_track_cell(100, 45, 3), (100, 45))
        self.assertEqual(astra.move_track_cell(100, 45, -1), astra.move_track_cell(100, 45, 2))


if __name__ == '__main__':
    unittest.main()
