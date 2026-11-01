"""独立弹丸重生成、只读检查与回读校验回归。"""
import subprocess
import sys
import unittest
import hashlib
import tempfile
from unittest.mock import patch
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'scripts'))
import generate_bullet as pp


class PublishCheckTests(unittest.TestCase):
    def test_check_pipeline_matches_delivered_assets(self):
        paths = list((REPO / 'assets/tank').rglob('*.vox*'))
        before = {p: (hashlib.sha256(p.read_bytes()).digest(), p.stat().st_mtime_ns)
                  for p in paths}
        proc = subprocess.run(
            [sys.executable, '-X', 'utf8', 'scripts/generate_bullet.py', '--check'],
            cwd=REPO, capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(
            proc.returncode, 0,
            f'--check 失败：\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}')
        self.assertIn('一致', proc.stdout)
        self.assertEqual(before, {p: (hashlib.sha256(p.read_bytes()).digest(), p.stat().st_mtime_ns)
                                  for p in paths}, '检查模式不得写入交付 VOX 或 meta')

    def test_bullet_rejects_invalid_meta_and_displaced_scene(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            delivered = root / 'bullet.vox'
            delivered.write_bytes(b'original')
            meta = delivered.with_suffix('.vox.meta')
            meta.write_text('voxel_size: invalid', encoding='utf-8')
            with patch.object(pp, 'BULLET', delivered):
                with self.assertRaises(pp.PublishError):
                    pp.author_bullet(root / 'invalid')
                meta.write_text('voxel_size: 0.02', encoding='utf-8')
                write = pp.write_vox
                def displaced(models, nodes, raw, path):
                    nodes[2].frames = [{'_t': '9 0 0', '_r': '4'}]
                    write(models, nodes, raw, path)
                with patch.object(pp, 'write_vox', side_effect=displaced):
                    with self.assertRaises(pp.PublishError):
                        pp.author_bullet(root / 'displaced')
            self.assertEqual(delivered.read_bytes(), b'original')
            self.assertEqual(meta.read_text(encoding='utf-8'), 'voxel_size: 0.02')

    def test_compare_reports_geometry_and_meta_drift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            staging = root / 'staging'
            staging.mkdir()
            delivered = root / 'body.vox'
            delivered.write_bytes(b'old')
            delivered.with_suffix('.vox.meta').write_text('old', encoding='utf-8')
            (staging / 'body.vox').write_bytes(b'new')
            (staging / 'body.vox.meta').write_text('new', encoding='utf-8')
            self.assertEqual(len(pp.compare_pair(staging, delivered, delivered)), 2)


if __name__ == '__main__':
    unittest.main()
