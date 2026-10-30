//! 中文固定界面文字；动态编号与拼接仍由界面代码负责，不包含日志和实体名称。

/// 原生窗口标题。
pub(crate) const WINDOW_TITLE: &str = "V-Tank";

/// 画廊固定文案；型号来自真实资产路径，不参与翻译。
pub(crate) mod gallery {
    pub(crate) const TITLE: &str = "模型画廊";
    pub(crate) const CATEGORIES: [&str; 5] = ["炮管", "履带", "车身", "坦克", "障碍物"];
    pub(crate) const LOADING: &str = "加载中…";
    pub(crate) const FAILED: &str = "模型加载失败";
    pub(crate) const UNIT: &str = "米";
    pub(crate) const COUNT: &str = "个模型";
}

/// 首页可见文案；品牌标题已烘焙在 PNG 中，不在此重复定义。
pub(crate) mod index {
    /// 标题栏页面标题（design.md：首页常驻 88px 标题栏，显示“主菜单”）。
    pub(crate) const TITLE: &str = "主菜单";
    /// 与菜单交互顺序一致的操作标签。
    pub(crate) const ACTIONS: [&str; 3] = ["开始游戏", "模型画廊", "测试场景"];
    /// 方向键提示后的动作说明。
    pub(crate) const HINT_SELECT: &str = "选择 ·";
    /// 回车键提示后的动作说明。
    pub(crate) const HINT_CONFIRM: &str = "确认";
    /// 版本文案分两部分：前缀为固定文字，版本号由 Cargo 从包配置注入，避免在此重复维护。
    pub(crate) const BUILD: &str = concat!("版本 ", env!("CARGO_PKG_VERSION"));
    /// 退出确认弹框标题与按钮标签。
    pub(crate) const EXIT_TITLE: &str = "确认退出游戏吗？";
    pub(crate) const EXIT_CANCEL: &str = "取消";
    pub(crate) const EXIT_CONFIRM: &str = "确认";
}

/// 详情尺寸与装备规格标签；分类与单位沿用画廊文案。
pub(crate) mod detail {
    pub(crate) const DIMENSIONS: [&str; 3] = ["长度", "宽度", "高度"];
    /// 炮管规格：初速与射速。
    pub(crate) const MUZZLE_VELOCITY: &str = "初速";
    pub(crate) const RATE_OF_FIRE: &str = "射速";
    /// 履带规格：前进与倒车速度。
    pub(crate) const FORWARD_SPEED: &str = "前进速度";
    pub(crate) const REVERSE_SPEED: &str = "倒车速度";
}

/// 田园预览的操作提示。
pub(crate) mod pastoral {
    pub(crate) const HELP: &str = "测试场景  ·  右键转向  /  WASD 移动  /  QE 升降  /  Shift 加速  /  Esc 返回";
}
