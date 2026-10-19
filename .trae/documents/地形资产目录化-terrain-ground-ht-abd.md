# 地形资产目录化：assets/terrain/ground/ground.ht + ground.abd

## 摘要

- 将两张地形图从 `assets/ground_height.bin`、`assets/ground_albedo.bin` 迁移到 **`assets/terrain/ground/ground.ht`、`assets/terrain/ground/ground.abd`**，后缀按功能命名（ht = 高度，abd = albedo）。
- 目标结构 `terrain/<地形名>/`：本次**只做目录重定位 + 路径常量化**，多地形加载机制（清单/目录枚举）等出现第二个地形时再加（YAGNI，用户已确认）。
- 调色板维持 Python/Rust 双侧常量 + 一致性测试，**本次不落盘**（用户已确认）。
- 历史设计文档 `.trae/documents/地板重做-*.md` 为历史记录，不改。

## 现状

- `src/ground.rs`：`GroundMapLoader` 注册扩展名 `["bin"]`（L86）；`load_ground` 写死 `assets.load("ground_height.bin")` / `("ground_albedo.bin")`（L120-L121）；模块文档与若干注释提到 `.bin`。
- `scripts/generate_ground.py`：`HEIGHT_OUTPUT`/`ALBEDO_OUTPUT` 常量指向 `assets/ground_*.bin`（L46-L47）；`write_assets()` 直接 `write_bytes`，**不会创建缺失目录**。
- `scripts/test_generate_ground.py`：文件路径全部经 `ground.HEIGHT_OUTPUT`/`ground.ALBEDO_OUTPUT` 常量引用，**无需改动**。
- `GLOSSARY.md` L30、L38 引用旧路径。
- 资产格式契约不变：各 1,440,000 字节，`10`=平地、每档 0.1 米；行沿 Bevy X、列沿 Bevy Z。

## 变更清单

### 1. 资产文件迁移（git mv，保留历史）

```bash
mkdir assets/terrain/ground   # git mv 前目标目录需存在（或 git mv 自动建）
git mv assets/ground_height.bin assets/terrain/ground/ground.ht
git mv assets/ground_albedo.bin assets/terrain/ground/ground.abd
```

### 2. `src/ground.rs`

- 扩展名注册：`extensions()` 返回 `&["ht", "abd"]`（一个 loader 服务两种后缀，长度校验共用）。
- 新增路径常量（放在 `MAP_SIDE` 附近）：

```rust
/// 地形资产根目录（相对 assets/）；每个子目录是一个地形，含 ground.ht / ground.abd。
const TERRAIN_ROOT: &str = "terrain";
/// 当前启用的地形目录名；将来多地形时改为按清单或参数选择。
const TERRAIN_NAME: &str = "ground";
```

- `load_ground` 改为拼路径：`assets.load(format!("{TERRAIN_ROOT}/{TERRAIN_NAME}/ground.ht"))`，albedo 同理（`AssetServer::load` 接受 `impl Into<AssetPath>`，`String` 可直接传入）。
- 同步更新注释：模块文档（L1-L5）、`GroundMap` 文档（L50-L53）、`GroundMapLoader` 文档（L55）中的 `.bin` 描述改为新路径/新后缀。
- 汇报与错误文案不变（"高度图/土色图"仍准确）。

### 3. `scripts/generate_ground.py`

- 输出常量：

```python
TERRAIN_DIR = ASSETS / 'terrain' / 'ground'
HEIGHT_OUTPUT = TERRAIN_DIR / 'ground.ht'
ALBEDO_OUTPUT = TERRAIN_DIR / 'ground.abd'
```

- `write_assets()` 写出前 `TERRAIN_DIR.mkdir(parents=True, exist_ok=True)`。
- 模块文档字符串（L4-L5）中的路径同步更新。

### 4. `GLOSSARY.md`

- L30：产物路径改为 `assets/terrain/ground/ground.ht`、`assets/terrain/ground/ground.abd`。
- L38：`ground_height.bin` 表述改为 `ground.ht`。

## 假设与决定

| # | 决定 | 理由 |
|---|---|---|
| D1 | 只重定位 + 常量化，不做清单/枚举机制 | 用户确认（YAGNI）；机制等第二个地形出现再设计 |
| D2 | 调色板不落盘 | 用户确认；避免夹带第二个变更 |
| D3 | 后缀 `ht` / `abd`，目录名 = 地形名 `ground` | 用户选定；loader 多扩展名注册即可，无需两个 loader |
| D4 | `git mv` 而非删除重建 | 保留二进制文件历史 |
| D5 | `.trae/documents` 旧设计文档不改 | 历史记录 |

## 验证步骤

1. `python3 scripts/generate_ground.py`：自动创建 `assets/terrain/ground/`，`ground.ht`、`ground.abd` 各 1,440,000 字节。
2. `scripts/` 目录下 `python3 -m unittest` 全绿（可重现性测试自动覆盖新路径）。
3. 工程根目录 `cargo check` 通过；`cargo test` 通过。
4. `cargo run`：日志出现 `地形地图加载完成：…`，无资源加载失败；画面检查地形渲染正常（cargo check/测试不能替代画面验证）。
