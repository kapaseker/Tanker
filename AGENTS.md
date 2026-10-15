# Tanker

基于 Bevy 的 Rust 3D 游戏工程。

## 工程概览

- 语言/工具链：Rust（edition 2024）、Cargo
- 引擎：Bevy 0.20.0，**仅从 crates.io 拉取依赖**（见下方 bevy 源码使用规则）
- 平台：Linux x11（winit 走 dlopen），3D 渲染走 Vulkan
- 构建命令：`cargo build`；运行：`cargo run`
- Git：远端 `origin`（GitHub `kapaseker/Tanker`），默认分支 **master**（非 main）；`Cargo.lock` 纳入版本控制（可执行程序惯例）
- 运行验证：`cargo run` 后日志应出现 `AdapterInfo { ... backend: Vulkan ... }` 与 `Creating new window Tanker`；无人值守检查可用 `timeout 30 cargo run` 看日志

## Bevy 源码使用规则（重要）

本仓库同级目录 `../bevy` 是 Bevy 引擎的完整源码检出（main 分支，0.20-dev），**只读参考，严禁编译或依赖**：

- ❌ 禁止在 `Cargo.toml` 中通过 `path = "../bevy"` 或 `git = ...` 引用源码
- ❌ 禁止以任何形式（workspace patch、`[patch.crates-io]` 指向源码等）替换 crates.io 版本
- ❌ 不要在 `../bevy` 内执行 cargo 命令或修改其中任何文件

✅ 允许且推荐的做法（查资料时优先级从高到低）：

1. **查 API 用法**：读 `../bevy/examples/`（按主题分类的官方示例，最贴近 0.20-dev API）和 `../bevy/crates/<crate>/src/`
2. **查特性定义**：`../bevy/Cargo.toml` 的 `[features]` 段与 `../bevy/crates/bevy_internal/Cargo.toml`
3. **查变更/差异**：`git -C ../bevy log/diff`，用于确认 API 是否为未发布改动
4. **查文档**：<https://bevy.org>（注意其文档版本可能落后于 0.20）

> ⚠️ 照抄示例前先核对特性：`../bevy/examples` 的不少示例依赖本工程已禁用的特性（`audio`、`bevy_gilrs` 手柄等），直接把示例的 bevy 特性配置搬进本工程 `Cargo.toml` 会编译失败。

## 代码约定

- 使用 Bevy 0.20 API 习惯：`commands.spawn((...))` 直接传组件元组；
- 系统（System）优先拆分为小函数，通过 `#[derive(Component)]` + Query 组合
- 注释与日志使用中文；标识符使用英文

## Agent skills

### Issue tracker

Issues live as local markdown files under `.scratch/<feature>/` in this repo (no remote tracker). See `docs/agents/issue-tracker.md`.

### Triage labels

Default canonical vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` + `docs/adr/`. See `docs/agents/domain.md`.
