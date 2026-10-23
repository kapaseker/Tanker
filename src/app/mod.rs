//! 应用组装、页面状态与过渡流程。

pub(crate) mod state;
pub(crate) mod transition;

use avian3d::prelude::PhysicsPlugins;
use bevy::{
    asset::LoadState,
    core_pipeline::tonemapping::Tonemapping,
    pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceTransmission, ScreenSpaceTransmissionQuality},
    post_process::bloom::Bloom,
    prelude::*,
    world_serialization::WorldAsset,
};
use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};

use crate::{font, gallery, gameplay, ground, menu, offscreen, tank, ui_resources};
use state::GameState;

/// 当前加载的 MagicaVoxel 场景资源路径（相对 `assets/`）。
/// 三部件按 `tank/<部件>/<车型>/<变体>/` 分包，共用画布；炮管与履带挂载到玩家根实体。
const VOX_SCENE_PATH: &str = "tank/bodies/player/anvil/body.vox";

/// 全局灯光的亮度（cd/m²）。场景不使用 `DirectionalLight`，全部光照由全局环境光
/// 与相机上的 `EnvironmentMapLight` 环境贴图提供，因此这里给得比默认值（80）高。
/// 观感不合适时只需调这一个数。
const GLOBAL_AMBIENT_BRIGHTNESS: f32 = 5000.0;

/// vox 场景资源句柄，供加载状态汇报系统轮询。
#[derive(Resource)]
struct VoxScene(Handle<WorldAsset>);

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
                title: ui_resources::strings_cn::WINDOW_TITLE.into(),
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
        font::UiFontPlugin,
        transition::TransitionPlugin,
        menu::MenuPlugin,
        gallery::GalleryPlugin,
    ))
    .insert_resource(GlobalAmbientLight {
        brightness: GLOBAL_AMBIENT_BRIGHTNESS,
        ..default()
    })
    .init_state::<GameState>()
    .add_systems(
        OnEnter(GameState::InGame),
        (setup_scene, ground::load_ground),
    )
    .add_systems(Update, report_vox_scene);
    if let Some(options) = options {
        offscreen::run(app, options)
    } else {
        app.run();
        Ok(())
    }
}

/// 搭建 voxel 场景：相机（环境贴图 + 玻璃透射）与场景图根节点。
/// 场景不放任何 `DirectionalLight`/点光/聚光灯，照明只靠 `GLOBAL_AMBIENT_BRIGHTNESS`
/// 的全局环境光（`GlobalAmbientLight`）与相机上的 `EnvironmentMapLight`。
fn setup_scene(
    mut commands: Commands,
    assets: Res<AssetServer>,
    target: Res<transition::SceneTarget>,
) {
    let tank_transform = Transform::from_xyz(0.0, tank::driving::TANK_SIZE.y / 2.0, 0.0);
    commands.spawn((
        Camera3d::default(),
        gameplay::camera::FollowCamera,
        transition::SessionEntity,
        Camera {
            order: -1,
            ..default()
        },
        bevy::camera::RenderTarget::Image(target.0.clone().into()),
        // SSAO 要求关闭 MSAA，两者必须成对配置
        Msaa::Off,
        // 让场景中的玻璃体素能折射其他物体
        ScreenSpaceTransmission {
            steps: 1,
            quality: ScreenSpaceTransmissionQuality::High,
        },
        gameplay::camera::camera_transform(&tank_transform),
        Tonemapping::BlenderFilmic,
        Bloom {
            intensity: 0.3,
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
        ScreenSpaceAmbientOcclusion::default(),
    ));

    // 加载整个 MagicaVoxel 场景为 Bevy 场景图
    let scene = assets.load(VOX_SCENE_PATH);
    commands.spawn((
        gameplay::player::Tank,
        transition::SessionEntity,
        tank::track::PLAYER_TRACKS,
        WorldAssetRoot(scene.clone()),
        tank::driving::tank_body(),
        tank_transform,
    ));
    commands.insert_resource(VoxScene(scene));
}

/// 加载状态汇报：成功与失败各打一行中文日志，作为无人值守验证的判据。
/// 主菜单期场景尚未生成，资源不存在时静默跳过。
fn report_vox_scene(
    scene: Option<Res<VoxScene>>,
    asset_server: Res<AssetServer>,
    mut reported: Local<bool>,
) {
    let Some(scene) = scene else {
        return;
    };
    if *reported {
        return;
    }

    match asset_server.get_load_state(&scene.0) {
        Some(LoadState::Loaded) => {
            info!("vox 场景加载完成：assets/{}", VOX_SCENE_PATH);
            *reported = true;
        }
        Some(LoadState::Failed(err)) => {
            error!("vox 场景加载失败：{err}");
            *reported = true;
        }
        _ => {}
    }
}
