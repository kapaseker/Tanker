"""独立读取交付 VOX，核对厘米网格、场景拼接及关键部件；不依赖游戏模型。"""
import struct
import unittest
from pathlib import Path

import numpy as np


def read_vox(path):
    """只接受本交付使用的无旋转单帧场景，并还原每个叶节点的全局厘米坐标。"""
    data = Path(path).read_bytes()
    assert data[:8] == b"VOX " + struct.pack("<I", 150)
    assert data[8:12] == b"MAIN"
    assert struct.unpack_from("<II", data, 12) == (0, len(data) - 20)
    models, transforms, shapes, groups = [], {}, {}, {}

    def dictionary(buf, offset):
        count, = struct.unpack_from("<i", buf, offset)
        offset += 4
        out = {}
        for _ in range(count):
            strings = []
            for _ in range(2):
                length, = struct.unpack_from("<i", buf, offset)
                offset += 4
                strings.append(buf[offset:offset + length].decode())
                offset += length
            out[strings[0]] = strings[1]
        return out, offset

    offset = 20
    while offset < len(data):
        kind, length, children = struct.unpack_from("<4sII", data, offset)
        assert children == 0
        buf = data[offset + 12:offset + 12 + length]
        assert len(buf) == length
        if kind == b"SIZE":
            size = np.array(struct.unpack("<III", buf))
            assert np.all((size > 0) & (size <= 256))
        elif kind == b"XYZI":
            count, = struct.unpack_from("<I", buf)
            cells = np.frombuffer(buf[4:], np.uint8).reshape(-1, 4).astype(np.int32)
            assert len(cells) == count and np.all(cells[:, :3] < size)
            assert np.all(cells[:, 3] != 0)
            models.append((size.copy(), cells))
        elif kind in (b"nTRN", b"nSHP", b"nGRP"):
            node, = struct.unpack_from("<i", buf)
            attrs, pos = dictionary(buf, 4)
            if kind == b"nTRN":
                child, reserved, layer, frames = struct.unpack_from("<iiii", buf, pos)
                assert frames == 1 and reserved == -1
                frame, _ = dictionary(buf, pos + 16)
                assert "_r" not in frame
                transforms[node] = (child, np.array(list(map(int, frame.get("_t", "0 0 0").split()))), attrs)
            elif kind == b"nSHP":
                count, model = struct.unpack_from("<ii", buf, pos)
                assert count == 1
                shapes[node] = model
            else:
                count, = struct.unpack_from("<i", buf, pos)
                groups[node] = struct.unpack_from(f"<{count}i", buf, pos + 4)
        offset += 12 + length
    assert offset == len(data)
    leaves = []

    def visit(node, translation, name):
        if node in transforms:
            child, delta, attrs = transforms[node]
            visit(child, translation + delta, attrs.get("_name", name))
        elif node in groups:
            for child in groups[node]:
                visit(child, translation, name)
        else:
            size, cells = models[shapes[node]]
            global_cells = cells.copy()
            global_cells[:, :3] += translation - size // 2
            leaves.append((name, global_cells))

    visit(0, np.zeros(3, dtype=np.int32), "")
    assert len(leaves) == len(models)
    return leaves


class ModelContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.parts = read_vox(Path(__file__).parent / "leopard_2a6.vox")
        cls.cells = np.concatenate([cells for _, cells in cls.parts])

    def test_full_dimensions_and_no_wrapped_coordinates(self):
        xyz = self.cells[:, :3]
        np.testing.assert_array_equal(xyz.min(axis=0), [0, 0, 0])
        np.testing.assert_array_equal(xyz.max(axis=0) + 1, [1097, 376, 438])
        self.assertGreater(len(self.cells), 500000)
        keys = (xyz[:, 0].astype(np.int64) * 376 + xyz[:, 1]) * 438 + xyz[:, 2]
        self.assertEqual(len(np.unique(keys)), len(keys), "分块不得重叠或重复体素")

    def test_named_mechanical_groups_and_exact_wheel_count(self):
        names = {name.split("/")[0] for name, _ in self.parts}
        for name in ("hull", "turret", "gun", "track_right", "track_left"):
            self.assertIn(name, names)
        self.assertEqual(len([n for n in names if n.startswith("roadwheel_")]), 14)
        self.assertEqual(len([n for n in names if n.startswith("idler_")]), 2)
        self.assertEqual(len([n for n in names if n.startswith("sprocket_")]), 2)

    def test_roof_axis_hatches_and_forward_muzzle(self):
        xyz = self.cells[:, :3]
        muzzle = xyz[xyz[:, 0] == 1096]
        self.assertTrue(np.all((muzzle[:, 2] >= 199) & (muzzle[:, 2] < 217)))
        # 炮膛为 120 mm；中心必须留空，不能用黑色实心端盖代替孔洞。
        self.assertFalse(np.any((muzzle[:, 1] == 188) & (muzzle[:, 2] == 208)))
        turret = np.concatenate([v for n, v in self.parts if n.startswith("turret/")])
        roof = turret[(turret[:, 0] > 340) & (turret[:, 0] < 390) & (turret[:, 1] > 170) & (turret[:, 1] < 200)]
        self.assertEqual(roof[:, 2].max() + 1, 264)
        for y in (110, 258):
            lid = turret[(turret[:, 0] > 303) & (turret[:, 0] < 326) & (abs(turret[:, 1] - y) < 12)]
            self.assertEqual(lid[:, 2].max() + 1, 270)

    def test_wheel_diameters_positions_and_track_envelope(self):
        for side in ("right", "left"):
            for i, x in enumerate((107, 196, 285, 374, 463, 552, 641), 1):
                wheel = np.concatenate([v for n, v in self.parts if n.startswith(f"roadwheel_{side}_{i:02d}/")])
                np.testing.assert_array_equal(wheel[:, :3].min(axis=0)[[0, 2]], [x - 40, 2])
                np.testing.assert_array_equal((wheel[:, :3].max(axis=0) + 1)[[0, 2]], [x + 40, 82])
                tire = wheel[wheel[:, 3] == 7]
                self.assertEqual(int(np.ptp(tire[:, 1])) + 1, 40)
            track = np.concatenate([v for n, v in self.parts if n.startswith(f"track_{side}/")])
            self.assertEqual(track[:, 2].min(), 0)
            self.assertEqual(track[:, 2].max() + 1, 96)

    def test_tail_lenses_are_mirrored_horizontal_and_unlit(self):
        for palette in (14, 15):
            lenses = self.cells[self.cells[:, 3] == palette, :3]
            left = lenses[lenses[:, 1] < 188]
            right = lenses[lenses[:, 1] > 188].copy()
            right[:, 1] = 375 - right[:, 1]
            self.assertEqual({tuple(v) for v in left}, {tuple(v) for v in right})
            self.assertTrue(np.all(left[:, 0] < 2))
        # 白色灯片只在车首，尾灯不得出现设计稿旧版的白色方灯。
        self.assertTrue(np.all(self.cells[self.cells[:, 3] == 16, 0] > 700))

    def test_fenders_cover_the_gap_between_hull_and_side_skirts(self):
        hull = np.concatenate([v for n, v in self.parts if n.startswith("hull/")])
        for y0, y1 in ((8, 59), (317, 368)):
            deck = hull[(hull[:, 0] >= 300) & (hull[:, 0] < 500) &
                        (hull[:, 1] >= y0) & (hull[:, 1] < y1) & (hull[:, 2] == 159)]
            self.assertGreater(len(deck), 5000, "外侧裙板必须通过水平翼子板连接车体")


if __name__ == "__main__":
    unittest.main()
