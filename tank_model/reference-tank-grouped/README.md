# 参考图坦克体素模型

保留的模型是 [0.02 米网格候选](candidate-002/README.md)；原 0.018 米 VOX 已移除，下述预览和验证记录来自原始版本。

交付候选：**`candidate-002/reference_tank.vox`**。

根据 `reference.png` 单张三分之四视角图片重建的静态整车。重点表现宽车体、楔形炮塔、七对负重轮、逐节履带、侧裙分块、观瞄塔、双天线、车顶机枪、发动机格栅、灯具和中空炮口。配色包含橄榄绿装甲、深色橡胶与钢材、磨损边缘和不透明观瞄玻璃。

背面、底部及被遮挡部分按可见结构推定补全；没有内部乘员舱、骨骼或动画。

## 导入

- VOX 150，使用 `SIZE` / `XYZI` / `RGBA` 与 `nTRN` / `nGRP` / `nSHP` 场景层级；附有 `MATL` 材质参数。
- 总包围盒 **640 × 239 × 298** 体素，约 **1140 万**实体体素。精确计数见 `validation.json`。
- 6 个具名部件组：车体及侧裙、左履带、右履带、炮塔、主炮、车顶设备，共 11 个体素数据块。
- 导入时需要遍历整个场景并应用节点平移；只读取首个 `XYZI` 块会缺失其他部分。
- 作者坐标：**+X 向前、+Y 向车辆左侧、+Z 向上**；原点位于车体纵向中心附近的地面高度。可按引擎需求统一缩放及转换坐标轴。
- 命名分组用于组织静态模型，未设旋转枢轴。炮塔与主炮动画需在引擎侧设置。
- 地面、灯光、阴影和背景只存在于预览，不包含在 VOX 文件中。

## 预览和工作文件

| 文件 | 内容 |
| --- | --- |
| `preview_front.png` | 接近参考图的前侧视角 |
| `preview_rear.png` | 后侧视角 |
| `preview_side.png` | 侧视角 |
| `preview_top.png` | 俯视图 |
| `reference.png` | 原始参考图 |
| `build_tank.py` | 参数化几何、配色、VOX 导出及回读校验 |
| `render_voxels.rs` | 本地体素光线追踪预览程序，无第三方 Rust 依赖 |
| `render_volume.bin` | 从最终 VOX 回读重建的渲染缓存 |
| `validation.json` | 格式、坐标、调色板和逐体素回读校验结果 |

预览使用真实导出体素，不是另行生成的概念图。预览对宽阔斜装甲面的光照法线做了柔化，实际轮廓保留体素台阶；在引擎中的材质、光照与阴影效果取决于渲染设置。

## 重新生成

以下是原始版本的历史复现流程，会重新生成已移除的大模型和渲染缓存。0.02 米候选请按其说明重建。在本目录执行，Python 需要已安装的 `numpy`，PNG 转换需要 `Pillow`：

```powershell
python build_tank.py
rustc --edition 2021 -O render_voxels.rs -o render_voxels.exe
.\render_voxels.exe render_volume.bin preview_front.ppm front 1800 12
.\render_voxels.exe render_volume.bin preview_rear.ppm rear 1600 8
.\render_voxels.exe render_volume.bin preview_side.ppm side 1600 8
.\render_voxels.exe render_volume.bin preview_top.ppm top 1600 8
python -c "from PIL import Image; from pathlib import Path; [Image.open(p).save(p.with_suffix('.png')) for p in Path('.').glob('preview_*.ppm')]"
```

校验已覆盖块长度、模型边界、重复坐标、非零颜色索引、场景引用、场景遍历及原始体素与导出回读结果逐点一致。已渲染检查四个视角；尚未在用户的游戏引擎或 MagicaVoxel 中实测导入。
