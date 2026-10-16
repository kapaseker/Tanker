# Tanker

基于 Bevy 的 Rust 3D 游戏工程（voxel 体素美术风格方向）。

## 工程概览

- 语言/工具链：Rust（edition 2024）、Cargo
- 引擎：Bevy **0.19**（`bevy = "0.19"`，crates.io），**仅从 crates.io 拉取依赖**（见下方 bevy 源码使用规则）
- **为何使用 0.19**：当前接入的 `bevy_vox_scene` 0.22 依赖 Bevy 0.19。升级引擎前必须核对所用 voxel 插件的兼容表与依赖声明；版本不兼容可能引入多版本 Bevy，导致编译或集成失败
- 平台：Linux x11（winit 走 dlopen），3D 渲染走 Vulkan
- 构建命令：`cargo build`；运行：`cargo run`
- Git：远端 `origin`（GitHub `kapaseker/Tanker`），默认分支 **master**（非 main）；`Cargo.lock` 纳入版本控制（可执行程序惯例）
- 运行验证：`cargo run` 后日志应出现 `AdapterInfo { ... backend: Vulkan ... }` 与 `Creating new window Tanker`；加载 vox 场景时还应出现 `tanker: vox 场景加载完成：assets/<场景>.vox`。这些日志仅证明启动和主场景资源加载，仍需检查依赖资源错误；涉及画面的改动还需实际检查渲染效果
- ⚠️ **资源根路径**：Bevy 的 `AssetPlugin` 按 `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` → 可执行文件所在目录 的优先级解析 `assets/`。未设置前两个环境变量时，直接执行 `./target/debug/tanker` 会从 `target/debug/assets/` 查找资源。使用 `cargo run`（Cargo 会注入 `CARGO_MANIFEST_DIR`），或从工程根目录显式设置 `BEVY_ASSET_ROOT="$PWD"`

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

- **`bevy_vox_scene` 0.22**：当前已接入，加载 MagicaVoxel `.vox` 文件并还原场景图及材质
  - 用法：`add_plugins(VoxScenePlugin::default())`，再 `commands.spawn(WorldAssetRoot(assets.load("xx.vox#节点名")))`
  - 能力边界：材质走 16x16 图集按 UV 索引，**做不了 Minecraft 那种逐体素平铺贴图**；后者要用 `bevy_vox_mesh`
- **`bevy_voxel_world`**：尚未引入；需要地形可挖放时再评估
- 版本升级提醒：插件与 Bevy 的版本号独立；新增或升级依赖前，核对目标版本的 compatibility 表和依赖声明

## 代码约定

- 使用 Bevy 0.19 API 习惯：`commands.spawn((...))` 直接传组件元组；
- 系统（System）优先拆分为小函数，通过 `#[derive(Component)]` + Query 组合
- 日志使用中文；标识符使用英文
- 新增或修改代码时，必须在必要位置补充中文注释；中文难以准确表达时可使用英文。类、结构体、枚举、关键成员字段、常量和函数应说明职责或含义；关键参数、局部变量与复杂逻辑应解释单位、坐标系、生命周期、边界条件或设计原因。Rust 声明优先使用 `///` 文档注释，局部说明使用 `//`；注释应提供代码本身未清楚表达的信息，并随实现同步更新。
- 编写或修改 Rust 代码后，必须在本工程根目录执行 `cargo check` 并通过；失败时修复后重新检查。环境限制导致无法完成时，明确说明阻碍，不宣称检查通过。`cargo check` 不能替代需要的运行与画面验证

## Agent skills

### Issue tracker

创建、读取或更新工单与规格时，先读 `docs/agents/issue-tracker.md`。工单位于 `.scratch/<feature>/`，不使用远程 tracker；该目录被 Git 忽略，仅在本机保留，不随仓库共享。

### Triage labels

分诊或设置工单状态时，先读 `docs/agents/triage-labels.md`，使用其中的标准标签。

### Domain docs

探索领域概念或做架构设计时，先读 `docs/agents/domain.md`，再按其指引查阅术语表和相关 ADR；缺失文件时按该文档约定继续。
