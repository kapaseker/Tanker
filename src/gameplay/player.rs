//! 玩家身份标记与方向键输入；过渡未放行时不写驾驶意图。

use crate::app::transition::Fade;
use crate::tank::articulation::{AimReset, AimSpec, TankAim};
use crate::tank::driving::{self, DrivingInput};
use avian3d::prelude::PhysicsSystems;
use bevy::prelude::*;

/// 标记玩家控制的坦克根实体；驾驶、射击、相机与验证入口以它定位唯一玩家。
#[derive(Component)]
#[require(AimInputSpeed, AimReset)]
pub(crate) struct Tank;

/// 对局中两轴的主动输入速度，按车型最高速度归一化。
#[derive(Component, Default)]
struct AimInputSpeed {
    /// 水平主动速度，按车型最大速度归一化。
    yaw: f32,
    /// 俯仰主动速度，按车型最大速度归一化。
    pitch: f32,
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
        // 弹框打开期间暂停对局：瞄准输入冻结，取消后从原角度继续。
        app.add_systems(Update, read_aim_input.run_if(not(any_with_component::<crate::ui::widgets::Dialog>)));
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

/// 双轴同时响应 WASD，R/F 单按进入回正、按车型角速度转回零位；
/// 失焦和页面过渡保持原角度并取消回正，不累计待执行输入。
fn read_aim_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    windows: Query<&Window>,
    fade: Option<Res<Fade>>,
    mut players: Query<(&mut TankAim, &AimSpec, &mut AimInputSpeed, &mut AimReset), With<Tank>>,
) {
    if !time.delta_secs().is_finite()
        || time.delta_secs() < 0.0
        || windows.iter().any(|window| !window.focused)
        || fade.as_ref().is_some_and(|fade| !fade.accepts_input())
    {
        for (_, _, mut speed, mut reset) in &mut players {
            *speed = AimInputSpeed::default();
            *reset = AimReset::default();
        }
        return;
    }
    let yaw =
        (i32::from(keys.pressed(KeyCode::KeyA)) - i32::from(keys.pressed(KeyCode::KeyD))) as f32;
    let pitch =
        (i32::from(keys.pressed(KeyCode::KeyW)) - i32::from(keys.pressed(KeyCode::KeyS))) as f32;
    for (mut aim, spec, mut speed, mut reset) in &mut players {
        if !aim.yaw.is_finite() || !aim.pitch.is_finite() {
            speed.yaw = 0.0;
            speed.pitch = 0.0;
            *reset = AimReset::default();
            continue;
        }
        // 单按 R/F 触发对应轴回正；对应轴有主动输入时输入优先，取消回正。
        if keys.just_pressed(KeyCode::KeyR) {
            reset.yaw = true;
        }
        if keys.just_pressed(KeyCode::KeyF) {
            reset.pitch = true;
        }
        if yaw != 0.0 {
            reset.yaw = false;
        }
        if pitch != 0.0 {
            reset.pitch = false;
        }
        let yaw_step = aim_input_step(&mut speed.yaw, yaw, time.delta_secs());
        let pitch_step = aim_input_step(&mut speed.pitch, pitch, time.delta_secs());
        // 玩家输入直接积分炮塔/炮管角；触限即止，不积压待执行角度。
        spec.advance(&mut aim, yaw_step, pitch_step, 1.0);
        // 回正按车型角速度渐进转回零位，到达即止。
        spec.reset_step(&mut aim, &mut reset, time.delta_secs());
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

    /// R/F 单按进入回正，炮塔/炮管按车型角速度渐进转回零位；敌车不受影响。
    #[test]
    fn reset_keys_return_aim_axes_gradually() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
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
        // 先转出非零姿态。
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyA);
            keys.press(KeyCode::KeyW);
        }
        for _ in 0..10 {
            app.update();
        }
        let aimed = *app.world().get::<TankAim>(player).unwrap();
        assert!(aimed.yaw > 0.0 && aimed.pitch > 0.0);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        // 单按 R：一帧只前进一步（30°/s × 0.1s = 3°），不瞬移。
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        let stepping = *app.world().get::<TankAim>(player).unwrap();
        assert!(
            ((aimed.yaw - stepping.yaw).to_degrees() - 3.0).abs() < 1e-3,
            "R 后一帧应按 3°/帧 渐进：{}",
            (aimed.yaw - stepping.yaw).to_degrees()
        );
        assert_eq!(stepping.pitch, aimed.pitch, "R 不应改动炮管俯仰");
        // 持续回正到零位。
        for _ in 0..20 {
            app.update();
        }
        let yaw_done = *app.world().get::<TankAim>(player).unwrap();
        assert_eq!(yaw_done.yaw, 0.0, "回正应在有限帧内到零");
        assert_eq!(yaw_done.pitch, aimed.pitch, "回正水平轴不影响俯仰");
        // 单按 F：同理渐进回正俯仰。
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyF);
        app.update();
        let pitch_stepping = *app.world().get::<TankAim>(player).unwrap();
        assert!(
            pitch_stepping.pitch > 0.0 && pitch_stepping.pitch < aimed.pitch,
            "F 后俯仰应渐进下降而非瞬移"
        );
        for _ in 0..20 {
            app.update();
        }
        let pitch_done = *app.world().get::<TankAim>(player).unwrap();
        assert_eq!(pitch_done.pitch, 0.0, "俯仰回正应在有限帧内到零");
        assert_eq!(pitch_done.yaw, 0.0, "F 不应改动炮塔水平角");
        // 回正途中按对应轴 WASD 立即取消回正，输入优先。
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyA);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        let cancelled = *app.world().get::<TankAim>(player).unwrap();
        assert!(cancelled.yaw > 0.0, "回正途中输入 A 应取消回正并正常转动");
        {
            let reset = app
                .world()
                .get::<crate::tank::articulation::AimReset>(player)
                .unwrap();
            assert!(!reset.yaw, "输入后回正标记应被清除");
        }
        // 敌车姿态始终为零，不接玩家快捷键。
        let enemy_aim = *app.world().get::<TankAim>(enemy).unwrap();
        assert_eq!(
            (enemy_aim.yaw, enemy_aim.pitch),
            (0.0, 0.0),
            "敌车不应受玩家 R/F 快捷键影响"
        );
    }

    /// 机械上限处按住抬炮不会积累角度；反向则可立即离开限位。
    #[test]
    fn elevation_limit_does_not_accumulate_input() {
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
            app.world().get::<TankAim>(player).unwrap().pitch,
            20_f32.to_radians(),
            "触限后持续抬炮不应超出上限"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.update();
        assert_eq!(
            app.world().get::<TankAim>(player).unwrap().pitch,
            20_f32.to_radians(),
            "松键后不应执行积压输入"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyS);
        app.update();
        assert!(app.world().get::<TankAim>(player).unwrap().pitch < 20_f32.to_radians());
        // 下限位处反向输入同样立即生效、不积压。
        app.world_mut().get_mut::<TankAim>(player).unwrap().pitch = -9_f32.to_radians();
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(
            app.world().get::<TankAim>(player).unwrap().pitch,
            -9_f32.to_radians(),
            "触下限后持续压炮不应超出下限"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        assert!(app.world().get::<TankAim>(player).unwrap().pitch > -9_f32.to_radians());
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
        // 使用同一车型参数，单独验证每个实体的驾驶意图和转向状态。
        let driving_spec = driving::TankConfigs::load(std::path::Path::new("assets"))
            .unwrap()
            .player
            .driving;
        let mut spawn = |player| {
            let mut entity = app.world_mut().spawn((
                DrivingInput::default(),
                driving_spec,
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
        // 一秒的实际前进距离应与车型速度一致。
        let speed = app.world().get::<crate::tank::driving::DrivingSpec>(tank).unwrap().forward_speed;
        assert!((app.world().get::<Transform>(tank).unwrap().translation.x + speed).abs() < 0.05);
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
        // 继续直行直到抵住 300 米处的墙，留足模拟帧数。
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
