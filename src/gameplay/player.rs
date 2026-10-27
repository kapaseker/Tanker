//! 玩家身份标记与方向键输入；过渡未放行时不写驾驶意图。

use crate::app::transition::Fade;
use crate::tank::driving::{self, DrivingInput};
use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;

/// 标记玩家控制的坦克根实体；驾驶、射击、相机与验证入口以它定位唯一玩家。
#[derive(Component)]
pub(crate) struct Tank;

/// 注册玩家方向键到驾驶意图的写入系统。
pub(crate) struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPostUpdate,
            // 输入写入先于驾驶消费，且两者都不越过物理步进。
            read_player_input
                .after(PhysicsSystems::Prepare)
                .before(PhysicsSystems::StepSimulation)
                .before(driving::drive_tank),
        );
    }
}

/// 方向相反的按键抵消；只由玩家输入系统调用。
fn input_from_keys(keys: &ButtonInput<KeyCode>) -> DrivingInput {
    DrivingInput {
        throttle: (i32::from(keys.pressed(KeyCode::ArrowUp))
            - i32::from(keys.pressed(KeyCode::ArrowDown))) as f32,
        turn: (i32::from(keys.pressed(KeyCode::ArrowLeft))
            - i32::from(keys.pressed(KeyCode::ArrowRight))) as f32,
    }
}

/// 仅玩家接收方向键，敌人保留自己的输入（当前恒为默认零值）。
fn read_player_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut players: Query<&mut DrivingInput, With<Tank>>,
    fade: Option<Res<Fade>>,
) {
    for mut input in &mut players {
        *input = if fade.as_ref().is_none_or(|fade| fade.accepts_input()) {
            input_from_keys(&keys)
        } else {
            DrivingInput::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::enemy::EnemyTank;
    use crate::terrain;
    use crate::tank::driving::{DrivingPlugin, TANK_SIZE, TurnRate, tank_body};
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 方向键映射：前后为油门、左右为转向，方向相反的按键互相抵消。
    #[test]
    fn arrow_keys_map_to_opposing_throttle_and_turn() {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ArrowUp);
        keys.press(KeyCode::ArrowLeft);
        let input = input_from_keys(&keys);
        assert_eq!((input.throttle, input.turn), (1.0, 1.0));
        keys.press(KeyCode::ArrowDown);
        keys.press(KeyCode::ArrowRight);
        let input = input_from_keys(&keys);
        assert_eq!((input.throttle, input.turn), (0.0, 0.0));
        keys.reset_all();
        keys.press(KeyCode::ArrowDown);
        keys.press(KeyCode::ArrowRight);
        let input = input_from_keys(&keys);
        assert_eq!((input.throttle, input.turn), (-1.0, -1.0));
    }

    /// 玩家键盘只写玩家意图；敌人显式输入复用同一驾驶算法，零输入靶车独立静止。
    #[test]
    fn player_and_enemies_share_driving_without_sharing_input_or_turn_state() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / 64.0,
            )))
            .add_systems(Update, (read_player_input, driving::drive_tank).chain());
        let mut spawn = |player| {
            let mut entity = app.world_mut().spawn((
                DrivingInput::default(),
                TurnRate::default(),
                avian3d::prelude::Rotation::default(),
                avian3d::prelude::LinearVelocity(Vec3::new(0.0, -0.4, 0.0)),
            ));
            if player {
                entity.insert(Tank);
            } else {
                entity.insert(EnemyTank);
            }
            entity.id()
        };
        let player = spawn(true);
        let moving_enemy = spawn(false);
        let idle_enemy = spawn(false);
        app.update();
        for (key, throttle, turn) in [
            (KeyCode::ArrowUp, 1.0, 1.0),
            (KeyCode::ArrowDown, -1.0, 1.0),
            (KeyCode::ArrowDown, -1.0, -1.0),
        ] {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.reset_all();
            keys.press(key);
            keys.press(if turn > 0.0 {
                KeyCode::ArrowLeft
            } else {
                KeyCode::ArrowRight
            });
            *app.world_mut()
                .get_mut::<DrivingInput>(moving_enemy)
                .unwrap() = DrivingInput { throttle, turn };
            for _ in 0..64 {
                app.update();
                assert_eq!(
                    app.world().get::<avian3d::prelude::LinearVelocity>(player).unwrap().0,
                    app.world().get::<avian3d::prelude::LinearVelocity>(moving_enemy).unwrap().0
                );
                assert_eq!(
                    app.world().get::<avian3d::prelude::Rotation>(player).unwrap().0,
                    app.world().get::<avian3d::prelude::Rotation>(moving_enemy).unwrap().0
                );
                assert_eq!(
                    app.world().get::<TurnRate>(player).unwrap().0,
                    app.world().get::<TurnRate>(moving_enemy).unwrap().0
                );
                assert_eq!(
                    app.world().get::<avian3d::prelude::LinearVelocity>(idle_enemy).unwrap().0,
                    Vec3::new(0.0, -0.4, 0.0)
                );
                assert_eq!(app.world().get::<TurnRate>(idle_enemy).unwrap().0, 0.0);
                assert_eq!(
                    app.world().get::<avian3d::prelude::Rotation>(idle_enemy).unwrap().0,
                    Quat::IDENTITY
                );
            }
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        *app.world_mut()
            .get_mut::<DrivingInput>(moving_enemy)
            .unwrap() = DrivingInput::default();
        for _ in 0..64 {
            app.update();
        }
        assert_eq!(
            app.world().get::<avian3d::prelude::LinearVelocity>(moving_enemy).unwrap().0,
            Vec3::new(0.0, -0.4, 0.0)
        );
        assert!(app.world().get::<TurnRate>(moving_enemy).unwrap().0.abs() < 0.01);
    }

    /// 无窗口运行真实 Avian 验证玩家输入链路：落地、行驶、松键、转向及边界碰撞。
    #[test]
    fn keyboard_drives_real_physics_until_arena_boundaries() {
        let step = Duration::from_secs_f64(1.0 / 64.0);
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            avian3d::prelude::PhysicsPlugins::default(),
            DrivingPlugin,
            PlayerPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(step))
        .add_systems(Startup, |mut commands: Commands| {
            terrain::spawn_arena(&mut commands)
        });
        // 手动逐帧测试也需完成 Avian 在 finish 阶段注册的求解器资源。
        app.finish();
        app.cleanup();
        let tank = app
            .world_mut()
            .spawn((
                Tank,
                tank_body(),
                // 驾驶系统按履带装备读取速度，缺省组件会导致坦克不动。
                crate::tank::track::PLAYER_TRACKS,
                Transform::from_xyz(0.0, 4.0, 0.0),
            ))
            .id();
        for _ in 0..192 {
            app.update();
        }
        let position = app.world().get::<Transform>(tank).unwrap().translation;
        assert!(
            (position.y - TANK_SIZE.y / 2.0).abs() < 0.02,
            "落地高度异常：{position:?}"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..64 {
            app.update();
        }
        // cleat 前进 16 米/秒，一秒推进 16 米（含物理求解的起步损失）。
        assert!((app.world().get::<Transform>(tank).unwrap().translation.x + 16.0).abs() < 0.05);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ArrowUp);
        let stopped = app.world().get::<Transform>(tank).unwrap().translation;
        for _ in 0..32 {
            app.update();
        }
        assert!(
            app.world()
                .get::<Transform>(tank)
                .unwrap()
                .translation
                .abs_diff_eq(stopped, 0.01)
        );
        // 直行直到抵住 300 米处的墙：16 米/秒 × 19 秒已够，3400 帧余量充足。
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..3400 {
            app.update();
        }
        let transform = app.world().get::<Transform>(tank).unwrap();
        let forward = transform.rotation * Vec3::NEG_X;
        let side = transform.rotation * Vec3::Z;
        let extent_x = forward.x.abs() * TANK_SIZE.x / 2.0 + side.x.abs() * TANK_SIZE.z / 2.0;
        let wall = terrain::FIELD_SIDE / 2.0;
        let stop = transform.translation.x - extent_x;
        assert!((stop + wall).abs() < 0.03, "未到达墙：{transform:?}");
        assert!(stop >= -wall - 0.03, "穿墙：stop={stop} wall={wall}");
    }
}
