"""整车转换边界：原始静态几何不丢失，履带沿闭环移动且负里程回绕。"""
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

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

    def test_failed_meta_publish_preserves_existing_pair_and_cleans_first_install(self):
        for existing in [False, True]:
            with self.subTest(existing=existing), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                staging = root/'staging'
                staging.mkdir()
                staged = staging/'tank.vox'
                staged_meta = staging/'tank.vox.meta'
                staged.write_bytes(b'new vox')
                staged_meta.write_bytes(b'new meta')
                target = root/'tank.vox'
                meta = root/'tank.vox.meta'
                if existing:
                    target.write_bytes(b'old vox')
                    meta.write_bytes(b'old meta')
                replace = os.replace
                def fail_meta(source, destination):
                    if Path(source) == staged_meta:
                        raise OSError('injected meta failure')
                    return replace(source, destination)
                with mock.patch('os.replace', side_effect=fail_meta):
                    with self.assertRaisesRegex(OSError, 'injected meta failure'):
                        astra.publish_astra_pair(staged, target)
                if existing:
                    self.assertEqual(target.read_bytes(), b'old vox')
                    self.assertEqual(meta.read_bytes(), b'old meta')
                else:
                    self.assertFalse(target.exists())
                    self.assertFalse(meta.exists())
                staged.write_bytes(b'new vox')
                astra.publish_astra_pair(staged, target)
                self.assertEqual(target.read_bytes(), b'new vox')
                self.assertEqual(meta.read_bytes(), b'new meta')


if __name__ == '__main__':
    unittest.main()
