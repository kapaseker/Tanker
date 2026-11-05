# Tanker

基于 Bevy 的 Rust 3D 游戏工程（voxel 体素美术风格方向）。

## 工程概览

- 语言/工具链：Rust（edition 2024）、Cargo
- 引擎：Bevy **0.20**（`bevy = "0.20"`，crates.io），**仅从 crates.io 拉取依赖**（见下方 bevy 源码使用规则）
- **为何使用 0.20**：当前接入的 `bevy_vox_scene` 0.23 依赖 Bevy 0.20
- 平台：Linux x11（winit 走 dlopen），3D 渲染走 Vulkan
- Git：远端 `origin`（GitHub `kapaseker/Tanker`），默认分支 **master**（非 main）
- 运行验证默认使用下方无窗口离屏入口，验收标准见[测试章节](#无窗口测试与画面验证)；`cargo run` 的可见窗口仅用于用户主动游玩或明确同意的窗口行为验证
- ⚠️ **资源根路径**：`AssetPlugin` 按 `BEVY_ASSET_ROOT` → `CARGO_MANIFEST_DIR` → 可执行文件所在目录解析 `assets/`；直接执行 `./target/debug/tanker` 会去 `target/debug/assets/` 找资源。用 `cargo run`（Cargo 注入 `CARGO_MANIFEST_DIR`），或设置 `BEVY_ASSET_ROOT="$PWD"`

## 无窗口测试与画面验证

- 测试、排查 bug 或对比画面效果时，全部在后台完成：纯逻辑优先 `cargo test`；`scripts/` 生成器用 `python3 -X utf8 -m unittest discover -s scripts -p "test_*.py"`；`web/` 自检用 `node web/ammunition.test.cjs`。浏览器验收（`web/*.browser.test.cjs`、`transition.test.cjs`）先从工程根目录起 `python3 -m http.server 8765 --bind 127.0.0.1`，再按文件头用 `NODE_PATH` 指向本机 Playwright 运行。
- 需要实际场景、GPU 或动态效果时使用离屏入口，从工程根目录执行：

  ```text
  cargo run -- --capture .scratch/capture.png
  cargo run -- --offscreen-test scene --output .scratch/offscreen/scene
  ```

- **预设选择**：按场景、驾驶、射击、瞄准、HUD、模型或页面改动，查 [离屏预设选择](docs/testing/offscreen-presets.md)；覆盖不到的 bug 应扩充离屏预设或逻辑测试。`--capture` 只生成一张默认场景 PNG，不替代预设断言。
- 离屏复用实际游戏场景、资源和渲染配置；分辨率、随机种子、模拟步长、落地帧数与超时以 `src/validation/mod.rs` 为准，截图等待不推进模拟时间。
- 产物目录自动创建，已有目标产物拒绝覆盖；重复验证使用新路径。只有断言通过且全部图片保存成功才返回 0，资源、渲染、断言、写入错误或超时均失败退出。
- 检查退出码和报告，并读取关键帧确认效果；图片不是纯色仅是最低自动检查，不代表模型、材质或动画正确。跨显卡/驱动不要求像素完全一致，固定种子与步长也不保证跨平台物理完全一致。
- 离屏入口失败或环境缺少 GPU 能力时，说明阻碍并保留证据，不退回可见窗口，也不宣称画面验证通过；报告使用 llvmpipe 或看不到 GPU 设备时，先按 [GPU 排查](docs/testing/gpu-diagnostics.md) 区分沙箱限制与宿主机驱动问题。后台渲染仍会占用 GPU。
- 焦点、真实键鼠、全屏、显示缩放等必须依赖窗口的验证，先说明原因并取得用户同意。
- `drive` 若报告真实地形上的直行阻塞，先读 `docs/testing/offscreen-findings.md` 中的现有发现；保留失败证据，不降低断言，也不在仅要求测试工具时顺带修改游戏碰撞。

## Bevy 源码使用规则（重要）

`../bevy` 是同级的 Bevy 引擎源码检出，只读参考，严禁编译或依赖。本工程只使用 crates.io 的 bevy：禁止 `path = "../bevy"`、`git = ...` 依赖，禁止 `[patch.crates-io]` 或任何形式的源码替换；不在 `../bevy` 内执行 cargo 或修改文件（按规则授权切换 tag 除外）。

查阅 `../bevy` 或核对 Bevy API、特性、版本差异前，先读 [Bevy 源码查阅规则](docs/agents/bevy-source.md)（版本一致性检查、只读边界、查找优先级）。

## Voxel 技术栈

- **`bevy_vox_scene` 0.23**：当前已接入，加载 MagicaVoxel `.vox` 文件并还原场景图及材质
  - 用法：`add_plugins(VoxScenePlugin::default())`，再 `commands.spawn(WorldAssetRoot(assets.load("xx.vox#节点名")))`
  - 能力边界：材质走 16x16 图集按 UV 索引，**做不了 Minecraft 那种逐体素平铺贴图**；后者要用 `bevy_vox_mesh`
- **`bevy_voxel_world`**：尚未引入；需要地形可挖放时再评估
- 版本升级提醒：插件与 Bevy 的版本号独立；新增或升级依赖前，核对目标版本的 compatibility 表和依赖声明

## 游戏美术风格

- 新增、修改或评审坦克、地形、建筑、植被、材质、灯光与特效前，必须先读取并遵循 [美术风格基线](docs/art-direction.md)。基线为“精细体素半写实风格”；参考图中的具体场景不限制地图题材。
- 调整风格方向时，同步更新基线、参考图和变更记录；实现与验收状态分别记录，不能以概念图确认替代实际引擎美术验收。

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

设计或调整坦克游戏数据时，参考 [World of Tanks Modern Armor 坦克百科](https://modernarmor.worldoftanks.com/en/encyclopedia/vehicles/)的对应车型；记录所用车型页面、配置和取值，并区分该游戏数值与本项目采用值、真实车辆数据。

新增或修改坦克模型前，先读 [坦克车型目录](docs/tank-catalog.md)，再查对应模型说明、尺寸来源与验收记录。确定或变更现实参考型号、还原范围、资源组合及验收状态时，同步更新目录和对应长期文档；最终设计依据与关键参考图纳入 Git，本地工单继续按下述 tracker 规则维护。设计目标与已实现/已验收状态分别记录。

### Issue tracker

创建、读取或更新工单与规格时，先读 `docs/agents/issue-tracker.md`。工单位于 `.scratch/<feature>/`，不使用远程 tracker；该目录被 Git 忽略，仅在本机保留，不随仓库共享。

### Triage labels

分诊或设置工单状态时，先读 `docs/agents/triage-labels.md`，使用其中的标准标签。

### Domain docs

探索领域概念或做架构设计时，先读 `docs/agents/domain.md`，再按其指引查阅术语表和相关 ADR；缺失文件时按该文档约定继续。
