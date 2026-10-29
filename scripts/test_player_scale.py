"""交付资产边界验收；期望值来自已冻结的 A6 尺寸表，不从生成实现计算。"""
import struct
import unittest
import tempfile
from unittest.mock import patch
from pathlib import Path
import subdivide_vox as sv
import player_a6

ROOT = Path(__file__).resolve().parents[1] / 'assets/tank'


def bounds(path, name, full=(720, 190, 135), section_x=None):
    """以实际分块和节点变换还原资产格边界，排除空白画布。"""
    source = sv.parse_vox(path.read_bytes(), str(path))
    node = next(n for n in source.nodes if n.node_dict.get('_name') == name)
    low, high = [10000] * 3, [-10000] * 3
    for child in source.nodes[node.child].children:
        trn = source.nodes[child]
        model = source.models[source.nodes[trn.child].models[0][0]]
        origin = tuple(t + (f-s)//2 for t, f, s in zip(sv.frame_t(trn.frames), full, model.size))
        for *point, _ in struct.iter_unpack('<BBBB', model.xyzi):
            if section_x and not section_x[0] <= point[0] + origin[0] < section_x[1]:
                continue
            for i in range(3):
                low[i] = min(low[i], point[i] + origin[i])
                high[i] = max(high[i], point[i] + origin[i] + 1)
    return tuple(low), tuple(high)


class PlayerScaleTests(unittest.TestCase):
    def test_authoring_rejects_displaced_staging_chunk(self):
        delivered = ROOT / 'bodies/player/anvil/body.vox'
        before = delivered.read_bytes(), delivered.stat().st_mtime_ns
        write = sv.write_vox

        def displaced(models, nodes, raw, path):
            chunk = next(n for n in nodes if n.kind == 'trn' and n.frames)
            offset = list(sv.frame_t(chunk.frames))
            offset[0] += 1
            chunk.frames = [{'_t': ' '.join(map(str, offset))}]
            write(models, nodes, raw, path)

        with tempfile.TemporaryDirectory() as directory:
            staged = Path(directory) / 'body.vox'
            staged.write_bytes(before[0])
            with patch.object(sv, 'write_vox', side_effect=displaced):
                with self.assertRaises(sv.ConvertError):
                    player_a6.author_package(staged)
        self.assertEqual((delivered.read_bytes(), delivered.stat().st_mtime_ns), before)

    def test_delivered_a6_dimensions_and_muzzle(self):
        body = ROOT / 'bodies/player/anvil/body.vox'
        barrel = ROOT / 'barrels/player/lance/barrel.vox'
        track = ROOT / 'tracks/player/cleat/track.vox'
        self.assertEqual(bounds(body, 'body'), ((169, 6, 22), (552, 184, 135)))
        self.assertEqual(bounds(barrel, 'barrel'), ((475, 85, 94), (718, 105, 114)))
        # 屋顶端点排除两个舱盖；管身截面验证炮轴为地面以上 2.08 米。
        self.assertEqual(bounds(body, 'body', section_x=(350, 355))[1][2], 132)
        self.assertEqual(bounds(barrel, 'barrel', section_x=(650, 655)),
                         ((650, 90, 99), (655, 100, 109)))
        # 梯形闭环：下段贴地、顶段 z=42 加凸耳至 47；内侧面板已删除，宽度外沿即履带带体。
        self.assertEqual(bounds(track, 'track_l_0'), ((169, 1, 0), (552, 35, 47)))
        self.assertEqual(bounds(track, 'track_r_0'), ((169, 155, 0), (552, 189, 47)))
        for path in (body, barrel, track):
            self.assertEqual(sv.read_meta_unit(path.with_suffix('.vox.meta')), 0.02)


if __name__ == '__main__':
    unittest.main()
