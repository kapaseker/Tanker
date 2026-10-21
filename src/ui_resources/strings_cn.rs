//! 中文固定界面文字；动态编号与拼接仍由界面代码负责，不包含日志和实体名称。

/// 原生窗口标题。
pub(crate) const WINDOW_TITLE: &str = "V-Tank";

/// 首页可见文案；品牌标题已烘焙在 PNG 中，不在此重复定义。
pub(crate) mod menu {
    /// 与菜单交互顺序一致的操作标签。
    pub(crate) const ACTIONS: [&str; 2] = ["开始游戏", "模型画廊"];
    /// 方向键提示后的动作说明。
    pub(crate) const HINT_SELECT: &str = "选择 ·";
    /// 回车键提示后的动作说明。
    pub(crate) const HINT_CONFIRM: &str = "确认";
    /// 版本文案分两部分：前缀为固定文字，版本号由 Cargo 从包配置注入，避免在此重复维护。
    pub(crate) const BUILD: &str = concat!("版本 ", env!("CARGO_PKG_VERSION"));
}
