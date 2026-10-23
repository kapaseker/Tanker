# 目录整理迁移基线（任务 01）

2026-10-23 完成基线记录；未修改生产代码、依赖、断言或原资产，未启动可见窗口。
基线提交为 `9a0bf8cb22c2d13efe845dd6b4e3c5bf485568ec`，分支 `master`。
开始时已有未跟踪的 `docs/plans/`，本任务没有修改或纳入提交。
范围来自本机 `.scratch/architecture-reorganization/issues/01-baseline.md` 与
`docs/plans/architecture-reorganization.md` 第 6 节。

原始证据目录为 `.scratch/architecture-reorganization/evidence/20261023-ticket01-01/`
（下称 `EVIDENCE`，被 Git 忽略，仅本机保留）。此文保存可同步的结果及资产清单；
原始日志、报告、PNG、审阅用联系表和隔离副本不随提交同步。

## 环境与执行边界

- Windows / PowerShell；cargo 1.99.1，rustc 1.99.1，Python 3.14.5。
- 7 份场景报告均记录 NVIDIA GeForce RTX 4060 / Vulkan；capture 使用相同入口，单图模式不写报告。
- 固定种子 42，默认 1280×720、1/60 秒模拟步长和 120 帧落地预热；menu/gallery 还测试各自预设尺寸。
- 继承环境 `RUST_LOG=warn`。没有检查其他显卡、Linux 或真实窗口行为。
- `reference-inventory.txt` 保存源码声明、模块引用、生成器路径和文档引用，作为后续搬迁对照。
- 本步骤没有新增逻辑或测试，运行现有测试，不启动 TDD 新测试流程。

## 编译与逻辑结果

| 命令 | 退出码 | 实际结果与日志 |
|---|---|---|
| `cargo check --locked` | 0 | 通过；`cargo-check.log` |
| `cargo test --locked` | 0 | 60 passed，0 failed/ignored；`cargo-test.log` |
| `python -m unittest discover -s scripts -p "test_*.py"` | 1 | 隔离副本执行，12 项（含导入失败占位项），2 errors；`python-tests.log` |
| `python -X utf8 -m unittest discover -s scripts -p "test_*.py"` | 1 | 编码诊断重跑，13 项，1 error；`python-tests-utf8.log` |
| `rustc --edition 2024 -O scripts/generate_player.rs -o target/generate_player-check.exe` | 0 | 独立编译通过，3 条 dead-code 警告；`generator-compile.log` |
| `target/generate_player-check.exe EVIDENCE/generator-output` | 0 | 自定义根目录生成车身、炮管、履带三包，与原资产 SHA-256 全部相同；`generator-output.log`、`generator-comparison.txt` |
| `git diff --check` | 0 | 基线工作区检查通过；提交前再次检查 |

Python 命令的工作目录是 `EVIDENCE/python-workspace`，其中完整复制了 `scripts/`、`src/`、`assets/`。
原因是 `test_generate_ground.py::setUpClass` 会调用 `ground.write_assets()`；直接在原工作区运行会写原资产。
未修改测试或生成器；副本保留相对路径关系。原工作区 16 个已跟踪资产的运行前后哈希完全一致，
分别保存在 `assets-before.sha256` 与 `assets-after.sha256`。
精确命令、退出码见 `results.csv`、`python-results.csv`；后者部分补充检查未计时，Seconds=0 不是实际耗时。

## 已有失败、诊断与非致命日志

1. 默认 Windows 编码为 GBK，`scripts/generate_enemy.py:15` 用无 encoding 参数的
   `read_text()` 读取 UTF-8 的 `src/enemy_model.rs`，导入时出现 `UnicodeDecodeError`。
   `-X utf8` 重跑后敌人模块的两项测试能够执行；这是编码环境兼容问题。
2. `scripts/test_generate_ground.py:30` 仍读取 `ground.SPAWN_PAD_M`，当前生成器只有
   `SPAWN_CLEAR_M`，触发 `AttributeError`。默认编码及 UTF-8 两次运行均复现；这是现有测试与生成器契约不一致，
   不能归因于后续迁移，也不能把 Python 全套记为通过。
3. menu/gallery 日志含 `ICU4X data error: No segmentation model for complex script: Chinese/Japanese`。
   两个入口的断言、保存和退出码仍通过，已读取文字与页面关键帧；未调查该日志的根因，不能据此断言所有中文排版能力正常。
4. Cargo 报增量缓存硬链接失败并回退复制；未阻断 check/test/run。

没有 GPU 不可用或离屏启动阻碍；以上失败和日志均在不改生产代码的基线上观测到。

## 全部离屏入口

每行从工程根目录执行 `cargo run --locked --` 后的参数如下。
实际命令中的 `EVIDENCE` 为前述完整相对路径；完整命令、退出码和耗时保存于 `offscreen-results.csv`。
每项日志为 `EVIDENCE/<名称>.log`；除 capture 外，每项输出目录均有 `report.txt`。

| 名称 | 参数 | 退出码 | 结果 |
|---|---|---|---|
| capture | `--capture EVIDENCE/capture.png` | 0 | 单图保存成功；无 report.txt |
| scene | `--offscreen-test scene --output EVIDENCE/scene` | 0 | 地形、落地、相机与资产就绪通过 |
| drive | `--offscreen-test drive --output EVIDENCE/drive` | 0 | 60 帧直行约 6 米，左右履带均约 6 米；180 帧转弯偏航断言通过 |
| combat | `--offscreen-test combat --output EVIDENCE/combat` | 0 | 子弹飞行、单个敌人击毁、爆炸与命中子弹清理通过 |
| wheels-player | `--offscreen-test wheels --output EVIDENCE/wheels-player` | 0 | 玩家 5+2 轮，前后进、双向转弯及停车断言通过 |
| wheels-enemy | `--offscreen-test wheels --output EVIDENCE/wheels-enemy --model enemy` | 0 | 敌人 4+2 轮，共用轮组输入与停车断言通过 |
| menu | `--offscreen-test menu --output EVIDENCE/menu` | 0 | 三轮进入/返回、GPU 就绪、实体清理、输入与尺寸预设通过 |
| gallery | `--offscreen-test gallery --output EVIDENCE/gallery` | 0 | 三轮进入/返回、预览实例及纹理清理、滚动、自转与尺寸预设通过 |

未发现资源缺失或渲染管线失败；保留上述 ICU4X 非致命错误日志，不把返回 0 等同于日志无错误。

## 关键帧读取记录

已读取由所有 106 张原始 PNG 生成的 12 张带文件名联系表（`visual-*.jpg`），
用来逐组观察静态画面和时序变化；缩略图不能代替细节级像素或连续动画验证。

- capture、scene 第 0 帧和 drive 第 0/60/180 帧：绿色玩家、红色敌人和地形可见，随直行及转弯相机保持跟随。
- combat 第 0/6/24/48/60/120 帧：靶车消失，爆炸由黄色亮团变为红色体素球，120 帧已清理。
- 两种 wheels 的 bounds/comparison/front/rear/side 及所有帧：玩家每侧五个大轮、两个端轮，敌人四个大轮、两个端轮；
  辐条角度随帧变化、正反向运动可见，240/250 帧停车姿态保持。包围框图为既有预设的调试叠加。
- menu 所有帧与 layout/hover/keyboard-focus/drift/control 图片：两项菜单、高亮、按键提示及背景方块可见；
  第 36/228 帧为预期过渡黑场，后续帧回到场景或首页，没有持续黑屏。
- gallery 所有图片：部件和整车预览、滚动后内容、自转、悬停边框及尺寸布局可见；
  enter/return 的 phase-1 为预期黑场，phase-2 和 return-menu 已恢复页面。

这些是本次人工读取联系表的观察，加上既有场景断言；不是跨平台一致性保证或正式 UI 设计符合性审计。

## 已跟踪资产 SHA-256

以下 16 项在运行前后相同。仅生成到隔离目录的玩家三包另做了相同哈希比较。

```text
6108E5872E3E23DB1A6635FBCFC4685CA4D57995D62C3106E46FDD04282724AE  assets/effects/explosion.vox
0DDEF90648998900175CFDCA9A6F087A2544C182F130B0AD4F7E94A03A115E79  assets/fonts/misans/MiSansVF.ttf
E133B1601BEF523AF48F3BE724840395F1BE37CBE036C623068C91976C56B90B  assets/menu/title.png
3094075B9DB292F17333B089AA9C2B2B1AC31AA6CCEB6CED3F849F3AAFCF09C2  assets/pisa_diffuse.ktx2
B5EDDBB37839319B5604A7BD97C6916E9351F73D872138D91B0917169285E42D  assets/pisa_specular.ktx2
7953B1CBE415BA4E601BD83CE920DD1E8A01B851EB65012F5BB2BEC7E041F006  assets/shaders/menu.wesl
0621406655A756ED302B3159B6A1D72EB53855BB5E90F1E9368E38688DE6D072  assets/study.vox
FD30AD4297273EE0CEF70B445721EDF87F6E160C624E951E2C7CB3AADB9A0846  assets/tank/barrels/enemy/spit/barrel.vox
5B2C6715D941236516D1E626D931AF9722DC9A8E1B31FE4EF094E687236E6398  assets/tank/barrels/player/lance/barrel.vox
481245DBFBF88B8395697864DC4CCC10E211C344038E850817E548741BB770AA  assets/tank/barrels/player/lance/bullet.vox
1877030EFEECDBD279995161C965647E2CBB4E7F8D0A7DC38C0F5E8CE8E24AEE  assets/tank/bodies/enemy/carapace/body.vox
52ACC6A1B3F16404A7C5D96AD01B4804AE91C153918759A50D6EFF6D59A7ADE2  assets/tank/bodies/player/anvil/body.vox
856004C855E1715A5F65316B7A7883FBEE2511D533052004C2111636D81C48E9  assets/tank/tracks/enemy/crawl/track.vox
277B2A537A8563119FB7CB65FBBD60B52DA71ACD731D45508258E334D08736C9  assets/tank/tracks/player/cleat/track.vox
F931E3BFA1E8EE73F4C738FEF41D983C06897FE2EB7FC7806885835EF790AEF8  assets/terrain/ground/ground.abd
6262BC27F2DA7CDB626B9CFBFDFE73B4E561F187A23867EBCBC5A10D68ADC99E  assets/terrain/ground/ground.ht
```

任务 01 的完成表示基线矩阵已执行、失败和限制已记录，不表示全部测试通过。
后续迁移应逐项对照本记录，不能用已有 Python 失败掩盖新失败。
