//! 敌人击毁处的体素爆炸表现：消费 [`EnemyDestroyed`] 消息，在被毁车体中心
//! 播放 `assets/tank/enemy/default/default_explosion.vox` 的 12 帧逐帧动画，播完删除整棵子树。
//!
//! 物理与逻辑清理（刚体、碰撞体、敌人实体）在射击系统命中瞬间已完成，
//! 本模块生成的实体不带任何物理组件，纯表现。

use crate::enemy::EnemyDestroyed;
use bevy::{asset::LoadState, prelude::*, world_serialization::WorldAsset};
use bevy_vox_scene::VoxelAnimationPlayer;
use std::time::Duration;

/// 爆炸场景资源路径（相对 assets/）。
const EXPLOSION_PATH: &str = "tank/enemy/default/default_explosion.vox";
/// 单帧显示时长，单位：秒；12 帧 × 1/12 ≈ 1 秒，与 scripts/generate_explosion.py 对齐。
const EXPLOSION_FRAME_RATE: f32 = 1.0 / 12.0;

/// 爆炸场景根标记；计时到期后整棵子树（含全部帧节点）随之销毁。
#[derive(Component)]
pub(super) struct Explosion;

/// 爆炸根实体的存活计时；时长由加载后的动画组件推导（见 [`arm_explosion_lifetime`]）。
#[derive(Component)]
struct ExplosionLifetime(Timer);

/// 预加载的爆炸场景句柄：只加载一次，各次命中克隆复用（句柄为引用计数）。
#[derive(Resource)]
struct ExplosionAssets(Option<Handle<WorldAsset>>);

/// 注册爆炸资源加载、击毁消息消费与到期清理。
pub(super) struct ExplosionPlugin;

impl Plugin for ExplosionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EnemyDestroyed>()
            .add_systems(Startup, load_explosion)
            .add_systems(
                Update,
                (
                    spawn_explosions,
                    report_explosion,
                    arm_explosion_lifetime,
                    despawn_finished_explosions,
                ),
            );
    }
}

/// 启动时预加载爆炸场景，存入资源供各次命中复用。
fn load_explosion(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ExplosionAssets(Some(assets.load(EXPLOSION_PATH))));
}

/// 成功或失败各只汇报一次；成功须等待依赖资源一并就绪。
fn report_explosion(assets: Res<ExplosionAssets>, server: Res<AssetServer>, mut reported: Local<bool>) {
    if *reported {
        return;
    }
    let Some(handle) = assets.0.as_ref() else { return };
    match server.get_load_state(handle) {
        Some(LoadState::Failed(error)) => {
            error!("爆炸资源加载失败：{error}");
            *reported = true;
        }
        _ if server.is_loaded_with_dependencies(handle) => {
            info!("爆炸资源加载完成：assets/{EXPLOSION_PATH}");
            *reported = true;
        }
        _ => {}
    }
}

/// 每条击毁消息在被毁车体中心生成一个爆炸场景根；资源未就绪时跳过并告警。
fn spawn_explosions(
    mut commands: Commands,
    mut destroyed: MessageReader<EnemyDestroyed>,
    assets: Res<ExplosionAssets>,
) {
    for event in destroyed.read() {
        let Some(handle) = assets.0.as_ref() else {
            warn!("爆炸资源尚未就绪，跳过本次爆炸");
            continue;
        };
        // mesh 以模型几何中心为原点（加载器默认 CENTER），根位置即爆炸球心。
        commands.spawn((
            Explosion,
            Transform::from_translation(event.position),
            WorldAssetRoot(handle.clone()),
        ));
    }
}

/// 动画实体出现后：覆盖帧率为 1/12 秒，并在爆炸根上挂等长存活计时。
///
/// 用轮询而非 Add observer：场景实例先脱离场景图生成、就绪后才挂到
/// WorldAssetRoot 根下，observer 触发时祖先链可能尚未建立。查询过滤
/// `Without<ExplosionLifetime>` 保证已武装的根不再命中，重复运行幂等。
fn arm_explosion_lifetime(
    mut commands: Commands,
    explosions: Query<(), (With<Explosion>, Without<ExplosionLifetime>)>,
    parents: Query<&ChildOf>,
    mut players: Query<(Entity, &mut VoxelAnimationPlayer)>,
) {
    'players: for (entity, mut player) in &mut players {
        // 动画实体挂在爆炸根的场景子树内；沿父链向上找到根实体。
        let mut root = entity;
        loop {
            if explosions.contains(root) {
                break;
            }
            match parents.get(root) {
                Ok(child_of) => root = child_of.parent(),
                // 场景实例尚未挂到爆炸根下，等下一帧再试。
                Err(_) => continue 'players,
            }
        }
        // 覆盖加载器默认的 1/8 秒帧率，与资产生成参数对齐。
        player.frame_rate = Duration::from_secs_f32(EXPLOSION_FRAME_RATE);
        // 提前半帧删除：crate 的 Forever 模式在播完最后一帧后会回跳第 0 帧
        //（亮黄起爆球回闪一帧），计时落在最后一帧播到一半处，正好卡在回闪前。
        let duration = EXPLOSION_FRAME_RATE * (player.frames.len() as f32 - 0.5);
        commands
            .entity(root)
            .insert(ExplosionLifetime(Timer::from_seconds(duration, TimerMode::Once)));
    }
}

/// 计时到期后删除爆炸根及其全部后代（despawn 递归删除子树）。
fn despawn_finished_explosions(
    mut commands: Commands,
    time: Res<Time>,
    mut explosions: Query<(Entity, &mut ExplosionLifetime)>,
) {
    for (entity, mut lifetime) in &mut explosions {
        lifetime.0.tick(time.delta());
        if lifetime.0.is_finished() {
            debug!("爆炸动画播放完毕，清理：{entity}");
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::AssetPlugin,
        camera::visibility::VisibilityClass,
        image::ImagePlugin,
        time::TimeUpdateStrategy,
        transform::components::TransformTreeChanged,
        world_serialization::WorldSerializationPlugin,
    };
    use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};
    use std::time::Instant;

    /// 真实 VOX 资产端到端：12 帧动画被识别、计时器按帧数推导、播完自清理。
    /// 同时兜底 scripts/generate_explosion.py 手写 nSHP 多模型字节布局的正确性。
    #[test]
    fn explosion_asset_plays_twelve_frames_then_self_destructs() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            ImagePlugin::default(),
            WorldSerializationPlugin,
            TransformPlugin,
            VoxScenePlugin {
                global_settings: Some(VoxLoaderSettings {
                    voxel_size: crate::VOXEL_SIZE,
                    ..default()
                }),
            },
            ExplosionPlugin,
        ))
        .register_type::<Visibility>()
        .register_type::<ViewVisibility>()
        .register_type::<InheritedVisibility>()
        .register_type::<VisibilityClass>()
        .register_type::<Transform>()
        .register_type::<GlobalTransform>()
        .register_type::<TransformTreeChanged>()
        .register_type::<Mesh3d>()
        .register_type::<MeshMaterial3d<StandardMaterial>>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 64.0,
        )));
        let asset = app.world().resource::<AssetServer>().load(EXPLOSION_PATH);
        let root = app
            .world_mut()
            .spawn((
                Explosion,
                WorldAssetRoot(asset),
                Transform::from_xyz(5.0, 0.7, 2.0),
            ))
            .id();
        // 真实资源异步加载；用现实时间限制等待，避免失败时测试无限循环。
        let mut players = app
            .world_mut()
            .query_filtered::<Entity, With<VoxelAnimationPlayer>>();
        let deadline = Instant::now() + Duration::from_secs(20);
        let player = loop {
            app.update();
            if let Some(entity) = players.iter(app.world()).next() {
                break entity;
            }
            assert!(Instant::now() < deadline, "爆炸动画资源未能就绪");
            std::thread::sleep(Duration::from_millis(1));
        };
        // 武装系统可能在发现 player 的下一帧才应用命令，先推进再断言。
        app.update();
        app.update();
        // 加载器识别出 12 个模型 → 12 帧；计时器按帧数推导为 (12-0.5)/12 秒。
        assert_eq!(
            app.world().get::<VoxelAnimationPlayer>(player).unwrap().frames.len(),
            12
        );
        let lifetime = app
            .world()
            .get::<ExplosionLifetime>(root)
            .expect("爆炸根应已挂存活计时");
        assert!(
            (lifetime.0.duration().as_secs_f32() - 11.5 / 12.0).abs() < 1e-5,
            "存活时长应为 11.5/12 秒：{:?}",
            lifetime.0.duration()
        );
        // 动画实体每帧挂一个子实体（VoxelAnimationFrame 未公开导出，按子数断言）。
        assert_eq!(app.world().get::<Children>(player).unwrap().len(), 12);
        // 推进 1.6 秒（8 × 0.2）超过 0.958 秒存活期，爆炸根与帧实体应一并删除。
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            0.2,
        )));
        for _ in 0..8 {
            app.update();
        }
        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().get_entity(player).is_err());
    }
}
