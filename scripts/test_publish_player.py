"""publish_player 统一入口回归测试：python -X utf8 -m unittest scripts.test_publish_player。

跑 --check 完整粗源兼容、细分和细网格作者化管线，逐字节比对四个交付配对，
并检查内容与写入时间不变；漂移诊断和配对回滚另有小范围回归。
前置：PATH 中有 rustc 与 python；首次运行会编译 target/generate_player(.exe)。
"""
import subprocess
import sys
import unittest
import hashlib
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / 'scripts'))
import publish_player as pp


class PublishCheckTests(unittest.TestCase):
    def test_check_pipeline_matches_delivered_assets(self):
        paths = list((REPO / 'assets/tank').rglob('*.vox*'))
        before = {p: (hashlib.sha256(p.read_bytes()).digest(), p.stat().st_mtime_ns)
                  for p in paths}
        proc = subprocess.run(
            [sys.executable, '-X', 'utf8', 'scripts/publish_player.py', '--check'],
            cwd=REPO, capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(
            proc.returncode, 0,
            f'--check 失败：\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}')
        self.assertIn('一致', proc.stdout)
        self.assertEqual(before, {p: (hashlib.sha256(p.read_bytes()).digest(), p.stat().st_mtime_ns)
                                  for p in paths}, '检查模式不得写入交付 VOX 或 meta')

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
