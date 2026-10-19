# 土色调色板迁移到 `.abd` 文件

## Summary

把地形土色调色板从代码常量（`GROUND_PALETTE`）迁移到 `ground.abd` 资产文件内部：`.abd` 改为"调色板 + 索引矩阵"格式，Rust 侧运行时从文件读调色板，删除代码中的硬编码颜色。`.ht` / `.abd` 校验失败一律在主线程 `panic!`，panic 消息即错误日志（不额外 `error!`）。Python 生成脚本同步写出新格式。

## 已确认决策

- **文件布局**：`[调色板数量 u8][数量×3 字节 RGB][1200×1200 索引字节]`，无魔数；数量可变（1–255）
- **panic 形式**：仅 `panic!("中文原因")`，消息本身就是日志，不重复 `error!`
- **`.ht` 同步改 panic**：两类资产统一"坏文件即死"，loader 不再做长度校验

## Current State

- [ground.rs](file:///e:/Workspace/Code/Rust/Tanker/src/ground.rs)
  - `GROUND_PALETTE`（L42–48）：代码内 5 色 sRGB 调色板，被 `terrain_texture`（L216）与 `palette_color`（L528）使用，越界下标用 `% len` 静默取模
  - `GroundMapLoader`（L64–93）：`.ht` / `.abd` 共用一个 loader，长度不符返回 `io::Error`（只 error! 不 panic，游戏带病空跑）
  - `spawn_terrain`（L137–199）：图就绪后编译网格；`ChunkSource.colors` 只传索引矩阵
- [generate_ground.py](file:///e:/Workspace/Code/Rust/Tanker/scripts/generate_ground.py)
  - `PALETTE`（L51）5 色；`write_assets`（L207–215）把 1440000 字节索引直接写盘，无调色板区
- `scripts/test_generate_ground.py`：校验生成物，需同步新格式

## Proposed Changes

### 1. `src/ground.rs` — 格式与校验

- **删除** `GROUND_PALETTE` 常量及其"与 Python 同步"注释
- **`GroundMapLoader::load`**：删除长度校验，只读字节返回 `GroundMap(bytes)`（IO 错误仍走 `Err`，由 `report_ground` 的 `error!` 汇报；"文件不规范"改由主线程 panic）
- **`spawn_terrain`**：
  - 高度图：`heights.0.len() != MAP_SIDE * MAP_SIDE` → `panic!("地形高度图不合规：应为 {} 字节，实际 {} 字节；请重新运行 scripts/generate_ground.py", ...)`
  - 新增 `fn parse_albedo(bytes: &[u8]) -> (Vec<[u8; 3]>, &[u8])`（纯函数，可单测）：
    - `count = bytes[0]`，要求 `1 <= count`；期望长度 `1 + count*3 + MAP_SIDE*MAP_SIDE`，不符 → `panic!("地形土色图不合规：...")`
    - 拆出调色板 `Vec<[u8; 3]>` 与索引切片
    - 索引扫描：任一 `index >= count` → `panic!("...下标 {} 超出调色板数量 {}", ...)`（替换原 `% len` 取模；校验一次，后续零成本）
- **调色板传参**：`terrain_texture(colors, &palette)`、`ChunkSource` 增加 `palette: &'a [[u8; 3]]` 字段、`palette_color(index, palette)`；删除两处 `% GROUND_PALETTE.len()`
- **模块头注释**（L1–6）与 `GroundMap` 文档注释（L55–56）同步新格式描述
- **单测**：
  - `parse_albedo` 合法用例 + 三类 panic 用例（长度不足/长度多余、索引越界）
  - 现有 `synthetic`（L543–557）为 `ChunkSource` 补 `palette` 字段

### 2. `scripts/generate_ground.py` — 写出新格式

- `write_assets`：土色图改为 `bytes([len(PALETTE)]) + RGB 串联 + 索引矩阵`
- 模块 docstring（L3–5）与 `PALETTE` 注释（L50）同步：调色板随文件走，不再提"与 Rust 侧同步"
- Python 侧 `PALETTE` 成为颜色唯一来源

### 3. `scripts/test_generate_ground.py` — 校验新格式

- 土色图断言改为：首字节 = 5，随后 15 字节 = `PALETTE` 展开，余下 1440000 字节索引

### 4. 资产再生成

- 运行 `python scripts/generate_ground.py` 重写 `assets/terrain/ground/ground.abd`（高度图字节不变，种子固定，内容应逐字节一致）
- [GLOSSARY.md](file:///e:/Workspace/Code/Rust/Tanker/GLOSSARY.md) 中 `.abd` 描述若有格式说明则同步一句

## Assumptions

- 索引 0 当前未被生成脚本使用（底色是 2），无需特殊处理
- IO 读取失败（非格式问题）保持现有 `Err` + `error!` 路径，不 panic
- 不引入版本号/魔数；未来格式演进时再议

## Verification

1. `cargo check` 通过
2. `cargo test` 通过（含新增 `parse_albedo` 用例）
3. `python scripts/test_generate_ground.py` 通过
4. `cargo run`：日志出现 `地形地图加载完成`，地面渲染颜色与迁移前一致（调色板值未变，仅搬家）
5. 故意截短 `ground.abd` 临时验证 panic 与中文原因输出后还原（可选）
