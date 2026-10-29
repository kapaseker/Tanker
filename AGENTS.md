# Tanker

基于 Bevy 的 Rust 3D 游戏工程（voxel 体素美术风格方向）。

## 工程概览

- 语言/工具链：Rust（edition 2024）、Cargo
- 引擎：Bevy **0.20**（`bevy = "0.20"`，crates.io），**仅从 crates.io 拉取依赖**（见下方 bevy 源码使用规则）
- **为何使用 0.20**：当前接入的 `bevy_vox_scene` 0.23 依赖 Bevy 0.20。升级引擎前必须核对所用 voxel 插件的兼容表与依赖声明；版本不兼容可能引入多版本 Bevy，导致编译或集成失败
- 平台：Linux x11（winit 走 dlopen），3D 渲染走 Vulkan
- 构建命令：`cargo build`；运行：`cargo run`
- Git：远端 `origin`（GitHub `kapaseker/Tanker`），默认分支 **master**（非 main）；`Cargo.lock` 纳入版本控制（可执行程序惯例）
- 运行验证默认使用下述无窗口离屏入口；检查 GPU/后端、资源与依赖错误、退出码及报告。成功启动或资源加载日志不代表测试通过；涉及画面的改动还必须读取生成的关键帧检查实际效果。`cargo run` 的可见窗口仅用于用户主动游玩或明确同意的窗口行为验证
- ⚠️ **资源根路径**：Bevy 的 `AssetPlugin` 按 `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` → 可执行文件所在目录 的优先级解析 `assets/`。未设置前两个环境变量时，直接执行 `./target/debug/tanker` 会从 `target/debug/assets/` 查找资源。使用 `cargo run`（Cargo 会注入 `CARGO_MANIFEST_DIR`），或从工程根目录显式设置 `BEVY_ASSET_ROOT="$PWD"`

## 无窗口测试与画面验证

- 测试、排查 bug 或对比画面效果时，尽量全部在后台完成：纯逻辑优先 `cargo test`，需要实际场景、GPU 或动态效果时使用离屏入口，不自行启动可见游戏窗口、抢占焦点或通过桌面截图验证。
- 从工程根目录执行：

  ```text
  cargo run -- --capture .scratch/capture.png
  cargo run -- --offscreen-test scene --output .scratch/offscreen/scene
  cargo run -- --offscreen-test drive --output .scratch/offscreen/drive
  cargo run -- --offscreen-test combat --output .scratch/offscreen/combat
  ```

- `--capture` 生成一张默认场景 PNG；三个场景分别验证静态场景、直行与转弯及履带、开炮命中与爆炸清理，输出 `frame-<六位模拟帧>.png` 和 `report.txt`。按改动选择相关场景，覆盖不到的 bug 应扩充离屏预设或逻辑测试，不默认改用窗口。
- 离屏模式复用实际游戏场景、资源和渲染配置，固定为 1280×720、随机种子 42、1/60 秒模拟步长。资源、实例与 GPU 管线就绪后先模拟 120 帧落地，再开始场景计帧；截图等待不推进模拟时间。
- 产物目录自动创建，已有目标产物拒绝覆盖；重复验证使用新路径。只有断言通过且全部图片保存成功才返回 0；资源、渲染、断言、写入错误或启动后超过 180 秒均失败退出，编译不计入超时。
- 验证时检查退出码和报告，并读取关键帧确认效果；图片不是纯色仅是最低自动检查，不代表模型、材质或动画正确。跨显卡/驱动不要求像素完全一致，固定种子与步长也不保证跨平台物理完全一致。
- 离屏入口失败或环境缺少 GPU 能力时，说明阻碍，不自动退回可见窗口，也不宣称画面验证通过。后台渲染仍会占用 GPU。
- 焦点、真实键鼠、全屏、显示缩放等必须依赖窗口的验证，先说明原因并取得用户同意；启动窗口前还应遵守后台进程的隐藏窗口要求。
- `drive` 若报告真实地形上的直行阻塞，先读 `docs/testing/offscreen-findings.md` 中的现有发现；保留失败证据，不降低断言，也不在仅要求测试工具时顺带修改游戏碰撞。

## Bevy 源码使用规则（重要）

本仓库同级目录 `../bevy` 是 Bevy 引擎的完整源码检出，**只读参考，严禁编译或依赖**。

### 查阅源码前的版本检查（必须遵守）

只有与本工程 `Cargo.lock` 中 bevy 版本完全一致的源码，才能作为本工程 API 的依据：

- 查阅 `../bevy` 前，执行 `git -C ../bevy status --short --branch` 与 `git -C ../bevy describe --tags --exact-match HEAD`，核对工作区状态和精确 tag；升级本工程依赖后重新检查
- `../bevy` 缺失或版本不匹配时，使用 Cargo 缓存中对应版本的源码，或 `https://docs.rs/bevy/<锁定版本>`；无需为无关任务切换源码版本
- 切换 `../bevy` 需用户明确授权，且工作区必须干净。授权后仅切换到匹配的 `v<锁定版本>` tag；本地缺少该 tag 时再 fetch。存在本地改动时停止切换并告知用户，不自动 stash、reset 或清理文件

### 只读约束

- ❌ 禁止在 `Cargo.toml` 中通过 `path = "../bevy"` 或 `git = ...` 引用源码
- ❌ 禁止以任何形式（workspace patch、`[patch.crates-io]` 指向源码等）替换 crates.io 版本
- ❌ 不要在 `../bevy` 内执行 cargo 命令或修改其中任何文件（按上述规则授权的 tag 切换除外）

### 推荐用法（查资料时优先级从高到低）

1. **查 API 用法**：通过上述版本检查后，读 `../bevy/examples/` 和 `../bevy/crates/<crate>/src/`；否则使用对应版本的 Cargo 缓存源码或 docs.rs
2. **查本工程版本 API**：`https://docs.rs/bevy/<锁定版本>`（版本取自 `Cargo.lock`，用于交叉验证）
3. **查特性定义**：匹配版本源码的 `Cargo.toml` 中 `[features]` 段与 `crates/bevy_internal/Cargo.toml`，或 `https://crates.io/api/v1/crates/bevy/<锁定版本>` 的 `features` 字段
4. **查变更/差异**：`git -C ../bevy log/diff`，用于确认 API 是否为未发布改动
5. **查文档**：<https://bevy.org>（最新版本文档，可能领先于本工程所用版本）

> ⚠️ 照抄示例前先核对特性：`../bevy/examples` 的不少示例依赖本工程已禁用的特性（`audio`、`bevy_gilrs` 手柄等），直接把示例的 bevy 特性配置搬进本工程 `Cargo.toml` 会编译失败。

## Voxel 技术栈

- **`bevy_vox_scene` 0.23**：当前已接入，加载 MagicaVoxel `.vox` 文件并还原场景图及材质
  - 用法：`add_plugins(VoxScenePlugin::default())`，再 `commands.spawn(WorldAssetRoot(assets.load("xx.vox#节点名")))`
  - 能力边界：材质走 16x16 图集按 UV 索引，**做不了 Minecraft 那种逐体素平铺贴图**；后者要用 `bevy_vox_mesh`
- **`bevy_voxel_world`**：尚未引入；需要地形可挖放时再评估
- 版本升级提醒：插件与 Bevy 的版本号独立；新增或升级依赖前，核对目标版本的 compatibility 表和依赖声明

## UI 设计规范

- 涉及 UI 界面的新增、修改或评审（布局、配色、字体、交互、动效、响应式或页面切换）时，必须先读取并遵循工程根目录的 [design.md](design.md)。
- 设计规则集中维护在 `design.md`；用户明确调整设计要求时，同步更新该文档、对应设计稿和相关实现。
- 设计或实现 UI 时，如发现规范、设计稿或交互方案存在不合理之处、相互冲突或明显的体验问题，应及时主动告知开发者，说明具体问题、影响及改进建议，并区分已确认的规则与待讨论的调整。

## 代码约定

- 使用 Bevy 0.20 API 习惯：`commands.spawn((...))` 直接传组件元组；
- 系统（System）优先拆分为小函数，通过 `#[derive(Component)]` + Query 组合
- 日志使用中文；标识符使用英文
- 新增或修改代码时，必须在必要位置补充中文注释；中文难以准确表达时可使用英文。类、结构体、枚举、关键成员字段、常量和函数应说明职责或含义；关键参数、局部变量与复杂逻辑应解释单位、坐标系、生命周期、边界条件或设计原因。Rust 声明优先使用 `///` 文档注释，局部说明使用 `//`；注释应提供代码本身未清楚表达的信息，并随实现同步更新。
- 编写或修改 Rust 代码后，必须在本工程根目录执行 `cargo check` 并通过；失败时修复后重新检查。环境限制导致无法完成时，明确说明阻碍，不宣称检查通过。`cargo check` 不能替代需要的运行与画面验证

## Agent skills

### Tank references

新增或修改坦克模型前，先读 [坦克车型目录](docs/tank-catalog.md)，再查对应模型说明、尺寸来源与验收记录。确定或变更现实参考型号、还原范围、资源组合及验收状态时，同步更新目录和对应长期文档；最终设计依据与关键参考图纳入 Git，本地工单继续按下述 tracker 规则维护。设计目标与已实现/已验收状态分别记录。

### Issue tracker

创建、读取或更新工单与规格时，先读 `docs/agents/issue-tracker.md`。工单位于 `.scratch/<feature>/`，不使用远程 tracker；该目录被 Git 忽略，仅在本机保留，不随仓库共享。

### Triage labels

分诊或设置工单状态时，先读 `docs/agents/triage-labels.md`，使用其中的标准标签。

### Domain docs

探索领域概念或做架构设计时，先读 `docs/agents/domain.md`，再按其指引查阅术语表和相关 ADR；缺失文件时按该文档约定继续。
