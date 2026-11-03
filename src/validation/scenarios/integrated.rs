//! 真实对局中驾驶、调炮、发射与生命周期组合验收。
use super::*;
use bevy::ecs::message::MessageCursor;
use crate::{
    app::{
        state::GameState,
        transition::{Fade, Phase},
    },
    gameplay::{player::Tank, session::SessionEntity},
};

/// 使用实际输入脚本验证组合操作、逐帧限速和退出重进。
pub(super) fn run(app: &mut App, options: &Options, report: &mut String) -> Result<(), String> {
    let player = app
        .world_mut()
        .query_filtered::<Entity, With<Tank>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    // 组合驾驶验证不应被出生点正前方的混凝土阻挡。
    let obstacles: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<crate::gameplay::obstacle::Concrete>>()
        .iter(app.world())
        .collect();
    for obstacle in obstacles {
        app.world_mut().despawn(obstacle);
    }
    let start = *app.world().get::<Transform>(player).ok_or("缺少玩家")?;
    let mut previous = *app
        .world()
        .get::<crate::tank::articulation::TankAim>(player)
        .ok_or("缺少炮塔")?;
    let mut reload_start = None;
    let mut reload_yaw = None;
    let mut coasting_start = None;
    let mut previous_barrel = None;
    let mut impact_cursor = MessageCursor::<crate::gameplay::combat::firing::ShotImpact>::default();
    let mut new_obstacle = None;
    let mut first_impact = None;
    for frame in 0..=400 {
        if frame > 0 {
            update(app, STEP, Some(frame))?;
        }
        let aim = *app
            .world()
            .get::<crate::tank::articulation::TankAim>(player)
            .ok_or("缺少炮塔")?;
        let yaw = (aim.yaw - previous.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let pitch = aim.pitch - previous.pitch;
        ensure(
            yaw.abs() <= 30_f32.to_radians() / 60.0 + 1e-5
                && pitch.abs() <= 9_f32.to_radians() / 60.0 + 1e-5,
            "组合输入超过角速度上限",
        )?;
        if (361..=366).contains(&frame) {
            ensure(app.world().resource::<crate::gameplay::combat::firing::Gun>().reload_progress() < 1.0, "输入检查期间装填已完成")?;
            match frame {
                361 => ensure(yaw < 0.0 && yaw.abs() < 0.1_f32.to_radians(), "装填中反向未低速起步")?,
                362 => ensure(yaw.abs() < 1e-6 && pitch > 0.0, "装填中单轴松键异常")?,
                363 | 365 => ensure(yaw.abs() < 1e-6, "装填中松键出现尾巴")?,
                364 | 366 => ensure(yaw > 0.0 && yaw < 0.1_f32.to_radians(), "装填中点按未低速起步")?,
                _ => unreachable!(),
            }
        }
        previous = aim;
        let barrel = *app.world_mut().query_filtered::<&GlobalTransform, With<crate::gameplay::combat::firing::PlayerBarrel>>().single(app.world()).map_err(|e| e.to_string())?;
        let direction = barrel.rotation() * Vec3::NEG_X;
        if frame == 100 {
            ensure(previous_barrel.is_some_and(|prior: Vec3| prior.distance(direction) > 1e-4), "发射前后炮口姿态未变化")?;
        }
        previous_barrel = Some(direction);
        if frame == 100 {
            let barrel = *app.world_mut().query_filtered::<&GlobalTransform, With<crate::gameplay::combat::firing::PlayerBarrel>>().single(app.world()).map_err(|e| e.to_string())?;
            let bullet = app.world_mut().query::<&crate::gameplay::combat::firing::Bullet>().single(app.world()).map_err(|e| format!("运动中未开火：{e}"))?;
            ensure(bullet.direction.abs_diff_eq(barrel.rotation() * Vec3::NEG_X, 1e-4), "运动中发射方向不是当帧炮口方向")?;
            let direction = bullet.direction;
            let position = app.world_mut().query_filtered::<&Transform, With<crate::gameplay::combat::firing::Bullet>>()
                .single(app.world()).map_err(|e| e.to_string())?.translation + direction * 40.0;
            impact_cursor.clear(app.world().resource::<Messages<crate::gameplay::combat::firing::ShotImpact>>());
            let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(Cuboid::new(8.0, 8.0, 8.0));
            let material = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(Color::srgb(0.65, 0.65, 0.62));
            new_obstacle = Some(app.world_mut().spawn((
                crate::gameplay::obstacle::Concrete,
                avian3d::prelude::RigidBody::Static,
                avian3d::prelude::Collider::cuboid(8.0, 8.0, 8.0),
                Mesh3d(mesh), MeshMaterial3d(material),
                Transform::from_translation(position), Position(position), Rotation::default(),
            )).id());
        }
        if (101..=110).contains(&frame) {
            for impact in impact_cursor.read(app.world().resource::<Messages<crate::gameplay::combat::firing::ShotImpact>>()) {
                if first_impact.is_none() {
                    first_impact = Some(*impact);
                }
            }
            if frame == 110 {
                ensure(first_impact.is_some_and(|impact| Some(impact.target) == new_obstacle && impact.point.is_finite()), "新进入弹道的障碍未被实际弹丸首先命中")?;
                app.world_mut().despawn(new_obstacle.ok_or("缺少动态障碍")?);
            }
        }
        if frame == 173 {
            // 独立验收后续静止射击，避免前一发的冷却影响布靶。
            crate::gameplay::combat::firing::reset_session(app.world_mut());
        }
        if frame == 150 {
            let body = app.world().get::<Transform>(player).ok_or("缺少车体")?;
            ensure(
                body.translation.distance(start.translation) > 0.2
                    && body.rotation.angle_between(start.rotation) > 0.1
                    && aim.yaw.abs() > 0.1
                    && aim.pitch.abs() > 0.05,
                "驾驶和双轴瞄准未同时生效",
            )?;
            coasting_start = Some((*body, aim));
        }
        if frame == 170 {
            let (start_body, start_aim) = coasting_start.ok_or("缺少无输入驾驶起点")?;
            let body = app.world().get::<Transform>(player).ok_or("缺少车体")?;
            // 无 WASD 输入时炮塔角度保持相对方向，行驶与转向不带动炮塔。
            ensure(
                body.translation.distance(start_body.translation) > 0.1
                    && body.rotation.angle_between(start_body.rotation) > 0.05
                    && (aim.yaw - start_aim.yaw).abs() < 1e-6
                    && (aim.pitch - start_aim.pitch).abs() < 1e-6,
                "无瞄准输入驾驶时炮塔未保持相对方向",
            )?;
        }
        if frame == 174 {
            let barrel = *app.world_mut().query_filtered::<&GlobalTransform, With<crate::gameplay::combat::firing::PlayerBarrel>>().single(app.world()).map_err(|e| e.to_string())?;
            let bullet = app
                .world_mut()
                .query::<&crate::gameplay::combat::firing::Bullet>()
                .single(app.world())
                .map_err(|e| format!("组合运动后未开火：{e}"))?;
            ensure(
                bullet
                    .direction
                    .abs_diff_eq(barrel.rotation() * Vec3::NEG_X, 1e-4),
                "组合运动后发射方向不是实际炮口",
            )?;
            reload_start = Some(
                app.world()
                    .resource::<crate::gameplay::combat::firing::Gun>()
                    .reload_progress(),
            );
        }
        if frame == 194 {
            let progress = app
                .world()
                .resource::<crate::gameplay::combat::firing::Gun>()
                .reload_progress();
            ensure(
                reload_start.is_some_and(|start| progress > start + 0.02),
                "冷却中重复开火重置了装填",
            )?;
        }
        if frame == 336 {
            reload_yaw = Some(aim.yaw);
        }
        if frame == 360 {
            let progress = app
                .world()
                .resource::<crate::gameplay::combat::firing::Gun>()
                .reload_progress();
            ensure(
                progress < 1.0 && reload_yaw.is_some_and(|start| (aim.yaw - start).abs() > 0.05),
                "装填中未继续瞄准",
            )?;
        }
        if options.scenario.frames().contains(&frame) {
            wait_render(app, false)?;
            capture(app, &options.image_path(frame))?;
        }
    }
    update(app, STEP, Some(501))?;
    let started = std::time::Instant::now();
    loop {
        update(app, STEP, None)?;
        ensure(
            started.elapsed() < std::time::Duration::from_secs(40),
            "退出过渡超时",
        )?;
        let fade = app.world().resource::<Fade>();
        if fade.phase == Phase::Idle
            && fade.alpha == 0.0
            && *app.world().resource::<State<GameState>>().get() == GameState::Index
        {
            break;
        }
    }
    let world = app.world_mut();
    ensure(
        world
            .query_filtered::<(), With<SessionEntity>>()
            .iter(world)
            .count()
            == 0,
        "退出后对局实体未清理",
    )?;
    update(app, STEP, Some(503))?;
    update(app, STEP, None)?;
    update(app, STEP, Some(505))?;
    let started = std::time::Instant::now();
    loop {
        update(app, STEP, None)?;
        ensure(
            started.elapsed() < std::time::Duration::from_secs(40),
            "重进过渡超时",
        )?;
        let fade = app.world().resource::<Fade>();
        if fade.phase == Phase::Idle
            && fade.alpha == 0.0
            && *app.world().resource::<State<GameState>>().get() == GameState::InGame
        {
            break;
        }
    }
    wait_render(app, true)?;
    let aim = *app
        .world_mut()
        .query_filtered::<&crate::tank::articulation::TankAim, With<Tank>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    ensure(aim.yaw == 0.0 && aim.pitch == 0.0, "重进后炮塔角度未复位")?;
    capture(app, &options.output.join("reentry.png"))?;
    update(app, STEP, Some(506))?;
    let aim = *app.world_mut().query_filtered::<&crate::tank::articulation::TankAim, With<Tank>>()
        .single(app.world()).map_err(|e| e.to_string())?;
    ensure(aim.yaw > 0.0 && aim.yaw < 0.1_f32.to_radians()
        && aim.pitch > 0.0 && aim.pitch < 0.03_f32.to_radians(), "重进后首次输入未从低速起步")?;
    update(app, STEP, Some(507))?;
    let released = *app.world_mut().query_filtered::<&crate::tank::articulation::TankAim, With<Tank>>()
        .single(app.world()).map_err(|e| e.to_string())?;
    ensure((released.yaw - aim.yaw).abs() < 1e-6 && (released.pitch - aim.pitch).abs() < 1e-6, "重进后松键出现输入尾巴")?;
    report.push_str("组合驾驶与双轴瞄准、当帧炮口发射、新障碍首碰、无瞄准驾驶、装填中释放反向点按、逐帧限速与退出重进输入通过\n");
    Ok(())
}
