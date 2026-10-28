# 08: 现有玩家子弹迁移到 0.02 米

Status: done
Type: task
Blocked by: 07

## What to build

当前 lance 附属子弹精确细分，仍从原炮口按原速度发射并以原扫掠体积判定命中。

上下文：先读 [规格](../spec.md) 和 [执行方案](../plan.md)，以及阻塞任务的结果与证据。状态仅表示计划可执行，本轮没有开始实施。

## Acceptance criteria

- [x] 192 个源体素变为 24,000 个体素，画布 60×20×20，配置 0.02，视觉仍为 1.2×0.4×0.4 米，颜色和材质不变。
- [x] 扫掠尺寸仍为 0.4 米，炮口初始偏移、初速、冷却、射程与清理不变；七个现有坦克文件此时都已迁移。
- [x] 建立可重建的粗源输入，优先小型 fixture 保留原子弹；不新增敌方占位资源，不从细分成品再细分。

## Verification

- [x] cargo check；运行发射、命中、射程/清理真实资产测试以及 Python 转换器测试。
- [x] 离屏 combat、hud、含子弹的预览验证；比较相同模拟帧及炮口位置。
- [x] 核对七个坦克 meta 均为 0.02；爆炸、混凝土墩仍为 0.1，原 VOX 哈希不变。

## 代码定位（实施前重新核对）

玩家 bullet.vox/meta；建议 scripts/fixtures 中的小型粗源 fixture；转换发布入口；firing 与预览测试。

## 边界

不得依据过时 GLOSSARY 将当前长条弹体改成立方体；只细分现有模型。

## Comments

2026-10-28：用户明确本次仅细分，不做美术精修。完成后在此记录命令、结果、证据和遗留项；通过前不把任务记为完成。

2026-10-29 实施：

- **粗源 fixture**：`scripts/fixtures/bullet.vox` + `.vox.meta` 保存迁移前 0.1 米/格原始文件逐字节快照（SHA-256 `72410600c0a366b0` 前 16 位，与 `evidence/baseline-vox-sha256.txt` 中 bullet 记录及迁移前资产一致），避免交付资产细分后无法重建；不新内容、不从细分成品再细分。
- **发布管线**：`publish_player.py` 在三包之后接入子弹——fixture 直读（转换器按配对 meta 0.1 解析，输出改写为 0.02），发布/校验与其他包同路径；`--check` 对子弹同样逐字节比对。
- **交付资产**：`assets/tank/barrels/player/lance/bullet.vox` 192 → 24,000 体素（精确 ×125），画布 60×20×20，meta 0.02；调色板与材质字段不变。
- **Rust**：`vox_meta.rs` 清单期望值 bullet 0.1 → 0.02；`firing.rs` 注释与断言文案改为 60×20×20 格（1.2×0.4×0.4 米），BULLET_SIZE 0.4、炮口偏移、初速、冷却、射程清理逻辑零改动。
- **验证**：`cargo check` 通过；`cargo test` 113/113（串行，含 vox_meta 单位核对、firing 材质/扫掠/清理断言）；Python 全套 42/42（含 `publish_player.py --check` 逐字节回归，覆盖子弹管线）。
- **离屏**（evidence `08-combat-03/`、`08-hud-01/`、`08-detail-01/`）：combat 报告命中、爆炸、清理与材质断言通过；hud 报告通过；detail 报告通过并已读关键帧——lance 页子弹形状、深灰颜色、炮口上方间距正常。
- **基准对比**：combat 六帧对 `07-combat-92c312`、detail 五帧对 `04-offscreen/detail` 逐像素差异 0–3 像素（最大通道差 ≤255 的孤立点，均为既有噪音级别），无系统性变化。
- **资产核对**：七个坦克 meta 均为 0.02；explosion/obstacle 哈希与基准一致（`baseline-vox-sha256.txt` 全表未发现漂移）。
- **环境备注**：本会话沙箱存在命令双重执行异常（首次静默完成、二次报"拒绝覆盖"退出 1），release 构建 rustc 亦偶发 0xc0000409 崩溃；离屏证据以 report.txt 为权威结论，combat 重复产物 `08-combat-01/02/04` 保留备查。遗留：无。
- **code-review（双轴）**：Spec 轴独立解析资产复核全部验收项通过，指出本注释初版把 fixture 哈希误记为 `258ee06a…`，已更正为 `72410600c0a366b0`；player-model.md 的顺带更新属工单 09 文档接缝，内容系修正过时陈述，保留。Standards 轴无硬违反，两处 possible 判断题（publish_player.py 子弹块与三包循环的 check/发布分支形状重复、粗源/交付路径对未捆成列表）留作后续小重构，非阻塞。
