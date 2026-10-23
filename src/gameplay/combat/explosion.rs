//! 敌人击毁处的体素爆炸表现：消费 [`EnemyDestroyed`] 消息，在被毁车体中心
//! 播放 `assets/effects/explosion.vox` 的 12 帧逐帧动画，播完删除整棵子树。
//!
//! 物理与逻辑清理（刚体、碰撞体、敌人实体）在射击系统命中瞬间已完成，
//! 本模块生成的实体不带任何物理组件，纯表现。

use crate::gameplay::enemy::EnemyDestroyed;
use bevy::{asset::LoadState, prelude::*, world_serialization::WorldAsset};
use bevy_vox_scene::{FlipbookFrame, FlipbookPlayer, FlipbookRepeatMode};
use flipbook::FlipbookSpeed;
use std::time::Duration;

/// 爆炸场景资源路径（相对 assets/）。
const EXPLOSION_PATH: &str = "effects/explosion.vox";
/// 单帧显示时长，单位：秒；12 帧 × 1/12 ≈ 1 秒，与 scripts/generate_explosion.py 对齐。
const EXPLOSION_FRAME_RATE: f32 = 1.0 / 12.0;
/// 原始动画直径4米，整体放大至8米以覆盖现有6.6米敌方车体；不改变播放时长。
const EXPLOSION_SCALE: f32 = 2.0;

/// 爆炸场景根标记；计时到期后整棵子树（含全部帧节点）随之销毁。
#[derive(Component)]
pub(crate) struct Explosion;

/// 爆炸根实体的存活计时；时长由加载后的动画组件推导（见 [`arm_explosion_lifetime`]）。
#[derive(Component)]
struct ExplosionLifetime(Timer);

/// 预加载的爆炸场景句柄：只加载一次，各次命中克隆复用（句柄为引用计数）。
#[derive(Resource)]
struct ExplosionAssets(Option<Handle<WorldAsset>>);

/// 离屏战斗开始前预热爆炸资源，命中帧不能因异步加载跳过表现。
pub(crate) fn assets_ready(world: &World) -> bool {
    world
        .get_resource::<ExplosionAssets>()
        .and_then(|assets| assets.0.as_ref())
        .is_some_and(|handle| {
            world
                .resource::<AssetServer>()
                .is_loaded_with_dependencies(handle)
        })
}

/// 注册爆炸资源加载、击毁消息消费与到期清理。
pub(crate) struct ExplosionPlugin;

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
fn report_explosion(
    assets: Res<ExplosionAssets>,
    server: Res<AssetServer>,
    mut reported: Local<bool>,
) {
    if *reported {
        return;
    }
    let Some(handle) = assets.0.as_ref() else {
        return;
    };
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
            crate::app::transition::SessionEntity,
            Transform::from_translation(event.position).with_scale(Vec3::splat(EXPLOSION_SCALE)),
            WorldAssetRoot(handle.clone()),
        ));
    }
}

/// 动画实体出现后：覆盖帧率为 1/12 秒、播完停在最后一帧，并在爆炸根上挂等长存活计时。
///
/// 用轮询而非 Add observer：场景实例先脱离场景图生成、就绪后才挂到
/// WorldAssetRoot 根下，observer 触发时祖先链可能尚未建立。爆炸根的
/// `Without<ExplosionLifetime>` 过滤保证已武装的根不再命中，重复运行幂等。
fn arm_explosion_lifetime(
    mut commands: Commands,
    explosions: Query<(), (With<Explosion>, Without<ExplosionLifetime>)>,
    parents: Query<&ChildOf>,
    frames: Query<(), With<FlipbookFrame>>,
    players: Query<(Entity, &Children), With<FlipbookPlayer>>,
) {
    'players: for (entity, children) in &players {
        // 帧实体与 player 同批生成于场景实例内；数不出帧说明尚未就绪，下帧再试。
        let frame_count = children
            .iter()
            .filter(|child| frames.contains(*child))
            .count();
        if frame_count == 0 {
            continue;
        }
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
        // 覆盖 FlipbookSpeed 默认的 8 fps 帧率，与资产生成参数对齐；
        // Count(1) 播完停在最后一帧（不再回跳第 0 帧闪回起爆球）。
        commands.entity(entity).insert((
            FlipbookSpeed(Duration::from_secs_f32(EXPLOSION_FRAME_RATE)),
            FlipbookRepeatMode::Count(1),
        ));
        // 存活时长恰好等于播放总时长，计时到期即清理整棵爆炸子树。
        let duration = EXPLOSION_FRAME_RATE * frame_count as f32;
        commands
            .entity(root)
            .insert(ExplosionLifetime(Timer::from_seconds(
                duration,
                TimerMode::Once,
            )));
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
        asset::AssetPlugin, camera::visibility::VisibilityClass, image::ImagePlugin,
        time::TimeUpdateStrategy, transform::components::TransformTreeChanged,
        world_serialization::WorldSerializationPlugin,
    };
    use bevy_vox_scene::{VoxLoaderSettings, VoxScenePlugin};
    use std::time::Instant;

    /// 返回唯一可见帧，重叠显示多帧也视为动画回归。
    fn visible_frame(world: &World, player: Entity) -> usize {
        let visible: Vec<_> = world
            .get::<Children>(player)
            .unwrap()
            .iter()
            .filter(|child| world.get::<Visibility>(*child) == Some(&Visibility::Inherited))
            .map(|child| world.get::<FlipbookFrame>(child).unwrap().0)
            .collect();
        assert_eq!(visible.len(), 1, "每个 player 必须恰好一帧可见");
        visible[0]
    }

    /// 复用真实动画插件和武装/清理系统；零步长用于建立层级而不消耗寿命。
    fn animation_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, flipbook::FlipbookPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .add_systems(
                Update,
                (arm_explosion_lifetime, despawn_finished_explosions).chain(),
            );
        app.update();
        app
    }

    /// 构造与 VOX 加载器相同的 12 帧直接子节点，不依赖资源加载时序。
    fn spawn_player(world: &mut World) -> Entity {
        let player = world.spawn(FlipbookPlayer::from_frame_count(12)).id();
        for index in 0..12 {
            world.spawn((
                ChildOf(player),
                FlipbookFrame(index),
                if index == 0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));
        }
        player
    }

    /// 等待层级挂载期间不得消耗寿命，武装后也不得每帧重新计时。
    #[test]
    fn delayed_attachment_arms_once_without_resetting_lifetime() {
        let mut app = animation_app();
        let root = app.world_mut().spawn(Explosion).id();
        let player = spawn_player(app.world_mut());
        app.world_mut()
            .get_mut::<FlipbookPlayer>(player)
            .unwrap()
            .is_paused = true;
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        for _ in 0..15 {
            app.update();
        }
        assert!(app.world().get::<ExplosionLifetime>(root).is_none());
        assert_eq!(
            app.world().get::<FlipbookSpeed>(player).unwrap().0,
            Duration::from_millis(125)
        );
        app.world_mut().entity_mut(player).insert(ChildOf(root));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        assert_eq!(
            app.world()
                .get::<ExplosionLifetime>(root)
                .unwrap()
                .0
                .elapsed(),
            Duration::ZERO
        );
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(
            app.world()
                .get::<ExplosionLifetime>(root)
                .unwrap()
                .0
                .elapsed(),
            Duration::from_millis(500)
        );
        for _ in 0..6 {
            app.update();
        }
        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().get_entity(player).is_err());
    }

    /// 错峰生成的动画必须各自推进、各自清理，且不能影响无关 player。
    #[test]
    fn staggered_explosions_play_and_despawn_independently() {
        let mut app = animation_app();
        let first = app.world_mut().spawn(Explosion).id();
        let first_player = spawn_player(app.world_mut());
        app.world_mut()
            .entity_mut(first_player)
            .insert(ChildOf(first));
        let unrelated = spawn_player(app.world_mut());
        app.update();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        for _ in 0..5 {
            app.update();
        }
        let second = app.world_mut().spawn(Explosion).id();
        let second_player = spawn_player(app.world_mut());
        app.world_mut()
            .entity_mut(second_player)
            .insert(ChildOf(second));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        app.update();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        for _ in 0..2 {
            app.update();
        }
        assert!(
            visible_frame(app.world(), first_player) > visible_frame(app.world(), second_player)
        );
        for _ in 0..4 {
            app.update();
        }
        assert!(app.world().get_entity(first).is_err());
        assert!(app.world().get_entity(first_player).is_err());
        assert!(app.world().get_entity(second).is_ok());
        assert_eq!(
            app.world()
                .get::<ExplosionLifetime>(second)
                .unwrap()
                .0
                .elapsed(),
            Duration::from_millis(600)
        );
        for _ in 0..5 {
            app.update();
        }
        assert!(app.world().get_entity(second).is_err());
        assert!(app.world().get_entity(second_player).is_err());
        assert_eq!(
            app.world().get::<FlipbookSpeed>(unrelated).unwrap().0,
            Duration::from_millis(125)
        );
    }

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
                    voxel_size: crate::tank::model::VOXEL_SIZE,
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
            0.0,
        )));
        // 走真实击毁消息，验证生产生成路径的尺寸和位置，而不是测试中手动缩放。
        let position = Vec3::new(5.0, crate::gameplay::enemy::ENEMY_SIZE.y / 2.0, 2.0);
        app.world_mut().write_message(EnemyDestroyed { position });
        app.update();
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<Explosion>>()
            .single(app.world())
            .unwrap();
        let transform = app.world().get::<Transform>(root).unwrap();
        assert_eq!(transform.translation, position);
        assert_eq!(transform.scale, Vec3::splat(2.0));
        // 真实资源异步加载；用现实时间限制等待，避免失败时测试无限循环。
        let mut players = app
            .world_mut()
            .query_filtered::<Entity, With<FlipbookPlayer>>();
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
        // 加载器识别出 12 个模型 → 12 个帧子实体；计时器按帧数推导为 1 秒。
        assert_eq!(app.world().get::<Children>(player).unwrap().len(), 12);
        assert_eq!(
            app.world().get::<FlipbookSpeed>(player).unwrap().0,
            Duration::from_secs_f32(EXPLOSION_FRAME_RATE)
        );
        assert!(matches!(
            app.world().get::<FlipbookRepeatMode>(player),
            Some(FlipbookRepeatMode::Count(1))
        ));
        let lifetime = app
            .world()
            .get::<ExplosionLifetime>(root)
            .expect("爆炸根应已挂存活计时");
        assert!(
            (lifetime.0.duration().as_secs_f32() - 1.0).abs() < 1e-5,
            "存活时长应为 12/12 秒：{:?}",
            lifetime.0.duration()
        );
        // 10ms 小步长遍历全部帧，检查末帧没有回闪、也没有提前清理。
        let descendants: Vec<_> = app
            .world()
            .get::<Children>(player)
            .unwrap()
            .iter()
            .collect();
        let mut seen = vec![visible_frame(app.world(), player)];
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            10,
        )));
        for step in 1..=99 {
            app.update();
            assert!(
                app.world().get_entity(root).is_ok(),
                "第 {step} 步不应提前清理"
            );
            let frame = visible_frame(app.world(), player);
            if frame != *seen.last().unwrap() {
                seen.push(frame);
            }
        }
        assert_eq!(
            seen,
            (0..12).collect::<Vec<_>>(),
            "必须顺序播放所有帧且不回闪"
        );
        // 清理系统与动画系统可能相差一帧，允许最多一个 10ms 更新的调度误差。
        for _ in 0..2 {
            app.update();
        }
        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().get_entity(player).is_err());
        assert!(
            descendants
                .iter()
                .all(|entity| app.world().get_entity(*entity).is_err())
        );
    }
}
