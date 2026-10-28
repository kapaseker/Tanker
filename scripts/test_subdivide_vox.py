"""subdivide_vox 转换器单测：python -X utf8 -m unittest scripts.test_subdivide_vox。

小样覆盖工单 03 的关键前提：块边界规划（奇偶画布）、跨块体素不重不漏、
空白画布、命名节点平移缩放与旋转保留、根帧原样、单位校验拒绝重复细分、
meta 改写、格式严格性（未知块/多帧/多模型/坏版本拒绝）与验证器自检
（篡改一个体素颜色必须失败）。
"""
import os
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import subdivide_vox as sv  # noqa: E402
# chunk/dictionary 直接复用被测模块所用的同一实现（generate_ground），保证
# fixture 字节格式与真实产出一致，同时避免维护第三份拷贝。
from generate_ground import chunk, dictionary  # noqa: E402


def build_vox(models, nodes, raw=()):
    """独立构造 fixture：models=[(size,[(x,y,z,c)...])]，nodes 用元组描述。

    节点元组：('trn', dict, child, layer, [frames]) / ('grp', dict, [children])
    / ('shp', dict, [(model_id, attrs)])；id 即出现顺序。
    """
    children = b''
    for size, voxels in models:
        children += chunk('SIZE', struct.pack('<III', *size))
        body = struct.pack('<I', len(voxels)) + b''.join(bytes(v) for v in voxels)
        children += chunk('XYZI', body)
    for tag, content in raw:
        children += chunk(tag, content)
    for spec in nodes:
        kind, node_dict, rest = spec[0], spec[1], spec[2:]
        if kind == 'trn':
            child, layer, frames = rest
            content = (struct.pack('<i', nodes.index(spec)) + dictionary(node_dict)
                       + struct.pack('<iiii', child, -1, layer, len(frames))
                       + b''.join(dictionary(f) for f in frames))
            children += chunk('nTRN', content)
        elif kind == 'grp':
            kids, = rest
            content = (struct.pack('<i', nodes.index(spec)) + dictionary(node_dict)
                       + struct.pack('<i', len(kids))
                       + b''.join(struct.pack('<i', k) for k in kids))
            children += chunk('nGRP', content)
        else:
            refs, = rest
            content = (struct.pack('<i', nodes.index(spec)) + dictionary(node_dict)
                       + struct.pack('<i', len(refs))
                       + b''.join(struct.pack('<i', m) + dictionary(a) for m, a in refs))
            children += chunk('nSHP', content)
    return b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children)


def root_scene(name, model_index, frames=None):
    """七文件同构的根：nTRN(根) → nGRP → nTRN(name) → nSHP(model)。"""
    return [
        ('trn', {}, 1, -1, frames if frames is not None else [{}]),
        ('grp', {}, [2]),
        ('trn', {'_name': name}, 3, -1, [{}]),
        ('shp', {}, [(model_index, {})]),
    ]


META = """(
    meta_format_version: "1.0",
    asset: Load(
        loader: "bevy_vox_scene::load::VoxSceneLoader",
        settings: (
            voxel_size: 0.1,
        ),
    ),
)
"""


class ChunkPlanTests(unittest.TestCase):
    """块规划：奇偶画布的精确切分与偏移。"""

    def test_odd_canvas_675(self):
        plan = sv.chunk_axis_plan(675)
        self.assertEqual(plan, ((0, 255), (255, 255), (510, 165)))

    def test_even_canvas_570(self):
        plan = sv.chunk_axis_plan(570)
        self.assertEqual(plan, ((0, 256), (256, 256), (512, 58)))

    def test_small_axis_single_chunk(self):
        self.assertEqual(sv.chunk_axis_plan(180), ((0, 180),))
        self.assertEqual(sv.chunk_axis_plan(1), ((0, 1),))
        self.assertEqual(sv.chunk_axis_plan(256), ((0, 256),))

    def test_plans_cover_axis(self):
        for full in (1, 2, 255, 256, 257, 510, 511, 512, 675, 570):
            plan = sv.chunk_axis_plan(full)
            self.assertEqual(sum(length for _o, length in plan), full)
            self.assertTrue(all(length <= 256 for _o, length in plan))
            for (start, length), (nstart, _n) in zip(plan, plan[1:]):
                self.assertEqual(start + length, nstart)


class ConvertTests(unittest.TestCase):
    """端到端小样转换：细分几何、帧语义、单位与 meta。"""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def write_source(self, data: bytes, unit='0.1', name='sample.vox'):
        src = self.root / name
        src.write_bytes(data)
        (self.root / (name + '.meta')).write_text(META.replace('0.1', unit), encoding='utf-8')
        return src

    def convert(self, src: Path, staging: Path, target=0.02, publish=False):
        return sv.convert(src, target, staging, publish)

    def test_odd_canvas_chunked_geometry(self):
        """135×36×26 稀疏小样：3 块、块尺寸/平移精确、跨块体素不重不漏。"""
        # 体素分布在三块交界两侧：x=50（块0）、x=51（块1）、x=104（块2）。
        voxels = [(50, 3, 4, 1), (51, 3, 4, 2), (104, 9, 0, 3), (0, 0, 0, 1)]
        data = build_vox([((135, 36, 26), voxels)], root_scene('body', 0))
        src = self.write_source(data)
        staging = self.root / 'staging'

        stats = self.convert(src, staging)
        self.assertEqual(stats['source_voxels'], 4)
        self.assertEqual(stats['output_voxels'], 500)
        self.assertEqual(stats['models'], (1, 3))

        out = sv.parse_vox((staging / 'sample.vox').read_bytes(), 'staged')
        chunk_sizes = sorted((m.size, ) for m in out.models)
        self.assertEqual([s for (s, ) in chunk_sizes],
                         sorted([(165, 180, 130), (255, 180, 130), (255, 180, 130)]))
        # 命名节点 → 分块组，三个匿名 nTRN 平移为 起点 + (C−F)/2。
        named = out.nodes[2]
        self.assertEqual(named.node_dict, {'_name': 'body'})
        group = out.nodes[named.child]
        offsets = sorted(sv.frame_t(out.nodes[c].frames) for c in group.children)
        self.assertEqual(offsets, sorted([(-210, 0, 0), (45, 0, 0), (255, 0, 0)]))

    def test_even_canvas_chunked_geometry(self):
        """114×34×26 小样：偶数画布取 K=256，三块全非空，平移含负值与奇数中点。"""
        # x=0 落 chunk0，x=56/57 落 chunk1（子格 280..289），x=113 落 chunk2。
        voxels = [(0, 0, 0, 1), (56, 4, 4, 2), (57, 4, 4, 2), (113, 0, 0, 4)]
        data = build_vox([((114, 34, 26), voxels)], root_scene('body', 0))
        src = self.write_source(data)
        staging = self.root / 'staging'

        stats = self.convert(src, staging)
        self.assertEqual(stats['output_voxels'], 500)
        self.assertEqual(stats['models'], (1, 3))

        out = sv.parse_vox((staging / 'sample.vox').read_bytes(), 'staged')
        self.assertEqual(sorted(m.size for m in out.models),
                         sorted([(58, 170, 130), (256, 170, 130), (256, 170, 130)]))
        group = out.nodes[out.nodes[2].child]
        offsets = sorted(sv.frame_t(out.nodes[c].frames) for c in group.children)
        self.assertEqual(offsets, sorted([(-157, 0, 0), (99, 0, 0), (256, 0, 0)]))

    def test_empty_interior_chunk_skipped(self):
        """内部空块不产生模型：114 画布只在首尾放体素 → 仅 2 块。"""
        voxels = [(0, 0, 0, 1), (113, 0, 0, 2)]
        data = build_vox([((114, 34, 26), voxels)], root_scene('body', 0))
        src = self.write_source(data)
        staging = self.root / 'staging'

        stats = self.convert(src, staging)
        self.assertEqual(stats['models'], (1, 2))
        out = sv.parse_vox((staging / 'sample.vox').read_bytes(), 'staged')
        group = out.nodes[out.nodes[2].child]
        self.assertEqual(len(group.children), 2)

    def test_negative_translation_and_rotation_preserved(self):
        """命名节点帧：_t ×5（负值），_r 原样；根帧原样保留。"""
        voxels = [(3, 1, 2, 1)]
        nodes = [
            ('trn', {}, 1, -1, [{'_t': '-2 -3 -1', '_r': '4'}]),  # 根帧原样
            ('grp', {}, [2]),
            ('trn', {'_name': 'bullet'}, 3, 0, [{'_t': '-2 -3 -1', '_r': '4'}]),
            ('shp', {}, [(0, {})]),
        ]
        raw = [('LAYR', struct.pack('<i', 0) + dictionary({'_name': 'bullet'}))]
        data = build_vox([((12, 4, 4), voxels)], nodes, raw)
        src = self.write_source(data)
        staging = self.root / 'staging'

        stats = self.convert(src, staging)
        self.assertEqual(stats['output_voxels'], 125)

        out = sv.parse_vox((staging / 'sample.vox').read_bytes(), 'staged')
        self.assertEqual(out.nodes[0].frames, [{'_t': '-2 -3 -1', '_r': '4'}])
        named = out.nodes[2]
        self.assertEqual(named.layer, 0)
        self.assertEqual(named.frames, [{'_t': '-10 -15 -5', '_r': '4'}])
        # LAYR/MATL/RGBA 原样保留。
        self.assertEqual([tag for tag, _ in out.raw_chunks], ['LAYR'])

    def test_empty_model_keeps_named_node(self):
        """空白画布：0 体素模型保留命名节点且不产生任何块模型。"""
        data = build_vox([((6, 5, 4), [])], root_scene('hollow', 0))
        src = self.write_source(data)
        staging = self.root / 'staging'

        stats = self.convert(src, staging)
        self.assertEqual((stats['source_voxels'], stats['output_voxels']), (0, 0))
        self.assertEqual(stats['models'], (1, 0))

        out = sv.parse_vox((staging / 'sample.vox').read_bytes(), 'staged')
        self.assertEqual(out.nodes[2].node_dict, {'_name': 'hollow'})

    def test_meta_rewritten_and_source_untouched(self):
        """staging 模式：meta 只改 voxel_size，源文件与源 meta 保持原样。"""
        voxels = [(0, 0, 0, 1), (1, 0, 0, 2)]
        data = build_vox([((2, 1, 1), voxels)], root_scene('body', 0))
        src = self.write_source(data)
        staging = self.root / 'staging'

        self.convert(src, staging)
        self.assertEqual(src.read_bytes(), data)  # 未发布，源不动
        staged_meta = (staging / 'sample.vox.meta').read_text(encoding='utf-8')
        self.assertIn('voxel_size: 0.02,', staged_meta)
        self.assertNotIn('0.1', staged_meta)
        self.assertIn('meta_format_version: "1.0"', staged_meta)

    def test_publish_replaces_source(self):
        """--publish：源文件与 meta 原子替换为细分结果。"""
        voxels = [(0, 0, 0, 1)]
        data = build_vox([((2, 1, 1), voxels)], root_scene('body', 0))
        src = self.write_source(data)
        staging = self.root / 'staging'

        stats = self.convert(src, staging, publish=True)
        self.assertTrue(stats['published'])
        self.assertEqual(stats['source_bytes'], len(data))
        out = sv.parse_vox(src.read_bytes(), 'published')
        self.assertEqual(sum(m.voxel_count for m in out.models), 125)
        self.assertIn('voxel_size: 0.02,',
                      (self.root / 'sample.vox.meta').read_text(encoding='utf-8'))
        # 备份与临时 staging 均已清理。
        self.assertFalse((self.root / 'sample.vox.subdiv-bak').exists())
        self.assertFalse(staging.exists() and any(staging.iterdir()))

    def test_publish_rollback_on_meta_failure(self):
        """发布时 meta 替换失败：VOX 与 meta 必须成对回滚到原始内容。"""
        voxels = [(0, 0, 0, 1)]
        data = build_vox([((2, 1, 1), voxels)], root_scene('body', 0))
        src = self.write_source(data)
        original_meta = (self.root / 'sample.vox.meta').read_text(encoding='utf-8')
        staging = self.root / 'staging'

        # 发布内 os.replace 调用序：源→备份、新 VOX→源、新 meta→源 meta；
        # 在第三次调用注入失败，模拟 meta 替换出错。
        real_replace = os.replace
        calls = {'n': 0}

        def flaky_replace(src_path, dst_path):
            calls['n'] += 1
            if calls['n'] == 3:
                raise OSError('注入的 meta 替换失败')
            return real_replace(src_path, dst_path)

        with mock.patch('os.replace', side_effect=flaky_replace):
            with self.assertRaises(OSError):
                self.convert(src, staging, publish=True)

        self.assertEqual(src.read_bytes(), data, 'VOX 未回滚到原始内容')
        self.assertEqual((self.root / 'sample.vox.meta').read_text(encoding='utf-8'),
                         original_meta, 'meta 未保持原样')
        # 回滚通过移动完成，备份不残留；staging 保留供取证。
        self.assertFalse((self.root / 'sample.vox.subdiv-bak').exists())
        self.assertTrue(staging.exists())

    def test_staging_refuses_overwrite(self):
        """重复运行到同一 staging 目录拒绝覆盖。"""
        voxels = [(0, 0, 0, 1)]
        src = self.write_source(build_vox([((2, 1, 1), voxels)], root_scene('body', 0)))
        staging = self.root / 'staging'
        self.convert(src, staging)
        with self.assertRaises(sv.ConvertError):
            self.convert(src, staging)


class UnitGateTests(unittest.TestCase):
    """单位校验：比例必须恰为 5 倍，重复细分被拒绝。"""

    def test_rejects_already_subdivided_source(self):
        with self.assertRaises(sv.ConvertError):
            sv.check_units(0.02, 0.02, 'x')

    def test_rejects_wrong_ratio(self):
        with self.assertRaises(sv.ConvertError):
            sv.check_units(0.05, 0.02, 'x')
        with self.assertRaises(sv.ConvertError):
            sv.check_units(0.1, 0.05, 'x')

    def test_accepts_exact_five(self):
        sv.check_units(0.1, 0.02, 'x')

    def test_meta_missing_unit_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            meta = Path(tmp) / 'a.vox.meta'
            meta.write_text('( settings: () )', encoding='utf-8')
            with self.assertRaises(sv.ConvertError):
                sv.read_meta_unit(meta)


class StrictnessTests(unittest.TestCase):
    """格式严格性：白名单外的输入必须明确报错。"""

    def convert_expect_error(self, data, unit='0.1', match=''):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            src = root / 'bad.vox'
            src.write_bytes(data)
            (root / 'bad.vox.meta').write_text(META.replace('0.1', unit), encoding='utf-8')
            with self.assertRaises(sv.ConvertError) as ctx:
                sv.convert(src, 0.02, root / 'staging', False)
            if match:
                self.assertIn(match, str(ctx.exception))

    def test_rejects_unknown_chunk(self):
        """MAIN 子块里出现白名单外的块（如 PACK）必须报错。"""
        nodes = root_scene('body', 0)
        children = b''
        children += chunk('SIZE', struct.pack('<III', 2, 1, 1))
        children += chunk('XYZI', struct.pack('<I', 1) + bytes((0, 0, 0, 1)))
        children += chunk('PACK', struct.pack('<III', 1, 1, 1))
        for spec in nodes:
            kind, node_dict, rest = spec[0], spec[1], spec[2:]
            if kind == 'trn':
                child, layer, frames = rest
                children += chunk('nTRN', struct.pack('<i', nodes.index(spec))
                                  + dictionary(node_dict)
                                  + struct.pack('<iiii', child, -1, layer, len(frames))
                                  + b''.join(dictionary(f) for f in frames))
        data = b'VOX ' + struct.pack('<I', 150) + chunk('MAIN', children=children)
        self.convert_expect_error(data, match='PACK')

    def test_rejects_bad_version(self):
        voxels = [(0, 0, 0, 1)]
        data = build_vox([((2, 1, 1), voxels)], root_scene('body', 0))
        self.convert_expect_error(data[:4] + struct.pack('<I', 200) + data[8:], match='200')

    def test_rejects_multiframe_animation(self):
        nodes = [
            ('trn', {}, 1, -1, [{}]),
            ('grp', {}, [2]),
            ('trn', {'_name': 'anim'}, 3, -1, [{'_t': '0 0 0'}, {'_t': '1 0 0'}]),
            ('shp', {}, [(0, {})]),
        ]
        data = build_vox([((2, 1, 1), [(0, 0, 0, 1)])], nodes)
        self.convert_expect_error(data, match='多帧')

    def test_rejects_multi_model_shape(self):
        nodes = [
            ('trn', {}, 1, -1, [{}]),
            ('grp', {}, [2]),
            ('trn', {'_name': 'flip'}, 3, -1, [{}]),
            ('shp', {}, [(0, {}), (1, {})]),
        ]
        models = [((2, 1, 1), [(0, 0, 0, 1)]), ((1, 1, 1), [(0, 0, 0, 2)])]
        self.convert_expect_error(build_vox(models, nodes), match='2 个模型')

    def test_rejects_unsupported_frame_key(self):
        nodes = [
            ('trn', {}, 1, -1, [{}]),
            ('grp', {}, [2]),
            ('trn', {'_name': 'x'}, 3, -1, [{'_f': '1'}]),
            ('shp', {}, [(0, {})]),
        ]
        data = build_vox([((2, 1, 1), [(0, 0, 0, 1)])], nodes)
        self.convert_expect_error(data, match='_f')

    def test_rejects_voxel_outside_canvas(self):
        data = build_vox([((2, 1, 1), [(2, 0, 0, 1)])], root_scene('body', 0))
        self.convert_expect_error(data, match='超出画布')


class VerifierSelfCheckTests(unittest.TestCase):
    """验证器自检：被篡改的 staged 输出必须被发现。"""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)

    def tearDown(self):
        self._tmp.cleanup()

    def test_color_flip_detected(self):
        voxels = [(50, 3, 4, 1), (51, 3, 4, 2)]  # 覆盖块交界
        data = build_vox([((135, 36, 26), voxels)], root_scene('body', 0))
        src = self.root / 's.vox'
        src.write_bytes(data)
        (self.root / 's.vox.meta').write_text(META, encoding='utf-8')
        staging = self.root / 'staging'
        sv.convert(src, 0.02, staging, False)

        # 篡改 staged：交换第一个 XYZI 记录的颜色字节。
        staged_bytes = bytearray((staging / 's.vox').read_bytes())
        for i in range(len(staged_bytes) - 4):
            if staged_bytes[i:i + 4] == b'XYZI':
                # XYZI 内容首 4 字节是数量，第一条记录随后；改其颜色字节。
                staged_bytes[i + 16 + 3] = 9
                break
        else:
            self.fail('staged 中未找到 XYZI')
        source = sv.parse_vox(data, 'src')
        with self.assertRaises(sv.ConvertError):
            sv.verify_staged(bytes(staged_bytes), source, sv.FACTOR, 's.vox')

    def test_dropped_voxel_detected(self):
        voxels = [(0, 0, 0, 1), (1, 1, 1, 2)]
        data = build_vox([((2, 2, 2), voxels)], root_scene('body', 0))
        src = self.root / 's.vox'
        src.write_bytes(data)
        (self.root / 's.vox.meta').write_text(META, encoding='utf-8')
        staging = self.root / 'staging'
        sv.convert(src, 0.02, staging, False)

        # 篡改 staged：把第二条记录替换为第一条的副本（制造重叠+缺失）。
        staged_bytes = bytearray((staging / 's.vox').read_bytes())
        for i in range(len(staged_bytes) - 4):
            if staged_bytes[i:i + 4] == b'XYZI':
                count, = struct.unpack_from('<I', staged_bytes, i + 12)
                self.assertEqual(count, 250)
                rec0 = bytes(staged_bytes[i + 16:i + 20])
                staged_bytes[i + 20:i + 24] = rec0
                break
        source = sv.parse_vox(data, 'src')
        with self.assertRaises(sv.ConvertError):
            sv.verify_staged(bytes(staged_bytes), source, sv.FACTOR, 's.vox')


if __name__ == '__main__':
    unittest.main()
