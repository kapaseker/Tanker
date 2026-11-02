//! 玩家身份标记与方向键输入；过渡未放行时不写驾驶意图。

use crate::app::transition::Fade;
use crate::tank::articulation::{AimSpec, TankAim};
use crate::tank::driving::{self, DrivingInput};
use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;

/// 标记玩家控制的坦克根实体；驾驶、射击、相机与验证入口以它定位唯一玩家。
#[derive(Component)]
#[require(AimInputSpeed, crate::gameplay::camera::PlayerView)]
pub(crate) struct Tank;

/// 对局中两轴的主动输入速度，按车型最高速度归一化。
#[derive(Component, Default)]
struct AimInputSpeed {
    /// 水平主动速度，按车型最大速度归一化。
    yaw: f32,
    /// 俯仰主动速度，按车型最大速度归一化。
    pitch: f32,
    /// 本帧主动输入前的炮塔角度，用于限制输入与会聚合计速度。
    previous: TankAim,
}

/// 从静止线性加速至最高速度所需时间，单位秒；手感尚待玩家确认。
const AIM_ACCELERATION_SECONDS: f32 = 0.15;

/// 积分归一化速度得到等效全速秒数；输入为 −1/0/1，dt 单位为秒。
/// 松键、抵消或反向时不保留旧速度。
fn aim_input_step(speed: &mut f32, input: f32, dt: f32) -> f32 {
    if input == 0.0 {
        *speed = 0.0;
        return 0.0;
    }
    if speed.signum() != input.signum() {
        *speed = 0.0;
    }
    let start = speed.abs();
    let accelerating = dt.min((1.0 - start) * AIM_ACCELERATION_SECONDS);
    let end = (start + accelerating / AIM_ACCELERATION_SECONDS).min(1.0);
    *speed = input * end;
    input * ((start + end) * 0.5 * accelerating + dt - accelerating)
}

/// 注册玩家方向键到驾驶意图的写入系统。
pub(crate) struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (read_aim_input, converge_aim).chain());
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

/// 双轴同时响应 WASD；失焦和页面过渡保持原角度，不累计待执行输入。
fn read_aim_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    windows: Query<&Window>,
    fade: Option<Res<Fade>>,
    mut players: Query<
        (
            &mut TankAim,
            &mut crate::gameplay::camera::PlayerView,
            &AimSpec,
            &mut AimInputSpeed,
        ),
        With<Tank>,
    >,
) {
    if !time.delta_secs().is_finite()
        || time.delta_secs() < 0.0
        || windows.iter().any(|window| !window.focused)
        || fade.as_ref().is_some_and(|fade| !fade.accepts_input())
    {
        for (aim, _, _, mut speed) in &mut players {
            *speed = AimInputSpeed {
                previous: *aim,
                ..default()
            };
        }
        return;
    }
    let yaw =
        (i32::from(keys.pressed(KeyCode::KeyA)) - i32::from(keys.pressed(KeyCode::KeyD))) as f32;
    let pitch =
        (i32::from(keys.pressed(KeyCode::KeyW)) - i32::from(keys.pressed(KeyCode::KeyS))) as f32;
    for (mut aim, mut view, spec, mut speed) in &mut players {
        speed.previous = *aim;
        if !aim.yaw.is_finite()
            || !aim.pitch.is_finite()
            || !view.0.yaw.is_finite()
            || !view.0.pitch.is_finite()
        {
            speed.yaw = 0.0;
            speed.pitch = 0.0;
            continue;
        }
        let yaw_step = aim_input_step(&mut speed.yaw, yaw, time.delta_secs());
        let pitch_step = aim_input_step(&mut speed.pitch, pitch, time.delta_secs());
        // 两个状态只推进共同可执行的输入，触限时不产生待执行视角。
        let requested = pitch_step * spec.elevation_speed_deg_s.to_radians();
        let lower = spec.min_elevation_deg.to_radians();
        let upper = spec.max_elevation_deg.to_radians();
        let delta = if requested >= 0.0 {
            requested
                .min((upper - aim.pitch).max(0.0))
                .min((upper - view.0.pitch).max(0.0))
        } else {
            requested
                .max((lower - aim.pitch).min(0.0))
                .max((lower - view.0.pitch).min(0.0))
        };
        spec.advance(&mut aim, yaw_step, 0.0, 1.0);
        spec.advance(&mut view.0, yaw_step, 0.0, 1.0);
        aim.pitch += delta;
        view.0.pitch += delta;
    }
}

/// 当前中心视线最近场景交点驱动炮口；全部角位移共享车型速度上限。
fn converge_aim(
    time: Res<Time>,
    spatial: avian3d::prelude::SpatialQuery,
    enemies: Query<&GlobalTransform, With<crate::gameplay::enemy::EnemyTank>>,
    environment: Query<
        (),
        Or<(
            With<crate::gameplay::obstacle::Concrete>,
            With<crate::terrain::TerrainSurface>,
            With<crate::terrain::TerrainWall>,
        )>,
    >,
    mut players: Query<
        (
            &Transform,
            &mut TankAim,
            &crate::gameplay::camera::PlayerView,
            &AimSpec,
            &AimInputSpeed,
        ),
        With<Tank>,
    >,
) {
    use avian3d::prelude::SpatialQueryFilter;
    for (body, mut aim, view, spec, speed) in &mut players {
        let dt = time.delta_secs();
        if !dt.is_finite()
            || dt <= 0.0
            || !body.translation.is_finite()
            || !body.rotation.is_finite()
            || !aim.yaw.is_finite()
            || !aim.pitch.is_finite()
            || !view.0.yaw.is_finite()
            || !view.0.pitch.is_finite()
        {
            continue;
        }
        let camera = crate::gameplay::camera::camera_transform(body, view.0.yaw, view.0.pitch);
        let direction = camera.forward();
        // 2000 米与弹丸最大射程一致；有交点时始终使用最近表面。
        let distance = spatial
            .cast_ray_predicate(
                camera.translation,
                direction,
                crate::gameplay::combat::firing::BULLET_RANGE,
                true,
                &SpatialQueryFilter::default(),
                &|entity| {
                    crate::gameplay::combat::firing::is_target(entity, &enemies, &environment)
                },
            )
            .map_or(crate::gameplay::combat::firing::BULLET_RANGE, |hit| {
                hit.distance
            });
        let target = camera.translation + direction.as_vec3() * distance;
        let turret = Vec3::from_array(crate::tank::model::astra::TURRET_POSITION);
        let barrel = Vec3::from_array(crate::tank::model::astra::BARREL_POSITION);
        let muzzle = Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION);
        let yaw_rotation = Quat::from_rotation_y(aim.yaw);
        let local_muzzle =
            turret + yaw_rotation * (barrel + Quat::from_rotation_z(-aim.pitch) * muzzle);
        let world_muzzle = body.transform_point(local_muzzle);
        let local = body.rotation.inverse() * (target - world_muzzle);
        if !local.is_finite() || local.length_squared() < 0.01 {
            continue;
        }
        let desired_yaw = local.z.atan2(-local.x);
        let desired_pitch = local.y.atan2(local.xz().length());
        let wrap = |angle: f32| {
            (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
        };
        // 主动角位移优先，修正只能使用剩余预算且不能抵消当帧玩家输入。
        let correction = |wanted: f32, active: f32, intent: f32, limit: f32| {
            let remaining = (limit - active.abs()).max(0.0);
            let correction = wanted.clamp(-remaining, remaining);
            if intent > 0.0 {
                correction.max(0.0)
            } else if intent < 0.0 {
                correction.min(0.0)
            } else {
                correction
            }
        };
        let active_yaw = wrap(aim.yaw - speed.previous.yaw);
        let active_pitch = aim.pitch - speed.previous.pitch;
        let yaw_correction = correction(
            wrap(desired_yaw - aim.yaw),
            active_yaw,
            speed.yaw,
            spec.traverse_speed_deg_s.to_radians() * dt,
        );
        let pitch_correction = correction(
            desired_pitch - aim.pitch,
            active_pitch,
            speed.pitch,
            spec.elevation_speed_deg_s.to_radians() * dt,
        );
        aim.yaw = (aim.yaw + yaw_correction).rem_euclid(std::f32::consts::TAU);
        aim.pitch = (aim.pitch + pitch_correction).clamp(
            spec.min_elevation_deg.to_radians(),
            spec.max_elevation_deg.to_radians(),
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
    use crate::tank::driving::{DrivingPlugin, TANK_SIZE, TurnRate, tank_body};
    use crate::terrain;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// 真正输入系统覆盖双轴、反向抵消、松手、失焦和切页；敌车不接玩家输入。
    #[test]
    fn wasd_controls_only_player_and_obeys_input_gates() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Fade>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(Update, read_aim_input);
        let spec = AimSpec {
            traverse_range_deg: 360.0,
            traverse_speed_deg_s: 30.0,
            elevation_speed_deg_s: 9.0,
            min_elevation_deg: -9.0,
            max_elevation_deg: 20.0,
        };
        let player = app.world_mut().spawn((Tank, TankAim::default(), spec)).id();
        let enemy = app
            .world_mut()
            .spawn((EnemyTank, TankAim::default(), spec))
            .id();
        app.update();
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyW);
            keys.press(KeyCode::KeyA);
        }
        for _ in 0..10 {
            app.update();
        }
        let aim = *app.world().get::<TankAim>(player).unwrap();
        assert!((aim.yaw.to_degrees() - 27.75).abs() < 1e-3);
        assert!((aim.pitch.to_degrees() - 8.325).abs() < 1e-3);
        assert_eq!(app.world().get::<TankAim>(enemy).unwrap().yaw, 0.0);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyS);
            keys.press(KeyCode::KeyD);
        }
        app.update();
        assert_eq!(app.world().get::<TankAim>(player).unwrap().yaw, aim.yaw);
        assert_eq!(app.world().get::<TankAim>(player).unwrap().pitch, aim.pitch);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        assert_eq!(app.world().get::<TankAim>(player).unwrap().yaw, aim.yaw);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        let window = app
            .world_mut()
            .spawn(Window {
                focused: false,
                ..default()
            })
            .id();
        app.update();
        assert_eq!(app.world().get::<TankAim>(player).unwrap().pitch, aim.pitch);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.world_mut()
            .resource_mut::<Fade>()
            .begin(crate::app::state::GameState::Index, false);
        app.update();
        assert_eq!(app.world().get::<TankAim>(player).unwrap().pitch, aim.pitch);
    }

    /// 机械上限处按住抬炮不会积累视角，反向则可立即离开限位。
    #[test]
    fn elevation_limit_does_not_queue_view_input() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .add_systems(Update, read_aim_input);
        let spec = AimSpec {
            traverse_range_deg: 360.,
            traverse_speed_deg_s: 30.,
            elevation_speed_deg_s: 9.,
            min_elevation_deg: -9.,
            max_elevation_deg: 20.,
        };
        let player = app
            .world_mut()
            .spawn((
                Tank,
                TankAim {
                    yaw: 0.,
                    pitch: 20_f32.to_radians(),
                },
                spec,
            ))
            .id();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(
            app.world()
                .get::<crate::gameplay::camera::PlayerView>(player)
                .unwrap()
                .0
                .pitch,
            0.
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        assert_eq!(
            app.world()
                .get::<crate::gameplay::camera::PlayerView>(player)
                .unwrap()
                .0
                .pitch,
            0.
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyS);
        app.update();
        assert!(app.world().get::<TankAim>(player).unwrap().pitch < 20_f32.to_radians());
        assert!(
            app.world()
                .get::<crate::gameplay::camera::PlayerView>(player)
                .unwrap()
                .0
                .pitch
                < 0.
        );
        app.world_mut().get_mut::<TankAim>(player).unwrap().pitch = 0.;
        app.world_mut()
            .get_mut::<crate::gameplay::camera::PlayerView>(player)
            .unwrap()
            .0
            .pitch = 20_f32.to_radians();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        assert_eq!(app.world().get::<TankAim>(player).unwrap().pitch, 0.);
    }

    /// 玩家反向时，未完成的视差修正不能抵消或逆转主动调炮。
    #[test]
    fn active_input_wins_over_opposing_convergence() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            avian3d::prelude::PhysicsPlugins::default(),
            PlayerPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        app.finish();
        app.cleanup();
        let spec = AimSpec {
            traverse_range_deg: 360.,
            traverse_speed_deg_s: 30.,
            elevation_speed_deg_s: 9.,
            min_elevation_deg: -9.,
            max_elevation_deg: 20.,
        };
        let player = app
            .world_mut()
            .spawn((Tank, TankAim::default(), spec, Transform::default()))
            .id();
        app.update();
        app.world_mut()
            .get_mut::<crate::gameplay::camera::PlayerView>(player)
            .unwrap()
            .0
            .yaw = 1.0;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyD);
        let before = app.world().get::<TankAim>(player).unwrap().yaw;
        app.update();
        let after = app.world().get::<TankAim>(player).unwrap().yaw;
        let delta = (after - before + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        assert!(delta < 0., "反向会聚不能覆盖玩家输入：{delta}");
    }

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
                    app.world()
                        .get::<avian3d::prelude::LinearVelocity>(player)
                        .unwrap()
                        .0,
                    app.world()
                        .get::<avian3d::prelude::LinearVelocity>(moving_enemy)
                        .unwrap()
                        .0
                );
                assert_eq!(
                    app.world()
                        .get::<avian3d::prelude::Rotation>(player)
                        .unwrap()
                        .0,
                    app.world()
                        .get::<avian3d::prelude::Rotation>(moving_enemy)
                        .unwrap()
                        .0
                );
                assert_eq!(
                    app.world().get::<TurnRate>(player).unwrap().0,
                    app.world().get::<TurnRate>(moving_enemy).unwrap().0
                );
                assert_eq!(
                    app.world()
                        .get::<avian3d::prelude::LinearVelocity>(idle_enemy)
                        .unwrap()
                        .0,
                    Vec3::new(0.0, -0.4, 0.0)
                );
                assert_eq!(app.world().get::<TurnRate>(idle_enemy).unwrap().0, 0.0);
                assert_eq!(
                    app.world()
                        .get::<avian3d::prelude::Rotation>(idle_enemy)
                        .unwrap()
                        .0,
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
            app.world()
                .get::<avian3d::prelude::LinearVelocity>(moving_enemy)
                .unwrap()
                .0,
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
