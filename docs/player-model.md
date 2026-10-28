# 玩家坦克模型契约

模块导航：共享体素尺度和 `WheelSpec` 位于 `src/tank/model/mod.rs`；生成器 `scripts/generate_player.rs` 复用车型契约，交付资产统一经 `scripts/publish_player.py`（生成→细分→发布）产出。驾驶、履带、炮管挂载归 `src/tank/`，对局生成归 `src/gameplay/session.rs`。目录整理验收见 [任务 10](testing/architecture-step10-20261023.md)。

默认玩家为原创现代主战坦克体素造型：橄榄绿倾斜首上、低矮棱角炮塔、侧裙板和深灰履带。保持现有驾驶速度、整车朝向开炮，不支持独立瞄准或游戏内换装。

## 尺寸和坐标

权威格坐标及非炮管碰撞边界在 `src/tank/model/player.rs`，生成器和运行时共同读取；坐标一律按参考格（每格 0.1 米，`REFERENCE_VOXEL_SIZE`）。交付的车身、炮管、履带与子弹资产已按 0.02 米/格细分（`.vox.meta` 声明），契约数值不随细分改变。车身、炮管与六个履带相位统一参考画布为 135×36×26 格，VOX +X 为车头、Y 为宽度、Z 为高度。画布中心 X=67.5 正好为车体中心；车尾方向留空 30 格，加载后不需要位置补偿。子弹为 60×20×20 格（1.2×0.4×0.4 米），长轴沿世界 X。

- 车体范围 X=[30,105)，长 7.5 米，宽含履带 3.6 米，地面至炮塔顶 2.6 米。
- 炮塔正面 X=90，炮管 X=[90,135)，外露长 4.5 米；炮口相对车体中心前方 6.75 米，轴线离地 2 米。
- 含炮管视觉全长为 10.5 米（车尾至炮口），不能作为碰撞框长度。
- 车身、履带、炮塔共用一个 7.5×2.6×3.6 米圆角碰撞框（Bevy 长×高×宽）；炮管没有碰撞体。框内装甲空隙仍按命中处理。
- 跟随相机后退 16 米、抬高 9 米，保持既有跟随方式，抬头角度调整为 8°，避免车尾被画面下沿裁切。

## 三包与替换规则

| 资源路径（相对 assets） | 节点 | 内容 |
|---|---|---|
| tank/bodies/player/anvil/body.vox | body | 车身、炮塔、炮盾、裙板、舱盖、格栅 |
| tank/barrels/player/lance/barrel.vox | barrel | 炮管根部、管身、抽烟器、炮口 |
| tank/tracks/player/cleat/track.vox | track_l_0..2 / track_r_0..2，wheel_l_0..6 / wheel_r_0..6 | 六个履带相位、十四个独立轮 |

三部件统一按 `tank/<部件>/<车型>/<变体>/` 分包，变体目录名即型号 ID（玩家车体 `anvil`、炮管 `lance`、履带 `cleat`），包内主模型固定为 `body.vox` / `barrel.vox` / `track.vox`。子弹是 `lance` 炮管的附属资源，与其同目录的 `bullet.vox`，运行时与发布管线均使用该路径。换装只需替换变体目录，详见 `docs/adr/0001-tank-asset-packages.md`。

三包共享调色板；车身、炮管和履带相位以全局画布零偏移挂载。轮子使用8×4×8（负重轮）或5×4×5（端轮）局部画布，按 `WHEELS` 的轴心安装。替换资源必须保持该契约；需要改变炮管长度或轴线时，先修改共享尺寸定义、生成器和相关测试，不通过运行时缩放掩盖不兼容。履带齿距三格、每行驶一格推进一相位，闭环168格。轮子按对应侧累计里程除以滚动半径绕游戏Z轴连续旋转，停车保留角度；每个节点必须完成加载和实例化，缺失节点会报错。

## 生成与验证

```powershell
python scripts/publish_player.py           # 生成粗样→细分车身/炮管/履带/子弹到0.02→原子发布交付资产
python scripts/publish_player.py --check   # 只校验管线产物与交付资产逐字节一致，不写文件
rustc --edition 2024 --test scripts/generate_player.rs -o target/test_generate_player.exe
./target/test_generate_player.exe
python -m unittest discover -s scripts -p test_player.py
python -m unittest discover -s scripts -p test_tracks.py
cargo check
cargo test
```

生成器不读取旧玩家或敌人资源，相同输入输出相同字节，只产出 0.1 米/格的粗样；交付必须走 `publish_player.py`，由 `scripts/subdivide_vox.py` 五倍保色细分车身、炮管、履带与子弹并配对改写 `.vox.meta`。子弹没有生成器：`scripts/fixtures/bullet.vox` 保存迁移前的 0.1 米/格粗源快照，作为细分管线的转换源，不从已细分成品再次细分。直接运行 `generate_player.exe` 会在检测到交付配对 meta 单位不是 0.1 时拒绝写入，防止把粗资产覆盖回 0.02 配置。资源测试解析场景树并还原分块坐标到参考格，检查尺寸、三包不重叠和连接面；Rust 集成测试检查真实加载后的组合尺寸、炮口和清理。离屏运行 scene、drive、combat 和 wheels 并读取关键帧。wheels输出侧面逐帧动画序列及side/front/rear/bounds视点，覆盖前进、倒车、左右转向与停车。

## 敌人尺寸兼容

敌人现采用6.6×2.6×3.4米非炮管碰撞体，高度直接复用玩家值，默认平地水平射击可命中。combat离屏预设已移除旧敌人的抬高台座，改为平地命中、爆炸和清理验证；敌方模型契约见 `docs/enemy-model.md`。下文旧验收记录中的台座结果仅反映当时版本。

## 本机验收记录（2026-10-20）

- `cargo check` 通过；`cargo test` 40/40 通过。
- 玩家生成器 3 项测试通过（含产物逐字节一致性）；玩家资源 2 项、履带资源 1 项 Python 测试通过。
- RTX 4060 / NVIDIA 610.88 / Vulkan：`scene`、`drive`、`combat` 均退出 0，报告通过，已读取关键帧。没有启动可见游戏窗口。
- 本机证据：`.scratch/offscreen/player-modern-scene-02/`、`player-modern-drive-01/`、`player-modern-combat-01/`。这些目录仅本机保留，不提交到仓库。
- `drive` 1 秒直行约 6 米，左右里程约 6/6 米；转弯偏航超过 1 弧度。`combat` 验证台座上的单个目标击毁、弹体及爆炸清理。
- Python 全套曾执行，发现已有 `test_generate_ground.py` 仍引用已删除的 `SPAWN_PAD_M` 而报错。本轮未修改该地形测试或地形生成逻辑；不宣称 Python 全套通过。
- 未验证其他显卡、Linux 或窗口输入行为。

## 轮廓优化尺寸（2026-10-20）

| 部件 | 本轮尺寸与位置 |
|---|---|
| 炮塔 | 长4.1米；底层宽2.6米、中层2.4米、顶层2.0米；主体仍离地1.6～2.5米，舱盖顶2.6米 |
| 炮塔分区 | VOX X=[49,62)尾舱1.3米、[62,77)中部1.5米、[77,90)前楔1.3米；中层前楔宽2.4→1.2米，尾舱顶降低0.1米 |
| 炮管 | 管身截面0.2×0.2米；根部0.6×0.4×0.4米，抽烟器0.6×0.4×0.4米，炮口段0.2×0.2×0.2米；抽烟器中心距炮塔正面2.3米 |
| 轮组 | 每侧5个直径0.8米负重轮，中心距车尾1.35/2.55/3.75/4.95/6.15米、离地0.6米；2个直径0.5米端轮，距车尾0.4/7.1米、离地0.65米；轮宽0.4米，横向轴心距中线1.4米；旋转包络半径分别≤0.43/0.28米 |
| 裙板 | 每侧7块0.9×0.6×0.2米面板，距车尾0.3米起、中心间距1米，板间实际间隙0.1米，内沿窄梁连接；离地1.4～1.6米 |
| 甲板 | 格栅区域1.4×1.6米，顶面嵌入甲板；车尾最后0.4米下降0.2米 |
| 灰色调色板 | RGB(58,62,56)，三包保持一致 |

履带主体高1.2米、顶齿最高1.3米；内部只保留闭环和内侧连接条，轮间透空，轮胎深灰、轮毂绿色、辐条浅色。

碰撞不是按画布推算：生成器对车身及全部履带相位的并集测量，并检查每个独立轮安装后的完整旋转包络，VOX最小边界[30,0,0]、最大边界[105,36,26]，与共同画布中心重合，无需平移补偿。运行时由这组边界生成7.5×2.6×3.6米圆角框，圆角0.2米；测试核对0°、45°、90°真实碰撞体AABB，炮管不扩大框。圆角会削去方框角部，框内装甲空隙仍参与碰撞。

### 轮廓优化验收

生成器4项、玩家资源3项、履带资源1项测试通过；Rust测试41项通过，`cargo check`通过。Windows RTX 4060 / Vulkan上的scene、drive、combat报告通过，已读取关键帧；drive直行约6米、转弯正常，combat仍使用抬高靶车。

本机证据为 `.scratch/offscreen/player-refined-{scene,drive,combat}-01/` 和 `.scratch/model-refined-inspection/`。后者包含side/front/rear同视点截图、collision黄色外包围框图及comparison.md前后对比。黄色框不描绘0.2米圆角，仅用于确认整体尺寸和中心；真实圆角范围由旋转碰撞体测试校验。拍摄代码仅在忽略目录的检查副本内，正式游戏镜头未改变。

## 独立轮组验证入口

```text
cargo run -- --offscreen-test wheels --output .scratch/offscreen/player-wheels
```

该预设沿用真实游戏输入与场景，依次前进、倒车、左右原地转向并等待停车稳定；输出20张侧面关键帧，以及 `side.png`、`front.png`、`rear.png`、`bounds.png`、`comparison.png` 和报告。红线仅表示碰撞体的外包围框，不描绘圆角；轮子没有独立碰撞体。

加载就绪检查覆盖六个相位和十四个轮节点，错误保留为 `TracksFailed` 并在离屏入口失败退出；真实资源测试检查局部网格中心、轮轴不漂移、旋转方向和显隐，玩家销毁测试逐个确认22个场景实例全部注销。敌人现使用六相位加十二个独立轮的包，命名与规格见 `docs/enemy-model.md`。

### 独立轮组本机验收（2026-10-20）

`cargo check`通过，Rust测试45项、生成器5项、玩家资源4项及履带资源1项通过。Linux llvmpipe（LLVM 20.1.2）/ Vulkan软件渲染下，scene、drive、combat及wheels全部退出0，已读取关键帧检查：轮间透空、三条辐条可辨、轮子绕自身轴心转动，未见明显穿插；停车稳定后第240～250帧的左右里程变化约0和0.00000024米。没有启动可见窗口；未验证硬件GPU表现。

证据位于 `.scratch/player-wheels/`：`before/`与`wheels-spokes-final/`保存同视点对照及碰撞外包围框，`comparison.png`、`wheels.gif`、`comparison.md`汇总画面，`scene-final/`、`drive-final/`、`combat-final/`保留回归报告。离屏检查同步修正了Bevy 0.20中可重试的`ShaderNotLoaded`/`ShaderImportNotYetAvailable`状态误判；真正的shader处理或模块创建失败仍失败退出。
