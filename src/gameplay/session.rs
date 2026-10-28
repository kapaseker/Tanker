//! 对局生命周期：场景生成、业务就绪汇总与退出清理的集中入口。
//!
//! `app::transition` 只协调页面切换并调用 [`content_ready`] / [`cleanup`]，
//! 不再逐项读取战斗、地形或履带细节；地形实体由 `terrain::reset_session`
//! 按地形模块自有的标记回收，本模块不直接了解地形内部结构。

use bevy::{
    asset::LoadState,
    prelude::*,
    world_serialization::{WorldAsset, WorldInstance, WorldInstanceSpawner},
};

use crate::app::transition;
use crate::gameplay::{camera, combat, enemy, health, player};
use crate::terrain;
use crate::tank;

/// 当前加载的 MagicaVoxel 场景资源路径（相对 `assets/`）。
/// 三部件按 `tank/<部件>/<车型>/<变体>/` 分包，共用画布；炮管与履带挂载到玩家根实体。
const VOX_SCENE_PATH: &str = "tank/bodies/player/anvil/body.vox";

/// 仅对局拥有的根实体标记；删除时连同场景实例与后代一起回收。
/// 玩家、敌人、混凝土墩、炮弹、爆炸与对局相机都挂此标记，[`cleanup`] 统一销毁。
#[derive(Component, Default, Clone)]
pub(crate) struct SessionEntity;

/// 启动注入的生成随机配置：`fixed_seed=Some` 为离屏验证的固定种子布局，
/// `None` 为普通游戏的每进程随机。业务只读本配置，不引用验证入口的 Options。
#[derive(Resource)]
pub(crate) struct SpawnConfig {
    pub(crate) fixed_seed: Option<u64>,
}

/// vox 场景资源句柄，供加载状态汇报系统轮询，同时保持资产存活。
/// 类型需 `pub(crate)` 以暴露给 `app` 注册的系统签名；句柄字段保持私有。
#[derive(Resource)]
pub(crate) struct VoxScene(Handle<WorldAsset>);

/// 进入 InGame 时搭建 voxel 场景：相机（环境贴图 + 玻璃透射）与玩家坦克根实体，
/// 并调用地形自身的加载入口登记地形图。
/// 场景不放任何 `DirectionalLight`/点光/聚光灯，照明只靠 `GlobalAmbientLight`
/// 的全局环境光与相机上的 `EnvironmentMapLight`。
pub(crate) fn setup(
    mut commands: Commands,
    assets: Res<AssetServer>,
    target: Res<transition::SceneTarget>,
) {
    let tank_transform = Transform::from_xyz(0.0, tank::driving::TANK_SIZE.y / 2.0, 0.0);
    commands.spawn((
        Camera3d::default(),
        camera::FollowCamera,
        SessionEntity,
        Camera {
            order: -1,
            ..default()
        },
        bevy::camera::RenderTarget::Image(target.0.clone().into()),
        // SSAO 要求关闭 MSAA，两者必须成对配置
        Msaa::Off,
        // 让场景中的玻璃体素能折射其他物体
        bevy::pbr::ScreenSpaceTransmission {
            steps: 1,
            quality: bevy::pbr::ScreenSpaceTransmissionQuality::High,
        },
        camera::camera_transform(&tank_transform),
        bevy::core_pipeline::tonemapping::Tonemapping::BlenderFilmic,
        bevy::post_process::bloom::Bloom {
            intensity: 0.3,
            ..default()
        },
        EnvironmentMapLight {
            diffuse_map: assets.load("pisa_diffuse.ktx2"),
            specular_map: assets.load("pisa_specular.ktx2"),
            intensity: 500.0,
            ..default()
        },
        bevy::pbr::ScreenSpaceAmbientOcclusion::default(),
    ));

    // 加载整个 MagicaVoxel 场景为 Bevy 场景图
    let scene = assets.load(VOX_SCENE_PATH);
    commands.spawn((
        player::Tank,
        SessionEntity,
        tank::track::PLAYER_TRACKS,
        WorldAssetRoot(scene.clone()),
        tank::driving::tank_body(),
        tank_transform,
        // 生命上限 100 点；当前无伤害系统，固定满血，HUD 由此读出比例。
        health::Health::full(100),
    ));
    commands.insert_resource(VoxScene(scene));

    // 地形入口由地形模块拥有：此处只负责在对局生成时调用其加载登记。
    terrain::load_ground(&mut commands, &assets);
    super::obstacle::spawn(&mut commands, &assets);
}

/// 加载状态汇报：成功与失败各打一行中文日志，作为无人值守验证的判据。
/// 主菜单期场景尚未生成，资源不存在时静默跳过。
pub(crate) fn report_load(
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

/// 对局业务就绪汇总：地形、武器资源、坦克与履带数量、场景实例全部到位。
/// 渲染/GPU 管线的真实就绪仍由过渡流程（及离屏入口）另行检查，不在此处替代。
pub(crate) fn content_ready(world: &mut World) -> bool {
    if !world.contains_resource::<terrain::TerrainReady>()
        || !combat::firing::assets_ready(world)
        || !combat::explosion::assets_ready(world)
    {
        return false;
    }
    let tanks = world
        .query_filtered::<(), Or<(With<player::Tank>, With<enemy::EnemyTank>)>>()
        .iter(world)
        .count();
    let tracks = world
        .query_filtered::<(), With<tank::track::TracksValidated>>()
        .iter(world)
        .count();
    if tanks != enemy::ENEMY_COUNT + 1 || tracks != tanks {
        return false;
    }
    let mut roots = world.query::<(&WorldAssetRoot, Option<&WorldInstance>)>();
    let server = world.resource::<AssetServer>();
    let spawner = world.resource::<WorldInstanceSpawner>();
    roots.iter(world).all(|(root, instance)| {
        server.is_loaded_with_dependencies(&root.0)
            && instance.is_some_and(|instance| spawner.instance_is_ready(**instance))
    })
}

/// 页面切回首页时移除整局：对局根实体、地形（经其自有清理入口）、
/// 敌人/射击的本局标志与 vox 场景句柄；预加载的共享资产继续复用。
/// 重复调用安全（幂等），二次清理不会 panic。
pub(crate) fn cleanup(world: &mut World) {
    let entities: Vec<_> = world
        .query_filtered::<Entity, With<SessionEntity>>()
        .iter(world)
        .collect();
    for entity in entities {
        world.despawn(entity);
    }
    terrain::reset_session(world);
    enemy::reset_session(world);
    combat::firing::reset_session(world);
    world.remove_resource::<VoxScene>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 重复退出必须幂等：首次清理回收对局实体与 vox 句柄资源，
    /// 第二次清理在资源已缺失时不 panic（防止状态机重复触发 OnEnter 造成崩溃）。
    #[test]
    fn repeated_cleanup_is_idempotent() {
        let mut world = World::new();
        let entity = world.spawn(SessionEntity).id();
        world.insert_resource(VoxScene(Handle::<WorldAsset>::default()));
        cleanup(&mut world);
        assert!(world.get_entity(entity).is_err(), "对局实体未被回收");
        assert!(!world.contains_resource::<VoxScene>(), "vox 句柄资源未被移除");
        // 二次清理：所有目标资源/实体均已缺失，仍须安全返回。
        cleanup(&mut world);
    }
}
