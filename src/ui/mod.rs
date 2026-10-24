//! UI 基础能力：字体加载、整体缩放规则与编译期界面资源（配色、尺寸、文案）。
pub(crate) mod font;
pub(crate) mod resources;
pub(crate) mod scaling;
pub(crate) mod widgets;

pub(crate) use scaling::zoom;
