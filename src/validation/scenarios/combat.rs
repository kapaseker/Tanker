//! 真实命中、爆炸与实体清理断言。

use super::*;

/// 将真实敌人调整为战斗测试布局；仍保留原有动态刚体、碰撞体和资源层级。
pub(super) fn arrange_combat(world: &mut World) {
    // 新敌人与玩家同高，在真实平地直接验证水平命中。
    let mut query = world.query_filtered::<Entity, With<crate::gameplay::enemy::EnemyTank>>();
    let mut enemies: Vec<_> = query.iter(world).collect();
    enemies.sort_by_key(|entity| entity.to_bits());
    for (index, entity) in enemies.into_iter().enumerate() {
        let position = if index == 0 {
            Vec3::new(-12.0, 3.0, 0.0)
        } else {
            Vec3::new(30.0 + index as f32 * 10.0, 3.0, 30.0)
        };
        world.entity_mut(entity).insert((
            Transform::from_translation(position),
            Position(position),
            Rotation::default(),
            LinearVelocity::ZERO,
        ));
    }
}

/// 在公共模拟循环的原有帧边界检查真实游戏状态。
pub(super) fn check(
    app: &mut App,
    frame: u32,
    report: &mut String,
    failures: &mut Vec<String>,
) -> Result<(), String> {
    if frame == 24 {
        let evidence = app.world().resource::<Evidence>();
        record_check(
            report,
            failures,
            evidence.bullet_seen
                && evidence.bullet_moved
                && evidence.destroyed == 1
                && evidence.explosion_seen,
            "子弹生成并飞行、单个敌人击毁及爆炸生成",
        );
    }
    if frame == 120 {
        let world = app.world_mut();
        record_check(
            report,
            failures,
            world
                .query_filtered::<(), With<crate::gameplay::enemy::EnemyTank>>()
                .iter(world)
                .count()
                == crate::gameplay::enemy::ENEMY_COUNT - 1,
            "单次命中只清理一辆敌人",
        );
        record_check(
            report,
            failures,
            world
                .query_filtered::<(), With<crate::gameplay::combat::explosion::Explosion>>()
                .iter(world)
                .count()
                == 0,
            "爆炸清理完成",
        );
        record_check(
            report,
            failures,
            world
                .query_filtered::<(), With<crate::gameplay::combat::firing::Bullet>>()
                .iter(world)
                .count()
                == 0,
            "命中子弹清理完成",
        );
    }
    Ok(())
}
