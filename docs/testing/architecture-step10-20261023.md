# 任务 10：架构整理最终验收

2026-10-23，从 `e42d39967b4f197817f3610ef2d5e46da76bb4ba` 开始，当前分支 master。本任务只更新导航、ADR、计划和验收记录，无 Rust、依赖或资产修改。

## 路径与依赖检查

修改前枚举 src、scripts、docs、GLOSSARY 的模块声明、导入、重导出和路径引用。main 只保留七个新模块入口；无旧根模块兼容层。四处重导出均为职责接口，保留原因及测试构建的跨领域引用见 [ADR-0002](../adr/0002-module-boundaries.md)。生产 tank/terrain/ui 无 app/pages/gameplay/validation 依赖，业务不引用验证 Options。画廊只注册预览与交互，不注册驾驶或武器；测试引用身份组件用于拒绝意外业务组件。

生成器路径：generate_player.rs 的 #[path] 指向 tank/model/mod.rs，generate_enemy.py 读取 tank/model/enemy.rs；Python 测试按 scripts 目录发现。模型文档路径已经迁移，本次补全职责导航；GLOSSARY 的出生位置导航修正为 gameplay/session::setup。历史基线、问题记录与计划问题表保留旧路径，新增当前入口导航；不改写历史执行结果。

## 命令与结果

原始证据：`.scratch/architecture-reorganization/evidence/20261023-ticket10-01/`（仅本机）。results.jsonl 保存完整命令、cwd、退出码与耗时，各项同名 log 保存输出。

| 检查 | 退出码 | 实际结果 |
|---|---|---|
| cargo check --locked | 0 | 通过 |
| cargo test --locked | 0 | 63 passed，0 failed/ignored |
| python -m unittest discover -s scripts -p "test_*.py" | 1 | 12 项、2 errors，与基线相同 |
| python -X utf8 -m unittest discover -s scripts -p "test_*.py" | 1 | 13 项、1 error，与基线相同 |
| rustc --edition 2024 -O scripts/generate_player.rs -o target/generate_player-check.exe | 0 | 独立编译通过 |
| target/generate_player-check.exe EVIDENCE/generator-output | 0 | 隔离生成三包，SHA-256 全部相同 |
| cargo run --locked -- --capture EVIDENCE/capture.png | 0 | 单图成功，无报告 |
| --offscreen-test scene --output EVIDENCE/scene | 0 | 场景/地形/相机通过 |
| --offscreen-test drive --output EVIDENCE/drive | 0 | 直行/转弯/履带通过 |
| --offscreen-test combat --output EVIDENCE/combat | 0 | 命中/爆炸/清理通过 |
| --offscreen-test wheels --output EVIDENCE/wheels-player | 0 | 玩家轮组通过 |
| --offscreen-test wheels --output EVIDENCE/wheels-enemy --model enemy | 0 | 敌人轮组通过 |
| --offscreen-test menu --output EVIDENCE/menu | 0 | 反复进出、GPU 就绪与清理通过 |
| --offscreen-test gallery --output EVIDENCE/gallery | 0 | 预览、滚动、自转与反复清理通过 |
| git diff --check | 0 | 通过 |

七个场景参数均使用 cargo run --locked -- 前缀；EVIDENCE 为上述目录。七份 report.txt 均为通过，GPU 为 NVIDIA GeForce RTX 4060 / Vulkan，保留种子42、固定步长和原有预热/超时规则。没有启动可见窗口。

Python 在完整复制 scripts/src/assets 的 python-workspace 隔离副本执行，因为地形测试会写资产。默认编码下敌人契约 read_text 的 UnicodeDecodeError 和地形 SPAWN_PAD_M 缺失均复现；UTF-8 消除编码错误，地形测试仍报 AttributeError。未降低断言或夹带修复，Python 全套未通过。

## 画面与资产

读取联系表中的静态场景、drive 0/60/180、combat 0/6/24/48/60/120、双方轮组多个相位和侧视图，以及全部菜单/画廊过渡、滚动、自转和尺寸预设。模型、材质、地形、相机、爆炸消散、布局及黑场衔接无可见回归。

pixel-comparison.json 对与任务01同相对路径的102张图片做RGB比较，101张完全相同，另一张为画廊图；像素比较不是画面验收替代。部分基线图片命名不同，没有纳入逐像素比较。截图阅读为关键帧/联系表检查，不是连续动画或跨平台证明。

assets-before.json 与 assets-after.json：16个已跟踪资产 SHA-256 完全相同，并与任务01基线清单相同。generator-comparison.json：玩家隔离生成的三包与当前资产完全相同。未运行生成器覆盖原资产。

## 限制

按 code-review 分 Standards / Spec 独立评审：Standards 无发现；Spec 指出本地工单状态尚未收尾，已同步为 done 并记录实际失败。原始评审记录保存在本机证据目录。

未执行 Linux、其他显卡或真实窗口键鼠/缩放验证。保留基线 ICU4X 分词错误日志；Cargo 硬链接回退复制警告未阻断。没有新逻辑，不新增测试或 TDD 接口；运行原有完整测试和实际离屏入口完成验收。完成表示范围及验收执行完毕，不表示 Python 已全绿。
