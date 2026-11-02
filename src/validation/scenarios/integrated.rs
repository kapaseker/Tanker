//! 集成验收：真实对局链路下方向键驾驶、WASD 调炮与空格开火的组合一致性。
//!
//! 玩家保持动态刚体在真实地形上驾驶；静态靶只在组合运动停止后按
//! 实际炮口姿态布置，保证预测与实射对照的确定性。

use super::aiming::check_layout;
use super::*;
use crate::app::state::GameState;
use crate::app::transition::{Fade, Phase};
use crate::gameplay::combat::firing::{
    self, Bullet, PlayerBarrel, PredictedShot, ShotImpact, ShotPrediction,
};
use crate::gameplay::enemy::EnemyTank;
use crate::gameplay::hud::reticle::{EdgeArrow, Reticle};
use crate::gameplay::obstacle::Concrete;
use crate::gameplay::player::Tank;
use crate::gameplay::session::SessionEntity;
use crate::tank::articulation::TankAim;
use avian3d::prelude::RigidBody;
use bevy::ecs::message::MessageCursor;

/// 组合脚本的最后模拟帧；之后使用注入帧 501/503/505 走真实 Esc 退出与重进链路。
const LAST_FRAME: u32 = 400;

/// 端到端组合验收入口；由 `run_scenario` 在预热与场景断言之后调用。
pub(super) fn run(app: &mut App, options: &Options, report: &mut String) -> Result<(), String> {
    let world = app.world_mut();
    let enemies: Vec<_> = world
        .query_filtered::<Entity, With<EnemyTank>>()
        .iter(world)
        .collect();
    for enemy in enemies {
        world.despawn(enemy);
    }
    let concrete = world
        .query_filtered::<Entity, With<Concrete>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let enemy_collision = world
        .resource::<crate::tank::driving::TankConfigs>()
        .enemy
        .collision;
    let enemy_visual = world
        .resource::<AssetServer>()
        .load("tank/enemy/carapace/body.vox");
    // 初始把混凝土墩移离弹道，遮挡阶段再搬回。
    move_obstacle(world, concrete, Vec3::new(100., 0., 100.));
    let mut cursor = MessageCursor::<ShotImpact>::default();
    cursor.clear(world.resource::<Messages<ShotImpact>>());
    capture(app, &options.image_path(0))?;
    report.push_str(
        "集成场景：动态玩家真实驾驶；1-60 直行、61-150 驾驶+转车+调炮组合、174 组合后开火、194 近距离遮挡、270 无命中十字、330 边缘箭头、336 装填中调炮、397-398 角度回绕、400 后 Esc 退出重进\n",
    );

    // 跨帧状态：待对照的开火（标签+预测）、阶段样本与回绕采样。
    // 标签：1 = 帧174 组合后前靶命中对照；2 = 帧194 混凝土遮挡命中对照。
    let mut pending: Option<(u8, PredictedShot)> = None;
    let mut front: Option<Entity> = None;
    let mut rear: Option<Entity> = None;
    let mut sample60: Option<Transform> = None;
    let mut wrap_offset: Option<Vec3> = None;
    let mut reload_base: Option<Vec3> = None;

    for frame in 1..=LAST_FRAME {
        // 帧前布置：读取上一帧结束时的实际炮口姿态。
        if frame == 171 {
            // 组合运动停止后按静止炮口布置前后双靶，验证首个碰撞对象选择。
            let (muzzle, direction, rotation) = muzzle_state(app.world_mut())?;
            let f = spawn_target(app, enemy_collision, &enemy_visual, muzzle + direction * 30., rotation)?;
            let r = spawn_target(app, enemy_collision, &enemy_visual, muzzle + direction * 38., rotation)?;
            front = Some(f);
            rear = Some(r);
        }
        if frame == 191 {
            // 遮挡案例：清除 C 阶段残靶，把混凝土墩搬到弹道 15 米处。
            clear_targets(app, front.take(), rear.take());
            let (muzzle, direction, rotation) = muzzle_state(app.world_mut())?;
            move_obstacle(app.world_mut(), concrete, muzzle + direction * 15.);
            let f = spawn_target(app, enemy_collision, &enemy_visual, muzzle + direction * 30., rotation)?;
            front = Some(f);
        }
        if frame == 211 {
            // 无命中/边缘案例：清空全部目标并把墩移回画外。
            clear_targets(app, front.take(), rear.take());
            move_obstacle(app.world_mut(), concrete, Vec3::new(100., 0., 100.));
        }
        if matches!(frame, 173 | 193 | 335) {
            // 开火帧前解除上一发冷却；预测在开火帧同帧重算。
            firing::reset_session(app.world_mut());
        }
        if frame == 397 {
            // 回绕准备：把偏航直接设定到 359.75°（俯仰保持当前值），本帧不按 KeyA，
            // 让真实相机先在 359.75° 落位；下一帧按 KeyA 跨越 360° 后比较机位连续性。
            let world = app.world_mut();
            world
                .query_filtered::<&mut TankAim, With<Tank>>()
                .single_mut(world)
                .map_err(|e| e.to_string())?
                .yaw = 359.75_f32.to_radians();
        }
        update(app, STEP, Some(frame))?;

        // 每帧一致性：相机公式、炮口方向公式与 HUD 最终布局必须同帧成立。
        let world = app.world_mut();
        let (body, camera) = read_body_camera(world)?;
        let aim = *world
            .query_filtered::<&TankAim, With<Tank>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        let expected = crate::gameplay::camera::camera_transform(&body, aim.yaw);
        ensure(
            camera.translation.distance(expected.translation) <= 0.001,
            &format!("帧{frame}相机机位与姿态公式偏差超过0.001米"),
        )?;
        // 逐分量比较四元数：glam 的 angle_between 在 128° 附近会把 dot 的 1 ulp
        // 舍入放大成 ~7e-4 rad 的假角差，逐分量差才能证明旋转与公式一致。
        let rot_diff = camera
            .rotation
            .to_array()
            .iter()
            .zip(expected.rotation.to_array())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        ensure(
            rot_diff <= 1e-6,
            &format!(
                "帧{frame}相机旋转偏离公式（炮管俯仰改变了相机姿态）：分量差{rot_diff}；camera={:?} expected={:?}",
                camera.rotation.to_array(),
                expected.rotation.to_array()
            ),
        )?;
        let barrel = *world
            .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        let expected_dir = body.rotation
            * Quat::from_rotation_y(aim.yaw)
            * Quat::from_rotation_z(-aim.pitch)
            * Vec3::NEG_X;
        ensure(
            (barrel.rotation() * Vec3::NEG_X).abs_diff_eq(expected_dir, 1e-4),
            &format!("帧{frame}炮口世界方向与姿态公式不符"),
        )?;
        if let Some(predicted) = world.resource::<ShotPrediction>().0 {
            check_layout(world, predicted.point, predicted.target.is_some())
                .map_err(|e| format!("帧{frame}：{e}"))?;
            if frame == 336 {
                reload_base = Some(predicted.point);
            }
            if frame == 174 {
                ensure(
                    predicted.target == front,
                    &format!("组合后开火预测未选择最近前靶：{predicted:?}"),
                )?;
                pending = Some((1, predicted));
            }
            if frame == 194 {
                ensure(
                    predicted.target == Some(concrete),
                    &format!("遮挡案例预测未选择混凝土墩：{predicted:?}"),
                )?;
                pending = Some((2, predicted));
            }
            if frame == 270 {
                ensure(
                    predicted.target.is_none(),
                    "无命中方向仍预测到目标",
                )?;
            }
            if frame == 180 {
                // 前靶已在帧176命中时被销毁：预测必须更新为存活的后靶，不保留旧目标。
                ensure(
                    world.get_entity(front.ok_or("前靶记录缺失")?).is_err(),
                    "帧180前靶应已随命中销毁",
                )?;
                ensure(
                    predicted.target == rear,
                    &format!("前靶销毁后预测未切换到后靶：{predicted:?}"),
                )?;
                report.push_str("断言通过：帧180前靶销毁后预测即时切换到后靶\n");
            }
        } else {
            return Err(format!("帧{frame}对局内预测缺失"));
        }
        // 装填冷却内不得出现第二发在途弹。
        ensure(
            world
                .query_filtered::<(), With<Bullet>>()
                .iter(world)
                .count()
                <= 1,
            &format!("帧{frame}冷却期内产生了第二发弹丸"),
        )?;

        // 命中对照：真实首碰对象与接触点必须与开火帧预测一致。
        let impacts: Vec<_> = cursor
            .read(app.world().resource::<Messages<ShotImpact>>())
            .copied()
            .collect();
        for impact in impacts {
            let (tag, predicted) = pending.take().ok_or("收到未登记的命中事件")?;
            ensure(
                predicted.target == Some(impact.target),
                "真实首个碰撞对象与预测不一致",
            )?;
            ensure(
                impact.point.distance(predicted.point) <= 0.01,
                &format!("真实接触点与预测偏差超过0.01米：{:?}", impact.point),
            )?;
            if tag == 1 {
                ensure(
                    app.world().get_entity(rear.ok_or("后靶缺失")?).is_ok(),
                    "前靶命中不应波及后靶",
                )?;
                super::wait_render(app, false)?;
                capture(app, &options.output.join("impact-front.png"))?;
                report.push_str(&format!("帧{frame}组合后开火命中前靶：预测与实射首碰一致，接触误差≤0.01米；后靶存活；impact-front.png\n"));
            } else {
                super::wait_render(app, false)?;
                capture(app, &options.output.join("impact-blocked.png"))?;
                report.push_str(&format!("帧{frame}遮挡开火首碰混凝土墩：与预测一致，接触误差≤0.01米；impact-blocked.png\n"));
            }
        }

        // 阶段断言。
        if frame == 60 {
            sample60 = Some(body);
        }
        if frame == 150 {
            // 玩家配置：偏航 30°/s×90 帧=45°、俯仰 9°/s×60 帧=9°（gameplay::player）。
            ensure(
                (aim.yaw.to_degrees() - 45.).abs() < 0.5,
                &format!("组合阶段偏航未按30°/s×90帧推进：{}", aim.yaw.to_degrees()),
            )?;
            ensure(
                (aim.pitch.to_degrees() - 9.).abs() < 0.5,
                &format!("组合阶段俯仰未按9°/s×60帧推进：{}", aim.pitch.to_degrees()),
            )?;
            let start = sample60.ok_or("直行阶段样本缺失")?;
            ensure(
                body.rotation.angle_between(start.rotation) > 0.1,
                "组合输入未转动车体",
            )?;
            ensure(
                (body.translation - start.translation).dot(start.rotation * Vec3::NEG_X) > 0.2,
                "组合输入未产生驾驶位移",
            )?;
            report.push_str("断言通过：帧150驾驶+转车+调炮同时发生，相机/炮口/准星同帧一致\n");
        }
        if frame == 270 {
            // 9° 起压 60 帧 × 9°/s = 恰好水平，预测点落在视线上方约 21°（半 FOV 24° 内）。
            ensure(
                aim.pitch.to_degrees().abs() < 0.5,
                &format!("KeyS 未把俯仰压回水平：{}", aim.pitch.to_degrees()),
            )?;
            let reticle = app
                .world_mut()
                .query_filtered::<&Node, With<Reticle>>()
                .single(app.world())
                .unwrap();
            ensure(
                reticle.display != Display::None,
                "无命中水平方向十字未显示",
            )?;
            report.push_str("断言通过：帧270无命中方向显示纯十字（无命中圆），预测目标为空\n");
        }
        if frame == 330 {
            let arrow = app
                .world_mut()
                .query_filtered::<&Node, With<EdgeArrow>>()
                .single(app.world())
                .unwrap();
            ensure(arrow.display != Display::None, "抬炮远端未显示边缘箭头")?;
            report.push_str("断言通过：帧330画面外方向显示钳制边缘箭头\n");
        }
        if frame == 360 {
            ensure(
                app.world().resource::<firing::Gun>().reload_progress() < 1.,
                "帧360装填应仍未完成",
            )?;
            let moved = app
                .world()
                .resource::<ShotPrediction>()
                .0
                .ok_or("装填中预测缺失")?;
            ensure(
                reload_base.is_some_and(|base| moved.point.distance(base) > 0.01),
                "装填中持续调炮未更新准星",
            )?;
            report.push_str("断言通过：帧360装填中调炮实时更新准星且未产生第二发\n");
        }
        if frame == 397 {
            // 相机已在 359.75° 姿态落位，记录偏移供跨越后比较。
            wrap_offset = Some(camera.translation - body.translation);
        }
        if frame == 398 {
            // 偏航归一化到 [0, 2π) 且 KeyA 使偏航增大：从 359.75° 跨 360° 后应落在
            // (0, 0.1rad) 区间；若归一化约定改变此断言需同步调整。
            ensure(
                aim.yaw < 0.1 && aim.yaw > 0.,
                &format!("炮塔未跨越回绕边界：{}", aim.yaw.to_degrees()),
            )?;
            let offset = camera.translation - body.translation;
            ensure(
                wrap_offset.is_some_and(|prev| (offset - prev).length() < 0.2),
                &format!("回绕时相机跳变：偏移差{}", (offset - wrap_offset.unwrap_or(offset)).length()),
            )?;
            report.push_str("断言通过：帧398偏航跨越360°回绕（359.75°→0.25°），相机连续无跳变\n");
        }

        if options.scenario.frames().contains(&frame) {
            super::wait_render(app, false)?;
            capture(app, &options.image_path(frame))?;
            report.push_str(&format!(
                "模拟帧：{frame}；图片：{}\n",
                options.image_path(frame).display()
            ));
        }
    }

    // 退出重进：真实 Esc → 首页残留检查 → ↓+Enter 重进 → 全新准星。
    update(app, STEP, Some(501))?;
    // 过渡等待上限 1200 帧（1/60s 步长约 20 秒墙钟模拟时间），超时视为过渡卡死。
    let mut iterations = 0;
    loop {
        update(app, STEP, None)?;
        iterations += 1;
        ensure(iterations < 1200, "退出过渡未完成")?;
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
        "退出后残留对局实体",
    )?;
    ensure(
        world
            .query_filtered::<(), Or<(With<Reticle>, With<EdgeArrow>)>>()
            .iter(world)
            .count()
            == 0,
        "退出后残留准星或边缘箭头",
    )?;
    ensure(
        world.resource::<ShotPrediction>().0.is_none(),
        "退出后残留预测",
    )?;
    report.push_str("断言通过：Esc 真实退出后对局实体、准星与预测全部清零\n");

    update(app, STEP, Some(503))?;
    update(app, STEP, None)?;
    ensure(
        app.world()
            .resource::<crate::pages::index::MenuInput>()
            .focus
            == Some(0),
        "重进前方向键未聚焦菜单项",
    )?;
    update(app, STEP, Some(505))?;
    iterations = 0;
    loop {
        update(app, STEP, None)?;
        iterations += 1;
        ensure(iterations < 1200, "重进过渡未完成")?;
        let fade = app.world().resource::<Fade>();
        if fade.phase == Phase::Idle
            && fade.alpha == 0.0
            && *app.world().resource::<State<GameState>>().get() == GameState::InGame
        {
            break;
        }
    }
    wait_render(app, true)?;
    validate_scene(app.world_mut())?;
    let world = app.world_mut();
    ensure(
        world
            .query_filtered::<(), With<crate::gameplay::camera::FollowCamera>>()
            .iter(world)
            .count()
            == 1,
        "重进后跟随相机数量异常",
    )?;
    let enemies: Vec<_> = world
        .query_filtered::<Entity, With<EnemyTank>>()
        .iter(world)
        .collect();
    for enemy in enemies {
        world.despawn(enemy);
    }
    // 重进的对局会重新生成默认位置的混凝土墩（高 8 米），先移开避免遮挡 10° 弹道。
    let new_concrete = world
        .query_filtered::<Entity, With<crate::gameplay::obstacle::Concrete>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    move_obstacle(world, new_concrete, Vec3::new(100., 0., 100.));
    *world
        .query_filtered::<&mut TankAim, With<Tank>>()
        .single_mut(world)
        .map_err(|e| e.to_string())? = TankAim {
        yaw: 0.,
        pitch: 10_f32.to_radians(),
    };
    for _ in 0..3 {
        update(app, STEP, None)?;
    }
    let (muzzle, direction, rotation) = muzzle_state(app.world_mut())?;
    let target = spawn_target(app, enemy_collision, &enemy_visual, muzzle + direction * 30., rotation)?;
    for _ in 0..3 {
        update(app, STEP, None)?;
    }
    let predicted = app
        .world()
        .resource::<ShotPrediction>()
        .0
        .ok_or("重进后预测未就绪")?;
    ensure(
        predicted.target == Some(target),
        &format!(
            "重进后准星未指向新目标：predicted={predicted:?}；target={target:?}；target_pos={:?}",
            app.world().get::<Transform>(target).map(|t| t.translation)
        ),
    )?;
    check_layout(app.world_mut(), predicted.point, true)
        .map_err(|e| format!("重进后：{e}"))?;
    super::wait_render(app, false)?;
    capture(app, &options.output.join("reentry.png"))?;
    report.push_str("断言通过：重进后全新对局预测、HUD 与相机一致；reentry.png\n");
    Ok(())
}

/// 读取实际炮管的世界炮口位置、单位方向与世界旋转。
fn muzzle_state(world: &mut World) -> Result<(Vec3, Vec3, Quat), String> {
    let barrel = *world
        .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    Ok((
        barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION)),
        barrel.rotation() * Vec3::NEG_X,
        barrel.rotation(),
    ))
}

/// 读取玩家车体与跟随相机的当前变换（均为 Copy，便于后续借用）。
fn read_body_camera(world: &mut World) -> Result<(Transform, Transform), String> {
    let body = *world
        .query_filtered::<&Transform, With<Tank>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let camera = *world
        .query_filtered::<&Transform, With<crate::gameplay::camera::FollowCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    Ok((body, camera))
}

/// 在已知世界位置生成静态敌靶，姿态与实际炮口一致。
fn spawn_target(
    app: &mut App,
    collision: crate::tank::driving::TankCollision,
    visual: &Handle<bevy::world_serialization::WorldAsset>,
    position: Vec3,
    rotation: Quat,
) -> Result<Entity, String> {
    let world = app.world_mut();
    let enemy = world
        .spawn((
            EnemyTank,
            SessionEntity,
            crate::tank::driving::body_for_collision(collision),
            WorldAssetRoot(visual.clone()),
            Transform::from_translation(position).with_rotation(rotation),
            Position(position),
            Rotation(rotation),
        ))
        .id();
    world.entity_mut(enemy).insert(RigidBody::Static);
    Ok(enemy)
}

/// 移除指定敌靶与在途弹丸，用于案例间隔离。
fn clear_targets(app: &mut App, front: Option<Entity>, rear: Option<Entity>) {
    let world = app.world_mut();
    for entity in [front, rear].into_iter().flatten() {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }
    let bullets: Vec<_> = world
        .query_filtered::<Entity, With<Bullet>>()
        .iter(world)
        .collect();
    for bullet in bullets {
        world.despawn(bullet);
    }
}

/// 把唯一混凝土墩搬到已知位置。
fn move_obstacle(world: &mut World, entity: Entity, position: Vec3) {
    world.entity_mut(entity).insert((
        Transform::from_translation(position),
        Position(position),
    ));
}
