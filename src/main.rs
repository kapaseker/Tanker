//! Tanker —— 基于 Bevy 的 3D voxel 游戏工程。
//!
//! 当前阶段：接入 `bevy_vox_scene`，加载自制 MagicaVoxel 场景 `assets/tank.vox`。
//! `assets/pisa_diffuse.ktx2` 与 `pisa_specular.ktx2` 仍取自 bevy_vox_scene 官方仓库示例资源（环境贴图）。

use bevy::{
    asset::LoadState,
    core_pipeline::tonemapping::Tonemapping,
    pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceTransmission, ScreenSpaceTransmissionQuality},
    post_process::bloom::Bloom,
    prelude::*,
    world_serialization::WorldAsset,
};
use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};

/// 当前加载的 MagicaVoxel 场景资源路径（相对 `assets/`）。
const VOX_SCENE_PATH: &str = "tank.vox";

/// 全局灯光的亮度（cd/m²）。场景不使用 `DirectionalLight`，全部光照由全局环境光
/// 与相机上的 `EnvironmentMapLight` 环境贴图提供，因此这里给得比默认值（80）高。
/// 观感不合适时只需调这一个数。
const GLOBAL_AMBIENT_BRIGHTNESS: f32 = 1000.0;

/// vox 场景资源句柄，供加载状态汇报系统轮询。
#[derive(Resource)]
struct VoxScene(Handle<WorldAsset>);

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Tanker".into(),
                    ..default()
                }),
                ..default()
            }),
            // voxel_size 统一缩放：MagicaVoxel 一格 = 0.05 世界单位
            VoxScenePlugin {
                global_settings: Some(VoxLoaderSettings {
                    voxel_size: 0.05,
                    ..default()
                }),
            },
        ))
        .insert_resource(GlobalAmbientLight {
            brightness: GLOBAL_AMBIENT_BRIGHTNESS,
            ..default()
        })
        .add_systems(Startup, setup_scene)
        .add_systems(Update, report_vox_scene)
        .run();
}

/// 搭建 voxel 场景：相机（环境贴图 + 玻璃透射）与场景图根节点。
/// 场景不放任何 `DirectionalLight`/点光/聚光灯，照明只靠 `GLOBAL_AMBIENT_BRIGHTNESS`
/// 的全局环境光（`GlobalAmbientLight`）与相机上的 `EnvironmentMapLight`。
fn setup_scene(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Camera3d::default(),
        // SSAO 要求关闭 MSAA，两者必须成对配置
        Msaa::Off,
        // 让场景中的玻璃体素能折射其他物体
        ScreenSpaceTransmission {
            steps: 1,
            quality: ScreenSpaceTransmissionQuality::High,
        },
        Transform::from_xyz(8.0, 1.5, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
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
    commands.spawn(WorldAssetRoot(scene.clone()));
    commands.insert_resource(VoxScene(scene));
}

/// 加载状态汇报：成功与失败各打一行中文日志，作为无人值守验证的判据。
fn report_vox_scene(
    scene: Res<VoxScene>,
    asset_server: Res<AssetServer>,
    mut reported: Local<bool>,
) {
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
