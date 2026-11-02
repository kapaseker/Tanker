# GLOSSARY —— Tanker 领域词汇与场景尺寸

本仓库为单上下文（single-context）结构，领域词汇与跨模块共享的尺寸常量集中在本文件。

**碰撞参数以对应 JSONC 配置为准，其他权威值以代码常量为准**，本表只做汇总与导航：数值的任何改动都必须落在表中“权威定义”一列所指向的常量上，并同步本表。单位一律为米（m），角度为度（°）。

## 度量约定

- 世界单位：1 世界单位 = 1 米；VOX 加载单位按每个 `.vox` 旁的 `.vox.meta`（voxel_size）配置，无全局覆盖（`VoxScenePlugin::default()`，`src/app/mod.rs`）。
- 参考体素格：作者化契约网格单格边长 = 0.1 米（`REFERENCE_VOXEL_SIZE`，`src/tank/model/mod.rs`）；尺寸/轮位/炮口契约按“格数 × 0.1”换算米制。资产加载单位另由每个 `.vox` 旁的 `.vox.meta`（voxel_size）决定，已细分资产为 0.02 米/格，两者不可混用。
- 地形栅格：地形图按 0.5 米采样作者化，视觉网格与碰撞体统一量化到 0.1 米台阶（`AUTHOR_RES` / `TERRAIN_STEP`，`src/terrain/mod.rs`）。
- VOX 轴向映射：VOX 的 Z 为高度（→ Bevy +Y），VOX 的 Y 为宽度（→ Bevy Z），VOX 的 +X 为车头方向（→ Bevy −X）。
- 尺寸约定：下表 X 为车长方向、Y 为高度、Z 为车宽方向；坦克车头朝局部 −X（`src/tank/driving.rs`）。

## 部件尺寸一览

| 部件 | 网格尺寸（VOX X×Y×Z，格） | X 长 | Y 高 | Z 宽 | 权威定义 | 备注 |
|---|---|---|---|---|---|---|
| LeoPard2 A6（玩家碰撞框） | astraH 整车（多块编码） | 7.68 | 2.68 | 3.76 | `assets/tank/player/leopard2_a6/tank.jsonc` | 简化圆角车体框，炮管与附件不碰撞；视觉全长 10.98 m、双天线顶 4.32 m，见 `docs/player-model.md` |
| 敌人坦克（碰撞框） | 车体及履带相位画布114×34×26 | 6.6 | 2.6 | 3.4 | `assets/tank/enemy/carapace/tank.jsonc` | 非炮管圆角整车框（圆角0.2 m）；车身、炮管、履带轮组三包，含炮管视觉全长9 m，见 `docs/enemy-model.md` |
| 子弹 | 资产格36×5×5 | 0.72 | 0.10 | 0.10 | `assets/tank/ammunition/apfsds/bullet.jsonc` | 长条弹体视觉 + 独立 0.72 × 0.04 × 0.04 米主体扫掠碰撞；资产 `assets/tank/ammunition/apfsds/bullet.vox`（0.02 米/格） |

坦克与敌人的视觉包围盒有集成测试断言，与上表一致（`src/gameplay/combat/firing.rs` 中 `real_vox_assets_fire_and_clean_up_scene_instances`）。

## 驾驶与履带

- 玩家与敌人共用 `body_for_size()` 刚体和驾驶系统，均保留重力与碰撞响应；全轴旋转锁定，偏航只由驾驶系统积分。
- `DrivingInput` 是逐车驾驶意图：`throttle`、`turn` 均为 −1..=1，正值分别表示前进、左转。玩家前进 16 米/秒、倒车 8 米/秒（由 `PLAYER_TRACKS` 指定），最大转向 90°/秒，转向平滑时间 0.15 秒。
- `TurnRate` 为逐车转向状态。只有玩家 `Tank` 接收方向键（`src/gameplay/player.rs`）；`EnemyTank` 默认零输入，没有 AI 或自动移动，但可受碰撞产生短暂位移。
- `TrackSpec` 指定每辆车的履带包与整车尺寸，新增组件时挂载一次。`TrackAnim` 独立保存左右累计里程与相位；直行、倒车及原地转向均使用共享动画逻辑，半履带距取各车宽度的一半。
- 当前敌方的三相位接口包含 `track_l_0..2` / `track_r_0..2` 六个节点，齿距 3 格，每行驶 1 格（0.1 米）切换一个相位；不在资源内部设置 `_hidden`。车体和对应履带采用相同模型尺寸与全局坐标，运行时零偏移对齐。
- 凸齿只沿侧面外圈闭环移动，玩家与敌人轮间均透空，轮组按各侧里程独立旋转。VOX +X 是车头：前进时上段向 +X、前端向下、下段向 −X、后端向上，倒车反向；外圈长度为齿距整数倍，保证三相位回绕无接缝跳齿。

- 敌人生成器 `scripts/generate_enemy.py` 读取 `src/tank/model/enemy.rs`，生成暗红车身、独立炮管和履带轮组三包；每侧4个直径1 m负重轮、2个直径0.6 m端轮，轮宽0.4 m，轮轴高0.65/0.7 m。履带主体/顶齿高1.3/1.4 m，闭环150格，侧裙高1.5～1.7 m。`TrackSpec` 以车型轮规格切片和画布安装轮组；敌人击毁时20个场景实例全部注销。

- 当前玩家 astraH 按整车组织：车体、炮塔、炮管及左右行走机构以运动关系装配，不表示可换装部件。炮塔附件跟随炮塔偏航，炮管与防盾相对炮塔俯仰；默认朝前水平；W/S 控制炮管俯仰、A/D 控制炮塔水平旋转，炮塔保持相对车体角度并随车转向。豹 2A6 详情独立复位，不把预览角度带入对局。轮位与米制安装轴心见 `src/tank/model/astra.rs`，左右每侧三相位按约 0.14 米板距循环，停车保持轮角和相位。
- 炮口世界姿态决定弹丸出生与飞行方向；弹丸是独立对象，离开发射器后不跟随炮塔，可以同时存在多发。

## 场地与竞技场

| 部件 | 尺寸 | 权威定义 |
|---|---|---|
| 场地落地尺寸 | 600 × 600 m，面积 360000 m² | `FIELD_SIDE` / `AREA`（`src/terrain/mod.rs`） |
| 地形高度图 / 土色图 | 各 1200 × 1200 采样，作者化间距 0.5 m/采样；高度图 1440000 字节，土色图另含文件自带调色板（`[数量 u8][数量×3 RGB][索引矩阵]`） | `MAP_SIDE` / `AUTHOR_RES`（`src/terrain/mod.rs`）/ `parse_albedo()`（`src/terrain/map.rs`）；产物 `assets/terrain/ground/ground.ht`、`assets/terrain/ground/ground.abd` |
| 地形数据来源 | 标准库脚本按固定种子生成两张紧凑字节图 | `scripts/generate_ground.py`（`PALETTE` 随 `.abd` 文件写出，Rust 侧从文件读取） |
| 地形高度 | 全图 Y=0，无坡道、坑洼或台阶；高度字节固定为 10 | `FLAT`（`src/terrain/mod.rs`） |
| 地形视觉网格 | 600 m 场地按 10 × 10 块编译（每块 60 m / 120 采样），纯平顶面按花色合并，沿用原有土色贴图 | `CHUNK_SIDE` / `CHUNK_COUNT` / `CHUNK_CELLS`（`src/terrain/mod.rs`）/ `build_chunk()`（`src/terrain/mesh.rs`） |
| 地面花色 | 保留原有土质五色与椭圆斑块，独立固定种子生成 | `PALETTE` / `build_colors()`（`scripts/generate_ground.py`） |
| 地形物理表面 | 每 60 m 分块一个 Avian compound，纯平块合并为一个 cuboid，顶面 Y=0、底面 −1.1 m | `Collider::compound` + `collider_cuboids()` / `COLLIDER_BASE_Y`（`src/terrain/collider.rs`） |
| 竞技场围墙 | 四面；厚 2 m、高 8 m，横竖墙长 = 边长 + 4 m，内侧与场地边缘对齐 | `WALL_THICKNESS` / `WALL_HEIGHT`（`src/terrain/collider.rs`） |

地形不再来自 `.vox` 模型：`assets/ground.vox` 已退役删除，视觉与物理均由 `ground.ht` 编译而来，两者同源，全图保持纯平。地形资产按 `assets/terrain/<地形名>/` 组织，每个地形目录含 `ground.ht`（高度）与 `ground.abd`（土色）。

## 混凝土墩样品

- `assets/obstacles/concrete/obstacle.vox`：长 8 m、底厚 1.5 m、顶厚 0.9 m、高 8 m；灰色哑光梯形，VOX 画布 15×80×80 格。
- `src/gameplay/obstacle.rs` 的 `SIZE` / `POSITION`：世界 X 为厚度、Y 为高度、Z 为长度，中心 (−25, 4.0, 0)，出生点正前方 25 m 横放，底部贴地。
- 静态凸刚体不可推动、不可摧毁；炮弹命中仅清除炮弹，不触发敌人爆炸。敌人出生按水平外接半径另留 1 m 净空。
- `scripts/generate_concrete.py` 使用标准库生成资产；凸碰撞体与分层体素表面允许不超过 0.1 m 的轮廓差。

## 与尺寸相关的其它常量

| 含义 | 数值 | 权威定义 |
|---|---|---|
| 玩家坦克出生中心 Y | 1.34（= 车型配置碰撞高度 / 2） | `src/gameplay/session.rs` `setup` |
| 敌人坦克出生中心 Y | 3.0（落地后平地中心约 1.3） | `SPAWN_DROP_Y`（`src/gameplay/enemy.rs`） |
| 炮口端面局部坐标（炮轴节点下，车头为 −X） | (−5.42, 0, 0) | `MUZZLE_POSITION`（`src/tank/model/astra.rs`）；发射时转换到世界坐标 |
| 相机水平后退距离 / 竖直高度 | 16 / 9 | `CAMERA_DISTANCE` / `CAMERA_HEIGHT`（`src/gameplay/camera.rs`） |
| 相机视线相对车身的上抬角 | 8° | `CAMERA_PITCH_UP_DEGREES`（`src/gameplay/camera.rs`） |
| 敌人数量 / 出生中心半径、与玩家及彼此最小中心距 | 10 / ±120 / 约8.87 / 约8.42 | `ENEMY_COUNT` / `SPAWN_LIMIT` / `spawn_clearance`（双方水平外接半径之和+`SPAWN_GAP` 1 m，`src/gameplay/enemy.rs`） |
| 场地面积、子弹射程 | 360000 m² / 2000 m | `AREA`（`src/terrain/mod.rs`）/ `BULLET_RANGE`（`src/gameplay/combat/firing.rs`） |

## 术语对照

| 领域词 | 代码标识 |
|---|---|
| 玩家坦克 | `Tank`（`src/gameplay/player.rs`）、`tank_body()`（`src/tank/driving.rs`） |
| 敌人坦克（靶车） | `EnemyTank`、`ENEMY_COUNT`（`src/gameplay/enemy.rs`） |
| 子弹 | `Bullet`、`Gun`（`src/gameplay/combat/firing.rs`） |
| 场地 / 地形 | `GroundPlugin`、`TerrainChunk`、`TerrainSurface`（`src/terrain/mod.rs`）、`GroundMap`（`src/terrain/map.rs`）；地形数据由 `scripts/generate_ground.py` 生成 |
| 竞技场边界 | `spawn_walls()`（`src/terrain/collider.rs`）：四面围墙；`spawn_arena()` 另加一块平坦物理地面，仅供测试 |
| 跟随相机 | `FollowCamera`、`camera_transform()`（`src/gameplay/camera.rs`） |

## 模块导航

- `app`：启动组装、页面状态、过渡和 GPU 就绪；`main` 解析参数并返回退出码。
- `pages`：首页与画廊；画廊 `preview` 管纯视觉实例，`ui` 管布局与操作。
- `gameplay`：玩家、敌人、相机、战斗；`session` 汇总对局生成、业务就绪与清理。
- `tank`：共用驾驶、整车活动层级、炮管挂载、履带与轮组；`model` 为运行时与生成器提供纯标准库契约。
- `terrain`：地图、网格、碰撞和自身生命周期；`ui`：字体及编译期文案、颜色、尺寸。
- `validation`：CLI、离屏运行、截图和场景断言，复用真实游戏系统。依赖规则见 [ADR-0002](docs/adr/0002-module-boundaries.md)。

## 弹药性能

- `AmmunitionSpec`：模型同名 `bullet.jsonc` 的初速（米/秒）与射速（发/分钟）；启动时读取并校验，修改后重启生效。
- 冷却时长：`60 / rate_of_fire` 秒；运行中的剩余冷却由 `Gun` 保存，不写入共享配置。
- HUD 分格最多 60 格，仅限制显示，不截断真实冷却。弹药详情显示初速与射速，坦克详情保留驾驶速度。
