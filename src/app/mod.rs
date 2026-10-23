//! 应用组装、页面状态与过渡流程。

pub(crate) mod state;
pub(crate) mod transition;

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;
use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};

use crate::{gameplay, ground, offscreen, pages, tank, ui};
use state::GameState;

/// 全局灯光的亮度（cd/m²）。场景不使用 `DirectionalLight`，全部光照由全局环境光
/// 与相机上的 `EnvironmentMapLight` 环境贴图提供，因此这里给得比默认值（80）高。
/// 观感不合适时只需调这一个数。
const GLOBAL_AMBIENT_BRIGHTNESS: f32 = 5000.0;

/// 普通游戏与离屏验证共用场景、资源及业务插件，仅切换运行和输出方式。
pub(super) fn run(options: Option<offscreen::Options>) -> Result<(), String> {
    if let Some(options) = &options {
        options.prepare()?;
    }
    let _watchdog = options.as_ref().map(offscreen::Options::watchdog);
    let mut app = App::new();
    if options.is_some() {
        offscreen::configure(&mut app);
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
        ground::GroundPlugin,
        gameplay::enemy::EnemyPlugin,
        gameplay::combat::explosion::ExplosionPlugin,
        gameplay::player::PlayerPlugin,
        tank::driving::DrivingPlugin,
        tank::track::TrackAnimationPlugin,
        gameplay::camera::FollowCameraPlugin,
        ui::font::UiFontPlugin,
        transition::TransitionPlugin,
        pages::menu::MenuPlugin,
        pages::gallery::GalleryPlugin,
    ))
    .insert_resource(GlobalAmbientLight {
        brightness: GLOBAL_AMBIENT_BRIGHTNESS,
        ..default()
    })
    // 生成随机配置由启动入口注入：离屏验证固定种子，普通游戏每进程随机。
    // 业务模块只读该配置，不引用验证入口的 Options。
    .insert_resource(gameplay::session::SpawnConfig {
        fixed_seed: options.as_ref().map(|_| offscreen::SEED),
    })
    .init_state::<GameState>()
    .add_systems(OnEnter(GameState::InGame), gameplay::session::setup)
    .add_systems(Update, gameplay::session::report_load);
    if let Some(options) = options {
        offscreen::run(app, options)
    } else {
        app.run();
        Ok(())
    }
}
