"""基于原作者几何生成独立的 0.02 米体素候选，不覆盖原模型。"""
from pathlib import Path
from contextlib import redirect_stdout
import io
import json

import numpy as np

import build_tank as model


def main():
    """按世界坐标采样原网格，并沿用原有 VOX 场景与回读校验。"""
    for side in (-1, 1):
        model.tracks(side)
    model.hull()
    model.turret()
    model.main_gun()
    model.equipment()
    model.surface_detail()
    model.calibrate_dimensions()

    # 目标格中心对应原格的 1.6 倍坐标，避免改变车辆世界尺寸。
    old_low = model.LO.copy()
    old_high = old_low + model.V.shape
    antenna = np.argwhere(model.PART == 7)
    antenna_colors = model.V[tuple(antenna.T)]
    low = np.floor(old_low * 0.625).astype(int)
    high = np.ceil(old_high * 0.625).astype(int)
    indices = [np.clip(np.floor((np.arange(a, b) + 0.5) * 1.6).astype(int) - origin, 0, size - 1)
               for a, b, origin, size in zip(low, high, old_low, model.V.shape)]
    selection = np.ix_(*indices)
    model.V, model.PART = model.V[selection], model.PART[selection]
    # 细天线可能落在采样中心之间，映射全部源格以保留末端。
    antenna_target = np.floor((antenna + old_low) * 0.625).astype(int) - low
    target = tuple(antenna_target.T)
    model.V[target] = antenna_colors
    model.PART[target] = 7
    model.LO, model.SHAPE = low, model.V.shape
    assert all(np.any(model.PART == group) for group in model.GROUPS)

    model.OUT = Path(__file__).resolve().parent / 'candidate-002'
    model.OUT.mkdir(exist_ok=True)
    path, _ = model.export_vox()
    # 原验证函数按旧单位打印；改写报告前暂不输出旧尺寸。
    with redirect_stdout(io.StringIO()):
        model.validate_vox(path)
    report_path = model.OUT / 'validation.json'
    report = json.loads(report_path.read_text(encoding='utf8'))
    report['meters_per_voxel'] = 0.02
    report['dimensions_meters'] = [n * 0.02 for n in report['dimensions_xyz']]
    assert all(abs(actual - expected) <= 0.021 for actual, expected in
               zip(report['dimensions_meters'], (10.975, 3.75, 3.65)))
    report.pop('hull_length_meters')
    report.pop('height_without_antennas_meters')
    report_path.write_text(json.dumps(report, indent=2), encoding='utf8')
    (model.OUT / 'render_volume.bin').unlink()
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
