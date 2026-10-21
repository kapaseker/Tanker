//! UI 专用的编译期资源：固定文案、配色与逻辑尺寸。
//!
//! 通用值定义在各模块顶层，首页专用值放在 `menu` 中；直接引用常量，
//! 不注册 ECS 资源。模型、世界尺寸、字体加载、图片与动效时间不归此模块管理。

pub(crate) mod colors;
pub(crate) mod dimens;
pub(crate) mod strings;
