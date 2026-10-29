"""玩家细网格作者源；坐标为共同 VOX 画布上的 0.02 米资产格，+X 朝前、+Z 向上。"""
import struct

from subdivide_vox import ConvertError, frame_t, parse_vox, write_vox

# 参考画布 ×5，仅用于分块坐标还原，不改变运行时的 0.1 米契约。
CANVAS = (675, 180, 130)


def author_body(path):
    """在临时生成的车身上制作凹入一格、宽两格的尾舱格栅，保留命名和分块。"""
    source = parse_vox(path.read_bytes(), str(path))
    original_count = sum(m.voxel_count for m in source.models)
    body = next(n for n in source.nodes if n.node_dict.get('_name') == 'body')
    group = source.nodes[body.child]
    for index in group.children:
        node = source.nodes[index]
        model = source.models[source.nodes[node.child].models[0][0]]
        origin = tuple(t + (f - s) // 2 for t, f, s in
                       zip(frame_t(node.frames), CANVAS, model.size))
        cells = {(x, y, z): c for x, y, z, c in struct.iter_unpack('<BBBB', model.xyzi)}
        # 1.3×1.6 米尾舱面板；槽距 0.14 米，无法由旧格同色细分生成。
        for x in range(170, 235):
            for y in range(50, 130):
                groove = (x - 170) % 7 < 2 and 53 <= y < 127
                for z in range(78, 80):
                    local = tuple(a - b for a, b in zip((x, y, z), origin))
                    if not all(0 <= a < b for a, b in zip(local, model.size)):
                        continue
                    if local not in cells:
                        raise ConvertError(f'尾舱作者源越出旧面板：{x},{y},{z}')
                    if groove and z == 79:
                        del cells[local]
                    else:
                        cells[local] = 4 if groove else 2
        model.xyzi = b''.join(bytes((*p, c)) for p, c in sorted(cells.items()))
    write_vox(source.models, source.nodes, source.raw_chunks, path)
    checked = parse_vox(path.read_bytes(), str(path))
    if [m.size for m in checked.models] != [m.size for m in source.models]:
        raise ConvertError('细网格作者化改变了分块画布')
    if sum(m.voxel_count for m in checked.models) != original_count - 1480:
        raise ConvertError('尾舱凹槽未完整写入：预期移除 1480 个表面细格')
