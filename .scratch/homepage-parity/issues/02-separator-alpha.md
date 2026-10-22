# 02 菜单分隔线 alpha 对齐网页观感（sRGB 0.15）

**Status: resolved**
**Blocked by: 无**

## 背景

网页原型（设计基准）菜单分隔线为 `rgba(235, 232, 223, 0.15)` sRGB。游戏实现 [colors.rs](file:///e:/Workspace/Code/Rust/Tanker/src/ui_resources/colors.rs) 分隔线为线性补偿值 `SEPARATOR 0.028`，比网页观感暗约 5 倍。

设计裁决：以网页观感为准。

## 注意

0.15 是 CSS sRGB alpha，不能直接写入游戏——游戏 UI 走线性渲染管线，需按既有补偿方法（参考 `SEPARATOR 0.028` 的推导注释）重新计算 sRGB 0.15 对应的线性 alpha。若渲染管线已做 sRGB→线性自动转换则直接用 0.15，以实际渲染观感与网页一致为最终标准。

## 验收

- 游戏内菜单分隔线亮度与网页 0.15 sRGB 观感一致（离屏截图对比）
- `SEPARATOR` 常量注释更新推导依据
- `cargo check` 通过

## Comments

### 2026-10-22 验证结论：原前提不成立，alpha 无需修改

工单背景中"暗约 5 倍"系拿 alpha 数值直接对比（0.15/0.028≈5.4）得出的误判——
0.15 是 sRGB 空间 alpha，0.028 是线性空间 alpha，两者不可直接相除。

双向像素实测（数据见 `.scratch/homepage-parity/verify-02/`）：

| | 分隔线 | 背景行 |
| --- | --- | --- |
| 网页原型（http://127.0.0.1:8931，DPR 1，zoom 1） | RGB(48,47,43) | RGB(15,14,12) |
| 游戏（`--offscreen-test menu` frame-000000，y=344/410/477） | RGB(46,45,42) | RGB(15,14,12) |

两侧亮度差 2/255（≈4%），在 8bit 量化与 1px 线抗锯齿噪声内，人眼不可辨。
`0.028` 正是 CSS `rgba(235,232,223,0.15)` 在背景 `rgb(15,14,12)` 上的正确线性等效值；
若直接写 0.15，bevy_ui 线性混合后会得到 sRGB≈100，反而比网页亮约一倍。

处置：

- alpha 值保持 `0.028` 不变
- [colors.rs](file:///e:/Workspace/Code/Rust/Tanker/src/ui_resources/colors.rs) `SEPARATOR` 注释已更新完整推导依据（含两侧实测值与直接用 0.15 的后果）
- `cargo check` 通过（2026-10-22）

验收条款全部满足：观感一致（实测）、注释已更新、check 通过。
