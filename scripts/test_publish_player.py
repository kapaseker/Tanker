"""publish_player 统一入口回归测试：python -X utf8 -m unittest scripts.test_publish_player。

跑 `--check` 全流程（rustc 生成粗源 → 转换器细分 → 逐字节比对交付资产），
断言交付的 body/barrel 细分对与 track 粗包和管线产物完全一致。这是"重新生成
不会把粗资产覆盖到 0.02 配置下"的可执行证明：入口失效或资产漂移都会在此失败。
前置：PATH 中有 rustc 与 python；首次运行会编译 target/generate_player(.exe)。
"""
import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


class PublishCheckTests(unittest.TestCase):
    def test_check_pipeline_matches_delivered_assets(self):
        proc = subprocess.run(
            [sys.executable, '-X', 'utf8', 'scripts/publish_player.py', '--check'],
            cwd=REPO, capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(
            proc.returncode, 0,
            f'--check 失败：\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}')
        self.assertIn('一致', proc.stdout)


if __name__ == '__main__':
    unittest.main()
