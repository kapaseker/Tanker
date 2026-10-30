//! 应用组装、页面状态与过渡流程。

pub(crate) mod chrome;
pub(crate) mod state;
pub(crate) mod transition;
pub(crate) mod vox_meta;

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;
use bevy_vox_scene::VoxScenePlugin;

use crate::{gameplay, pages, tank, terrain, ui, validation};
use state::GameState;

/// 全局灯光的亮度（cd/m²）。现有对局不使用 `DirectionalLight`，全部光照由全局环境光
/// 与相机上的 `EnvironmentMapLight` 环境贴图提供，因此这里给得比默认值（80）高。
/// 田园测试场景在自身相机上覆盖环境光，不受此亮度影响。
const GLOBAL_AMBIENT_BRIGHTNESS: f32 = 5000.0;

/// 普通游戏与离屏验证共用场景、资源及业务插件，仅切换运行和输出方式。
pub(super) fn run(options: Option<validation::Options>) -> Result<(), String> {
    if let Some(options) = &options {
        options.prepare()?;
    }
    let _watchdog = options.as_ref().map(validation::Options::watchdog);
    // 资产单位配置完整性：缺配置的 VOX 会被插件静默按默认 1 米/格加载，
    // 这里在任何加载开始前拦截，失败原因带路径直接 panic（与地形资产同策略）。
    if let Err(reason) = vox_meta::validate(&bevy::asset::io::file::FileAssetReader::get_base_path().join("assets")) {
        panic!("{reason}");
    }
    let mut app = App::new();
    if options.is_some() {
        validation::configure(&mut app);
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: ui::resources::strings_cn::WINDOW_TITLE.into(),
                ..default()
            }),
            ..default()
        }));
    }
    app.add_plugins((
        // 每份 VOX 的单位（米/格）来自资产旁的 .vox.meta；global_settings 必须保持
        // None，否则插件全局值会覆盖各文件配置（见 tests/vox_meta_settings.rs）。
        VoxScenePlugin::default(),
        gameplay::combat::firing::FiringPlugin,
        PhysicsPlugins::default(),
        terrain::GroundPlugin,
        gameplay::enemy::EnemyPlugin,
        gameplay::combat::explosion::ExplosionPlugin,
        gameplay::player::PlayerPlugin,
        tank::driving::DrivingPlugin,
        tank::track::TrackAnimationPlugin,
        gameplay::camera::FollowCameraPlugin,
        ui::font::UiFontPlugin,
        ui::widgets::WidgetsPlugin,
        transition::TransitionPlugin,
        pages::index::IndexPlugin,
        pages::gallery::GalleryPlugin,
    ))
    // 元组插件数已到上限，HUD 单独注册；其系统按状态门控，与注册顺序无关。
    .add_plugins((gameplay::hud::HudPlugin, pages::pastoral::PastoralPlugin))
    // 必须在 TransitionPlugin 之后：chrome 相机需要已插入的 SceneTarget。
    .add_plugins((pages::detail::DetailPlugin, chrome::ChromePlugin))
    .insert_resource(GlobalAmbientLight {
        brightness: GLOBAL_AMBIENT_BRIGHTNESS,
        ..default()
    })
    // 生成随机配置由启动入口注入：离屏验证固定种子，普通游戏每进程随机。
    // 业务模块只读该配置，不引用验证入口的 Options。
    .insert_resource(gameplay::session::SpawnConfig {
        fixed_seed: options.as_ref().map(|_| validation::SEED),
    })
    .init_state::<GameState>()
    .add_systems(OnEnter(GameState::InGame), gameplay::session::setup)
    .add_systems(Update, gameplay::session::report_load)
    // 每帧最早把退化窗口分辨率钳制到下限，防止最小化/极小窗口时 UI 断言崩溃。
    .add_systems(First, clamp_window_resolution);
    if let Some(options) = options {
        validation::run(app, options)
    } else {
        app.run();
        Ok(())
    }
}

/// 窗口分辨率下限（宽、高，物理像素）。对应首页 zoom = 0.5、10px 字号实际按
/// 5px 光栅化，仍处正常渲染区，远离两处退化断言（Rect 尺寸、skrifa 自动微调）。
const MIN_WINDOW_RESOLUTION: (u32, u32) = (640, 360);

/// Windows 最小化窗口时 winit 会把物理分辨率上报为 0×0（`react_to_resize` 无防护地
/// 写入 `Window.resolution`），引发两类崩溃：
/// 1. taffy 算出零尺寸 UI 节点，触发 bevy_math `Rect::from_center_size`
///    的 "Rect size must be positive" 断言；
/// 2. 分辨率 0 使首页菜单相机的 zoom 归零，中文文字以近零字号进入 skrifa
///    自动微调，触发定点减法溢出（"attempt to subtract with overflow"）。
/// 把退化分辨率钳制到下限即可；bevy_winit 不会把该修改回推给 OS，恢复窗口时
/// winit 的真实 Resized 事件会覆盖回正确值。小于下限的真实窗口（如拖到 100px）
/// 同样会撞上第 2 类崩溃，故统一按下限钳制。
fn clamp_window_resolution(mut windows: Query<&mut Window>) {
    let (min_w, min_h) = MIN_WINDOW_RESOLUTION;
    for mut window in &mut windows {
        let size = window.resolution.physical_size();
        if size.x < min_w || size.y < min_h {
            window
                .resolution
                .set_physical_resolution(size.x.max(min_w), size.y.max(min_h));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::Assets;
    use bevy::color::Color;
    use bevy::ecs::entity::Entity;
    use bevy::image::Image;
    use bevy::math::Vec2;
    use bevy::text::{
        ComputedTextBlock, Font, FontAtlasSet, FontCx, FontHinting, FontSize, FontSource, FontWeight,
        Justify, LayoutCx, LetterSpacing, LineHeight, RemSize, ScaleCx, TextBounds, TextElement,
        TextFont, TextLayout, TextLayoutInfo, TextPipeline,
    };
    use bevy::window::WindowResolution;

    /// 退化分辨率（最小化的 0×0、拖拽到极小的 100×50）必须被钳制到下限，
    /// 否则 UI 布局与文字光栅化断言崩溃。
    #[test]
    fn degenerate_window_resolution_is_clamped_to_floor() {
        let mut app = App::new();
        app.add_systems(First, clamp_window_resolution);
        let (min_w, min_h) = MIN_WINDOW_RESOLUTION;
        for (w, h) in [(0, 0), (100, 50)] {
            let entity = app
                .world_mut()
                .spawn(Window {
                    resolution: WindowResolution::new(w, h),
                    ..default()
                })
                .id();
            app.update();
            let window = app.world().get::<Window>(entity).unwrap();
            assert_eq!(
                window.resolution.physical_size(),
                uvec2(min_w, min_h),
                "{w}×{h} 应被钳制到下限"
            );
        }
    }

    /// 用项目实际 UI 字体 MiSans 以指定字号（scale_factor = 1.0 时 font_size 即
    /// 最终进入 skrifa 的 ppem）完整走一遍「排版 + 光栅化」管线。
    fn rasterize_chinese(bytes: &[u8], font_size: f32, weight: u16) -> Result<(), String> {
        let mut fonts = Assets::<Font>::default();
        let handle = fonts.add(Font::from_bytes(bytes.to_vec()));
        let text_font = TextFont {
            font: FontSource::Handle(handle),
            font_size: FontSize::Px(font_size),
            weight: FontWeight(weight),
            ..default()
        };

        let mut computed = ComputedTextBlock::default();
        let mut pipeline = TextPipeline::default();
        // 形状排版：把文本写进 ComputedTextBlock 的布局缓冲
        pipeline
            .create_text_measure(
                Entity::PLACEHOLDER,
                &fonts,
                std::iter::once((
                    Entity::PLACEHOLDER,
                    0,
                    TextElement::Text {
                        text: "坦克",
                        font: &text_font,
                        color: Color::WHITE,
                        line_height: LineHeight::RelativeToFont(1.2),
                        letter_spacing: LetterSpacing::Px(0.0),
                    },
                )),
                1.0,
                &TextLayout::no_wrap(),
                &mut computed,
                &mut FontCx::default(),
                &mut LayoutCx::default(),
                Vec2::MAX,
                RemSize(16.0),
            )
            .map_err(|err| format!("排版失败：{err}"))?;
        // 光栅化：字形画进图集，skrifa 自动微调在这一步执行
        pipeline
            .update_text_layout_info(
                &mut TextLayoutInfo::default(),
                &mut FontAtlasSet::default(),
                &mut Assets::<Image>::default(),
                &mut computed,
                &mut ScaleCx::default(),
                TextBounds::UNBOUNDED,
                Justify::default(),
                // bevy_ui 的 Text 组件默认 require FontHinting::Enabled，
                // 崩溃栈中的 skrifa autohint 正是从此开启
                FontHinting::Enabled,
            )
            .map_err(|err| format!("光栅化失败：{err}"))
    }

    /// 下限分辨率下 UI 文字的最小实际字号必须能正常走完排版 + 光栅化管线。
    /// 640×360 对应 zoom = 0.5，最小 10px 字号按 5px 光栅化（正常小窗口区间，
    /// 远离最小化时近零字号触发的 skrifa 自动微调定点溢出）。
    #[test]
    fn cjk_rasterization_at_clamp_floor_is_safe() {
        for weight in [400_u16, 700] {
            rasterize_chinese(&misans_bytes(), 5.0, weight)
                .unwrap_or_else(|err| panic!("下限字号 5px/字重 {weight} 光栅化失败：{err}"));
        }
    }

    fn misans_bytes() -> Vec<u8> {
        std::fs::read("assets/fonts/misans/MiSansVF.ttf").expect("读取 MiSans 字体")
    }
}
