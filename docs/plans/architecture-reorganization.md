# Tanker 目录与职责整理计划

- 日期：2026-10-23
- 状态：任务 01–10 已执行；最终验收含既有 Python 失败，详见文末执行记录。
- 范围：源码目录、模块职责、调用关系及受影响的生成器和文档。
- 执行任务：本机 `.scratch/architecture-reorganization/issues/`，一任务一文件。
- 持久入口：本文件纳入版本控制范围；本地工单被 Git 忽略，不随仓库同步。换机后可按本文任务表恢复。
- 下文保留原计划的问题分析与验收要求；实际结果以文末记录和各验收文档为准。

## 1. 目标与约束

让新增功能有明确归属，修改一次功能时尽量集中在所属目录，并消除通用能力对玩家、页面和验证入口的反向依赖。

保留单个 Cargo package、当前依赖版本、资源布局及所有现有玩法、视觉和命令行行为。继续使用 Bevy ECS 与现有插件组织方式，不引入新依赖、通用服务框架、换装系统、AI 或性能优化。此次“优化”指可维护性整理，不承诺运行性能提升。

不按 components/systems/resources 技术类型横向拆分整个工程；不建立没有具体职责的 common/utils；不为每个文件新增 Plugin。单元测试留在所属模块，只有真实跨模块行为才增加集成检查。目录树是目标导航，只有实际承载职责的文件才创建。

现有 ADR-0001 的坦克部件资产规则继续有效；不搬迁 assets 下的模型或改名变体。UI 排版、样式、字体和交互保持不变；涉及页面代码时重新读取 design.md。

## 2. 当前问题与来源

| 当前文件 | 问题 | 处理方向 |
|---|---|---|
| src/main.rs | 应用组装、状态、场景生成、玩家模型和相机配置混合 | 入口只处理启动与退出；职责分到 app/gameplay |
| src/driving.rs | 共用驾驶、玩家输入、跟随相机混合，依赖 firing::Tank 和过渡状态 | 驾驶下沉 tank；输入和相机归 gameplay |
| src/firing.rs | 玩家标记、视觉炮管挂载和战斗混合 | 玩家身份归 player，表现归 tank，射击归 combat |
| src/transition.rs | 合成和过渡同时负责战斗就绪细节及对局清理 | 过渡协调流程，对局汇总自身就绪和清理 |
| src/player_model.rs、src/enemy_model.rs | 共用 WheelSpec 定义在玩家模型中 | 提到 tank/model 的共用契约 |
| src/track.rs | 共用履带规格引用驾驶模块的玩家尺寸和敌人模块的敌人尺寸 | 模型尺寸归模型契约，表现不依赖敌人生成 |
| src/enemy.rs | 业务直接读取 offscreen::Options 和验证随机种子 | 由启动入口注入生成所需随机配置 |
| src/gallery.rs、src/gallery/ui.rs | 页面同时承担预览资源和交互布局 | 页面编排、模型预览、UI 分开 |
| src/ground.rs | 地图解析、网格、碰撞和生命周期集中 | 分离数据与生成实现，保留统一地形入口 |
| src/offscreen.rs | 参数、渲染器、截图与六种场景集中 | 验证基础设施与场景分开 |

这些是源码静态分析，不是运行时故障结论。文件大小包含内联测试，不以行数作为拆分验收指标。

## 3. 目标结构

```text
src/
├── main.rs
├── app/
│   ├── mod.rs                 # 启动组装与窗口/离屏分支
│   ├── state.rs               # 页面状态
│   └── transition.rs          # 过渡、合成、输入放行
├── pages/
│   ├── mod.rs
│   ├── menu/
│   │   └── mod.rs
│   └── gallery/
│       ├── mod.rs
│       ├── preview.rs
│       └── ui.rs
├── gameplay/
│   ├── mod.rs
│   ├── session.rs             # 对局生成、业务就绪、清理
│   ├── player.rs              # 玩家身份、生成与输入
│   ├── enemy.rs               # 敌人生成、销毁消息
│   ├── camera.rs
│   └── combat/
│       ├── mod.rs
│       ├── firing.rs
│       └── explosion.rs
├── tank/
│   ├── mod.rs
│   ├── driving.rs
│   ├── visual.rs
│   ├── track.rs
│   └── model/
│       ├── mod.rs             # WheelSpec、体素尺度及共用几何契约
│       ├── player.rs
│       └── enemy.rs
├── terrain/
│   ├── mod.rs
│   ├── map.rs
│   ├── mesh.rs
│   └── collider.rs
├── ui/
│   ├── mod.rs
│   ├── font.rs
│   └── resources/             # 保留现有 colors/dimens/strings 结构
└── validation/
    ├── mod.rs
    ├── cli.rs
    ├── offscreen.rs
    ├── capture.rs
    └── scenarios/
        ├── mod.rs
        ├── scene.rs
        ├── drive.rs
        ├── combat.rs
        ├── wheels.rs
        ├── menu.rs
        └── gallery.rs
```

scripts、web、assets 保留现有位置。docs/plans 保存计划，docs/adr 保存正式决策，docs/testing 保存运行证据与限制。

## 4. 依赖与公共接口

- app::mod 是组装入口，可以连接页面、对局、通用能力和验证入口。其他模块不得调用应用启动函数。
- app::state 和过渡流程接口供页面、对局接入；它们不是通用坦克和地形的依赖。
- pages 与 gameplay 是不同使用场景，均可复用 tank、terrain、ui；画廊不加载玩家输入、敌人生成、武器和物理驾驶。
- tank::model 是尺寸、轮位和模型作者化数据的权威来源；保持生成器所需的纯标准库契约。不增加运行时资源配置层。
- tank::driving 消费 DrivingInput，不识别玩家身份、键盘、GameState 或 Fade。玩家控制器在过渡允许输入时写入意图，未来其他控制器可使用相同输入。
- tank::track 和 tank::visual 使用模型契约，不依赖 gameplay::enemy 或 combat。驾驶尺寸由相同契约推导，避免复制常量。
- terrain 自己提供加载、就绪和清理入口，gameplay::session 调用这些入口；terrain 不依赖 SessionEntity 或页面状态。地形根实体可由本模块拥有的标记管理。
- gameplay::session 拥有对局实体标记和完整销毁流程，聚合坦克、地形、武器就绪状态。transition 协调切换并调用聚合入口，不逐项了解敌人数和履带数。
- 真实 GPU 管线与纹理就绪检查继续保留在渲染/过渡流程中，不能用“业务资产加载完成”替代。
- 业务不引用 validation。普通随机和固定种子通过实际需要的生成配置注入；不把完整验证 Options 传给敌人系统。
- EnemyDestroyed 继续归属敌人领域，由战斗生产、爆炸消费；不为此次重构泛化消息总线。
- 子模块默认私有，优先 pub(super)，只有实际跨领域调用点使用 pub(crate)。检查搬迁导致的 pub(super) 语义变化，不靠全面公开声明解决编译问题。

最终应消除上述反向依赖。迁移中可暂时保留旧路径重导出，但必须在收尾删除，不形成永久双入口。Rust 单 crate 不会自动禁止逻辑循环，需要在最终引用检查中验证规则。

## 5. 任务顺序与完成标准

每个任务保持可编译；纯搬迁与职责调整分开。表中的文件是职责范围，调用点路径更新必须在同一步完成，不能留下半迁移状态。若一次任务同时涉及两个独立行为调整，执行前再拆为更小工单。

| 编号 | 内容与主要文件 | 依赖 | 验收条件与验证 |
|---|---|---|---|
| 01 | 记录迁移基线：Cargo、Rust/Python 测试、现有全部离屏场景、资产哈希 | 无 | 保存命令、退出码、GPU/后端、报告与关键帧检查；区分基线失败及环境阻碍；不把已有失败算成迁移引入 |
| 02 | 提取 app 组装与状态：main.rs → app/mod.rs、state.rs；transition 先原样搬入 app | 01 | main 保留启动/退出逻辑；插件注册及系统顺序不变；cargo check/test，menu/gallery 离屏验证 |
| 03 | 集中模型契约：player_model、enemy_model → tank/model，提取 WheelSpec 和尺寸来源 | 02 | 所有使用者同源；生成器路径同步；cargo check/test、Python 全套、玩家生成器独立 rustc 编译；资产哈希不变 |
| 04 | 整理共用坦克能力：driving、track、tank_visual → tank；输入、相机 → gameplay | 03 | 玩家标记从 firing 提到 player；通用驾驶/表现不再依赖玩家控制、敌人生成或过渡；驾驶及轮组测试，drive/wheels（玩家与敌人）离屏验证 |
| 05 | 整理战斗与敌人归属：enemy → gameplay，firing/explosion → gameplay/combat | 04 | 保留射击时序、EnemyDestroyed、冷却、场景实例回收与失败处理；cargo check/test，combat 离屏验证 |
| 06 | 集中对局生命周期：setup_scene、清理和业务就绪 → gameplay/session；注入随机配置 | 05 | 地形使用自身清理入口；transition 不逐项读取战斗细节；业务不引用验证 Options；固定种子保持一致；新增必要的就绪/重复退出逻辑检查，scene/menu/gallery/combat 离屏验证 |
| 07 | 整理页面与 UI：menu/gallery → pages，font/ui_resources → ui；画廊提取 preview | 06 | token、文案和 design.md 规则不变；画廊只使用视觉能力；cargo check/test，menu/gallery 离屏验证并对照基线关键帧 |
| 08 | 拆分地形实现：ground → terrain 的 map/mesh/collider 和入口 | 07 | 地图解释、量化、网格及碰撞几何不变；不改地形算法；cargo check/test、地形 Python 测试，scene/drive 离屏验证 |
| 09 | 拆分验证入口：offscreen → validation 的 cli/offscreen/capture/scenarios | 08 | 不改既有命令、种子、步长、预热、超时、输出拒绝覆盖及失败退出；保留 CLI/像素/文件输出已有测试；cargo check/test 与全部离屏入口验证 |
| 10 | 清理临时重导出，更新术语导航、模型/测试文档和架构决策，完成最终回归 | 09 | 无旧源码路径和反向依赖；完整验收矩阵通过，或明确列出未通过及阻碍；本计划和工单状态与真实执行一致 |

依赖链：01 → 02 → 03 → 04 → 05 → 06 → 07 → 08 → 09 → 10。

检查点：

- 03 后：应用仍可运行，模型和生成器仍遵循原契约。
- 06 后：共享坦克能力与对局/页面解耦，重复进出和资源清理正常。
- 09 后：验证工具自身的命令行和输出保证不变。
- 10 后：删除迁移过渡层，所有目录规则和运行结果均有证据。

检查点用于验收，不自动创建提交、推送或启动可见窗口。每个逻辑步骤应便于独立审查；提交和推送需要用户另行授权。

## 6. 验证命令与证据

### 编译与逻辑

从工程根目录执行：

```text
cargo check --locked
cargo test --locked
python -m unittest discover -s scripts -p "test_*.py"
rustc --edition 2024 -O scripts/generate_player.rs -o target/generate_player-check.exe
git diff --check
```

check 是每次 Rust 修改的必需项；测试按任务选择相关范围，最终收尾再跑完整集合。Python 和 rustc 的具体可用工具路径在基线阶段确认。生成器检查不得意外重写现有资产：比较内存生成结果或输出到独立临时目录；先检查生成器是否支持自定义输出。不能隔离输出时只编译并如实记录生成验证缺口，不新增生成器功能来扩大范围。

### 无窗口运行与画面

用每次唯一的输出目录，以下 RUN_ID 是需要替换的占位符，不能复用已有产物：

```text
cargo run --locked -- --capture .scratch/architecture-reorganization/evidence/RUN_ID/capture.png
cargo run --locked -- --offscreen-test scene --output .scratch/architecture-reorganization/evidence/RUN_ID/scene
cargo run --locked -- --offscreen-test drive --output .scratch/architecture-reorganization/evidence/RUN_ID/drive
cargo run --locked -- --offscreen-test combat --output .scratch/architecture-reorganization/evidence/RUN_ID/combat
cargo run --locked -- --offscreen-test wheels --output .scratch/architecture-reorganization/evidence/RUN_ID/wheels-player
cargo run --locked -- --offscreen-test wheels --output .scratch/architecture-reorganization/evidence/RUN_ID/wheels-enemy --model enemy
cargo run --locked -- --offscreen-test menu --output .scratch/architecture-reorganization/evidence/RUN_ID/menu
cargo run --locked -- --offscreen-test gallery --output .scratch/architecture-reorganization/evidence/RUN_ID/gallery
```

每项检查退出码、report.txt（单张 capture 除外）、资源错误和 GPU 后端。读取场景、左右转向/履带、开炮命中与爆炸清理、菜单过渡及画廊往返关键帧，与基线比较模型、相机、材质、布局和黑场衔接。不能用“图片非纯色”或启动成功代替通过。

保留固定种子 42、1280×720、现有固定步长、120 帧落地预热及超时规则。保持系统调度的 chain/after/before、状态进入退出时机与场景实例清理顺序。跨 GPU 不要求逐像素相同。

离屏环境不可用时记录阻碍，不回退可见窗口。drive 失败时先读 docs/testing/offscreen-findings.md，保留证据，不降低断言或夹带游戏碰撞修复。不同原因的基线失败必须单独记录，不能一概视为已知问题。

### 路径和资产

- 记录并比较 assets 下已跟踪文件的 SHA-256，目录迁移不得改变二进制资产。
- 检查 scripts/generate_player.rs 的 #[path]、generate_enemy.py 的 read_text 路径及测试间导入。
- 更新 GLOSSARY.md、docs/player-model.md、docs/enemy-model.md 和实际包含旧路径的测试文档。
- 使用 rg 检查旧模块路径、根级常量、crate::offscreen/validation 反向引用及错误的 pub(super)；逐条判断，不以文本零命中代替依赖分析。
- 验证命令中不执行 ../bevy 内的 Cargo；需要引擎源码时先按 AGENTS.md 核对锁定版本和参考源码版本。

## 7. 主要风险与处理

| 风险 | 处理 |
|---|---|
| 目录变化掩盖了调度顺序变化 | 先原样搬迁，再拆职责；逐项保留调度约束 |
| 对局标记迁移漏掉炮弹、爆炸或地形根 | 对照当前清理清单，验证多轮进出后的根实体、实例和动态纹理回收 |
| 就绪汇总过早放行，出现黑帧 | 分别保留业务就绪和实际 GPU 就绪条件，读取过渡关键帧 |
| 模型路径改变导致脚本失效 | 模型迁移与脚本引用原子更新，独立编译生成器并跑 Python 测试 |
| 画廊误接入驾驶或战斗 | 保持纯视觉组装入口，检查插件依赖及画廊离屏结果 |
| 文件全公开掩盖职责问题 | 优先模块私有和局部导出，不创建全项目 prelude |
| 为追求目录整齐提前泛化 | 保留常量和既有实现，不增加配置/主题/资源框架 |
| 本地工单丢失 | 任务目标和验收标准完整保留在本文件，本地工单只承载执行进度和证据 |

## 8. 完成定义

所有既有启动与验证入口保留；main 不再生成具体游戏实体；通用坦克、模型和地形不依赖页面或验证入口；对局生成/就绪/清理集中；生成器和文档路径一致；没有新增依赖或资产变化。完整回归结果必须列明通过、失败和未执行项。

正式实施后，再按仓库 ADR 约定记录最终采用的模块归属和依赖规则。任何无法满足的边界应先在实施记录中说明具体原因及最小调整，不能静默放宽完成条件。

## 9. 执行记录（2026-10-23）

任务 01–09 完成基线与逐步迁移；任务 10 完成旧入口、依赖、生成器与文档收尾检查。最终结构与第 3 节一致，没有新增依赖或资产变化。最终边界及测试引用例外见 [ADR-0002](../adr/0002-module-boundaries.md)。

完整矩阵见 [任务 10 验收](../testing/architecture-step10-20261023.md)，基线见 [任务 01](../testing/architecture-baseline-20261023.md)。完成表示迁移与要求的验收均已执行，不表示既有 Python 失败已修复；失败和限制单独记录。

