//! 真实对局中驾驶、调炮、中心十字与生命周期组合验收。
use super::*;
use crate::{
    app::{
        state::GameState,
        transition::{Fade, Phase},
    },
    gameplay::{camera::PlayerView, hud::reticle::Reticle, player::Tank, session::SessionEntity},
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
    let mut reload_view = None;
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
        ensure(
            yaw.abs() <= 30_f32.to_radians() / 60.0 + 1e-5
                && (aim.pitch - previous.pitch).abs() <= 9_f32.to_radians() / 60.0 + 1e-5,
            "组合输入超过角速度上限",
        )?;
        previous = aim;
        super::aiming::check_layout(app.world_mut())?;
        if frame == 150 {
            let body = app.world().get::<Transform>(player).ok_or("缺少车体")?;
            let view = &app.world().get::<PlayerView>(player).ok_or("缺少视角")?.0;
            ensure(
                body.translation.distance(start.translation) > 0.2
                    && body.rotation.angle_between(start.rotation) > 0.1
                    && view.yaw.abs() > 0.1
                    && view.pitch.abs() > 0.05,
                "驾驶和双轴瞄准未同时生效",
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
            reload_view = Some(
                app.world()
                    .get::<PlayerView>(player)
                    .ok_or("缺少视角")?
                    .0
                    .yaw,
            );
        }
        if frame == 360 {
            let progress = app
                .world()
                .resource::<crate::gameplay::combat::firing::Gun>()
                .reload_progress();
            let yaw = app
                .world()
                .get::<PlayerView>(player)
                .ok_or("缺少视角")?
                .0
                .yaw;
            ensure(
                progress < 1.0 && reload_view.is_some_and(|start| (yaw - start).abs() > 0.05),
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
            == 0
            && world
                .query_filtered::<(), With<Reticle>>()
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
    super::aiming::check_layout(app.world_mut())?;
    let view = &app
        .world_mut()
        .query_filtered::<&PlayerView, With<Tank>>()
        .single(app.world())
        .map_err(|e| e.to_string())?
        .0;
    ensure(view.yaw == 0.0 && view.pitch == 0.0, "重进后视角未复位")?;
    capture(app, &options.output.join("reentry.png"))?;
    report.push_str("驾驶、双轴瞄准、实际炮口发射、装填中调炮、逐帧限速、中心布局与退出重进通过\n");
    Ok(())
}
