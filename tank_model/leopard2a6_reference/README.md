# 豹 2A6 比例 · 参考图体素坦克

保留的模型：**[0.02 米网格候选](candidate-002/leopard2a6.vox)**，重建方法与限制见[候选说明](candidate-002/README.md)。原 0.0125 米 VOX 已移除；下文尺寸、预览和验证记录描述的是原始版本，供对照使用。本目录为独立工作目录；生成脚本复用并修改自工作区 `reference_tank`，原目录未修改。

外观依据本目录 `reference.png`：橄榄绿装甲、楔形炮塔、分块侧裙、七对负重轮、逐节履带及橡胶垫、长炮管和中空炮口、指挥塔、车顶机枪、双天线、发动机格栅、灯具、牵引环和储物篮。图片里的夸张装甲及观瞄设备予以保留，因此这是按豹 2A6 总体尺寸约束制作的参考图游戏资产，不是军用工程复刻。

## 比例与坐标

原始版本的体素为等边立方体，**1 voxel = 0.0125 米（1.25 厘米）**；保留的候选版本为 0.02 米。VOX 本身不规定米制单位，需要在引擎导入时设置对应比例。

| 项目 | 本模型实测/建模尺寸 |
| --- | --- |
| 含炮总长 | 10.975 m，10.97 m 目标按网格取整 |
| 车体及挡泥板纵向范围 | 7.700 m |
| 最大宽度 | 3.750 m |
| 不含天线的最高点 | 3.000 m |
| 含天线总高 | 3.650 m，天线按图片估算 |
| 整体包围盒 | 878 × 300 × 292 voxels |
| 实体体素 | 20,101,269 |
| 文件大小 | 约 76.7 MiB |

尺寸采用豹 2A6 常见公开规格的建模基准；本次在线检索服务不可用，未完成官方资料交叉核验。单张透视图不能确定所有局部尺寸，局部比例和隐藏面经过艺术推定。高度口径为本资产的观瞄设备顶端，不将天线计入 3.00 m。

坐标为 **+X 车头、+Y 车辆左侧、+Z 向上**。地面位于 Z=0，车体纵向中心附近为原点。体素的世界格坐标范围为 X=-304…573、Y=-150…149、Z=0…291；这些是体素索引，外表面到对应最大索引加一。

生成时分别校准底盘、炮塔和车顶设备高度，并补偿轮组及炮管截面，避免简单整体压扁造成椭圆轮与扁炮管。

## 引擎导入

- VOX 150，包含 `PACK`、`SIZE`、`XYZI`、`RGBA`、`MATL` 与 `nTRN` / `nGRP` / `nSHP` 场景图。
- 20 个数据块，7 个具名组：`Hull_and_skirts`、`Track_left`、`Track_right`、`Turret`、`Main_gun`、`Roof_equipment`、`Antennas`。
- 必须遍历完整场景图并应用节点变换。仅读取第一个 `XYZI` 的导入器无法载入完整模型。
- 每块尺寸不超过 240³，切块是格式存储要求，不是降采样。拼接后是一辆完整坦克。
- 当前是静态整车，没有骨骼、履带动画、内部乘员舱或碰撞体。具名组不等于动画枢轴；需要旋转炮塔时，应在引擎侧设置枢轴并让炮管、车顶设备和天线随炮塔运动。
- 背景、地面、灯光与阴影只用于预览，不包含在模型中。玻璃使用不透明深色体素。

## 文件及复现

| 文件 | 用途 |
| --- | --- |
| `candidate-002/leopard2a6.vox` | 保留的 0.02 米网格候选模型 |
| `reference.png` | 用户提供的参考图片 |
| `build_tank.py` | 参数化建模、比例校准、导出及回读验证 |
| `render_voxels.rs` | 无第三方 Rust 依赖的体素光线追踪器 |
| `render_volume.bin` | 从最终 VOX 解析回读的渲染缓存 |
| `preview_front.png` | 正面三分之四预览 |
| `preview_rear.png` | 背面三分之四预览 |
| `preview_side.png` / `preview_top.png` | 侧面 / 顶部预览 |
| `preview_views.png` | 四视角总览 |
| `validation.json` | 实际导出计数、尺寸及校验结果 |

以下是原始版本的历史复现流程；运行 `build_tank.py` 会重新生成已移除的大模型和渲染缓存。0.02 米候选请按其说明重建。在本目录执行，使用已安装的 Python + NumPy、Pillow 和 Rust：

```powershell
python build_tank.py
rustc --edition 2021 -O render_voxels.rs -o render_voxels.exe
.\render_voxels.exe render_volume.bin preview_front.ppm front 1800 12
.\render_voxels.exe render_volume.bin preview_rear.ppm rear 1600 12
.\render_voxels.exe render_volume.bin preview_side.ppm side 1600 12
.\render_voxels.exe render_volume.bin preview_top.ppm top 1600 12
python -c "from PIL import Image; from pathlib import Path; [Image.open(p).save(p.with_suffix('.png')) for p in Path('.').glob('preview_*.ppm')]"
```

预览直接光线追踪导出后回读的体素，不是另外生成的概念图。宽阔斜面的预览光照法线有柔化；实际体素台阶和轮廓保留，最终游戏内效果取决于引擎的光照与材质处理。

## 验证状态

已通过：总体尺寸、车体长度、不含天线高度、轮组截面补偿断言；VOX 块长度、块内坐标边界、重复坐标、非零颜色索引、调色板、场景引用及完整场景遍历；导出回读后与作者体素逐点一致。已查看最终前、后、侧、顶四个视角。

未在用户的游戏引擎或 MagicaVoxel 中实测导入。背面和底部依照可见风格补全，未提供实物测绘精度保证。
