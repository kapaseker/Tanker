//! UI 整体缩放规则：窄窗口等比缩小，宽窗口保持设计尺寸。

use crate::ui::resources::dimens;

/// scale.js 的 zoom 语义：可用宽度不足参考宽度时整体等比缩小，只缩不放大，高度不参与。
pub(crate) fn zoom(available_width: f32) -> f32 {
    (available_width / dimens::REFERENCE_WIDTH).min(1.0)
}
