//! 共享 VOX 地块铺设与独立的平面物理场地。

use avian3d::prelude::*;
use bevy::{asset::LoadState, prelude::*, world_serialization::WorldAsset};

/// 正方形场地总面积，单位：平方米。
const AREA: f32 = 1000.0;
/// 每轴重复地块数；单块 158 格，避免 VOX 的 u8 坐标上限。
const TILES: u32 = 4;
/// 地块未缩放的边长，单位：米（158 格 × 0.05 米）。
const TILE_SIZE: f32 = 7.9;
/// 不可见边界的厚度和高度，单位：米。
const WALL_THICKNESS: f32 = 1.0;
const WALL_HEIGHT: f32 = 4.0;

/// 保持地板资产存活，并用于一次性汇报依赖加载结果。
#[derive(Resource)]
struct GroundAsset(Handle<WorldAsset>);

/// 注册地板铺设、物理边界及资源加载汇报。
pub(super) struct GroundPlugin;

impl Plugin for GroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ground)
            .add_systems(Update, report_ground);
    }
}

/// 返回第 x、z 块的世界变换；整体居中且水平面积精确匹配场地。
fn tile_transform(x: u32, z: u32) -> Transform {
    let spacing = AREA.sqrt() / TILES as f32;
    let center = (TILES - 1) as f32 / 2.0;
    // 模型高 6 格，中心为 0；再下移一格，使列顶高度落在 −0.1～+0.1 米。
    Transform::from_xyz(
        (x as f32 - center) * spacing,
        -0.05,
        (z as f32 - center) * spacing,
    )
    .with_scale(Vec3::new(spacing / TILE_SIZE, 1.0, spacing / TILE_SIZE))
}

/// 生成一个平面碰撞体、四面不可见边界和 16 个共用资产的视觉地块。
fn setup_ground(mut commands: Commands, assets: Res<AssetServer>) {
    let scene = assets.load("ground.vox");
    for x in 0..TILES {
        for z in 0..TILES {
            commands.spawn((WorldAssetRoot(scene.clone()), tile_transform(x, z)));
        }
    }
    commands.insert_resource(GroundAsset(scene));
    spawn_arena(&mut commands);
}

/// 物理表面为 Y=0；墙的内侧与场地边缘对齐，墙角相交以封住四角。
pub(super) fn spawn_arena(commands: &mut Commands) {
    let side = AREA.sqrt();
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(side, 1.0, side),
        Transform::from_xyz(0.0, -0.5, 0.0),
        Friction::ZERO,
    ));
    for sign in [-1.0, 1.0] {
        let edge = sign * (side + WALL_THICKNESS) / 2.0;
        for (size, position) in [
            (
                Vec3::new(WALL_THICKNESS, WALL_HEIGHT, side + 2.0 * WALL_THICKNESS),
                Vec3::new(edge, WALL_HEIGHT / 2.0, 0.0),
            ),
            (
                Vec3::new(side + 2.0 * WALL_THICKNESS, WALL_HEIGHT, WALL_THICKNESS),
                Vec3::new(0.0, WALL_HEIGHT / 2.0, edge),
            ),
        ] {
            commands.spawn((
                RigidBody::Static,
                Collider::cuboid(size.x, size.y, size.z),
                Transform::from_translation(position),
                Friction::ZERO,
            ));
        }
    }
}

/// 加载失败与含依赖加载完成各汇报一次，避免只检查主资产掩盖材质错误。
fn report_ground(ground: Res<GroundAsset>, assets: Res<AssetServer>, mut reported: Local<bool>) {
    if *reported {
        return;
    }
    match assets.get_load_state(&ground.0) {
        Some(LoadState::Failed(error)) => {
            error!("地板资源加载失败：{error}");
            *reported = true;
        }
        _ if assets.is_loaded_with_dependencies(&ground.0) => {
            info!("vox 场景加载完成：assets/ground.vox（1000 平方米，4×4 地块）");
            *reported = true;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 检查整体面积、相邻块接边与中心对称，防止缩放或半格偏移留下裂缝。
    #[test]
    fn tiles_cover_a_centered_thousand_square_meters() {
        let first = tile_transform(0, 0);
        let last = tile_transform(3, 3);
        let width = TILE_SIZE * first.scale.x;
        assert!(((width * 4.0).powi(2) - AREA).abs() < 0.001);
        assert!((tile_transform(1, 0).translation.x - first.translation.x - width).abs() < 1e-5);
        assert!((first.translation.x + last.translation.x).abs() < 1e-5);
        assert!((first.translation.x - width / 2.0 + AREA.sqrt() / 2.0).abs() < 1e-5);
    }
}
