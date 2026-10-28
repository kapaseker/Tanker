//! UI 尺寸：除明确标注的比例外均为逻辑像素，不包含世界单位或测试输出分辨率。

/// 初始 UI 排版参考宽度，实际布局仍跟随视口。
pub(crate) const REFERENCE_WIDTH: f32 = 1280.0;
/// 初始 UI 排版参考高度。
pub(crate) const REFERENCE_HEIGHT: f32 = 720.0;
/// MiSans normal 行高相对字号的比例。
pub(crate) const LINE_HEIGHT: f32 = 4.0 / 3.0;
/// UI 细边框宽度。
pub(crate) const BORDER_WIDTH: f32 = 1.0;

/// 战斗 HUD 左下状态簇逻辑像素（以网页设计稿 style.css 的 .status 为基准）。
pub(crate) mod hud {
    /// 状态簇距屏幕左、下边缘的安全边距（hud.safe-inset）。
    pub(crate) const INSET: f32 = 24.0;
    /// 簇总宽（含边框与内边距）；内宽 222 = 16 图标 + 8 间距 + 198 轨道。
    pub(crate) const WIDTH: f32 = 240.0;
    /// 簇内边距，上下左右一致。
    pub(crate) const PADDING: f32 = 8.0;
    /// 行间距、图标与轨道的水平间距（网页 gap 8px）。
    pub(crate) const ROW_GAP: f32 = 8.0;
    /// 图标显示边长（PNG 资产为 8 倍分辨率）。
    pub(crate) const ICON_SIZE: f32 = 16.0;
    /// 生命轨道高 10px；装填轨道略矮 8px，形成主次。
    pub(crate) const HEALTH_TRACK_HEIGHT: f32 = 10.0;
    pub(crate) const RELOAD_TRACK_HEIGHT: f32 = 8.0;
    /// 装填分格数：5 秒冷却 → 5 格，每格 1 秒；间隔变化时与 firing 同步。
    pub(crate) const RELOAD_TICKS: u32 = 5;
}

/// 统一标题栏逻辑像素：全页面共用一个标题栏控件（高度、ESC 返回框、标题、全屏宽底部分割线），
/// 以画廊网页设计稿 .header/.back 为基准；新页面直接复用本模块常量，不再各自定义。
pub(crate) mod title_bar {
    /// 标题栏高度，也是内容区与滚动计算的顶部起点。
    pub(crate) const HEIGHT: f32 = 88.0;
    /// 以下矩形依次为左、上、宽、高，单位为所属父节点内的逻辑像素。
    /// 返回框在标题栏内的固定位置与尺寸（兼作屏幕逻辑坐标命中区域）。
    pub(crate) const BACK_RECT: (f32, f32, f32, f32) = (64.0, 20.0, 102.0, 48.0);
    /// 返回框内容水平内边距与子项间距（网页 .back 的 padding 18px / gap 12px）。
    pub(crate) const BACK_PADDING_X: f32 = 18.0;
    pub(crate) const BACK_GAP: f32 = 12.0;
    /// 返回箭头字号（网页 .back__arrow 的 16px）。
    pub(crate) const BACK_ARROW_SIZE: f32 = 16.0;
    /// 电源符号显示边长（首页变体，与箭头同规格 16px；PNG 资产为 8 倍分辨率）。
    pub(crate) const POWER_ICON_SIZE: f32 = 16.0;
    /// 页面标题在标题栏内的固定矩形与字号。文字行盒（1.2×20，Bevy 文本在固定高
    /// 节点内顶对齐）位于节点顶部；top=33 时实测墨迹中心 43.5，与 ESC 返回框
    /// 中心（栏中心 44）及框内键帽墨迹中心对齐。
    pub(crate) const TITLE_RECT: (f32, f32, f32, f32) = (182.0, 33.0, 300.0, 24.0);
    pub(crate) const TITLE_SIZE: f32 = 20.0;
}

/// 画廊逻辑像素，与固定四列的网页设计稿保持一致。
pub(crate) mod gallery {
    /// 页面内容宽度，以及卡片和网格的固定尺寸。
    pub(crate) const WIDTH: f32 = 1152.0;
    pub(crate) const CARD_WIDTH: f32 = 270.0;
    pub(crate) const CARD_HEIGHT: f32 = 282.0;
    pub(crate) const GAP: f32 = 24.0;
    pub(crate) const SECTION_GAP: f32 = 60.0;
    pub(crate) const PREVIEW_HEIGHT: f32 = 180.0;
    pub(crate) const CARD_PADDING: f32 = 20.0;
    pub(crate) const HEADING_SIZE: f32 = 24.0;
    pub(crate) const NAME_SIZE: f32 = 18.0;
    pub(crate) const META_SIZE: f32 = 12.0;
    pub(crate) const SCROLL_STEP: f32 = 64.0;
    pub(crate) const NAME_RECT: (f32, f32, f32, f32) = (20.0, 216.0, 228.0, 26.0);
    pub(crate) const META_RECT: (f32, f32, f32, f32) = (20.0, 248.0, 228.0, 20.0);
    /// 分类标题到卡片的距离，以及页面末尾留白。
    pub(crate) const SECTION_TO_CARD: f32 = 56.0;
    /// 网页 stage 的 40px 下内边距与 60px 页脚，均随内容滚动。
    pub(crate) const CONTENT_BOTTOM: f32 = 100.0;
    /// 型号字距与悬停底线高度。
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
    /// 首页内容区顶部标题栏高度（design.md：首页常驻 88px 标题栏，正文预留栏高；
    /// 其余页面同高但首页独用此常量，底部保留区仍为 BOTTOM）。
    pub(crate) const TITLE_BAR: f32 = 88.0;
    /// 底部保留区高度（网页 `.hud` 底部 30px 内边距）。
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
    /// 退出确认弹框面板最小宽度（网页 `.exit-dialog__panel`）。
    pub(crate) const EXIT_PANEL_WIDTH: f32 = 880.0;
    /// 弹框标题字号、行高与上/下留白（网页 `.exit-dialog__title` 48px 48px 84px）。
    pub(crate) const EXIT_TITLE_SIZE: f32 = 24.0;
    pub(crate) const EXIT_TITLE_LINE_HEIGHT: f32 = 1.2;
    pub(crate) const EXIT_TITLE_PAD_TOP: f32 = 48.0;
    pub(crate) const EXIT_TITLE_PAD_BOTTOM: f32 = 84.0;
    pub(crate) const EXIT_TITLE_PAD_X: f32 = 48.0;
    /// 弹框按钮固定尺寸（网页为文字自适应宽，游戏内取固定值使命中区与视觉一致：
    /// 标签两字 32px + gap 8px + 键帽约 19px + 水平内边距 32px）。
    pub(crate) const EXIT_BUTTON_WIDTH: f32 = 96.0;
    pub(crate) const EXIT_BUTTON_HEIGHT: f32 = 40.0;
    /// 弹框按钮字号与行内键帽间距（网页 `.exit-dialog__btn` 16px/700、gap 8px）。
    pub(crate) const EXIT_BUTTON_SIZE: f32 = 16.0;
    pub(crate) const EXIT_BUTTON_GAP: f32 = 8.0;
    /// 按钮行右对齐内边距、按钮间距与贴底留白（网页 `.exit-dialog__actions`）。
    pub(crate) const EXIT_ACTIONS_PAD_RIGHT: f32 = 48.0;
    pub(crate) const EXIT_ACTIONS_PAD_BOTTOM: f32 = 48.0;
    pub(crate) const EXIT_ACTIONS_GAP: f32 = 32.0;
    /// 弹框淡入时长（网页 `exit-dialog-in` 120ms ease-out）。
    pub(crate) const EXIT_FADE_IN: f32 = 0.12;
}

/// 伴随子弹与模型上缘的标准间距（世界单位：米，design.md 约定）。
pub(crate) const BULLET_GAP: f32 = 0.8;

/// 详情查看区与浮层的逻辑像素，取自 model-detail.html。
pub(crate) mod detail {
    /// 平面网格间距；从标题栏底部开始计格。
    pub(crate) const GRID: f32 = 48.0;
    /// 左上角分类眉题相对查看区的位置。
    pub(crate) const SPECS_LEFT: f32 = 64.0;
    pub(crate) const SPECS_TOP: f32 = 32.0;
    /// 眉题字号与字距（网页 .eyebrow 的 18px / 4px）。
    pub(crate) const EYEBROW_SIZE: f32 = 18.0;
    pub(crate) const EYEBROW_SPACING: f32 = 4.0;
    /// 眉题前置横线（网页 .eyebrow::before 的 26×1px）与文字间距。
    pub(crate) const EYEBROW_LINE: (f32, f32) = (26.0, 1.0);
    pub(crate) const EYEBROW_GAP: f32 = 12.0;
    /// 装备规格行（网页 .specs/.spec）：眉题与规格行、标签与数值的间距，
    /// 以及标签/数值/单位的字号与字距。
    pub(crate) const SPECS_GAP: f32 = 16.0;
    pub(crate) const SPEC_LABEL_GAP: f32 = 8.0;
    pub(crate) const SPEC_LABEL_SIZE: f32 = 18.0;
    pub(crate) const SPEC_LABEL_SPACING: f32 = 2.0;
    pub(crate) const SPEC_VALUE_SIZE: f32 = 28.0;
    pub(crate) const SPEC_VALUE_GAP: f32 = 8.0;
    pub(crate) const SPEC_UNIT_SIZE: f32 = 14.0;
    pub(crate) const SPEC_UNIT_SPACING: f32 = 2.0;
    /// 以下为尺寸轴几何（世界单位：米）；px→米换算按网页模型 37.8px/m 取整。
    /// 轴线与模型边沿的间距（网页 GAP 14px ≈ 0.37m）。
    pub(crate) const AXIS_GAP: f32 = 0.4;
    /// 端点刻度半长（网页 HALF_TICK 7px ≈ 0.19m）。
    pub(crate) const HALF_TICK: f32 = 0.2;
    /// 轴线/刻度/延长线的截面厚度；细长方体任意视角可见，等效网页 1px 线。
    pub(crate) const AXIS_THICKNESS: f32 = 0.02;
    /// 高度标签在高度轴外侧的偏移（网页 16px ≈ 0.42m）。
    pub(crate) const HEIGHT_TAG_OFFSET: f32 = 0.4;
    /// 尺寸标签沿离心方向推开的屏幕距离（逻辑像素，网页 LABEL_PUSH）。
    pub(crate) const LABEL_PUSH: f32 = 36.0;
    /// 尺寸标签字号（网页 .dims__name/.dims__val/.dims__unit 的 10/14/10px）。
    pub(crate) const TAG_NAME_SIZE: f32 = 10.0;
    pub(crate) const TAG_VALUE_SIZE: f32 = 14.0;
    pub(crate) const TAG_UNIT_SIZE: f32 = 10.0;
}
