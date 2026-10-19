# Enemy 击毁爆炸动画 + 子弹越界失效

## 1. Summary

当子弹命中敌人坦克时：

1. 敌人实体（含 `RigidBody` + `Collider`）立即销毁，物理刚体随之消失；
2. 在原车体中心位置播放一段 12 帧、约 1 秒的体素爆炸动画（纯表现，不带任何物理组件）；
3. 子弹同帧销毁（本就不穿多个坦克，已有行为，无需改动）；
4. 新增：子弹越过场地边界（±300m）即刻失效，不再穿墙飞出 2000m。

新增资产由 Python 脚本生成（仓库既有的 .vox 生成方式），播完由 Rust 侧的计时器删除整棵爆炸子树。

## 2. Current State Analysis（已核实的现状）

| 事实 | 位置 |
|---|---|
| 场地为 600m 见方，`FIELD_SIDE = 600.0`，四周有静态围墙（内壁 ±300m，带 collider） | [ground.rs](file:///e:/Workspace/Code/Rust/Tanker/src/ground.rs#L22)、[ground.rs L267-L289](file:///e:/Workspace/Code/Rust/Tanker/src/ground.rs#L267-L289) |
| 敌人 = `EnemyTank` + `RigidBody::Dynamic` + `Collider::cuboid(ENEMY_SIZE)` + `WorldAssetRoot(enemy_tank.vox)` + `Transform`，10 辆，在 `TerrainReady` 后生成 | [enemy.rs L69-L111](file:///e:/Workspace/Code/Rust/Tanker/src/enemy.rs#L69-L111) |
| 命中判定在 `move_bullets`：`spatial.cast_shape_predicate()` 形状投射，谓词只认 `EnemyTank`（**不认围墙/地形**），命中即 `despawn` 敌人与子弹，并用 `dying` 列表跳过同帧已销毁目标 | [firing.rs L155-L197](file:///e:/Workspace/Code/Rust/Tanker/src/firing.rs#L155-L197) |
| 子弹射程 `BULLET_RANGE` 远超场地对角线 → 现在会穿过 ±300m 围墙继续飞 | [firing.rs](file:///e:/Workspace/Code/Rust/Tanker/src/firing.rs) 常量区 |
| `commands.entity(e).despawn()` 在 Bevy 0.19 会递归删除后代（无需 `despawn_descendants`） | Bevy 0.19 API |
| `bevy_vox_scene` 判定动画的条件：一个 `nSHP` 里列了多个 model；加载后根实体挂 `VoxelAnimationPlayer`，每帧一个子实体 + `VoxelAnimationFrame(i)`，只有当前帧可见 | [parse_scene.rs L388-L425](file:///c:/Users/PC/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_vox_scene-0.22.0/src/load/parse_scene.rs#L388-L425)、[systems.rs L11-L40](file:///c:/Users/PC/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_vox_scene-0.22.0/src/systems.rs#L11-L40) |
| `VoxelAnimationPlayer` 字段全部 pub：`frames: Vec<usize>`、`frame_rate: Duration`、`repeat_mode`（默认 `Forever`）、`despawn_on_finish`、`is_paused`、`timer`；默认 `frame_rate = 1/8 s`，默认 `repeat_mode = Forever` → **会无限循环** | [components.rs L37-L66](file:///c:/Users/PC/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_vox_scene-0.22.0/src/load/components.rs#L37-L66) |
| 帧推进条件为 `stopwatch.elapsed() > frame_rate`；`Forever` 模式下走完最后一帧会回到第 0 帧 | [components.rs L99-L133](file:///c:/Users/PC/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_vox_scene-0.22.0/src/load/components.rs#L99-L133) |
| 仓库现有 .vox 全部由 Python 手写分块生成并提交；脚本带 unittest（断言字节结构后与磁盘文件比对） | [generate_enemy.py](file:///e:/Workspace/Code/Rust/Tanker/scripts/generate_enemy.py)、[test_generate_enemy.py](file:///e:/Workspace/Code/Rust/Tanker/scripts/test_generate_enemy.py) |
| 通用分块/VOX 组装/调色板/MATL 辅助函数（含 `dictionary()`、`chunk()`） | [generate_ground.py](file:///e:/Workspace/Code/Rust/Tanker/scripts/generate_ground.py) |
| Bevy 0.19 用 `Message`（`#[derive(Message)]`，`add_message::<M>()`，`MessageWriter`/`MessageReader`）；旧 `Event` 已更名为 Message | bevy_ecs 0.19.1 `message/` |
| `EntityCommands::entry()` 在 0.19 **不存在**（备选退路只能用 `commands.queue(|world: &mut World| …)`） | bevy_ecs 0.19.1 |

**结论**：需求 2 中的"子弹不穿多个坦克"已实现，"去掉物理刚体"由 `despawn` 敌人实体一并完成；真正要新增的只有爆炸表现（资产 + 生成 + 计时清理）和子弹越界判定。

## 3. Proposed Changes

### 3.1 新增 `scripts/generate_explosion.py`

职责：生成 `assets/explosion.vox`——12 帧逐帧爆炸动画。

- 常量：`FRAMES = 12`、`SIZE = (40, 40, 40)`、`SHELL_START = 4.0`、`SHELL_END = 20.0`、`SHELL_THICKNESS = 3.0`；
- 单帧几何：第 `i` 帧（`i = 0..11`）取 `t = i / (FRAMES - 1)`，球壳半径 `r = SHELL_START + t * (SHELL_END - SHELL_START)`，保留满足 `abs(|p| - r) <= SHELL_THICKNESS / 2` 的体素（空心球壳，逐帧外扩）；
- 稀疏化：`t > 0.66` 的帧按确定性抖动丢弃约 1/3 体素（`(x + 2*y + 3*z) % 3 != 0`），形成消散感；
- 颜色：每帧一种主色（亮黄 → 橙 → 暗红 → 灰烟），逐帧写入调色板索引；`MATL` 条目沿用既有脚本的 `_type: '_diffuse'` + `_rough` 写法（**不引入发光材质**，避免猜测 `_emit`/`_flux` 键的语义）；
- 场景结构（与 `generate_enemy.py` 同构）：`nTRN(root)` → `nGRP` → `nTRN(_name: 'explosion')` → **一个 `nSHP` 列出 12 个 model**（`model_count = 12`，每个 model 带 `_f` 字典）；
  - ⚠️ `nSHP` 的字节布局是全局最高风险点，必须严格按 dot_vox 的解析顺序写：`node_id u32 + dict + model_count u32 + (model_id u32 + dict) × model_count`；
- 产物路径：`assets/explosion.vox`，随仓库提交（与既有 .vox 一致）；
- 顶层暴露 `build_vox() -> bytes`（测试直接调用），`__main__` 时写盘。

### 3.2 新增 `scripts/test_generate_explosion.py`

沿用 [test_generate_enemy.py](file:///e:/Workspace/Code/Rust/Tanker/scripts/test_generate_enemy.py) 的写法（自解析字节），断言：

1. 文件头 `VOX ` + version 150、`MAIN` 长度自洽；
2. 恰好 12 个 `XYZI`，各帧体素坐标均在 `SIZE` 范围内（`min == 0` 对球壳不成立，故只断上限与"非空"）；
3. 帧体素数随帧号递增（外扩球壳的表面积单调增）；
4. 恰好 1 个 `nSHP`，其 `model_count == 12`；
5. 存在 `RGBA`（1024 字节）与 `MATL`；
6. `build_vox()` 可复现，且与 `assets/explosion.vox` 磁盘内容一致。

### 3.3 新增 `src/explosion.rs`

职责：消费"敌人被毁"消息 → 生成爆炸场景根 → 按动画真实帧数挂计时器 → 到期删除整棵子树。

```rust
/// 爆炸场景资源路径（相对 assets/）。
const EXPLOSION_PATH: &str = "explosion.vox";
/// 单帧显示时长，单位：秒；12 帧 × 1/12 ≈ 1s，与 scripts/generate_explosion.py 对齐。
const EXPLOSION_FRAME_RATE: f32 = 1.0 / 12.0;

/// 爆炸场景根标记；计时到期后整棵子树（含 12 帧节点）随之销毁。
#[derive(Component)]
pub(super) struct Explosion;

/// 爆炸根实体的存活计时；时长由动画组件推导，不等长则不会误留"第 0 帧回闪"。
#[derive(Component)]
struct ExplosionLifetime(Timer);

/// 预加载的爆炸场景句柄：只加载一次，命中时克隆复用。
#[derive(Resource)]
pub(super) struct ExplosionAssets(Option<Handle<WorldAsset>>);

pub(super) struct ExplosionPlugin;
```

系统与 observer：

| 项 | 职责 |
|---|---|
| `load_explosion`（Startup） | `assets.load(EXPLOSION_PATH)` 存入 `ExplosionAssets`，中文 `info!` 日志 |
| `report_explosion`（Update） | 仿 `firing.rs::report_bullet`：`LoadState::Failed` → `error!`；`Loaded` → `info!("爆炸资源加载完成：assets/explosion.vox")` |
| `spawn_explosions`（Update） | 读 `MessageReader<EnemyDestroyed>`，每个消息生成 `(Explosion, Transform::from_translation(position), WorldAssetRoot(handle))`；句柄为 `None` 时 `warn!` 并跳过 |
| `arm_explosion_lifetime`（observer，`On<Add, VoxelAnimationPlayer>`） | 待 3.5 详述：回溯到 `Explosion` 根，插入 `ExplosionLifetime`，并覆盖 `frame_rate` |
| `despawn_finished_explosions`（Update） | 遍历 `Query<(Entity, &mut ExplosionLifetime)>`，`tick(time.delta())`，`finished()` → `commands.entity(e).despawn()`（递归删子树） |

时长公式（3.5 详述）：`duration = EXPLOSION_FRAME_RATE × (frames.len() as f32 - 0.5)`，12 帧 → `11.5 / 12 ≈ 0.958s`。

- **为什么是 `- 0.5` 而不是完整 12 帧**：crate 在 `elapsed > frame_rate` 才推进，且 `repeat_mode` 默认 `Forever`，满 12 帧后会在第 13 次推进时**跳回第 0 帧**（亮黄球体回闪一帧）。计时器落在"最后一帧播放到一半"处，正好在回闪前把整棵子树删掉。此取舍写进代码注释。
- **为什么用 observer 而不是固定常量**：帧数只有一个来源（资产的 `nSHP` model 数 → `frames.len()`），改脚本后 Rust 侧不会漂移。

插件注册：

```rust
impl Plugin for ExplosionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EnemyDestroyed>()          // 生产/消费两侧共用
            .add_systems(Startup, load_explosion)
            .add_systems(Update, (
                spawn_explosions,
                report_explosion,
                despawn_finished_explosions,
            ))
            .add_observer(arm_explosion_lifetime);
    }
}
```

### 3.4 修改 `src/enemy.rs`

新增消息类型（放在敌人域模块，作为生产方 `firing.rs` 与消费方 `explosion.rs` 的共享契约）：

```rust
/// 敌人被击毁的消息；位置为被毁车体中心的世界坐标，供表现层（爆炸）消费。
#[derive(Message)]
pub(super) struct EnemyDestroyed {
    pub(super) position: Vec3,
}
```

理由：`firing.rs` 已经依赖 `enemy.rs`（`EnemyTank`），消息属于敌人域事件；避免 `firing.rs` 反向依赖表现模块，也避免让射击系统直接持有资产句柄。

### 3.5 修改 `src/firing.rs`

1. **新增常量**：`/// 场地边界半径（米），与 ground 的 FIELD_SIDE 保持一致。` —— 直接引用 `crate::ground::FIELD_SIDE * 0.5`；若 `FIELD_SIDE` 当前不是 `pub(super)`，仅调整可见性为 `pub(super)`（单行改动，不改数值）。
2. **命中分支**：`enemies` 查询由 `Query<Entity, With<EnemyTank>>` 改为 `Query<&Transform, With<EnemyTank>>`（`enemies.contains(target)` 依然可用），命中后取车体中心并写消息：

```rust
if let Some(hit) = hit {
    dying.push(hit.entity);
    // 爆炸中心用被击毁车体的世界位置：与 despawn 对象一一对应，且不偏向受击面。
    if let Ok(enemy) = enemies.get(hit.entity) {
        destroyed_enemies.write(EnemyDestroyed { position: enemy.translation });
    }
    commands.entity(hit.entity).despawn();
    commands.entity(entity).despawn();
    ...
}
```

   系统签名新增 `mut destroyed_enemies: MessageWriter<EnemyDestroyed>`。
   > 只有敌人并行销毁、视觉交由 explosion 模块：物理帧内不做任何资产操作，射击系统的测试也不必引入 AssetServer。
3. **越界失效**：在每个子弹循环体的最前面加守卫，先判后动：

```rust
// 子弹越过场地边界即失效；单帧最大位移 BULLET_SPEED/fps ≈ 0.67m，越界误差可忽略。
if transform.translation.x.abs() > half_field || transform.translation.z.abs() > half_field {
    debug!("子弹飞出场地，销毁：{entity}");
    commands.entity(entity).despawn();
    continue;
}
```

   放在移动之前：子弹出生点在场地内部，因此不会误删刚生成的子弹。

### 3.6 修改 `src/main.rs`

```rust
mod explosion;   // 与其他 mod 并列
...
.add_plugins(explosion::ExplosionPlugin)
```

放在 `EnemyPlugin` / `FiringPlugin` 之后，避免帧内系统顺序被 `.chain()` 约束（`add_message` 与 update 系统顺序无关，不确定顺序即并行，安全）。

### 3.7 测试补充

**Rust（`src/firing.rs` 的 `mod tests`）**

- 已有 3 个使用 `move_bullets` 的 App（`hit_test_app` 等）各补 1 行 `.add_message::<EnemyDestroyed>()`。**必须补**：`MessageWriter` 有 `validation_message = "Message not initialized"`，未注册会在系统运行时 panic。
- 新增用例 `bullets_expire_outside_the_field`：一发子弹置于 `x = 299` 朝 +X 飞 → 越界后被销毁；另置于 `x = 299` 朝 -X 飞 → 存活（验证不是误杀）。

**Rust（`src/explosion.rs` 新增 `mod tests`）**

- `explosion_asset_animation_has_twelve_frames_and_self_destructs`：参照 [firing.rs](file:///e:/Workspace/Code/Rust/Tanker/src/firing.rs) 内 `real_vox_assets_fire_and_clean_up_scene_instances` 的做法（真 `AssetPlugin` + `VoxScenePlugin` + 手动推进 `Time`），加载 `explosion.vox`、生成一个 `Explosion` 根，等待若干个 App 迭代直到动画实体出现，然后断言：
  1. `VoxelAnimationPlayer.frames.len() == 12`，且 `ExplosionLifetime` 的时长 ≈ `11.5 / 12` 秒；
  2. 12 个 `VoxelAnimationFrame` 子实体存在，仅第 0 帧可见；
  3. 推进时间超过 1s 后，根实体与全部帧实体均已 despawn。
  - 这一条同时是 §3.1 中"手写 `nSHP` 多模型字节布局"的端到端兜底。

## 4. Assumptions & Decisions

已被你确认：

1. 爆炸资产由脚本生成（`scripts/generate_explosion.py`），不是手工 .vox、不是运行时程序生成；
2. 爆炸播完的收尾 = 根实体挂计时器到期删除整棵子树（不留残骸）；
3. 子弹越界 = 每帧坐标检查 `|x| > 300 || |z| > 300` 即销毁（不做墙碰撞、不改 BULLET_RANGE）；
4. 12 帧 / 1 秒（沿用 1/12 s 帧率）；爆炸位置 = 被击毁车体中心；时长从动画组件的 `frames.len()` 推导。

执行阶段可用的默认判断（无需再问）：

5. 爆炸实体**不带任何物理组件**（不刚体、不碰撞体），纯 `Transform` + `WorldAssetRoot`；
6. 复用同一份已加载资产句柄克隆（`Handle` 是引用计数），每辆敌人不重复 IO；
7. 越界与命中都沿用现有 `debug!`/`info!` 中文日志风格；
8. 新增的中文注释按 AGENTS.md 要求写在结构体、常量、关键分支与公式处（尤其 `- 0.5` 的理由）。

范围外（不做）：

- 不重构 `firing.rs` 两处资产加载为统一 setup（不为两处重复的 12 行新增抽象）；
- 不新增 crate 依赖、不改 `Cargo.toml`、不升级任何依赖；
- 不改敌人生成时序（`TerrainReady` 采样高度的既有方案不动）、不改物理参数、不给子弹加碰撞体；
- 不加残骸、不加音效、不加屏幕震动、不给爆炸加发光材质。

## 5. Verification

```powershell
# 1) 资产生成 + Python 结构自检（cwd 需在 scripts/ 内，测试以同目录导入方式写）
python scripts/generate_explosion.py
python -m unittest test_generate_explosion     # cwd=scripts
# 2) 编译 + 全部测试（含 3 处测试 App 的消息注册改动）
cargo check
cargo test
# 3) 运行验证
cargo run
```

`cargo run` 需确认：

- 日志出现 `爆炸资源加载完成：assets/explosion.vox`（与 AGENTS.md 的"资源加载完成"惯例一致）；
- 击毁一辆敌人：日志 `击毁敌人坦克，剩余 N 辆`，画面在原车体中心播放约 1s 爆炸且**播完不残留**、**不出现亮黄回闪**；
- 子弹朝围墙飞出时，日志 `子弹飞出场地，销毁`，且不再有子弹飞出场地外的画面；
- 一辆坦克被一颗子弹击毁后，该子弹不再命中第二辆（既有行为回归确认）。

画面相关改动按 AGENTS.md 要求实际观察渲染效果后视为完成。

## 6. Risks

| 风险 | 缓解 |
|---|---|
| 手写 `nSHP` 多模型布局与 dot_vox 解析不符 → 资产加载失败或退化为静态模型 | Rust 侧真实资产测试断言 12 帧与 `VoxelAnimationPlayer` 存在（§3.7）；Python 侧断言 `model_count == 12` |
| observer 内使用 `Query<&mut VoxelAnimationPlayer>` 若被借用检查拒绝（`EntityCommands::entry()` 在 0.19 不存在） | 退路：`commands.queue(move \|world: &mut World\| { if let Some(mut p) = world.get_mut::<VoxelAnimationPlayer>(e) { p.frame_rate = ... } })` |
| 资产加载失败 → 爆炸根永远没有动画组件 → 计时器不挂 → 实体残留 | `report_explosion` 用 `error!` 暴露；单场景 10 辆敌人的失败面有限，作为已知取舍记录（有测试兜底格式正确性） |
| 3 个既有 `move_bullets` 测试未注册消息 → 运行期 panic | 已列入 §3.7 必做改动；`cargo test` 即暴露 |

## 7. 关于新增测试的说明（需你确认）

原需求未要求测试。计划包含 Python 与 Rust 各一组断言，理由是仓库已有 `test_generate_enemy.py` 先例、且"手写 VOX 字节布局 + 计时清理公式"属于非平凡逻辑。若你希望只保留 `cargo check` + 手动运行验证，请在批准时指出，我会删去 §3.2 与 §3.7 中的新增用例（仅保留既有测试因消息注册而必须的 2 行改动）。
