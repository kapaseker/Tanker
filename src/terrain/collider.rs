//! 地形碰撞几何：由量化档位编译体素台阶复合碰撞体，并铺设不可见边界墙。
//!
//! 碰撞体与视觉网格同源（同一份量化档位），按档位（忽略颜色）贪心合并矩形，
//! 顶面即台阶顶、侧面即竖直坎，不存在高度场插值的隐形斜坡。

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{AUTHOR_RES, FIELD_SIDE, TERRAIN_HALF_RANGE, TERRAIN_STEP, TerrainEntity};

/// 碰撞体 cuboid 的统一底面高度，单位：米；比最低档（−1.0 米）再低一级。
/// 所有 cuboid 统一延伸到此底面：静态碰撞体在地下互相重叠无副作用，
/// 同时保证最低档的 cuboid 仍有 0.1 米厚度而不退化为零厚度。
const COLLIDER_BASE_Y: f32 = -TERRAIN_HALF_RANGE - TERRAIN_STEP;
/// 不可见边界的厚度和高度，单位：米。
const WALL_THICKNESS: f32 = 2.0;
const WALL_HEIGHT: f32 = 8.0;

/// 由量化档位编译一块的体素台阶复合碰撞体 cuboid 集合。
///
/// 按高度档位（忽略颜色，比视觉顶面合并更宽松）贪心合并出矩形集合，
/// 每个矩形生成一个竖直 cuboid：顶面在档位高度（台阶顶），底面统一延伸到
/// [`COLLIDER_BASE_Y`]（地下重叠对静态碰撞体无副作用，且最低档不退化为零厚度）。
/// 相邻矩形的竖直落差由 cuboid 侧面天然构成台阶墙，坦克撞上的是真实的坎，
/// 不存在高度场相邻采样插值出的隐形斜坡。
///
/// `levels` 为整图量化档位（0 为平地，单位 0.1 米），行沿 X、列沿 Z；
/// 越界采样按平地处理，与 `build_chunk` 的 `level_at` 语义一致，
/// 保证场地外缘也有竖直包络。
pub(super) fn collider_cuboids(
    levels: &[i32],
    side: usize,
    start: (usize, usize),
    cells: usize,
    origin: (f32, f32),
) -> Vec<(Vec3, Vec3)> {
    let (row0, col0) = start;
    let level_at = |i: i64, j: i64| {
        if i < 0 || j < 0 || i >= side as i64 || j >= side as i64 {
            0
        } else {
            levels[j as usize * side + i as usize]
        }
    };
    let world_x = |i: i64| origin.0 + (i - row0 as i64) as f32 * AUTHOR_RES;
    let world_z = |j: i64| origin.1 + (j - col0 as i64) as f32 * AUTHOR_RES;

    let mut merged = vec![false; cells * cells];
    let mut cuboids = Vec::new();
    for dj in 0..cells {
        for di in 0..cells {
            if merged[dj * cells + di] {
                continue;
            }
            let i = row0 as i64 + di as i64;
            let j = col0 as i64 + dj as i64;
            let level = level_at(i, j);
            // 沿 X 扩到同档位尽头，再沿 Z 整行验证后继续扩，与视觉顶面合并同策略。
            let mut width = 1;
            while di + width < cells && level_at(i + width as i64, j) == level {
                width += 1;
            }
            let mut height = 1;
            'expand_z: while dj + height < cells {
                for step in 0..width {
                    if level_at(i + step as i64, j + height as i64) != level {
                        break 'expand_z;
                    }
                }
                height += 1;
            }
            for row in 0..height {
                for step in 0..width {
                    merged[(dj + row) * cells + di + step] = true;
                }
            }
            let (x0, x1) = (world_x(i), world_x(i + width as i64));
            let (z0, z1) = (world_z(j), world_z(j + height as i64));
            let top = level as f32 * TERRAIN_STEP;
            let size = Vec3::new(x1 - x0, top - COLLIDER_BASE_Y, z1 - z0);
            let center = Vec3::new((x0 + x1) / 2.0, (top + COLLIDER_BASE_Y) / 2.0, (z0 + z1) / 2.0);
            cuboids.push((center, size));
        }
    }
    cuboids
}

/// 物理边界：墙的内侧与场地边缘对齐，墙角相交以封住四角。
pub(super) fn spawn_walls(commands: &mut Commands) {
    for sign in [-1.0, 1.0] {
        let edge = sign * (FIELD_SIDE + WALL_THICKNESS) / 2.0;
        for (size, position) in [
            (
                Vec3::new(WALL_THICKNESS, WALL_HEIGHT, FIELD_SIDE + 2.0 * WALL_THICKNESS),
                Vec3::new(edge, WALL_HEIGHT / 2.0, 0.0),
            ),
            (
                Vec3::new(FIELD_SIDE + 2.0 * WALL_THICKNESS, WALL_HEIGHT, WALL_THICKNESS),
                Vec3::new(0.0, WALL_HEIGHT / 2.0, edge),
            ),
        ] {
            commands.spawn((
                TerrainEntity,
                RigidBody::Static,
                Collider::cuboid(size.x, size.y, size.z),
                Transform::from_translation(position),
                Friction::ZERO,
            ));
        }
    }
}

/// 平坦场地 + 墙，仅供物理测试使用，以便驾驶与敌人测试不受地形起伏影响。
#[cfg(test)]
pub(crate) fn spawn_arena(commands: &mut Commands) {
    // `Collider::cuboid` 接收全长而非半长，这里给场地整边长。
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(FIELD_SIDE, 2.0, FIELD_SIDE),
        Transform::from_xyz(0.0, -1.0, 0.0),
        Friction::ZERO,
    ));
    spawn_walls(commands);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{
        FLAT, MAP_SIDE, TerrainSurface,
        map::real_heights,
    };

    /// 体素碰撞体必须按档位合并并与视觉格子对齐：同档位相邻格合并成一个
    /// cuboid（顶面 = 档位 × 0.1 米），全部 cuboid 底面统一在 [`COLLIDER_BASE_Y`]，
    /// 且所有格子中心恰好被某个 cuboid 的 XZ 覆盖（无缺口、无重复）。
    #[test]
    fn collider_cuboids_merge_by_level_and_align_with_cells() {
        // 与视觉测试同一份合成档位：X=1、Z=1..2 抬起 3 档（0.3 米），
        // X=2、Z=3 下沉 2 档（−0.2 米），其余平地。碰撞体只看档位、不看颜色。
        let mut levels = vec![0i32; 16];
        levels[1 * 4 + 1] = 3;
        levels[2 * 4 + 1] = 3;
        levels[3 * 4 + 2] = -2;
        let cuboids = collider_cuboids(&levels, 4, (0, 0), 4, (0.0, 0.0));
        assert!(!cuboids.is_empty(), "必须产出 cuboid");
        for (center, size) in &cuboids {
            let bottom = center.y - size.y / 2.0;
            assert!(
                (bottom - COLLIDER_BASE_Y).abs() < 1e-6,
                "cuboid 底面应统一在 COLLIDER_BASE_Y：{center:?} {size:?}"
            );
        }
        // 抬起区两格（沿 Z 相邻、同档位）必须合并为一个 cuboid：
        // X [0.5, 1.0]、Z [0.5, 1.5]、顶面 0.3 米。
        let raised: Vec<_> = cuboids
            .iter()
            .filter(|(c, s)| (c.y + s.y / 2.0 - 0.3).abs() < 1e-6)
            .collect();
        assert_eq!(raised.len(), 1, "抬起 3 档的格子应合并为一个 cuboid：{raised:?}");
        let (c, s) = raised[0];
        assert!((c.x - 0.75).abs() < 1e-6 && (s.x - 0.5).abs() < 1e-6, "X 范围 [0.5,1.0]：{c:?} {s:?}");
        assert!((c.z - 1.0).abs() < 1e-6 && (s.z - 1.0).abs() < 1e-6, "Z 范围 [0.5,1.5]：{c:?} {s:?}");
        // 下沉格：X [1.0, 1.5]、Z [1.5, 2.0]、顶面 −0.2 米。
        let sunk: Vec<_> = cuboids
            .iter()
            .filter(|(c, s)| (c.y + s.y / 2.0 - -0.2).abs() < 1e-6)
            .collect();
        assert_eq!(sunk.len(), 1, "下沉 2 档的格子应为一个 cuboid：{sunk:?}");
        let (c, s) = sunk[0];
        assert!((c.x - 1.25).abs() < 1e-6 && (c.z - 1.75).abs() < 1e-6, "下沉格位置：{c:?} {s:?}");
        // 覆盖性：16 个格子中心都必须恰好落在某个 cuboid 的 XZ 覆盖内。
        for j in 0..4i64 {
            for i in 0..4i64 {
                let (x, z) = (i as f32 * 0.5 + 0.25, j as f32 * 0.5 + 0.25);
                let hits = cuboids
                    .iter()
                    .filter(|(c, s)| {
                        (c.x - x).abs() <= s.x / 2.0 && (c.z - z).abs() <= s.z / 2.0
                    })
                    .count();
                assert_eq!(hits, 1, "格子 ({i},{j}) 中心 ({x},{z}) 应恰好被一个 cuboid 覆盖");
            }
        }
    }

    /// 真实高度图的体素复合碰撞体不得出现幽灵接触：出生保护区是 24 米半径的
    /// 绝对平地，坦克直行 1 秒（6 米）加车长仍留在平区内；平区外即使有台阶，
    /// 也全部是可翻越的 1 档坎（见 map 测试 `real_terrain_has_only_one_level_steps_and_flat_spawn`）。
    #[test]
    fn voxel_collider_keeps_straight_driving_on_real_terrain() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        // 与离屏 drive 场景一致：1/60 秒固定步长，重力由 DrivingPlugin 翻倍。
        let step = Duration::from_secs_f64(1.0 / 60.0);
        let levels: Vec<i32> = real_heights()
            .iter()
            .map(|value| *value as i32 - FLAT as i32)
            .collect();
        // 测试直接用整图一份复合碰撞体（不必按 60 米分块拆分），几何与运行时同源。
        let cuboids = collider_cuboids(&levels, MAP_SIDE, (0, 0), MAP_SIDE, (-FIELD_SIDE / 2.0, -FIELD_SIDE / 2.0));
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            crate::tank::driving::DrivingPlugin,
            crate::gameplay::player::PlayerPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(step));
        // 手动逐帧测试也需完成 Avian 在 finish 阶段注册的求解器资源。
        app.finish();
        app.cleanup();
        // 碰撞体直接铺进世界（等价于 Startup 生成，且避免闭包按值捕获复合碰撞体）。
        app.world_mut().spawn((
            TerrainSurface,
            RigidBody::Static,
            Collider::compound(
                cuboids
                    .into_iter()
                    .map(|(center, size)| {
                        (center, Rotation::IDENTITY, Collider::cuboid(size.x, size.y, size.z))
                    })
                    .collect(),
            ),
            Friction::ZERO,
        ));
        let tank = app
            .world_mut()
            .spawn((
                crate::gameplay::player::Tank,
                crate::tank::driving::tank_body(),
                Transform::from_xyz(0.0, 4.0, 0.0),
            ))
            .id();
        // 先模拟 120 帧让坦克在出生区落地，再按住前进 1 秒（车头朝世界 −X）。
        for _ in 0..120 {
            app.update();
        }
        let settled = app.world().get::<Transform>(tank).unwrap().translation;
        assert!((settled.y - crate::tank::driving::TANK_SIZE.y / 2.0).abs() < 0.05, "落地高度异常：{settled:?}");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..60 {
            app.update();
        }
        let moved = app.world().get::<Transform>(tank).unwrap().translation;
        assert!(
            moved.x - settled.x < -4.0,
            "体素复合碰撞体上平地直行不应被幽灵接触挡住：{moved:?}"
        );
    }

    /// 体素台阶 + 圆角盒坦克（能翻 1 档）的组合行为：坦克必须能翻过一档
    /// （0.1 米）的坎继续行驶，并停在坎顶高度；这是"体素碰撞可翻越矮坎"的回归测试。
    #[test]
    fn tank_climbs_one_voxel_step() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let step = Duration::from_secs_f64(1.0 / 60.0);
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            PhysicsPlugins::default(),
            crate::tank::driving::DrivingPlugin,
            crate::gameplay::player::PlayerPlugin,
        ))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(step));
        app.finish();
        app.cleanup();
        // 平地（x > 0，顶 0 米）+ 一档台阶（x < 0，顶 0.1 米），坦克从 x=8 向 −X 开。
        for (x0, x1, top) in [(0.0, 50.0, 0.0f32), (-50.0, 0.0, TERRAIN_STEP)] {
            let size_y = top - COLLIDER_BASE_Y;
            app.world_mut().spawn((
                TerrainSurface,
                TerrainEntity,
                RigidBody::Static,
                Collider::cuboid(x1 - x0, size_y, 20.0),
                Transform::from_xyz((x0 + x1) / 2.0, (top + COLLIDER_BASE_Y) / 2.0, 0.0),
                Friction::ZERO,
            ));
        }
        let tank = app
            .world_mut()
            .spawn((
                crate::gameplay::player::Tank,
                crate::tank::driving::tank_body(),
                Transform::from_xyz(8.0, 4.0, 0.0),
            ))
            .id();
        for _ in 0..120 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        // 4 秒 = 到达台阶（约 1 秒）+ 翻坎 + 在坎上继续行驶，留足余量。
        for _ in 0..240 {
            app.update();
        }
        let moved = app.world().get::<Transform>(tank).unwrap().translation;
        assert!(
            moved.x < -2.0,
            "坦克应翻过一档坎并继续行驶：{moved:?}"
        );
        assert!(
            (moved.y - (crate::tank::driving::TANK_SIZE.y / 2.0 + TERRAIN_STEP)).abs() < 0.05,
            "坦克应停在坎顶高度 {}：{moved:?}",
            crate::tank::driving::TANK_SIZE.y / 2.0 + TERRAIN_STEP
        );
    }
}
