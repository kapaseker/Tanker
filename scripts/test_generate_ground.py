"""校验纯平地图、原有花色与可重复生成；测试不写回正式资产。"""
import hashlib
import unittest
import generate_ground as ground


class GroundTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.heights, cls.colors = ground.generate_maps()

    def test_entire_map_is_flat(self):
        self.assertEqual(len(self.heights), 1200 * 1200)
        self.assertEqual(set(self.heights), {ground.FLAT})
        self.assertEqual(ground.HEIGHT_OUTPUT.read_bytes(), bytes(self.heights))

    def test_original_colors_are_preserved(self):
        albedo = (bytes([len(ground.PALETTE)])
                  + b''.join(bytes(rgb) for rgb in ground.PALETTE)
                  + bytes(self.colors))
        self.assertEqual(len(self.colors), len(self.heights))
        self.assertGreater(len(set(self.colors)), 1)
        self.assertEqual(albedo, ground.ALBEDO_OUTPUT.read_bytes())
        # 锁定本次铺平前的花色，防止删去起伏逻辑时误改随机序列或调色板。
        self.assertEqual(hashlib.sha256(albedo).hexdigest(),
                         'f931e3bfa1e8ee73f4c738fef41d983c06897fe2eb7fc7806885835ef790aef8')

    def test_generated_assets_are_reproducible(self):
        heights, colors = ground.generate_maps()
        self.assertEqual(heights, self.heights)
        self.assertEqual(colors, self.colors)


if __name__ == '__main__':
    unittest.main()
