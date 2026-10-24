//! UI 尺寸：除明确标注的比例外均为逻辑像素，不包含世界单位或测试输出分辨率。

/// 初始 UI 排版参考宽度，实际布局仍跟随视口。
pub(crate) const REFERENCE_WIDTH: f32 = 1280.0;
/// 初始 UI 排版参考高度。
pub(crate) const REFERENCE_HEIGHT: f32 = 720.0;
/// MiSans normal 行高相对字号的比例。
pub(crate) const LINE_HEIGHT: f32 = 4.0 / 3.0;
/// UI 细边框宽度。
pub(crate) const BORDER_WIDTH: f32 = 1.0;

/// 画廊逻辑像素，与固定四列的网页设计稿保持一致。
pub(crate) mod gallery {
    /// 页面内容宽度，以及卡片和网格的固定尺寸。
    pub(crate) const WIDTH: f32 = 1152.0;
    pub(crate) const CARD_WIDTH: f32 = 270.0;
    pub(crate) const CARD_HEIGHT: f32 = 282.0;
    pub(crate) const GAP: f32 = 24.0;
    pub(crate) const SECTION_GAP: f32 = 60.0;
    pub(crate) const HEADER_HEIGHT: f32 = 88.0;
    pub(crate) const FOOTER_HEIGHT: f32 = 60.0;
    pub(crate) const PREVIEW_HEIGHT: f32 = 180.0;
    pub(crate) const CARD_PADDING: f32 = 20.0;
    pub(crate) const HEADING_SIZE: f32 = 24.0;
    pub(crate) const NAME_SIZE: f32 = 18.0;
    pub(crate) const META_SIZE: f32 = 12.0;
    pub(crate) const SCROLL_STEP: f32 = 64.0;
    /// 以下矩形依次为左、上、宽、高，单位为所属父节点内的逻辑像素。
    pub(crate) const BACK_RECT: (f32, f32, f32, f32) = (64.0, 20.0, 102.0, 48.0);
    /// 返回框内容水平内边距与子项间距（网页 .back 的 padding 18px / gap 12px）。
    pub(crate) const BACK_PADDING_X: f32 = 18.0;
    pub(crate) const BACK_GAP: f32 = 12.0;
    /// 返回箭头字号（网页 .back__arrow 的 16px）。
    pub(crate) const BACK_ARROW_SIZE: f32 = 16.0;
    pub(crate) const TITLE_RECT: (f32, f32, f32, f32) = (182.0, 30.0, 300.0, 30.0);
    pub(crate) const NAME_RECT: (f32, f32, f32, f32) = (20.0, 216.0, 228.0, 26.0);
    pub(crate) const META_RECT: (f32, f32, f32, f32) = (20.0, 248.0, 228.0, 20.0);
    /// 分类标题到卡片的距离，以及页面末尾留白。
    pub(crate) const SECTION_TO_CARD: f32 = 56.0;
    pub(crate) const CONTENT_BOTTOM: f32 = 40.0;
    /// 页面标题字号、型号字距与悬停底线高度。
    pub(crate) const TITLE_SIZE: f32 = 20.0;
    pub(crate) const LETTER_SPACING: f32 = 2.0;
    pub(crate) const HOVER_LINE_HEIGHT: f32 = 2.0;
}

/// 首页设计参数；全部为 1280×720 参考分辨率下的固定设计常量（以网页原型
/// `web/style.css` 为基准），窗口适配由整体等比缩放承担，不再有断点与逐控件缩小。
pub(crate) mod index {
    /// 标题设计字号（网页 `.title` 的 154px）。
    pub(crate) const TITLE_SIZE: f32 = 154.0;
    /// 标题美术资源的导出参考字号；TITLE_SIZE / 本值 = 标题纹理缩放。
    pub(crate) const TITLE_REFERENCE_SIZE: f32 = 176.0;
    /// 导出字体实测字宽（含字距），参考字号见 TITLE_REFERENCE_SIZE。
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
    /// 插图固定宽度（网页 `.voxels` 的 384px）。
    pub(crate) const CUBE_WIDTH: f32 = 384.0;
    /// 插图原始 viewBox 宽度。
    pub(crate) const CUBE_VIEWBOX_WIDTH: f32 = 470.0;
    /// 插图原始 viewBox 高度。
    pub(crate) const CUBE_VIEWBOX_HEIGHT: f32 = 320.0;
    /// 标题组与插图的固定间距（网页 `.layout` 的 gap 76px）。
    pub(crate) const HERO_GAP: f32 = 76.0;
    /// 操作标签字号与字距（网页 `.menu__label` 的 24px / 2px）。
    pub(crate) const LABEL_SIZE: f32 = 24.0;
    pub(crate) const LABEL_SPACING: f32 = 2.0;
    /// 菜单项纵向内边距之和（网页 `.menu__item` 上下各 16px）。
    pub(crate) const ROW_PADDING: f32 = 32.0;
    /// 标题与菜单的固定间距（网页 `.menu` 的 margin-top 32px）。
    pub(crate) const MENU_MARGIN: f32 = 32.0;
    /// 菜单最大宽度。
    pub(crate) const MENU_MAX_WIDTH: f32 = 480.0;
    /// 提示与菜单间距（网页 `.status` 的 margin-top 22px）。
    pub(crate) const HINT_GAP: f32 = 22.0;
    /// 提示行预留高度（提示字号 × 行高）。
    pub(crate) const HINT_HEIGHT: f32 = 16.0;
    /// 顶部/底部保留区高度（网页 `.hud` 上下各 30px 内边距）。
    pub(crate) const TOP: f32 = 60.0;
    pub(crate) const BOTTOM: f32 = 60.0;
    /// 内容区顶部/底部内边距（网页 `.stage` 的 16px / 40px）。
    pub(crate) const PADDING_TOP: f32 = 16.0;
    pub(crate) const PADDING_BOTTOM: f32 = 40.0;
    /// 编号字号与字距（网页 `.menu__index` 的 12px / 2px）。
    pub(crate) const INDEX_SIZE: f32 = 12.0;
    pub(crate) const INDEX_SPACING: f32 = 2.0;
    /// 标签相对编号的左侧偏移：索引列实测宽 + 16px 网格间距（按新字号等比换算）。
    pub(crate) const LABEL_OFFSET: f32 = 32.0;
    /// 箭头字号与右侧留白（网页 `.menu__arrow` 的 16px 字号、12px padding-right）。
    pub(crate) const ARROW_SIZE: f32 = 16.0;
    pub(crate) const ARROW_INSET: f32 = 12.0;
    /// 菜单内容左侧留白（网页 `.menu__item` 的水平内边距 6px）。
    pub(crate) const ROW_INSET: f32 = 6.0;
    /// 聚焦时菜单内容的位移距离（padding-left 6px → 14px）。
    pub(crate) const ACTION_TRAVEL: f32 = 8.0;
    /// 聚焦时箭头的额外位移距离（网页 translateX(6px)）。
    pub(crate) const ARROW_TRAVEL: f32 = 6.0;
    /// 插图上下漂浮的位移距离。
    pub(crate) const AMBIENT_TRAVEL: f32 = 18.0;
    /// 键盘提示字号与字距（网页 `.status` 的 12px / 2px）。
    pub(crate) const HINT_SIZE: f32 = 12.0;
    pub(crate) const HINT_SPACING: f32 = 2.0;
    /// 键盘提示行内各项间距（网页 `.hud__hint` 的 gap 8px）。
    pub(crate) const HINT_ITEM_GAP: f32 = 8.0;
    /// 键帽字号与字距（网页 `kbd` 的 10px / 2px）。
    pub(crate) const KBD_SIZE: f32 = 10.0;
    pub(crate) const KBD_SPACING: f32 = 2.0;
    /// 键帽水平/垂直内边距（网页 `kbd` 的 4px 6px）。
    pub(crate) const KBD_PADDING_X: f32 = 6.0;
    pub(crate) const KBD_PADDING_Y: f32 = 4.0;
    /// 键帽圆角（网页 `kbd` 的 4px）。
    pub(crate) const KBD_RADIUS: f32 = 4.0;
    /// 版本文字字号与字距（网页 `.hud` 的 10px / 2px）。
    pub(crate) const BUILD_SIZE: f32 = 10.0;
    pub(crate) const BUILD_SPACING: f32 = 2.0;
    /// 版本文字右下角固定偏移（网页 `.hud__build` 的 right 54px / bottom 30px）。
    pub(crate) const BUILD_RIGHT: f32 = 54.0;
    pub(crate) const BUILD_BOTTOM: f32 = 30.0;
    /// 角标边长。
    pub(crate) const CORNER_SIZE: f32 = 22.0;
    /// 角标距视口边缘的距离（网页 `.frame` 的 inset 18px）。
    pub(crate) const CORNER_INSET: f32 = 18.0;
}
