# Tanker

基于 Bevy 的 Rust 3D 游戏工程（voxel 体素美术风格方向）。

## 工程概览

- 语言/工具链：Rust（edition 2024）、Cargo
- 引擎：Bevy **0.19**（`bevy = "0.19"`，crates.io），**仅从 crates.io 拉取依赖**（见下方 bevy 源码使用规则）
- **为何锁在 0.19**：voxel 生态目前最高只支持 Bevy 0.19（`bevy_vox_scene` 0.22、`bevy_voxel_world` 0.17 的依赖均为 `bevy ^0.19`）。升级引擎前必须先确认所用 voxel 插件是否已跟进，否则 Cargo 直接解析失败
- 平台：Linux x11（winit 走 dlopen），3D 渲染走 Vulkan
- 构建命令：`cargo build`；运行：`cargo run`
- Git：远端 `origin`（GitHub `kapaseker/Tanker`），默认分支 **master**（非 main）；`Cargo.lock` 纳入版本控制（可执行程序惯例）
- 运行验证：`cargo run` 后日志应出现 `AdapterInfo { ... backend: Vulkan ... }` 与 `Creating new window Tanker`；加载 vox 场景时还应出现 `tanker: vox 场景加载完成：assets/<场景>.vox`；无人值守检查可用 `timeout 30 <二进制>` 看日志
- ⚠️ **资源根路径**：Bevy 的 `AssetPlugin` 按 `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` → 可执行文件所在目录 的优先级解析 `assets/`。因此**直接执行 `./target/debug/tanker` 会把资源根变成 `target/debug/`，所有资源报 `Path not found`**。要么用 `cargo run`（cargo 会注入 `CARGO_MANIFEST_DIR`），要么显式加 `BEVY_ASSET_ROOT=$PWD`

## Bevy 源码使用规则（重要）

本仓库同级目录 `../bevy` 是 Bevy 引擎的完整源码检出，**只读参考，严禁编译或依赖**。

### 版本同步规则（必须遵守）

`../bevy` 的检出版本**必须与本工程 `Cargo.lock` 中的 bevy 版本一致**（当前：`v0.19.1`，detached tag 检出）：

- 升级/变更本工程 bevy 依赖版本时，**必须同步**把 `../bevy` 切到对应 tag：
  `git -C ../bevy fetch --tags && git -C ../bevy checkout v<新版本>`
- 动手前先核对：`git -C ../bevy describe --tags` 应输出与本工程 bevy 版本一致的 tag；不一致就先切，再查资料
- 同步检查（`git status`）发现 `../bevy` 停在 dev 分支或别的版本上，视作版本错位，先修复再继续

### 只读约束

- ❌ 禁止在 `Cargo.toml` 中通过 `path = "../bevy"` 或 `git = ...` 引用源码
- ❌ 禁止以任何形式（workspace patch、`[patch.crates-io]` 指向源码等）替换 crates.io 版本
- ❌ 不要在 `../bevy` 内执行 cargo 命令或修改其中任何文件（检出切换除外，且仅限 checkout tag）

### 推荐用法（查资料时优先级从高到低）

1. **查 API 用法**：读 `../bevy/examples/`（按主题分类的官方示例，与本工程版本一致）和 `../bevy/crates/<crate>/src/`
2. **查本工程版本 API**：<https://docs.rs/bevy/0.19.1>（精确匹配本工程依赖，用于交叉验证）
3. **查特性定义**：`../bevy/Cargo.toml` 的 `[features]` 段与 `../bevy/crates/bevy_internal/Cargo.toml`；0.19 的特性集可查 <https://crates.io/api/v1/crates/bevy/0.19.0> 的 `features` 字段
4. **查变更/差异**：`git -C ../bevy log/diff`，用于确认 API 是否为未发布改动
5. **查文档**：<https://bevy.org>（最新版本文档，可能领先于本工程所用版本）

> ⚠️ 照抄示例前先核对特性：`../bevy/examples` 的不少示例依赖本工程已禁用的特性（`audio`、`bevy_gilrs` 手柄等），直接把示例的 bevy 特性配置搬进本工程 `Cargo.toml` 会编译失败。

## Voxel 技术栈

- **`bevy_vox_scene` 0.22**：加载 MagicaVoxel `.vox` 文件，还原为 Bevy scene graph，并尽量复刻 MagicaVoxel 渲染页材质（金属度/粗糙度/自发光/玻璃透射/云体素）
  - 版本约定：`bevy_vox_scene` 的版本号即其支持的 bevy 版本（0.22 ↔ bevy 0.19）
  - 用法：`add_plugins(VoxScenePlugin::default())`，再 `commands.spawn(WorldAssetRoot(assets.load("xx.vox#节点名")))`
  - 能力边界：材质走 16x16 图集按 UV 索引，**做不了 Minecraft 那种逐体素平铺贴图**；后者要用 `bevy_vox_mesh`
- **`bevy_voxel_world` 0.17**：可编辑体素世界（多线程 meshing + chunk 自动装卸），需要地形可挖放时再引入
- 版本升级提醒：这些插件都用"自身版本号 == 目标 bevy 版本号"的约定，加依赖前务必查其 compatibility 表

## 代码约定

- 使用 Bevy 0.19 API 习惯：`commands.spawn((...))` 直接传组件元组；
- 系统（System）优先拆分为小函数，通过 `#[derive(Component)]` + Query 组合
- 注释与日志使用中文；标识符使用英文

## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature>/` in this repo (no remote tracker). See `docs/agents/issue-tracker.md`.

### Triage labels

Default canonical vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`. See `docs/agents/domain.md`.
