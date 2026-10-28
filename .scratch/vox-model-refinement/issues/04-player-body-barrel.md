# 04: 玩家车身和炮管迁移到 0.02 米

Status: done
Type: task
Blocked by: 03

## What to build

在实际对局与预览中使用细分后的玩家车身和炮管，并能与尚未转换的旧履带和子弹正确组合。

上下文：先读 [规格](../spec.md) 和 [执行方案](../plan.md)，以及阻塞任务的结果与证据。状态仅表示计划可执行，本轮没有开始实施。

## Acceptance criteria

- [x] 玩家 body/barrel 各自体素数为粗源 125 倍，配置为 0.02，分块后形状、颜色和中心保持；其余未迁移文件仍是 0.1。
- [x] 澄清旧 VOXEL_SIZE 为参考坐标网格的语义及所有调用；碰撞、车轮安装点、炮口米制位置保持，不将契约数值全部机械乘五。
- [x] 玩家生成流程从原几何到 staging，再走同一转换器输出已验证的 VOX+meta；重新生成不会把粗资产覆盖到 0.02 配置下。

## Verification

- [x] cargo check；运行玩家资源、装配、炮口、预览相关 Rust 测试以及 Python 玩家/转换器测试。
- [x] scene、combat、gallery、detail 离屏按直接相关场景选择执行，比较相同关键帧；检查混合单位装配无缝。
- [x] 从粗源还原彩色占用，核对完整视觉全长 10.5 米与原米制炮口；保存本项前后证据。

## 代码定位（实施前重新核对）

scripts/generate_player.rs 与统一发布入口；玩家 body/barrel 及 meta；src/tank/model/mod.rs 的参考网格语义及其调用；相关资源测试。

## 边界

不改炮管截面、不添加舱盖或凹槽，不调整相机掩盖偏差。源生成器仍可使用粗参考网格。

## Comments

2026-10-28：用户明确本次仅细分，不做美术精修。完成后在此记录命令、结果、证据和遗留项；通过前不把任务记为完成。

2026-10-28 实施记录（agent）：

**改动**
- `src/tank/model/mod.rs`：`VOXEL_SIZE` 改名 `REFERENCE_VOXEL_SIZE`，文档注释明确"始终表示参考格（0.1 米），细分不改动契约数值"；调用点同步改名（track.rs、driving.rs、enemy.rs、firing.rs、model/enemy.rs），数值一律未动。GLOSSARY.md 同步修正过时的全局 VoxLoaderSettings 描述。
- `scripts/subdivide_vox.py`：`convert()` 与 CLI 新增 `--dest` 发布目标（仅与 `--publish` 搭配），发布/备份/回滚针对 dest 配对，单位门与 meta 改写仍读 src 旁 meta；新增 dest 发布与回滚测试。
- `scripts/publish_player.py`（新）：统一入口。rustc 编译生成器→生成 0.1 粗三包→body/barrel 走转换器五倍细分到 0.02→原子发布交付对；track 粗字节直发（meta 非 0.1 拒绝）。`--check` 在 staging 逐字节比对交付 VOX+meta。工作目录建在 `target/` 下（Windows os.replace 需同卷）。
- `scripts/generate_player.rs`：main 每文件写前 `refuse_if_migrated`——交付配对 meta 单位 ≠0.1 时 panic 提示改走 publish_player.py；删除"产物==交付"字节对比测试（细分后必然不成立），余 4 项几何测试。
- `src/app/vox_meta.rs`：清单测试更新为 barrel/body=0.02、其余 0.1。
- `scripts/test_tracks.py`：新增 `read_reference_models`——解析场景树、按 meta 单位分派还原分块坐标到参考格（含降采样颜色冲突断言）；`test_player.py` 全部用例改走命名节点参考格读取，新增全局占用 ∈ 135×36×26 断言。
- `src/gameplay/combat/firing.rs`：测试中子弹网格查找从"全局按半尺寸 find"改为"子弹子树内遍历"——细分后车体分块 AABB 可能与弹体半尺寸巧合，全局查找有歧义（迁移后实测触发误匹配）。断言语义（1.2×0.4×0.4 米、无纹理、base_color [32,35,33,255]）未降低。
- `docs/player-model.md`：生成与验证段改为 publish_player.py 入口，说明粗样/细分/守卫规则。

**命令与结果**
- `python scripts/publish_player.py`：发布成功；body 28610→3576250 体素、barrel 292→36500（均 ×125），产物与 03 证据 `evidence/03-converter/run-213301/` 逐字节一致（Get-FileHash）。
- `cargo check` 通过；`cargo test` 113 项全过（104+5+4）。
- `rustc --edition 2024 --test scripts/generate_player.rs` 4 项过。
- Python 全套 41 项过（含 `publish_player.py --check` 一致性回归）。
- 离屏 RTX 4060/Vulkan 全过：`evidence/04-offscreen/{scene,combat,gallery,detail}`，已读关键帧；scene 帧与 `legacy-scene-02` 基准同帧目视一致，混合单位装配无缝，gallery 断言整车 10.5×3.6×2.6 米、combat 帧确认炮口出弹位置不变。

**遗留**
- 履带/子弹/敌人三包仍为 0.1（按工单范围本工单不动，后续工单处理）。
- 细分资产加载与内存开销增大（body ≈14.3 MB），离屏帧率未见异常；如后续场景实例数上升需关注。

**code-review 评审后修复**（双轴评审其余为判断项，未采纳：meta 解析三份实现系 Rust 独立脚本客观限制；回滚测试与 os.replace 次序耦合、compare_pair 命名维持）
- `publish_player.py`：`ensure_generator` 改为监视整个 `src/tank/model/` 目录（生成器经 #[path] 内嵌 player.rs 契约，原只监视 mod.rs 会漏重编）。
- `test_tracks.py`：`read_reference_models` 的 canvas 改为必填并新增 PLAYER/ENEMY_CANVAS 常量与分块居中奇偶校验，消除敌人包误套玩家画布的隐患。
- `test_player.py`：恢复履带六相位包 SIZE==135×36×26 断言（track 本工单仍粗资产，不应失去该覆盖）。
- 修复后 Python 全套 41 项复跑通过。
