//! UI 配色使用 sRGB；着色器接收原始 sRGB 分量，在最终输出时才转换为线性值。

use bevy::prelude::Color;

/// 主要文字色。
pub(crate) const ON_SURFACE: Color = Color::srgb_u8(235, 232, 223);
/// 次要文字色。
pub(crate) const ON_SURFACE_MUTED: Color = Color::srgb_u8(164, 157, 144);
/// 薄荷强调色。
pub(crate) const PRIMARY: Color = Color::srgb_u8(200, 219, 177);
/// 强调渐变的深色端。
pub(crate) const PRIMARY_DEEP: Color = Color::srgb_u8(157, 182, 126);
/// 强调色背景上的文字色。
pub(crate) const ON_PRIMARY: Color = Color::srgb_u8(18, 17, 13);

/// 画廊面板和细边框；不透明色取 CSS 在暗底合成后的色值。
pub(crate) mod gallery {
    use super::*;
    pub(crate) const SURFACE: Color = ON_PRIMARY;
    pub(crate) const OUTLINE: Color = Color::srgb_u8(48, 46, 41);
    pub(crate) const OUTLINE_ACTIVE: Color = Color::srgb_u8(87, 100, 73);
    pub(crate) const NAME_ACTIVE: Color = Color::srgb_u8(234, 244, 218);
}

/// 首页专用配色与渲染补偿值；保留当前实际画面，不套用通用透明度。
pub(crate) mod index {
    use super::*;

    /// 页面底色。
    pub(crate) const BACKGROUND: Color = Color::srgb_u8(15, 14, 12);
    /// 暖色环境光，alpha 为最大强度。
    pub(crate) const ATMOSPHERE_WARM: Color =
        Color::srgba(150.0 / 255.0, 116.0 / 255.0, 82.0 / 255.0, 0.42);
    /// 薄荷环境光，alpha 为最大强度。
    pub(crate) const ATMOSPHERE_MINT: Color = with_alpha(PRIMARY, 0.10);
    /// 地面网格的颜色与合成强度。
    pub(crate) const GRID: Color = with_alpha(PRIMARY, 0.10 * 0.60);
    /// 扫描线的颜色与合成强度。
    pub(crate) const SCAN: Color = with_alpha(Color::srgb(0.0, 0.0, 0.0), 0.16 * 0.22);
    /// 暗角最大遮罩强度。
    pub(crate) const VIGNETTE: Color = with_alpha(Color::srgb(0.0, 0.0, 0.0), 0.62);
    /// 装饰方块整体投影颜色与强度。
    pub(crate) const BLOCK_SHADOW: Color = with_alpha(Color::srgb(0.0, 0.0, 0.0), 0.45 * 0.35);
    /// 装饰方块面色的不透明度，仍乘每个方块自身的艺术权重。
    pub(crate) const BLOCK_OPACITY: f32 = 0.95;
    /// 四种装饰方块的顶面颜色，与着色器 palette 索引对应。
    pub(crate) const BLOCK_TOPS: [Color; 4] = [
        Color::srgb_u8(226, 237, 207),
        Color::srgb_u8(216, 219, 216),
        Color::srgb_u8(139, 118, 96),
        Color::srgb_u8(58, 56, 48),
    ];
    /// 四种装饰方块的左面颜色。
    pub(crate) const BLOCK_LEFTS: [Color; 4] = [
        Color::srgb_u8(183, 203, 159),
        Color::srgb_u8(176, 181, 178),
        Color::srgb_u8(109, 91, 73),
        Color::srgb_u8(44, 42, 36),
    ];
    /// 四种装饰方块的右面颜色。
    pub(crate) const BLOCK_RIGHTS: [Color; 4] = [
        Color::srgb_u8(145, 169, 122),
        Color::srgb_u8(134, 140, 137),
        Color::srgb_u8(82, 68, 55),
        Color::srgb_u8(33, 31, 26),
    ];
    /// 菜单分隔线，对应网页原型的 rgba(235,232,223,0.15)。
    /// CSS 在 sRGB 空间做透明合成；bevy_ui 在线性空间混合，不能直接用 0.15
    /// （直接使用会得到 sRGB≈100，约为网页观感的两倍亮度）。
    /// 0.028 是 0.15 在背景 rgb(15,14,12) 上的等效线性 alpha：
    /// 0.028 线性混合后编码回 sRGB≈46，与网页实测 48 一致
    /// （离屏验证 2026-10-22，见 .scratch/homepage-parity/issues/02）。
    pub(crate) const SEPARATOR: Color = with_alpha(ON_SURFACE, 0.028);
    /// 角标边框。
    pub(crate) const CORNER: Color = with_alpha(PRIMARY, 0.14);
    /// 键帽底色：原稿的 rgba(255,255,255,0.03) 换算到线性合成后的等效 alpha。
    pub(crate) const KBD_BACKGROUND: Color = with_alpha(Color::srgb(1.0, 1.0, 1.0), 0.0034);
}

/// Bevy 的 Alpha trait 不能用于常量求值；本文件的颜色均明确使用 sRGB。
const fn with_alpha(color: Color, alpha: f32) -> Color {
    match color {
        Color::Srgba(mut value) => {
            value.alpha = alpha;
            Color::Srgba(value)
        }
        _ => panic!("UI 常量必须使用 sRGB 颜色"),
    }
}
