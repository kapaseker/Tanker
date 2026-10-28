"""玩家资产统一发布入口：python -X utf8 scripts/publish_player.py [--check]。

流程：rustc 编译 scripts/generate_player.rs（缺失或源更新时重编）→ 生成粗
body/barrel/track 到临时目录 → 按交付 meta 复制字段并强制 0.1 单位生成粗样
meta → body/barrel/track 走同一转换器细分到 0.02 并原子发布到 assets。

- 默认模式：真实发布；每个部件转换与逐格验证通过后分别替换，失败保留该部件原资产。
- --check：全流程只到临时目录，把管线产物与交付资产逐字节比对（body/barrel/track
  细分对），不一致退出码非零；绝不写 assets/。这是"重新生成不会
  把粗资产覆盖到 0.02 配置下"的可回归证明。
- 工作目录放在 target/ 下：Windows 的 os.replace 要求源与目标同卷。
"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from subdivide_vox import ConvertError, convert, rewrite_meta_unit

REPO = Path(__file__).resolve().parents[1]
BODY = REPO / 'assets/tank/bodies/player/anvil/body.vox'
BARREL = REPO / 'assets/tank/barrels/player/lance/barrel.vox'
TRACK = REPO / 'assets/tank/tracks/player/cleat/track.vox'
GENERATOR_RS = REPO / 'scripts/generate_player.rs'
# 生成器经 #[path] 内嵌 model 模块，其下 player.rs/enemy.rs 契约更新同样需要重编；
# 监视整个 src/tank/model 目录，避免陈旧 exe 通过 --check 并发布旧几何。
GENERATOR_CONTRACT_DIR = REPO / 'src/tank/model'
GENERATOR_EXE = REPO / 'target' / ('generate_player.exe' if sys.platform == 'win32' else 'generate_player')
# 参考网格单位（米/格），与 Rust 侧 REFERENCE_VOXEL_SIZE 一致；粗样 meta 用。
COARSE_UNIT = 0.1
# 细分目标单位（米/格）。
FINE_UNIT = 0.02
# 玩家三包均按同一管线细分；子弹另由后续工单迁移。
SUBDIVIDED = {'body': BODY, 'barrel': BARREL, 'track': TRACK}


class PublishError(Exception):
    """发布前置条件不满足或产物与交付不一致；消息即最终诊断。"""


def ensure_generator() -> Path:
    """二进制缺失，或任一输入源比它新时用 rustc 重编；返回可执行文件路径。"""
    newest = max(p.stat().st_mtime for p in
                 [GENERATOR_RS, *GENERATOR_CONTRACT_DIR.rglob('*.rs')])
    if not GENERATOR_EXE.exists() or GENERATOR_EXE.stat().st_mtime < newest:
        GENERATOR_EXE.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(['rustc', '--edition', '2024', '-O',
                        str(GENERATOR_RS), '-o', str(GENERATOR_EXE)],
                       check=True, cwd=REPO)
    return GENERATOR_EXE


def generate_coarse(gen_root: Path) -> dict[str, Path]:
    """运行生成器把粗三包写到 gen_root/assets 下；返回 {部件: 粗 .vox 路径}。"""
    exe = ensure_generator()
    subprocess.run([str(exe), str(gen_root)], check=True, cwd=REPO)
    coarse = {name: gen_root / delivered.relative_to(REPO)
              for name, delivered in (('body', BODY), ('barrel', BARREL), ('track', TRACK))}
    for path in coarse.values():
        if not path.is_file():
            raise PublishError(f'生成器未产出 {path}')
    return coarse


def write_coarse_meta(coarse_vox: Path, gen_root: Path) -> None:
    """为临时粗样写配对 meta：字段复制交付 meta，单位强制回参考网格 0.1。

    交付 meta 是唯一字段真值（迁移后为 0.02 也没关系，只取结构与其余字段）；
    转换器随后把它改写为细分目标单位，发布产物与交付配置逐字段可比。
    """
    delivered = REPO / coarse_vox.relative_to(gen_root)
    text = delivered.with_name(delivered.name + '.meta').read_text(encoding='utf-8')
    coarse_vox.with_name(coarse_vox.name + '.meta').write_text(
        rewrite_meta_unit(text, COARSE_UNIT), encoding='utf-8')


def compare_pair(staging: Path, vox: Path, delivered: Path) -> list[str]:
    """比对 staging 细分产物与交付资产对；返回不一致项（空 = 一致）。"""
    problems = []
    if (staging / vox.name).read_bytes() != delivered.read_bytes():
        problems.append(f'{delivered.name} 体素字节与管线产物不一致')
    staged_meta = (staging / (vox.name + '.meta')).read_text(encoding='utf-8')
    if staged_meta != delivered.with_name(delivered.name + '.meta').read_text(encoding='utf-8'):
        problems.append(f'{delivered.name} 的 meta 与管线产物不一致')
    return problems


def run(check: bool) -> int:
    """执行完整管线；check 模式逐字节比对并返回 0/1，发布模式原子替换。"""
    # 工作目录在 target/ 下：与 assets 同卷，os.replace 才能原子移动。
    work = Path(tempfile.mkdtemp(prefix='publish-player-', dir=str(REPO / 'target')))
    failed: list[str] = []
    try:
        gen_root = work / 'coarse'
        coarse = generate_coarse(gen_root)
        for name, delivered in SUBDIVIDED.items():
            write_coarse_meta(coarse[name], gen_root)
            if check:
                staging = work / f'staged-{name}'
                convert(coarse[name], FINE_UNIT, staging, publish=False)
                problems = compare_pair(staging, coarse[name], delivered)
                print(f'{"不一致" if problems else "一致"} {delivered.relative_to(REPO)}')
                failed += problems
            else:
                stats = convert(coarse[name], FINE_UNIT, work / f'staging-{name}',
                                publish=True, dest_path=delivered)
                print(f'已发布 {delivered.relative_to(REPO)}：体素 {stats["source_voxels"]}'
                      f' → {stats["output_voxels"]}，字节 {stats["source_bytes"]}'
                      f' → {stats["output_bytes"]}')
    finally:
        shutil.rmtree(work, ignore_errors=True)
    if failed:
        for problem in failed:
            print(f'校验失败：{problem}', file=sys.stderr)
        return 1
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description='玩家资产统一发布入口（生成→细分→发布/比对）')
    parser.add_argument('--check', action='store_true',
                        help='全流程只到临时目录并与交付资产逐字节比对，不写 assets/')
    args = parser.parse_args(argv)
    try:
        return run(args.check)
    except (PublishError, ConvertError, OSError, subprocess.CalledProcessError) as exc:
        print(f'发布入口失败：{exc}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
