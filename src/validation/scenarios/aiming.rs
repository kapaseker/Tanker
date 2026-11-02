//! 已知几何布靶，观察完整预测→HUD→真实开火，不用预测生成预期接触位置。

use super::*;
use crate::gameplay::session::SessionEntity;
use crate::gameplay::{
    combat::firing::{self, Bullet, PlayerBarrel, ShotImpact, ShotPrediction},
    enemy::EnemyTank,
    hud::reticle::{HitRing, Reticle},
    obstacle::Concrete,
    player::Tank,
};
use crate::tank::articulation::TankAim;
use avian3d::prelude::RigidBody;
use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::ecs::{message::MessageCursor, system::RunSystemOnce};

/// 保留真实玩家视角与模型，把目标固定到已知平面；每例通过空格实际发射。
pub(super) fn run(app: &mut App, options: &Options, report: &mut String) -> Result<(), String> {
    let world = app.world_mut();
    let player = world
        .query_filtered::<Entity, With<Tank>>()
        .single(world)
        .map_err(|e| e.to_string())?;
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
    world.entity_mut(player).insert((
        RigidBody::Static,
        Transform::from_xyz(0., 4., 0.),
        Position(Vec3::new(0., 4., 0.)),
        Rotation::default(),
        LinearVelocity::ZERO,
    ));
    let enemy_collision = world
        .resource::<crate::tank::driving::TankConfigs>()
        .enemy
        .collision;
    let enemy_visual = world
        .resource::<AssetServer>()
        .load("tank/enemy/carapace/body.vox");
    // 距离、俯仰、偏航、遮挡、擦边及无命中；使用实际敌车圆角碰撞配置。
    for (index, (distance, yaw, pitch, blocked, lateral, hit)) in [
        (14., 0., 0., false, 0., true),
        (40., 0., 20., false, 0., true),
        (40., 0., -9., false, 0., true),
        (40., 90., 0., false, 0., true),
        (60., 0., 0., true, 0., true),
        (
            40.,
            0.,
            0.,
            false,
            enemy_collision.size[2] / 2. + 0.01,
            true,
        ),
        (0., 0., 0., false, 0., true),
        (2100., 0., 0., false, 0., false),
        (350., 0., 0., false, 0., false),
        (40., 0., 0., false, 5., false),
        (40., 180., 0., false, 0., true),
        (300., 0., 0., false, 0., true),
        (325., 0., 0., false, 0., false),
    ]
    .into_iter()
    .enumerate()
    {
        let world = app.world_mut();
        let old: Vec<_> = world
            .query_filtered::<Entity, Or<(With<EnemyTank>, With<Bullet>)>>()
            .iter(world)
            .collect();
        for entity in old {
            world.despawn(entity);
        }
        *world.get_mut::<TankAim>(player).unwrap() = TankAim {
            yaw: f32::to_radians(yaw),
            pitch: f32::to_radians(pitch),
        };
        update(app, STEP, None)?;
        let world = app.world_mut();
        let barrel = *world
            .query_filtered::<&GlobalTransform, With<PlayerBarrel>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        let direction = barrel.rotation() * Vec3::NEG_X;
        let side = barrel.rotation() * Vec3::Z;
        let muzzle =
            barrel.transform_point(Vec3::from_array(crate::tank::model::astra::MUZZLE_POSITION));
        let position = muzzle + direction * distance + side * lateral;
        let rotation = barrel.rotation();
        let enemy = world
            .spawn((
                EnemyTank,
                SessionEntity,
                crate::tank::driving::body_for_collision(enemy_collision),
                WorldAssetRoot(enemy_visual.clone()),
                Transform::from_translation(position).with_rotation(rotation),
                Position(position),
                Rotation(rotation),
            ))
            .id();
        world.entity_mut(enemy).insert(RigidBody::Static);
        // 后靶确保首个碰撞对象不会错误选择远处目标。
        let rear = position + direction * 8.;
        let rear_enemy = world
            .spawn((
                EnemyTank,
                SessionEntity,
                crate::tank::driving::body_for_collision(enemy_collision),
                WorldAssetRoot(enemy_visual.clone()),
                Transform::from_translation(rear).with_rotation(rotation),
                Position(rear),
                Rotation(rotation),
            ))
            .id();
        world.entity_mut(rear_enemy).insert(RigidBody::Static);
        let obstacle_position = if blocked {
            muzzle + direction * 20.
        } else {
            Vec3::new(100., 0., 100.)
        };
        world.entity_mut(concrete).insert((
            Transform::from_translation(obstacle_position).with_rotation(rotation),
            Position(obstacle_position),
            Rotation(rotation),
        ));
        for _ in 0..3 {
            update(app, STEP, None)?;
        }
        let world = app.world_mut();
        let predicted = world.resource::<ShotPrediction>().0.ok_or("预测未就绪")?;
        let expected_target = hit.then_some(if blocked { concrete } else { enemy });
        ensure(
            predicted.target == expected_target,
            &format!("案例 {index} 最近目标错误：{predicted:?}"),
        )?;
        if hit && distance > 0. && lateral == 0. && !blocked {
            let face = muzzle + direction * (distance - enemy_collision.size[0] / 2.);
            // 接触面上两个切向坐标不是唯一值，只检查独立已知迎弹平面的法向距离。
            ensure(
                (predicted.point - face).dot(direction).abs() <= 0.01,
                "预测接触点不在已知迎弹面",
            )?;
        }
        if lateral > 0. && hit {
            let ray_hit = world
                .run_system_once(move |spatial: SpatialQuery| {
                    spatial.cast_ray_predicate(
                        muzzle,
                        Dir3::new(direction).unwrap(),
                        100.,
                        true,
                        &SpatialQueryFilter::default(),
                        &|target| target == enemy,
                    )
                })
                .map_err(|e| e.to_string())?;
            ensure(ray_hit.is_none(), "擦边用例的中心射线意外命中")?;
        }
        ensure(predicted.point.is_finite(), "起始重叠产生无效点")?;
        check_layout(world, predicted.point, hit)?;
        super::wait_render(app, false)?;
        capture(app, &options.image_path(index as u32))?;
        let mut cursor = MessageCursor::<ShotImpact>::default();
        cursor.clear(app.world().resource::<Messages<ShotImpact>>());
        firing::reset_session(app.world_mut());
        update(app, STEP, Some(1))?;
        let mut actual = None;
        for _ in 0..110 {
            update(app, STEP, None)?;
            actual = cursor
                .read(app.world().resource::<Messages<ShotImpact>>())
                .next()
                .copied();
            if actual.is_some() {
                break;
            }
        }
        ensure(
            actual.map(|impact| impact.target) == expected_target,
            "真实首个碰撞对象与预测不一致",
        )?;
        if let Some(actual) = actual {
            if actual.target == enemy {
                ensure(
                    app.world()
                        .resource::<ShotPrediction>()
                        .0
                        .is_none_or(|shot| shot.target != Some(enemy)),
                    "敌人销毁后仍保留旧命中",
                )?;
            }
            ensure(
                actual.point.distance(predicted.point) <= 0.01,
                &format!(
                    "真实接触点与预测偏差：{:?} / {:?}",
                    actual.point, predicted.point
                ),
            )?;
        }
        if index == 0 {
            update(app, STEP, Some(2))?;
            let current = app
                .world()
                .resource::<ShotPrediction>()
                .0
                .ok_or("装填中未更新准星")?;
            ensure(
                app.world().resource::<firing::Gun>().reload_progress() < 1.,
                "装填状态异常",
            )?;
            ensure(
                current.point.distance(predicted.point) > 0.01,
                "真实调炮后仍保留旧位置",
            )?;
            check_layout(app.world_mut(), current.point, current.target.is_some())?;
        }
        report.push_str(&format!("案例 {index} 通过：距离={distance} 俯仰={pitch} 偏航={yaw} 遮挡={blocked} 擦边={lateral}；预测={predicted:?}；真实首碰及接触误差≤0.01m；HUD≤2px\n"));
    }
    // 缺失活动炮管/相机和非有限预测须在同帧隐藏，不能遗留旧准星。
    let barrel = app
        .world_mut()
        .query_filtered::<Entity, With<PlayerBarrel>>()
        .single(app.world())
        .map_err(|e| e.to_string())?;
    app.world_mut().entity_mut(barrel).remove::<PlayerBarrel>();
    update(app, STEP, None)?;
    ensure(
        app.world().resource::<ShotPrediction>().0.is_none(),
        "炮管缺失后仍保留预测",
    )?;
    ensure(
        app.world_mut()
            .query_filtered::<&Node, With<Reticle>>()
            .single(app.world())
            .unwrap()
            .display
            == Display::None,
        "炮管缺失后准星未隐藏",
    )?;
    crate::gameplay::session::cleanup(app.world_mut());
    ensure(
        app.world_mut()
            .query_filtered::<Entity, With<Reticle>>()
            .iter(app.world())
            .count()
            == 0
            && app.world().resource::<ShotPrediction>().0.is_none(),
        "退出对局残留准星或预测",
    )?;
    report.push_str(
        "缺失炮管即时隐藏、已毁目标刷新、对局统一清理通过；画面外暂隐藏，边缘提示留待03。\n",
    );
    Ok(())
}

/// 最终布局中心对比相机投影，读取布局结果而非 Node 输入值。
fn check_layout(world: &mut World, point: Vec3, hit: bool) -> Result<(), String> {
    let (camera, transform) = world
        .query_filtered::<(&Camera, &GlobalTransform), With<crate::gameplay::camera::FollowCamera>>(
        )
        .single(world)
        .map_err(|e| e.to_string())?;
    let projected = camera.world_to_viewport(transform, point).ok();
    let viewport = camera.logical_viewport_rect().ok_or("相机无视口")?;
    let expected = projected.filter(|position| {
        position.is_finite()
            && position.cmpge(viewport.min + Vec2::splat(14.)).all()
            && position.cmple(viewport.max - Vec2::splat(14.)).all()
    });
    let (node, layout, transform) = world
        .query_filtered::<(&Node, &ComputedNode, &UiGlobalTransform), With<Reticle>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    if expected.is_some() {
        ensure(
            node.display != Display::None && layout.size == Vec2::splat(28.),
            "准星没有完成当帧28px布局",
        )?;
        ensure(
            transform.translation.distance(projected.unwrap()) <= 2.,
            "最终准星中心与投影偏差超过2px",
        )?;
        let gap = Rect::from_center_size(transform.translation, Vec2::splat(4.));
        let root = world
            .query_filtered::<Entity, With<Reticle>>()
            .single(world)
            .unwrap();
        let mut bars = world.query::<(
            &BackgroundColor,
            &ComputedNode,
            &UiGlobalTransform,
            &ChildOf,
        )>();
        for (color, computed, transform, parent) in bars.iter(world) {
            let belongs = parent.parent() == root
                || world
                    .get::<ChildOf>(parent.parent())
                    .is_some_and(|parent| parent.parent() == root);
            if belongs && color.0 != Color::NONE {
                let occupied = Rect::from_center_size(transform.translation, computed.size);
                let overlap = occupied.intersect(gap);
                ensure(
                    overlap.width() <= 0. || overlap.height() <= 0.,
                    "十字或描边侵入4×4透明空隙",
                )?;
            }
        }
        let ring = world
            .query_filtered::<&Node, With<HitRing>>()
            .single(world)
            .map_err(|e| e.to_string())?;
        ensure((ring.display != Display::None) == hit, "命中圆语义错误")?;
    } else {
        ensure(node.display == Display::None, "画面外或无效投影未隐藏")?;
    }
    Ok(())
}
