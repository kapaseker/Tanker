//! 应用组装、页面状态与过渡流程。

pub(crate) mod chrome;
pub(crate) mod state;
pub(crate) mod transition;

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;
use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};

use crate::{gameplay, pages, tank, terrain, ui, validation};
use state::GameState;

/// 全局灯光的亮度（cd/m²）。场景不使用 `DirectionalLight`，全部光照由全局环境光
/// 与相机上的 `EnvironmentMapLight` 环境贴图提供，因此这里给得比默认值（80）高。
/// 观感不合适时只需调这一个数。
const GLOBAL_AMBIENT_BRIGHTNESS: f32 = 5000.0;

/// 普通游戏与离屏验证共用场景、资源及业务插件，仅切换运行和输出方式。
pub(super) fn run(options: Option<validation::Options>) -> Result<(), String> {
    if let Some(options) = &options {
        options.prepare()?;
    }
    let _watchdog = options.as_ref().map(validation::Options::watchdog);
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
        // 统一资源尺寸：1 世界单位 = 1 米。
        VoxScenePlugin {
            global_settings: Some(VoxLoaderSettings {
                voxel_size: crate::tank::model::VOXEL_SIZE,
                ..default()
            }),
        },
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
    // 每帧最早修正最小化窗口的 0×0 分辨率，防止 UI 布局断言崩溃。
    .add_systems(First, clamp_zero_window_size);
    if let Some(options) = options {
        validation::run(app, options)
    } else {
        app.run();
        Ok(())
    }
}

/// Windows 上最小化窗口时，winit 会把物理分辨率上报为 0×0（`react_to_resize` 无防护地
/// 写入 `Window.resolution`），taffy 据此算出零尺寸 UI 节点，触发 bevy_math
/// `Rect::from_center_size` 的 "Rect size must be positive" 断言崩溃。
/// 每帧最早把零尺寸钳制为 1×1；bevy_winit 不会把该修改回推给 OS，恢复窗口时
/// winit 的真实 Resized 事件会覆盖回正确值。
fn clamp_zero_window_size(mut windows: Query<&mut Window>) {
    for mut window in &mut windows {
        let size = window.resolution.physical_size();
        if size.x == 0 || size.y == 0 {
            window
                .resolution
                .set_physical_resolution(size.x.max(1), size.y.max(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::window::WindowResolution;

    /// 最小化导致的 0×0 分辨率必须被钳制为正值，否则 UI 布局断言崩溃。
    #[test]
    fn zero_window_size_is_clamped_to_positive() {
        let mut app = App::new();
        app.add_systems(First, clamp_zero_window_size);
        let entity = app
            .world_mut()
            .spawn(Window {
                resolution: WindowResolution::new(0, 0),
                ..default()
            })
            .id();
        app.update();
        let window = app.world().get::<Window>(entity).unwrap();
        assert!(window.width() > 0.0, "逻辑宽度应为正值：{}", window.width());
        assert!(window.height() > 0.0, "逻辑高度应为正值：{}", window.height());
    }
}
