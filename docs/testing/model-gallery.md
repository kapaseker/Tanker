# 模型画廊验收

当前入口为 `src/pages/gallery/mod.rs`，共用预览归 `src/pages/model_preview.rs`，交互归 `ui.rs`，详情归 `src/pages/detail.rs`；无窗口预设在 `src/validation/scenarios/gallery.rs`。目录迁移后的最终回归见 [任务 10](architecture-step10-20261023.md)。

## 页面约定

首页第二项进入模型画廊。只保留子弹与坦克两类，名称独立于内部资源目录：

| 分类 | 名称 | 可见几何长 × 宽 × 高（米） |
| --- | --- | --- |
| 子弹 | APFSDS | 0.72 × 0.10 × 0.10 |
| 坦克 | LeoPard2 A6 | 10.98 × 3.76 × 4.32 |
| 坦克 | carapace | 9.0 × 3.4 × 2.6 |

弹丸独立展示；敌方无弹丸资源，不生成占位项。混凝土墩仍用于对局，不在画廊展示。弹药详情显示初速与射速，玩家详情显示前进/倒车速度两项；其他项无空规格行。

标题栏固定，内容独立滚动。方向键按分类保持列，目标为空时停止；Tab/Shift+Tab 遍历返回框与三张卡片。详情返回恢复焦点和滚动。整车绕 body 原点旋转，弹丸绕自身几何中心；尺寸由可见网格测量，取景覆盖完整旋转包络。

当前整理的验证记录见 [模型目录整理](model-catalog-2026-11-01.md)。以下保留历史验收，旧分类和旧路径不表示当前清单。

## 2026-10-26详情实现与验证

- `cargo check`通过；`cargo test`全部86项通过。新增测试覆盖网格方向边界、自动滚入、Tab焦点归属、点击取消、拖拽生命周期、旋转缩放限位和跨页键鼠释放锁，并用失败重现测试覆盖返回框跨页点击残留。
- `detail`离屏验证退出0，证据位于`.scratch/model-detail/final-detail/`：八项真实模型、三种宽度、鼠标/键盘导航、原位置恢复、首次加载失败揭幕与返回、反复清理均通过。已读取模型、旋转缩放、布局、错误提示和过渡关键帧。
- `gallery`三轮往返离屏回归退出0，证据位于`.scratch/model-detail/final-gallery/`，已读取顶部和整车关键帧。
- `index`三轮首页/对局双向过渡回归退出0，最终证据位于`.scratch/model-detail/final-index/`；已读取首页关键帧确认透视背景正常，详情平面网格未影响首页。
- 当前环境为llvmpipe软件Vulkan，未验证硬件GPU或真实窗口输入；没有启动可见游戏窗口。已有ICU4X中日文分词告警仍存在，所检查的中文标签与错误提示可见。
- 保留中间证据：`index-regression/`进程收到SIGTERM，未产生完整报告；`index-regression-02/`断言通过，但画面检查发现详情网格分支误用于首页，随后收紧分支条件。最终首页回归单独执行，使用新目录保存证据，不覆盖先前结果。

## 2026-10-23验证记录

- `cargo check --locked`通过，`cargo test --locked`全部60项通过；新增无GPU真实资产测试覆盖八项尺寸、轮组、隐藏相位、实例清理及缺失模型的失败收束。
- Windows / NVIDIA GeForce RTX 4060 / Vulkan画廊离屏验证退出0：`.scratch/gallery/verified/`；首页到对局三轮双向过渡回归退出0：`.scratch/gallery/menu-final/`。
- 已读取画廊顶部、整车、自转、窄屏、超宽屏及过渡关键帧，核对真实模型、文本和裁切。没有打开可见游戏窗口。
- 首轮曾发现重复进入时部分延迟生成网格漏分层，失败报告保留在`.scratch/gallery/first/`；修复后再次完整验证通过。
- 反转隐藏相位判断后，真实预览测试按预期失败；恢复原实现。测试能够发现可见性回归。
- 环境仍输出已有的ICU4X中日文分词数据告警；所检查的中文标签正常显示。Cargo增量缓存硬链接失败后改为复制，不影响检查结果。

### 整车旋转轴调整

整车取消按组合包围盒居中的平移，直接绕body根节点原点旋转；独立部件保留原有居中方式。镜头包围球改为以实际旋转轴为球心，使用最远边界的距离确定半径，完整尺寸读数保持不变。

`cargo check --locked`和60项测试通过。真实资产测试先复现旧实现约1.5米的车身偏移，再验证新轴心为零，并在一整周的15度采样角度下检查可见网格边界不超出相机视野。

最新构建产物通过画廊离屏三次往返，输出位于`.scratch/gallery/body-pivot/`。报告验证两辆整车绕body原点，已读取`gallery-bottom.png`与`gallery-rotation.png`确认实际效果。运行时显式设置`BEVY_ASSET_ROOT`为工程根目录；未启动可见窗口。

### 上下拖拽方向修正

原实现把CSS的俯仰减法直接用于Bevy，导致向下拖拽时前侧表面反而上移；仅修正符号仍不足以解决水平转到侧面时的侧滚，因为原组合顺序让俯仰绕偏航后的模型局部轴进行。现改为正向累加屏幕Y增量，并先应用偏航、再绕屏幕水平轴俯仰，±80°限位保持不变。

方向回归测试先复现0°偏航时的反向，再复现仅改符号后90°偏航时没有竖直位移的问题；修正后覆盖0°、45°、90°、180°、270°的上下拖拽。`cargo check`和全部87项测试通过。离屏脚本新增独立向下、向上拖拽的实际输入与关键帧，已读取`.scratch/model-detail/drag-direction/detail-drag-down.png`和`detail-drag-up.png`核对方向。

本次完整`detail`离屏在llvmpipe软件Vulkan环境中两次触及120秒上限，**完整场景未通过**；第二次仅以`LP_NUM_THREADS=4`限制线程重试，没有修改断言或超时。失败报告、日志和已生成图片分别保留在`.scratch/model-detail/drag-direction/`、`.scratch/model-detail/drag-direction-verified/`。没有进行真实窗口或硬件GPU验证。

## 2026-10-26：混凝土墩归档与展示

- 资产整理为 `assets/obstacles/concrete/obstacle.vox`，同步对局、生成器和文档引用；重新生成后 SHA-256 保持 `5bb5ffc425f472fd4514d7c9a845e21540d9acd1b73899e1a74f1130f02fe280`。
- 新增第五类“障碍物”及 concrete 卡片，显示实际可见几何尺寸 4.0×1.5×3.0 米；画廊与详情共享纯视觉加载。最后一行右侧留空，方向键不进入空位，Tab遍历真实卡片数。
- `cargo check`、`cargo build` 通过，Rust 90项、Python 12项测试通过。新增末行单卡导航测试先复现旧实现失败；真实VOX测试覆盖九项尺寸与85个实例的清理。
- 画廊离屏 `.scratch/gallery/concrete-01/` 退出0、报告通过；新增真实键盘进入混凝土墩详情、返回焦点与滚动恢复检查。已读取 `gallery-concrete.png` 与 `gallery-concrete-detail.png`，核对卡片、分类、型号、长宽高及真实模型。三轮往返清理通过。
- 迁移后的对局 `scene` 离屏 `.scratch/offscreen/concrete-relocated-01/` 退出0、报告通过，继续验证墩体真实网格、静态碰撞尺寸与贴地位置。
- 后端为llvmpipe软件Vulkan（`LP_NUM_THREADS=4`）；未启动可见窗口，未验证硬件GPU。此次未重新运行完整九项 `detail` 预设，新增混凝土墩详情已由扩展后的 `gallery` 预设覆盖。

弹药初速与射速的权威来源是 `assets/tank/ammunition/apfsds/ammunition.jsonc`；冷却按 `60 / rate_of_fire` 推导，剩余冷却只存在于对局状态。配置缺失或非法时阻止启动，无数值兜底。
