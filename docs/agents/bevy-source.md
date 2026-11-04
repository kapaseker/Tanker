# Bevy 源码查阅规则

本仓库同级目录 `../bevy` 是 Bevy 引擎的完整源码检出，**只读参考，严禁编译或依赖**。本工程只使用 crates.io 的 bevy（锁定版本取自 `Cargo.lock`）。

## 查阅源码前的版本检查（必须遵守）

只有与本工程 `Cargo.lock` 中 bevy 版本完全一致的源码，才能作为本工程 API 的依据：

- 查阅 `../bevy` 前，执行 `git -C ../bevy status --short --branch` 与 `git -C ../bevy describe --tags --exact-match HEAD`，核对工作区状态和精确 tag；升级本工程依赖后重新检查
- 版本或 tag 不匹配时，优先向用户申请把 `../bevy` 切换到匹配的 `v<锁定版本>` tag，不直接读 Cargo 缓存源码。切换需用户明确授权，且工作区必须干净：授权后仅切换到该 tag，本地缺少该 tag 时先 fetch；存在本地改动时停止并告知用户，不自动 stash、reset 或清理文件
- 用户不同意切换、`../bevy` 缺失或无法干净切换时，改用 Cargo 缓存中对应版本的源码，或 `https://docs.rs/bevy/<锁定版本>`

## 只读约束

- ❌ 禁止在 `Cargo.toml` 中通过 `path = "../bevy"` 或 `git = ...` 引用源码
- ❌ 禁止以任何形式（workspace patch、`[patch.crates-io]` 指向源码等）替换 crates.io 版本
- ❌ 不要在 `../bevy` 内执行 cargo 命令或修改其中任何文件（按上述规则授权的 tag 切换除外）

## 推荐用法（查资料时优先级从高到低）

1. **查 API 用法**：通过上述版本检查后，读 `../bevy/examples/` 和 `../bevy/crates/<crate>/src/`；否则使用对应版本的 Cargo 缓存源码或 docs.rs
2. **查本工程版本 API**：`https://docs.rs/bevy/<锁定版本>`（版本取自 `Cargo.lock`，用于交叉验证）
3. **查特性定义**：匹配版本源码的 `Cargo.toml` 中 `[features]` 段与 `crates/bevy_internal/Cargo.toml`，或 `https://crates.io/api/v1/crates/bevy/<锁定版本>` 的 `features` 字段
4. **查变更/差异**：`git -C ../bevy log/diff`，用于确认 API 是否为未发布改动
5. **查文档**：<https://bevy.org>（最新版本文档，可能领先于本工程所用版本）

> ⚠️ 照抄示例前先核对特性：部分 Bevy 示例需要额外启用默认配置未包含的特性。
