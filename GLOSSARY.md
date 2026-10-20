# GLOSSARY —— Tanker 领域词汇与场景尺寸

本仓库为单上下文（single-context）结构，领域词汇与跨模块共享的尺寸常量集中在本文件。

**权威值以代码常量为准**，本表只做汇总与导航：数值的任何改动都必须落在表中“权威定义”一列所指向的常量上，并同步本表。单位一律为米（m），角度为度（°）。

## 度量约定

- 世界单位：1 世界单位 = 1 米，由 `VoxScenePlugin` 的全局 `VoxLoaderSettings` 统一设定（`src/main.rs`）。
- 体素格：MagicaVoxel 单格边长 = 0.1 米（`VOXEL_SIZE`，`src/main.rs`）；所有 `.vox` 模型尺寸都按“格数 × 0.1”换算。
- 地形栅格：地形图按 0.5 米采样作者化，视觉网格与碰撞体统一量化到 0.1 米台阶（`AUTHOR_RES` / `TERRAIN_STEP`，`src/ground.rs`）。
- VOX 轴向映射：VOX 的 Z 为高度（→ Bevy +Y），VOX 的 Y 为宽度（→ Bevy Z），VOX 的 +X 为车头方向（→ Bevy −X）。
- 尺寸约定：下表 X 为车长方向、Y 为高度、Z 为车宽方向；坦克车头朝局部 −X（`src/driving.rs`）。

## 部件尺寸一览

| 部件 | 网格尺寸（VOX X×Y×Z，格） | X 长 | Y 高 | Z 宽 | 权威定义 | 备注 |
|---|---|---|---|---|---|---|
| 玩家坦克（碰撞框） | 车体及履带相位画布135×36×26 | 7.5 | 2.6 | 3.6 | `COLLISION_MIN/MAX`（`src/player_model.rs`）→ `TANK_SIZE` / `STEP_CLIMB_RADIUS`（`src/driving.rs`） | 整车圆角箱形碰撞体（圆角 0.2 m，能翻 1 档 0.1 m 台阶，总尺寸不变）；`body.vox`（含炮塔）+ `barrel.vox` + 玩家履带包；含炮管视觉全长 10.5 m，炮管不碰撞，见 `docs/player-model.md` |
| 敌人坦克（碰撞框） | 车体及履带相位画布114×34×26 | 6.6 | 2.6 | 3.4 | `COLLISION_MIN/MAX`（`src/enemy_model.rs`）→ `ENEMY_SIZE`（高度复用 `TANK_SIZE.y`） | 非炮管圆角整车框（圆角0.2 m）；车身、炮管、履带轮组三包，含炮管视觉全长9 m，见 `docs/enemy-model.md` |
| 子弹 | 4×4×4 | 0.4 | 0.4 | 0.4 | `BULLET_SIZE`（`src/firing.rs`） | 立方体；子弹性 `assets/bullet.vox` |

坦克与敌人的视觉包围盒有集成测试断言，与上表一致（`src/firing.rs` 中 `real_vox_assets_fire_and_clean_up_scene_instances`）。

## 驾驶与履带

- 玩家与敌人共用 `body_for_size()` 刚体和驾驶系统，均保留重力与碰撞响应；全轴旋转锁定，偏航只由驾驶系统积分。
- `DrivingInput` 是逐车驾驶意图：`throttle`、`turn` 均为 −1..=1，正值分别表示前进、左转。前进 6 米/秒、倒车 4 米/秒，最大转向 90°/秒，转向平滑时间 0.15 秒。
- `TurnRate` 为逐车转向状态。只有玩家 `Tank` 接收方向键；`EnemyTank` 默认零输入，没有 AI 或自动移动，但可受碰撞产生短暂位移。
- `TrackSpec` 指定每辆车的履带包与整车尺寸，新增组件时挂载一次。`TrackAnim` 独立保存左右累计里程与相位；直行、倒车及原地转向均使用共享动画逻辑，半履带距取各车宽度的一半。
- lugged 包含 `track_l_0..2` / `track_r_0..2` 六个节点，齿距 3 格，每行驶 1 格（0.1 米）切换一个相位；不在资源内部设置 `_hidden`。车体和对应履带采用相同模型尺寸与全局坐标，运行时零偏移对齐。
- 凸齿只沿侧面外圈闭环移动，玩家与敌人轮间均透空，轮组按各侧里程独立旋转。VOX +X 是车头：前进时上段向 +X、前端向下、下段向 −X、后端向上，倒车反向；外圈长度为齿距整数倍，保证三相位回绕无接缝跳齿。
- 玩家履带包另含 `wheel_l_0..6` / `wheel_r_0..6`：每侧五个直径0.8米负重轮、两个直径0.5米端轮；局部画布按轴心居中，轮位及滚动半径共用 `src/player_model.rs::WHEELS`。轮角为对应侧里程÷半径，绕游戏+Z轴，停车保持。履带主体/顶齿高1.2/1.3米，裙板离地1.4～1.6米。
- 敌人生成器 `scripts/generate_enemy.py` 读取 `src/enemy_model.rs`，生成暗红车身、独立炮管和履带轮组三包；每侧4个直径1 m负重轮、2个直径0.6 m端轮，轮宽0.4 m，轮轴高0.65/0.7 m。履带主体/顶齿高1.3/1.4 m，闭环150格，侧裙高1.5～1.7 m。`TrackSpec` 以车型轮规格切片和画布安装轮组；敌人击毁时20个场景实例全部注销。

## 场地与竞技场

| 部件 | 尺寸 | 权威定义 |
|---|---|---|
| 场地落地尺寸 | 600 × 600 m，面积 360000 m² | `FIELD_SIDE` / `AREA`（`src/ground.rs`） |
| 地形高度图 / 土色图 | 各 1200 × 1200 采样，作者化间距 0.5 m/采样；高度图 1440000 字节，土色图另含文件自带调色板（`[数量 u8][数量×3 RGB][索引矩阵]`） | `MAP_SIDE` / `AUTHOR_RES` / `parse_albedo()`（`src/ground.rs`）；产物 `assets/terrain/ground/ground.ht`、`assets/terrain/ground/ground.abd` |
| 地形数据来源 | 标准库脚本按固定种子生成两张紧凑字节图 | `scripts/generate_ground.py`（`PALETTE` 随 `.abd` 文件写出，Rust 侧从文件读取） |
| 地形起伏 | ±1 m，量化到 0.1 m 一级（平地字节值 10，0/20 对应 −1.0/+1.0 m） | `TERRAIN_HALF_RANGE` / `TERRAIN_STEP` / `FLAT`（`src/ground.rs`） |
| 地形视觉网格 | 600 m 场地按 10 × 10 块编译（每块 60 m / 120 采样），顶面贪心合并、边界生成 0.1 m 台阶竖直面 | `CHUNK_SIDE` / `CHUNK_COUNT` / `CHUNK_CELLS` / `build_chunk()`（`src/ground.rs`） |
| 地形分布 | 少量大片区 + 若干小片区，往返旋转椭圆；非平地覆盖 25%–45%（目标 26%–34%）；出生点周围 8 m 强制平地 | `COVER_RANGE` / `BIG_DIAMETER` / `SMALL_DIAMETER` / `SPAWN_PAD_M`（`scripts/generate_ground.py`） |
| 地形物理表面 | 体素台阶复合碰撞体：每 60 m 分块一个 Avian compound，cuboid 由同档位（忽略颜色）贪心合并矩形生成，顶面量化 0.1 m、底面统一 −1.1 m，侧面即竖直坎；无高度场插值隐形斜坡 | `Collider::compound` + `collider_cuboids()` / `COLLIDER_BASE_Y`（`src/ground.rs`） |
| 竞技场围墙 | 四面；厚 2 m、高 8 m，横竖墙长 = 边长 + 4 m，内侧与场地边缘对齐 | `WALL_THICKNESS` / `WALL_HEIGHT`（`src/ground.rs`） |

地形不再来自 `.vox` 模型：`assets/ground.vox` 已退役删除，视觉与物理均由 `ground.ht` 编译而来，两者同源，因此坑洼与坡道既可见也可开上去。地形资产按 `assets/terrain/<地形名>/` 组织，每个地形目录含 `ground.ht`（高度）与 `ground.abd`（土色）。

## 与尺寸相关的其它常量

| 含义 | 数值 | 权威定义 |
|---|---|---|
| 玩家坦克出生中心 Y | 1.3（= `TANK_SIZE.y / 2`） | `src/main.rs` `setup_scene` |
| 敌人坦克出生中心 Y | 3.0（落地后平地中心约 1.3） | `SPAWN_DROP_Y`（`src/enemy.rs`） |
| 炮口局部坐标（车体根节点下，车头为 −X） | (−6.95, 0.7, 0) | `MUZZLE_POSITION`（`src/firing.rs`） |
| 相机水平后退距离 / 竖直高度 | 16 / 9 | `CAMERA_DISTANCE` / `CAMERA_HEIGHT`（`src/driving.rs`） |
| 相机视线相对车身的上抬角 | 8° | `CAMERA_PITCH_UP_DEGREES`（`src/driving.rs`） |
| 敌人数量 / 出生中心半径、与玩家及彼此最小中心距 | 10 / ±120 / 约8.87 / 约8.42 | `ENEMY_COUNT` / `SPAWN_LIMIT` / `spawn_clearance`（双方水平外接半径之和+`SPAWN_GAP` 1 m，`src/enemy.rs`） |
| 场地面积、子弹射程 | 360000 m² / 2000 m | `AREA`（`src/ground.rs`）/ `BULLET_RANGE`（`src/firing.rs`） |

## 术语对照

| 领域词 | 代码标识 |
|---|---|
| 玩家坦克 | `Tank`（`src/firing.rs`）、`tank_body()`（`src/driving.rs`） |
| 敌人坦克（靶车） | `EnemyTank`、`ENEMY_COUNT`（`src/enemy.rs`） |
| 子弹 | `Bullet`、`Gun`（`src/firing.rs`） |
| 场地 / 地形 | `GroundPlugin`、`GroundMap`、`TerrainChunk`、`TerrainSurface`（`src/ground.rs`）；地形数据由 `scripts/generate_ground.py` 生成 |
| 竞技场边界 | `spawn_walls()`（`src/ground.rs`）：四面围墙；`spawn_arena()` 另加一块平坦物理地面，仅供测试 |
| 跟随相机 | `FollowCamera`、`camera_transform()`（`src/driving.rs`） |
