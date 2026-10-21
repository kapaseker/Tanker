//! UI 尺寸：除明确标注的比例外均为逻辑像素，不包含世界单位或测试输出分辨率。

/// 初始 UI 排版参考宽度，实际布局仍跟随视口。
pub(crate) const REFERENCE_WIDTH: f32 = 1280.0;
/// 初始 UI 排版参考高度。
pub(crate) const REFERENCE_HEIGHT: f32 = 720.0;
/// MiSans normal 行高相对字号的比例。
pub(crate) const LINE_HEIGHT: f32 = 4.0 / 3.0;
/// UI 细边框宽度。
pub(crate) const BORDER_WIDTH: f32 = 1.0;

/// 首页设计参数；响应式公式与计算结果仍由菜单 Layout 持有。
pub(crate) mod menu {
    /// 窄屏排版断点（含边界）。
    pub(crate) const NARROW_BREAKPOINT: f32 = 640.0;
    /// 收起装饰插图的断点（含边界）。
    pub(crate) const COMPACT_BREAKPOINT: f32 = 900.0;
    /// 标题字号按视口宽度取值的比例。
    pub(crate) const TITLE_SCALE: f32 = 0.12;
    /// 窄屏标题字号比例。
    pub(crate) const TITLE_SCALE_NARROW: f32 = 0.20;
    /// 标题字号下限。
    pub(crate) const TITLE_MIN: f32 = 64.0;
    /// 标题字号上限。
    pub(crate) const TITLE_MAX: f32 = 176.0;
    /// 标题美术资源的导出参考字号；不随响应式字号上限变化。
    pub(crate) const TITLE_REFERENCE_SIZE: f32 = 176.0;
    /// 窄屏标题字号下限。
    pub(crate) const TITLE_MIN_NARROW: f32 = 54.4;
    /// 窄屏标题字号上限。
    pub(crate) const TITLE_MAX_NARROW: f32 = 80.0;
    /// 导出字体实测字宽，参考字号见 TITLE_REFERENCE_SIZE。
    pub(crate) const TITLE_TEXT_WIDTH: f32 = 736.046875;
    /// 标题纹理在参考字号下的左侧光学溢出。
    pub(crate) const TITLE_OVERHANG_X: f32 = 80.0;
    /// 标题纹理在参考字号下的顶部光学溢出。
    pub(crate) const TITLE_OVERHANG_Y: f32 = 40.0;
    /// 标题纹理逻辑宽度。
    pub(crate) const TITLE_IMAGE_WIDTH: f32 = 897.0;
    /// 标题纹理逻辑高度。
    pub(crate) const TITLE_IMAGE_HEIGHT: f32 = 278.0;
    /// 标题布局行高相对字号的比例。
    pub(crate) const TITLE_LINE_HEIGHT: f32 = 0.84;
    /// 窄屏左右留白之和。
    pub(crate) const NARROW_GUTTERS: f32 = 44.8;
    /// 单侧留白相对视口宽度的比例。
    pub(crate) const GUTTER_SCALE: f32 = 0.05;
    /// 单侧留白下限。
    pub(crate) const GUTTER_MIN: f32 = 22.4;
    /// 单侧留白上限。
    pub(crate) const GUTTER_MAX: f32 = 80.0;
    /// 插图宽度相对视口宽度的比例。
    pub(crate) const CUBE_SCALE: f32 = 0.30;
    /// 插图宽度下限。
    pub(crate) const CUBE_MIN: f32 = 260.0;
    /// 插图宽度上限。
    pub(crate) const CUBE_MAX: f32 = 420.0;
    /// 插图原始 viewBox 宽度。
    pub(crate) const CUBE_VIEWBOX_WIDTH: f32 = 470.0;
    /// 插图原始 viewBox 高度。
    pub(crate) const CUBE_VIEWBOX_HEIGHT: f32 = 320.0;
    /// 标题与插图间距相对视口宽度的比例。
    pub(crate) const HERO_GAP_SCALE: f32 = 0.06;
    /// 标题与插图间距下限。
    pub(crate) const HERO_GAP_MIN: f32 = 32.0;
    /// 标题与插图间距上限。
    pub(crate) const HERO_GAP_MAX: f32 = 96.0;
    /// 操作标签字号相对视口宽度的比例。
    pub(crate) const LABEL_SCALE: f32 = 0.021;
    /// 操作标签字号下限。
    pub(crate) const LABEL_MIN: f32 = 18.4;
    /// 操作标签字号上限。
    pub(crate) const LABEL_MAX: f32 = 24.0;
    /// 窄屏操作标签字号。
    pub(crate) const LABEL_NARROW: f32 = 17.6;
    /// 操作标签字距相对字号的比例。
    pub(crate) const LABEL_TRACKING: f32 = 0.06;
    /// 菜单项纵向内边距之和。
    pub(crate) const ROW_PADDING: f32 = 33.6;
    /// 窄屏菜单项纵向内边距之和。
    pub(crate) const ROW_PADDING_NARROW: f32 = 30.4;
    /// 标题与菜单间距相对视口高度的比例。
    pub(crate) const MENU_MARGIN_SCALE: f32 = 0.045;
    /// 标题与菜单间距下限。
    pub(crate) const MENU_MARGIN_MIN: f32 = 32.0;
    /// 标题与菜单间距上限。
    pub(crate) const MENU_MARGIN_MAX: f32 = 51.2;
    /// 窄屏标题与菜单间距。
    pub(crate) const MENU_MARGIN_NARROW: f32 = 28.8;
    /// 菜单最大宽度。
    pub(crate) const MENU_MAX_WIDTH: f32 = 480.0;
    /// 提示与菜单间距。
    pub(crate) const HINT_GAP: f32 = 21.6;
    /// 提示预留行高。
    pub(crate) const HINT_HEIGHT: f32 = 15.0;
    /// 顶部保留区高度。
    pub(crate) const TOP: f32 = 60.8;
    /// 窄屏顶部保留区高度。
    pub(crate) const TOP_NARROW: f32 = 41.6;
    /// 底部保留区高度。
    pub(crate) const BOTTOM: f32 = 74.78;
    /// 窄屏底部保留区高度。
    pub(crate) const BOTTOM_NARROW: f32 = 59.2;
    /// 内容区顶部内边距。
    pub(crate) const PADDING_TOP: f32 = 16.0;
    /// 窄屏内容区顶部内边距。
    pub(crate) const PADDING_TOP_NARROW: f32 = 24.0;
    /// 内容区底部内边距。
    pub(crate) const PADDING_BOTTOM: f32 = 40.0;
    /// 窄屏内容区底部内边距。
    pub(crate) const PADDING_BOTTOM_NARROW: f32 = 32.0;
    /// 编号字号。
    pub(crate) const INDEX_SIZE: f32 = 11.52;
    /// 编号字距。
    pub(crate) const INDEX_SPACING: f32 = 1.3824;
    /// 标签相对编号的左侧偏移。
    pub(crate) const LABEL_OFFSET: f32 = 30.72;
    /// 箭头字号与预留宽度。
    pub(crate) const ARROW_SIZE: f32 = 16.0;
    /// 箭头右侧留白。
    pub(crate) const ARROW_INSET: f32 = 12.0;
    /// 菜单内容左侧留白。
    pub(crate) const ROW_INSET: f32 = 5.6;
    /// 窄屏菜单内容左侧留白。
    pub(crate) const ROW_INSET_NARROW: f32 = 3.2;
    /// 聚焦时菜单内容的位移距离。
    pub(crate) const ACTION_TRAVEL: f32 = 8.0;
    /// 聚焦时箭头的额外位移距离。
    pub(crate) const ARROW_TRAVEL: f32 = 5.0;
    /// 插图上下漂浮的位移距离。
    pub(crate) const AMBIENT_TRAVEL: f32 = 18.0;
    /// 键盘提示字号。
    pub(crate) const HINT_SIZE: f32 = 11.2;
    /// 键盘提示字距。
    pub(crate) const HINT_SPACING: f32 = 1.344;
    /// 版本文字字号。
    pub(crate) const BUILD_SIZE: f32 = 10.88;
    /// 版本文字字距。
    pub(crate) const BUILD_SPACING: f32 = 1.9584;
    /// 版本文字底部留白。
    pub(crate) const BUILD_BOTTOM: f32 = 30.4;
    /// 窄屏版本文字底部留白。
    pub(crate) const BUILD_BOTTOM_NARROW: f32 = 25.6;
    /// 窄屏版本文字左侧留白。
    pub(crate) const BUILD_LEFT_NARROW: f32 = 19.2;
    /// 版本文字右侧留白相对视口宽度的比例。
    pub(crate) const BUILD_RIGHT_SCALE: f32 = 0.045;
    /// 版本文字右侧留白下限。
    pub(crate) const BUILD_RIGHT_MIN: f32 = 22.4;
    /// 版本文字右侧留白上限。
    pub(crate) const BUILD_RIGHT_MAX: f32 = 54.4;
    /// 角标边长。
    pub(crate) const CORNER_SIZE: f32 = 22.0;
    /// 角标距视口边缘的距离。
    pub(crate) const CORNER_INSET: f32 = 18.0;
}
