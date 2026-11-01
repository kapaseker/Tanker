# 模型目录整理（2026-11-01）

## 用户确认的结果

玩家整车正式名称为 **LeoPard2 A6**，沿用 astraH 原始模型与当前运动装配。玩家运行资源迁至 `assets/tank/player/leopard2_a6/`，内部 `astra` 模块与 `prepare_astra.py` 名称保留来源信息。敌方整车 **carapace** 的运行资产统一为 `assets/tank/enemy/carapace/{body,barrel,track}.vox`。

画廊仅保留两个分类、三个模型：子弹 **APFSDS**；坦克 **LeoPard2 A6**、**carapace**。APFSDS 是独立模型，按真实可见几何测量；敌方没有弹丸资源，不生成占位子弹。炮管、履带和车身用于内部装配，不再单独展示。混凝土墩继续用于对局，不在画廊登记。

旧 anvil/lance/cleat 玩家三包、`model/player.rs`、旧专用生成器、测试及 `player_original.vox` 夹具已删除。原始 astraH 文件与历史模型说明保留。弹丸原样迁至 `assets/tank/ammunition/apfsds/bullet.vox`；参考 DM53/DM63 系列 APFSDS 飞行体，未确定具体弹种，不重新制作外观。

## 当前生成与回归入口

```text
python3 scripts/prepare_astra.py --check
python3 scripts/generate_bullet.py --check
python3 scripts/generate_enemy.py --check
python3 -m unittest discover -s scripts -p 'test_*.py'
cargo check
cargo test -- --test-threads=1
cargo run -- --offscreen-test gallery --output .scratch/model-catalog/gallery-new
cargo run -- --offscreen-test detail --output .scratch/model-catalog/detail-new
cargo run -- --offscreen-test combat --output .scratch/model-catalog/combat-new
```

敌方生成器复用 `subdivide_vox.py` 中的配对比对逻辑，不再依赖旧玩家发布脚本；弹丸独立入口保留生成回读、元数据校验与配对发布回滚。当前玩家完整几何、轮组、发射与实例销毁继续由真实资产集成测试覆盖，旧模型专用断言随资产退役。

## 验证记录

- `cargo check`、`cargo build` 通过；串行 Rust 全套 120 项（111 单元、5+4 集成）全部通过。
- Python 全套 42 项通过；`prepare_astra.py --check` 确认 5,205,391 个原始有色体素和调色板保持；弹丸及敌方生成入口的逐字节一致性由全套测试覆盖。所有迁移 VOX、meta 和玩家 validation.json 与迁移前逐字节一致。
- Radeon 780M / Vulkan：`gallery-02` 和 `combat-01` 均退出 0、报告通过。画廊三次往返、分类排列、单卡空位导航、敌方详情、焦点与滚动恢复和实例清理通过；战斗开火、命中与弹丸/爆炸清理通过。
- `detail-02` 最终复测退出 0、报告通过：三个模型的进入/返回、拖拽、缩放、规格与尺寸标签、缺失资源错误处理及实体/纹理/实例清理均通过；已读取三张初始关键帧。首轮三个模型的功能断言均通过，但读取 APFSDS 关键帧发现米制尺寸轴沿用坦克尺度，造成标注过大和裁切。新增回归先复现失败，再将轴线、刻度、间距与标签锚点按模型尺度统一缩小；屏幕文字大小保持不变。新初始帧已读取，APFSDS 三个尺寸标签清晰且完整。
- 首轮 `gallery-01` 在第三轮选中敌车时失败：测试预设只给 Detail 映射了右方向键，Gallery 未映射；已补齐真实键注入及回归断言，重跑 `gallery-02` 通过。失败证据保留，未降低断言或放宽超时。
- 双轴复核：Standards 无遗留问题；Spec 指出的 ADR-0006 旧目录/保留策略已补记修正。
- 网页原型模型数据通过 Node 语法检查。现有 `web/transition.test.cjs` 未运行成功：环境缺少 `playwright`，未新增依赖；网页浏览器交互不作为本次已验证范围。
- 未启动可见游戏窗口。日志仍有既有 ICU4X 中日文分词数据告警；关键帧中文分类、尺寸和标签可见。此次不重复验证驾驶、轮组动作或新美术风格，当前模型外观没有重新制作。

实际引擎关键帧：

[画廊](model-catalog-2026-11-01/gallery.png) · [APFSDS](model-catalog-2026-11-01/apfsds.png) · [玩家整车](model-catalog-2026-11-01/player.png) · [敌方整车](model-catalog-2026-11-01/enemy.png) · [命中](model-catalog-2026-11-01/combat.png)

完整本机证据在 `.scratch/model-catalog/`，其中 `gallery-01`、`detail-01` 是修正过程证据，最终以 `gallery-02`、`detail-02`、`combat-01` 为准。
