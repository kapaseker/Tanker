//! 全局 UI 字体：启动时加载 MiSans 可变字体并保留句柄与光学度量。
//!
//! 首页菜单、键盘提示和版本信息共用字体；句柄保存在 [`UiFont`] 中，避免资产被释放。
//! [`TextOptics`] 从字体文件解析，供控件计算文字垂直光学居中补偿（见其文档）。
//! 许可：MiSans 由小米提供，免费商用但要求在软件中注明使用了 MiSans 字体；
//! 发布前需在界面中加入该声明，并随包附上官方许可协议。

use bevy::{asset::LoadState, prelude::*};
use std::path::PathBuf;

/// MiSans 可变字体路径（相对 `assets/`）；`wght` 轴范围 150–700。
const UI_FONT_PATH: &str = "fonts/misans/MiSansVF.ttf";

/// Bevy 0.20 `TextFont` 未显式设置行高时的默认值：行高 = 1.2 × 字号
/// （`bevy_text::LineHeight::RelativeToFont(1.2)`）。光学补偿模型必须按同一行高推导。
const BEVY_LINE_HEIGHT_FACTOR: f32 = 1.2;

/// 字体光学度量（字体单位坐标系，以 upem 归一），用于计算文字在
/// 固定高度容器内的垂直光学居中补偿。启动时从字体文件同步解析一次；
/// 全部 UI 文字共用同一字体，因此随 [`UiFont`] 一起携带传递。
/// 字段取值规则与 Bevy 文本后端（skrifa `Metrics::new`，FreeType 策略）一致：
/// OS/2 置 `USE_TYPO_METRICS` 标志时用 typo 三元组，否则用 hhea 三元组。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TextOptics {
    /// units_per_em：字体 em 方格的整数高度。
    upem: f32,
    /// 所选来源的 ascent，基线上方高度（字体单位，向上为正）。
    ascender: f32,
    /// 所选来源的 descent 绝对值，基线下方深度（字体单位）。
    descender: f32,
    /// 所选来源的 line_gap，排版行间距（字体单位）。
    line_gap: f32,
    /// OS/2.sCapHeight 大写字母墨迹高度（字体单位）。
    cap_height: f32,
}

impl TextOptics {
    /// 从字体文件原始字节解析光学度量；解析失败返回 None，调用方退化为零补偿。
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        use read_fonts::{FontRef, TableProvider, tables::os2::SelectionFlags};
        let font = FontRef::new(bytes).ok()?;
        let upem = font.head().ok()?.units_per_em() as f32;
        let hhea = font.hhea().ok()?;
        let mut ascender = i16::from(hhea.ascender()) as f32;
        let mut descender = (i16::from(hhea.descender()) as f32).abs();
        let mut line_gap = i16::from(hhea.line_gap()) as f32;
        let os2 = font.os2().ok();
        // sCapHeight 仅 OS/2 v2+ 提供且可能为 0；缺失时按大写字母 ≈ 0.7×ascender 近似。
        let cap_height = os2
            .as_ref()
            .and_then(|os2| os2.s_cap_height())
            .filter(|v| *v > 0)
            .map(|v| v as f32)
            .unwrap_or(ascender * 0.7);
        // 与 skrifa 相同的行高度量来源选择：USE_TYPO_METRICS 置位则改用 OS/2 typo 三元组。
        if let Some(os2) = &os2
            && os2
                .fs_selection()
                .contains(SelectionFlags::USE_TYPO_METRICS)
        {
            ascender = i16::from(os2.s_typo_ascender()) as f32;
            descender = (i16::from(os2.s_typo_descender()) as f32).abs();
            line_gap = i16::from(os2.s_typo_line_gap()) as f32;
        }
        (upem > 0.0 && ascender > 0.0).then_some(Self {
            upem,
            ascender,
            descender,
            line_gap,
            cap_height,
        })
    }

    /// 大写字母墨迹在 Bevy 文本行框内垂直居中所需的向下偏移（CSS 像素）。
    ///
    /// 完整复刻 parley 0.11 的量化行盒模型（`line_break.rs`，mimic Chrome）：
    /// 行高 `lh = 1.2 × font_size`（Bevy 默认行高）；ascent/descent 分别四舍五入
    /// 到整像素得 `asc_r`、`desc_r`；富余 `leading = lh − asc_r − desc_r` 按
    /// `above = floor(leading/2)`、`below = round(leading) − above` 分配；
    /// 基线位于行框顶部下 `baseline = asc_r + above` 处。大写字母墨迹占
    /// `[baseline − cap, baseline]`，其中心与行框中心 `lh/2` 之差即补偿量。
    /// 对任意字体、任意字号成立，取代按单一字号标定的硬编码常数。
    /// 例：MiSans 10px → asc_r=10、desc_r=3、leading=−1、baseline=9、cap=7.4 → shift=0.7。
    pub(crate) fn optical_shift(&self, font_size: f32) -> f32 {
        if self.upem <= 0.0 {
            return 0.0;
        }
        let scale = font_size / self.upem;
        let line_height = font_size * BEVY_LINE_HEIGHT_FACTOR;
        let ascent = (self.ascender * scale).round();
        let descent = (self.descender * scale).round();
        let leading_above = ((line_height - ascent - descent) * 0.5).floor();
        let baseline = ascent + leading_above;
        let cap = self.cap_height * scale;
        line_height * 0.5 - (baseline - cap * 0.5)
    }
}

/// 全局 UI 字体句柄与光学度量；首页通过 `Res<UiFont>` 取用。
#[derive(Resource)]
pub(crate) struct UiFont(pub(crate) Handle<Font>, pub(crate) TextOptics);

/// 注册字体加载与状态汇报系统。
pub(crate) struct UiFontPlugin;

impl Plugin for UiFontPlugin {
    fn build(&self, app: &mut App) {
        // 构建期直接发起加载并插入资源：初始 OnEnter(Index) 发生在启动期
        // StateTransition（早于 Startup），spawn_index 依赖本资源必须提前就位。
        let handle = app.world().resource::<AssetServer>().load(UI_FONT_PATH);
        // 光学度量在构建期同步读盘解析：控件在字体资产加载完成前就已 spawn，
        // 内边距补偿必须当场确定，不能等异步资产就绪。
        let optics = font_file_bytes().and_then(|bytes| TextOptics::from_bytes(&bytes));
        if optics.is_none() {
            warn!("字体光学度量解析失败：文字垂直光学居中补偿退化为 0");
        }
        app.insert_resource(UiFont(handle, optics.unwrap_or_default()))
            .add_systems(Update, report_ui_font);
    }
}

/// 按 Bevy AssetPlugin 的根目录优先级（BEVY_ASSET_ROOT → 工程目录 → 可执行文件目录）
/// 依次尝试读取字体文件原始字节。
fn font_file_bytes() -> Option<Vec<u8>> {
    let relative = PathBuf::from("assets").join(UI_FONT_PATH);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()));
    [
        std::env::var("BEVY_ASSET_ROOT").ok().map(PathBuf::from),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
        exe_dir,
    ]
    .into_iter()
    .flatten()
    .find_map(|root| std::fs::read(root.join(&relative)).ok())
}

/// 成功与失败各汇报一次；失败必须显式报错，避免等到首个界面才发现缺字体。
fn report_ui_font(font: Res<UiFont>, assets: Res<AssetServer>, mut reported: Local<bool>) {
    if *reported {
        return;
    }
    match assets.get_load_state(&font.0) {
        Some(LoadState::Loaded) => {
            info!("UI 字体加载完成：assets/{UI_FONT_PATH}");
            *reported = true;
        }
        Some(LoadState::Failed(error)) => {
            error!("UI 字体加载失败：{error}");
            *reported = true;
        }
        _ => {}
    }
}

/// 字体是否已加载完成；离屏验证据此等待，确保日志与资源状态真实可见。
pub(crate) fn assets_ready(world: &World) -> bool {
    let Some(font) = world.get_resource::<UiFont>() else {
        return false;
    };
    world
        .get_resource::<AssetServer>()
        .is_some_and(|assets| assets.is_loaded_with_dependencies(&font.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn misans_optics_shift_matches_measured_keycap_offset() {
        let bytes = font_file_bytes().expect("MiSans 字体文件应可从工程 assets 目录读取");
        let optics = TextOptics::from_bytes(&bytes).expect("MiSans 应含 head/hhea/OS2 表");
        // MiSans 未置 USE_TYPO_METRICS，应取 hhea：asc=1044、desc=282、gap=0、cap=740。
        assert_eq!((optics.ascender, optics.descender, optics.cap_height), (1044.0, 282.0, 740.0));
        // 模型推导 10px 补偿 0.7px；像素实测（墨迹中心 vs 边框中心）偏上约 1px，量级一致。
        let shift = optics.optical_shift(10.0);
        assert!(
            (0.5..=1.0).contains(&shift),
            "optical_shift(10px) = {shift}，超出预期范围"
        );
        // 20px 时补偿仍为正且与字号同量级（量化取整使比例非严格线性）。
        let shift20 = optics.optical_shift(20.0);
        assert!((0.0..=2.5).contains(&shift20), "optical_shift(20px) = {shift20}");
    }

    #[test]
    fn default_optics_never_compensates() {
        assert_eq!(TextOptics::default().optical_shift(10.0), 0.0);
    }
}
