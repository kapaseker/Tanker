"""独立 APFSDS 飞行体生成入口；--check 回读并逐字节比对，不写交付资产。"""
import argparse
import shutil
import sys
import tempfile
from pathlib import Path
from subdivide_vox import (Model, Node, write_vox, parse_vox, read_meta_unit,
                          rewrite_meta_unit, publish_pair, compare_pair, VOXEL_SIZE_RE, ConvertError)

REPO = Path(__file__).resolve().parents[1]
BULLET = REPO / 'assets/tank/ammunition/apfsds/bullet.vox'
FINE_UNIT = 0.02


class PublishError(Exception):
    """弹丸几何或元数据未通过发布前校验。"""


def author_bullet(staging: Path) -> None:
    """以 0.02 米格直接作者化飞行体；画布 +X 为尖端方向。"""
    staging.mkdir()
    voxels = set()
    for x in range(36):
        voxels.add((x, 2, 2, 1))
        # 0.04 米杆在前 0.06 米收窄到单格，形成可辨的体素尖端。
        if x < 33:
            for y, z in ((2, 3), (3, 2), (3, 3)):
                voxels.add((x, y, z, 1))
        if x < 6:
            for radius in range(1, 3 if x < 3 else 2):
                for y, z in ((2-radius, 2), (2+radius, 2), (2, 2-radius), (2, 2+radius)):
                    voxels.add((x, y, z, 1))
    data = bytes(c for voxel in sorted(voxels) for c in voxel)
    root, group, named, shape = [Node(kind, i, attrs) for i, (kind, attrs) in enumerate(
        [('trn', {}), ('grp', {}), ('trn', {'_name': 'bullet'}), ('shp', {})])]
    root.child, root.layer = 1, -1
    group.children = [2]
    named.child, named.layer = 3, -1
    # 相对扫掠中心前移 0.16 米，尾端恰在炮口端面。
    named.frames = [{'_t': '8 0 0', '_r': '4'}]
    shape.models = [(0, {})]
    palette = bytes((32, 35, 33, 255)) + bytes((0, 0, 0, 255)) * 255
    path = staging / BULLET.name
    write_vox([Model((36, 5, 5), data)],
              [root, group, named, shape], [('RGBA', palette)], path)
    check = parse_vox(path.read_bytes(), str(path))
    if (len(check.models) != 1 or check.models[0].size != (36, 5, 5)
            or check.models[0].xyzi != data or check.raw_chunks != [('RGBA', palette)]
            or len(check.nodes) != 4 or [n.kind for n in check.nodes] != ['trn', 'grp', 'trn', 'shp']
            or check.nodes[0].child != 1 or check.nodes[0].frames
            or check.nodes[1].children != [2] or check.nodes[2].node_dict != {'_name': 'bullet'}
            or check.nodes[2].child != 3 or check.nodes[2].frames != [{'_t': '8 0 0', '_r': '4'}]
            or check.nodes[3].models != [(0, {})]):
        raise PublishError('弹丸回读不一致')
    meta = BULLET.with_name(BULLET.name + '.meta').read_text(encoding='utf-8')
    if len(VOXEL_SIZE_RE.findall(meta)) != 1 or read_meta_unit(BULLET.with_name(BULLET.name + '.meta')) != FINE_UNIT:
        raise PublishError('弹丸元数据单位无效')
    meta_path = staging / (BULLET.name + '.meta')
    meta_path.write_text(rewrite_meta_unit(meta, FINE_UNIT), encoding='utf-8')
    if read_meta_unit(meta_path) != FINE_UNIT:
        raise PublishError('弹丸元数据回读不一致')


def main():
    """沿用原子配对发布；暂存在同卷 target 中，失败产物不混入运行资源。"""
    parser = argparse.ArgumentParser(description='APFSDS 弹丸生成与校验')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    (REPO / 'target').mkdir(exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix='apfsds-', dir=REPO / 'target'))
    try:
        staging = work / 'staged'
        author_bullet(staging)
        if args.check:
            problems = compare_pair(staging, BULLET, BULLET)
            if problems:
                raise PublishError('; '.join(problems))
            print('一致 APFSDS')
        else:
            publish_pair(staging / BULLET.name, staging / (BULLET.name + '.meta'), BULLET)
            print('已发布 APFSDS')
    except Exception:
        print(f'失败诊断保留于 {work}', file=sys.stderr)
        raise
    else:
        shutil.rmtree(work)


if __name__ == '__main__':
    try:
        main()
    except (PublishError, ConvertError, OSError, ValueError) as exc:
        print(f'弹丸生成失败：{exc}', file=sys.stderr)
        sys.exit(1)
