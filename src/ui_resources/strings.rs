//! 固定界面文字；动态编号与拼接仍由界面代码负责，不包含日志和实体名称。

/// 原生窗口标题。
pub(crate) const WINDOW_TITLE: &str = "Tanker";

/// 首页可见文案；品牌标题已烘焙在 PNG 中，不在此重复定义。
pub(crate) mod menu {
    /// 与菜单交互顺序一致的操作标签。
    pub(crate) const ACTIONS: [&str; 2] = ["START GAME", "MODEL GALLERY"];
    /// 操作方向符号。
    pub(crate) const ARROW: &str = "→";
    /// 当前键盘导航提示。
    pub(crate) const KEYBOARD_HINT: &str = "↑ ↓ 选择 · ENTER 确认";
    /// 保留设计稿中的固定版本文案，不随 Cargo 包版本自动变化。
    pub(crate) const BUILD: &str = "BUILD 0.1.0";
}
