//! 对局相机、场景表面、实际炮口与固定十字的离屏验收。
use super::*;
use crate::gameplay::{
    camera::FollowCamera,
    combat::firing::{self, PlayerBarrel, ShotImpact},
    enemy::EnemyTank,
    hud::reticle::Reticle,
    obstacle::Concrete,
    player::Tank,
};
use crate::tank::articulation::TankAim;
use bevy::ecs::{message::MessageCursor, system::RunSystemOnce};

/// 布置远近不同的混凝土迎弹面，以独立平面几何验证最终炮口方向及真实首碰。
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
    let player = world
        .query_filtered::<Entity, With<Tank>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let body = *world.get::<Transform>(player).ok_or("缺少玩家")?;
    world
        .get_mut::<crate::gameplay::camera::PlayerView>(player)
        .ok_or("缺少视角")?
        .0
        .pitch = 0_f32.to_radians();
    update(app, STEP, None)?;
    // 使用真实 UI 布局结果检查窄屏、超宽屏和非 1 UI 缩放。
    for (width, height, scale) in [(640, 720, 0.5), (1920, 720, 1.0), (1280, 720, 1.0)] {
        let extent = bevy::render::render_resource::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        for handle in [
            app.world().resource::<Target>().0.clone(),
            app.world()
                .resource::<crate::app::transition::SceneTarget>()
                .0
                .clone(),
        ] {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .get_mut(&handle)
                .ok_or("缺少渲染目标")?
                .resize(extent);
        }
        app.world_mut().resource_mut::<UiScale>().0 = scale;
        for _ in 0..3 {
            update(app, STEP, None)?;
        }
        check_layout(app.world_mut())?;
        let world = app.world_mut();
        let (node, transform) = world
            .query_filtered::<(&ComputedNode, &UiTransform), With<Reticle>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        ensure(
            (node.size * transform.scale).distance(Vec2::splat(28.)) < 0.1,
            "缩放后的十字尺寸异常",
        )?;
        capture(
            app,
            &options
                .output
                .join(format!("reticle-layout-{width}x{height}.png")),
        )?;
    }
    // 无 Winit 的窗口组件只提供显示缩放信息，不创建系统窗口。
    let simulated_window = app
        .world_mut()
        .spawn((
            Window {
                focused: true,
                resolution: bevy::window::WindowResolution::new(1280, 720)
                    .with_scale_factor_override(1.5),
                ..default()
            },
            super::articulation::SimulatedFocus,
        ))
        .id();
    for _ in 0..3 {
        update(app, STEP, None)?;
    }
    let world = app.world_mut();
    let target_state = format!("{:?}", world.query_filtered::<&bevy::camera::RenderTarget, With<FollowCamera>>().single(world).map_err(|e| e.to_string())?);
    let window_scale = world.query::<&Window>().single(world).map_err(|e| e.to_string())?.resolution.scale_factor();
    let viewport = world
        .query_filtered::<&Camera, With<FollowCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?
        .logical_viewport_rect()
        .ok_or("缩放后视口未就绪")?;
    let (node, center, transform) = world
        .query_filtered::<(&ComputedNode, &UiGlobalTransform, &UiTransform), With<Reticle>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    ensure(
        (center.translation / 1.5).distance(viewport.center()) <= 2.0
            && (node.size * transform.scale / 1.5).distance(Vec2::splat(28.)) < 0.1,
        &format!("1.5 显示缩放下实际十字中心或尺寸错误：center={:?} viewport={:?} size={:?} transform={:?} window={} target={}", center.translation, viewport, node.size, transform.scale, window_scale, target_state),
    )?;
    capture(app, &options.output.join("reticle-display-scale-1.5.png"))?;
    app.world_mut().despawn(simulated_window);
    for _ in 0..3 {
        update(app, STEP, None)?;
    }
    report.push_str("实际 UI 布局：640×720、1920×720、1280×720、0.5 UI 缩放及 1.5 显示缩放居中，尺寸 28 逻辑像素\n");
    for (index, distance) in [80.0, 50.0, 26.0, 110.0].into_iter().enumerate() {
        let camera = *app
            .world_mut()
            .query_filtered::<&Transform, With<FollowCamera>>()
            .single(app.world())
            .map_err(|e| e.to_string())?;
        let center = camera.translation + camera.forward().as_vec3() * distance;
        app.world_mut().entity_mut(concrete).insert((
            Transform::from_translation(center).with_rotation(body.rotation),
            Position(center),
            Rotation(body.rotation),
        ));
        let rear_enemy = if index == 1 {
            let position = camera.translation + camera.forward().as_vec3() * 110.;
            Some(
                app.world_mut()
                    .spawn((
                        EnemyTank,
                        crate::gameplay::session::SessionEntity,
                        avian3d::prelude::RigidBody::Static,
                        avian3d::prelude::Collider::cuboid(6., 4., 4.),
                        Transform::from_translation(position),
                        Position(position),
                        Rotation::default(),
                    ))
                    .id(),
            )
        } else {
            None
        };
        let mut previous = *app.world().get::<TankAim>(player).ok_or("缺少炮塔状态")?;
        for _ in 0..360 {
            update(app, STEP, None)?;
            let aim = *app.world().get::<TankAim>(player).ok_or("缺少炮塔状态")?;
            let yaw = (aim.yaw - previous.yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            ensure(
                yaw.abs() <= 30_f32.to_radians() / 60.0 + 1e-5
                    && (aim.pitch - previous.pitch).abs() <= 9_f32.to_radians() / 60.0 + 1e-5,
                "会聚超过角速度上限",
            )?;
            previous = aim;
        }
        let world = app.world_mut();
        let barrel = *world
            .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        let muzzle =
            barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION));
        let direction = barrel.rotation() * Vec3::NEG_X;
        // 靶面为车体局部 X 方向上的已知平面；视线与炮口分别交该平面。
        let normal = body.rotation * Vec3::X;
        let plane = center + normal * 0.7;
        let camera = *world
            .query_filtered::<&Transform, With<FollowCamera>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        let ray = camera.forward().as_vec3();
        let view_point =
            camera.translation + ray * ((plane - camera.translation).dot(normal) / ray.dot(normal));
        let gun_point = muzzle + direction * ((plane - muzzle).dot(normal) / direction.dot(normal));
        if distance == 26.0 {
            ensure(
                previous.pitch.to_degrees() > 19.9 && view_point.distance(gun_point) > 1.0,
                &format!(
                    "不可达近目标未保持俯仰上限：角度 {}°，平面误差 {} 米",
                    previous.pitch.to_degrees(),
                    view_point.distance(gun_point)
                ),
            )?;
        } else {
            ensure(
                view_point.distance(gun_point) < 0.15,
                &format!("静态迎弹面未会聚：{} 米", view_point.distance(gun_point)),
            )?;
        }
        check_layout(world)?;
        wait_render(app, false)?;
        capture(app, &options.image_path(index as u32))?;
        if distance == 26.0 {
            let initial_view = app
                .world()
                .get::<crate::gameplay::camera::PlayerView>(player)
                .ok_or("缺少视角")?
                .0
                .pitch;
            for _ in 0..30 {
                update(app, STEP, Some(2))?;
            }
            let held_view = app
                .world()
                .get::<crate::gameplay::camera::PlayerView>(player)
                .ok_or("缺少视角")?
                .0
                .pitch;
            ensure(
                (held_view - initial_view).abs() < 1e-6,
                "炮管触限后仍积累视角输入",
            )?;
            update(app, STEP, None)?;
            ensure(
                (app.world()
                    .get::<crate::gameplay::camera::PlayerView>(player)
                    .ok_or("缺少视角")?
                    .0
                    .pitch
                    - held_view)
                    .abs()
                    < 1e-6,
                "松键后视角执行了积压输入",
            )?;
            report.push_str("26 米：不可达时保持俯仰上限与中心十字，持续抬炮不积压视角\n");
            continue;
        }
        firing::reset_session(app.world_mut());
        let mut cursor = MessageCursor::<ShotImpact>::default();
        cursor.clear(app.world().resource::<Messages<ShotImpact>>());
        update(app, STEP, Some(1))?;
        let mut impact = None;
        for _ in 0..60 {
            update(app, STEP, None)?;
            impact = cursor
                .read(app.world().resource::<Messages<ShotImpact>>())
                .next()
                .copied();
            if impact.is_some() {
                break;
            }
        }
        ensure(
            impact.is_some_and(|hit| hit.target == concrete && hit.point.is_finite()),
            "实际弹丸未首先命中混凝土",
        )?;
        if let Some(enemy) = rear_enemy {
            ensure(
                app.world().get_entity(enemy).is_ok(),
                "近混凝土未保护后方敌人",
            )?;
            app.world_mut().despawn(enemy);
            report.push_str("近混凝土与远敌车同在视线：取近表面、实射首碰混凝土且远敌存活\n");
        }
        report.push_str(&format!(
            "距离 {distance} 米：中心十字、逐帧限速、独立迎弹面会聚与混凝土实射通过\n"
        ));
    }
    // 近敌车与远混凝土同处视线，必须按距离取点并实射先击毁近靶。
    let camera = *app
        .world_mut()
        .query_filtered::<&Transform, With<FollowCamera>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    let ray = camera.forward().as_vec3();
    let near_position = camera.translation + ray * 50.;
    let far_position = camera.translation + ray * 80.;
    app.world_mut().entity_mut(concrete).insert((
        Transform::from_translation(far_position),
        Position(far_position),
        Rotation::default(),
    ));
    let enemy = app
        .world_mut()
        .spawn((
            EnemyTank,
            crate::gameplay::session::SessionEntity,
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(6., 4., 4.),
            Transform::from_translation(near_position),
            Position(near_position),
            Rotation::default(),
        ))
        .id();
    for _ in 0..360 {
        update(app, STEP, None)?;
    }
    let barrel = *app
        .world_mut()
        .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    let muzzle =
        barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION));
    let direction = barrel.rotation() * Vec3::NEG_X;
    let normal = body.rotation * Vec3::X;
    let face = near_position + normal * 3.;
    let view_point =
        camera.translation + ray * ((face - camera.translation).dot(normal) / ray.dot(normal));
    let gun_point = muzzle + direction * ((face - muzzle).dot(normal) / direction.dot(normal));
    ensure(
        view_point.distance(gun_point) < 0.15,
        "近敌车与远混凝土同时在视线时未会聚到近敌车",
    )?;
    firing::reset_session(app.world_mut());
    let mut cursor = MessageCursor::<ShotImpact>::default();
    cursor.clear(app.world().resource::<Messages<ShotImpact>>());
    update(app, STEP, Some(1))?;
    let mut impact = None;
    for _ in 0..30 {
        update(app, STEP, None)?;
        impact = cursor
            .read(app.world().resource::<Messages<ShotImpact>>())
            .next()
            .copied();
        if impact.is_some() {
            break;
        }
    }
    ensure(
        impact.is_some_and(|hit| hit.target == enemy) && app.world().get_entity(concrete).is_ok(),
        "近敌车未首先截获实际弹丸",
    )?;
    for _ in 0..360 {
        update(app, STEP, None)?;
    }
    let barrel = *app
        .world_mut()
        .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    let muzzle =
        barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION));
    let direction = barrel.rotation() * Vec3::NEG_X;
    let face = far_position + normal * 0.7;
    let view_point =
        camera.translation + ray * ((face - camera.translation).dot(normal) / ray.dot(normal));
    let gun_point = muzzle + direction * ((face - muzzle).dot(normal) / direction.dot(normal));
    ensure(
        view_point.distance(gun_point) < 0.15,
        "近敌车删除后未重新会聚后方混凝土",
    )?;
    report.push_str("近敌车与远混凝土同在视线：独立迎弹面会聚及实射首碰近敌车通过；目标删除后重新会聚后方表面\n");

    // 水平视线在场地内壁 x=-300 米命中围墙；用独立已知平面验证会聚。
    app.world_mut().entity_mut(concrete).insert((
        Transform::from_xyz(100., 0., 100.),
        Position(Vec3::new(100., 0., 100.)),
    ));
    for _ in 0..360 {
        update(app, STEP, None)?;
    }
    let world = app.world_mut();
    let camera = *world
        .query_filtered::<&Transform, With<FollowCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let barrel = *world
        .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let muzzle =
        barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION));
    let ray = camera.forward().as_vec3();
    let gun = barrel.rotation() * Vec3::NEG_X;
    let view_point = camera.translation + ray * ((-300. - camera.translation.x) / ray.x);
    let gun_point = muzzle + gun * ((-300. - muzzle.x) / gun.x);
    ensure(
        view_point.distance(gun_point) < 0.2,
        "炮口未会聚到竞技场围墙内壁",
    )?;
    wait_render(app, false)?;
    capture(app, &options.output.join("wall-convergence.png"))?;
    report.push_str("围墙：以已知 x=-300 米内壁独立几何验证最近表面会聚\n");

    // 天空无场景交点时，用与弹丸射程一致的有限远点检查炮口方向。
    app.world_mut()
        .get_mut::<crate::gameplay::camera::PlayerView>(player)
        .ok_or("缺少视角")?
        .0
        .pitch = 5_f32.to_radians();
    app.world_mut().entity_mut(concrete).insert((
        Transform::from_xyz(100., 0., 100.),
        Position(Vec3::new(100., 0., 100.)),
    ));
    for _ in 0..360 {
        update(app, STEP, None)?;
    }
    let world = app.world_mut();
    let camera = *world
        .query_filtered::<&Transform, With<FollowCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let barrel = *world
        .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let muzzle =
        barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION));
    let gun = barrel.rotation() * Vec3::NEG_X;
    let fallback = camera.translation + camera.forward().as_vec3() * 2000.;
    ensure(
        (fallback - muzzle).cross(gun).length() < 0.15,
        "天空有限远参照未会聚",
    )?;
    wait_render(app, false)?;
    capture(app, &options.output.join("sky-fallback.png"))?;
    report.push_str("天空：无交点时炮口会聚至 2000 米有限参照\n");

    // 从天空持续抬炮，验证实际炮管可达到车型 +20° 而不是被相机俯角截断。
    for _ in 0..180 {
        update(app, STEP, Some(2))?;
    }
    let aim = *app.world().get::<TankAim>(player).ok_or("缺少炮塔")?;
    ensure(
        (aim.pitch.to_degrees() - 20.).abs() < 0.1,
        &format!("天空持续抬炮未达到实际 +20°：{}", aim.pitch.to_degrees()),
    )?;
    let before = app
        .world()
        .get::<crate::gameplay::camera::PlayerView>(player)
        .ok_or("缺少视角")?
        .0
        .pitch;
    for _ in 0..20 {
        update(app, STEP, None)?;
    }
    let after = app
        .world()
        .get::<crate::gameplay::camera::PlayerView>(player)
        .ok_or("缺少视角")?
        .0
        .pitch;
    ensure((after - before).abs() < 1e-6, "松键后视角继续运动")?;
    wait_render(app, false)?;
    capture(app, &options.output.join("sky-elevation-limit.png"))?;
    report.push_str("天空持续输入：实际炮管达到 +20°，松键视角立即停止\n");
    app.world_mut()
        .get_mut::<crate::gameplay::camera::PlayerView>(player)
        .ok_or("缺少视角")?
        .0
        .pitch = 0_f32.to_radians();
    for _ in 0..240 {
        update(app, STEP, None)?;
    }

    // 相机位于障碍内时没有可靠的远侧表面；只要求姿态有限且限速。
    let camera = *app
        .world_mut()
        .query_filtered::<&Transform, With<FollowCamera>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    app.world_mut().entity_mut(concrete).insert((
        Transform::from_translation(camera.translation),
        Position(camera.translation),
    ));
    let mut previous = *app.world().get::<TankAim>(player).ok_or("缺少炮塔")?;
    for _ in 0..60 {
        update(app, STEP, None)?;
        let aim = *app.world().get::<TankAim>(player).ok_or("缺少炮塔")?;
        let yaw = (aim.yaw - previous.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        ensure(
            aim.yaw.is_finite()
                && aim.pitch.is_finite()
                && yaw.abs() <= 30_f32.to_radians() / 60. + 1e-5
                && (aim.pitch - previous.pitch).abs() <= 9_f32.to_radians() / 60. + 1e-5,
            "相机与障碍重叠时姿态非法或超速",
        )?;
        previous = aim;
    }
    check_layout(app.world_mut())?;
    // 移开障碍，以已知 y=0 地平面独立验证会聚；不使用业务射线结果作预期。
    app.world_mut().entity_mut(concrete).insert((
        Transform::from_xyz(100., 0., 100.),
        Position(Vec3::new(100., 0., 100.)),
    ));
    app.world_mut()
        .get_mut::<crate::gameplay::camera::PlayerView>(player)
        .ok_or("缺少视角")?
        .0
        .pitch = -5_f32.to_radians();
    for _ in 0..360 {
        update(app, STEP, None)?;
    }
    let world = app.world_mut();
    let camera = *world
        .query_filtered::<&Transform, With<FollowCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let barrel = *world
        .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let muzzle =
        barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION));
    let gun = barrel.rotation() * Vec3::NEG_X;
    let view = camera.forward().as_vec3();
    let ground_view = camera.translation - view * (camera.translation.y / view.y);
    let ground_gun = muzzle - gun * (muzzle.y / gun.y);
    ensure(
        ground_view.distance(ground_gun) < 0.2,
        "未对准已知地平面交点",
    )?;
    wait_render(app, false)?;
    capture(app, &options.output.join("ground-convergence.png"))?;
    report.push_str("相机与障碍重叠：姿态有限且限速；地面独立几何会聚通过\n");
    app.world_mut()
        .get_mut::<crate::gameplay::camera::PlayerView>(player)
        .ok_or("缺少视角")?
        .0
        .pitch = 0_f32.to_radians();
    update(app, STEP, None)?;

    // 相机看见远靶，但近障碍只挡实际炮口，真实弹丸必须先命中障碍。
    let world = app.world_mut();
    world.entity_mut(concrete).insert((
        Transform::from_xyz(100., 0., 100.),
        Position(Vec3::new(100., 0., 100.)),
    ));
    let camera = *world
        .query_filtered::<&Transform, With<FollowCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let target_position = camera.translation + camera.forward().as_vec3() * 80.;
    let visual = world
        .resource::<AssetServer>()
        .load("tank/enemy/carapace/body.vox");
    let enemy = world
        .spawn((
            EnemyTank,
            crate::gameplay::session::SessionEntity,
            WorldAssetRoot(visual),
            avian3d::prelude::RigidBody::Static,
            avian3d::prelude::Collider::cuboid(6., 4., 4.),
            Transform::from_translation(target_position),
            Position(target_position),
            Rotation::default(),
        ))
        .id();
    for _ in 0..360 {
        update(app, STEP, None)?;
    }
    let barrel = *app
        .world_mut()
        .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    let muzzle =
        barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION));
    let direction = barrel.rotation() * Vec3::NEG_X;
    let obstacle = muzzle + direction * 8. - Vec3::Y * 1.5;
    app.world_mut().entity_mut(concrete).insert((
        Transform::from_translation(obstacle),
        Position(obstacle),
        Rotation::default(),
    ));
    for _ in 0..3 {
        update(app, STEP, None)?;
    }
    let camera = *app
        .world_mut()
        .query_filtered::<&Transform, With<FollowCamera>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    let ray_hit = app
        .world_mut()
        .run_system_once(move |spatial: avian3d::prelude::SpatialQuery| {
            spatial.cast_ray_predicate(
                camera.translation,
                camera.forward(),
                2000.,
                true,
                &avian3d::prelude::SpatialQueryFilter::default(),
                &|entity| entity == concrete || entity == enemy,
            )
        })
        .map_err(|e| e.to_string())?;
    ensure(
        ray_hit.is_some_and(|hit| hit.entity == enemy),
        "近障碍遮住了相机视线，无法验证视差遮挡",
    )?;
    wait_render(app, false)?;
    capture(
        app,
        &options.output.join("camera-visible-muzzle-blocked.png"),
    )?;
    firing::reset_session(app.world_mut());
    let mut cursor = MessageCursor::<ShotImpact>::default();
    cursor.clear(app.world().resource::<Messages<ShotImpact>>());
    update(app, STEP, Some(1))?;
    let mut impact = None;
    for _ in 0..30 {
        update(app, STEP, None)?;
        impact = cursor
            .read(app.world().resource::<Messages<ShotImpact>>())
            .next()
            .copied();
        if impact.is_some() {
            break;
        }
    }
    ensure(
        impact.is_some_and(|hit| hit.target == concrete) && app.world().get_entity(enemy).is_ok(),
        "炮口近障碍未保护相机可见的远靶",
    )?;
    app.world_mut().despawn(enemy);
    for _ in 0..3 {
        update(app, STEP, None)?;
    }
    check_layout(app.world_mut())?;
    report.push_str(
        "远靶在相机视线上、炮口被近障碍遮挡：真实首碰混凝土，远靶存活；删除远靶后十字仍居中\n",
    );
    Ok(())
}

/// 比较实际布局结果与游戏相机逻辑视口中心。
pub(super) fn check_layout(world: &mut World) -> Result<(), String> {
    let viewport = world
        .query_filtered::<&Camera, With<FollowCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?
        .logical_viewport_rect()
        .ok_or("视口尚未就绪")?;
    let (node, computed, transform) = world
        .query_filtered::<(&Node, &ComputedNode, &UiGlobalTransform), With<Reticle>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    ensure(
        node.display != Display::None && computed.size.x > 0.0,
        "中心十字未显示",
    )?;
    ensure(
        transform.translation.distance(viewport.center()) <= 2.0,
        "十字偏离视口中心超过 2 像素",
    )
}
