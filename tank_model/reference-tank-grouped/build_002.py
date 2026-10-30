"""把参考坦克的 0.018 米作者网格转换为独立的 0.02 米候选。"""
from contextlib import redirect_stdout
import io
import json
from pathlib import Path

import numpy as np

import build_tank as model


def main():
    """按世界坐标采样，导出并回读校验，不覆盖原始模型。"""
    for side in (-1, 1):
        model.tracks(side)
    model.hull()
    model.turret()
    model.main_gun()
    model.equipment()
    model.surface_detail()

    old_low = model.LO.copy()
    old_shape = model.V.shape
    source_cells = np.argwhere(model.V)
    source_dimensions = (source_cells.max(0) - source_cells.min(0) + 1) * 0.018
    low = np.floor(old_low * 0.9).astype(int)
    high = np.ceil((old_low + old_shape) * 0.9).astype(int)
    indices = [np.clip(np.floor((np.arange(a, b) + 0.5) / 0.9).astype(int) - origin, 0, size - 1)
               for a, b, origin, size in zip(low, high, old_low, old_shape)]
    selection = np.ix_(*indices)
    model.V, model.PART = model.V[selection], model.PART[selection]
    model.LO, model.SHAPE = low, model.V.shape
    assert all(np.any(model.PART == group) for group in model.GROUPS)
    target_cells = np.argwhere(model.V)
    target_dimensions = (target_cells.max(0) - target_cells.min(0) + 1) * 0.02
    assert np.all(np.abs(target_dimensions - source_dimensions) <= 0.04)

    model.OUT = Path(__file__).resolve().parent / 'candidate-002'
    model.OUT.mkdir(exist_ok=True)
    path, _ = model.export_vox()
    with redirect_stdout(io.StringIO()):
        model.validate_vox(path)
    report_path = model.OUT / 'validation.json'
    report = json.loads(report_path.read_text(encoding='utf8'))
    report.pop('suggested_meters_per_voxel')
    report['meters_per_voxel'] = 0.02
    report['dimensions_meters'] = target_dimensions.tolist()
    report['source_dimensions_meters'] = source_dimensions.tolist()
    report_path.write_text(json.dumps(report, indent=2), encoding='utf8')
    (model.OUT / 'render_volume.bin').unlink()
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
