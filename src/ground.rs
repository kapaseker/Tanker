//! 加载包含 16 个独立模型的 VOX 场地，物理表面仍为平面。

use avian3d::prelude::*;
use bevy::{asset::LoadState, prelude::*, world_serialization::WorldAsset};

/// 正方形场地总面积，单位：平方米。
const AREA: f32 = 4000.0;
/// 全图未缩放边长，单位：米（632 格 × 0.1 米），模型定位由 VOX 场景图提供。
const SOURCE_SIDE: f32 = 63.2;
/// 不可见边界的厚度和高度，单位：米。
const WALL_THICKNESS: f32 = 2.0;
const WALL_HEIGHT: f32 = 8.0;

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

/// 整体居中并缩放至 4000 平方米，Y 不缩放以保留单格 10 cm 的起伏。
fn ground_transform() -> Transform {
    let scale = AREA.sqrt() / SOURCE_SIDE;
    // 声明高度 6 格，以包围盒中心为原点；列高 3/4/5 对应 −10/0/+10 cm。
    Transform::from_xyz(0.0, -0.1, 0.0).with_scale(Vec3::new(scale, 1.0, scale))
}

/// 一个根节点加载整张非周期场景，视觉分块不影响平面碰撞与边界。
fn setup_ground(mut commands: Commands, assets: Res<AssetServer>) {
    let scene = assets.load("ground.vox");
    commands.spawn((WorldAssetRoot(scene.clone()), ground_transform()));
    commands.insert_resource(GroundAsset(scene));
    spawn_arena(&mut commands);
}

/// 物理表面为 Y=0；墙的内侧与场地边缘对齐，墙角相交以封住四角。
pub(super) fn spawn_arena(commands: &mut Commands) {
    let side = AREA.sqrt();
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(side, 2.0, side),
        Transform::from_xyz(0.0, -1.0, 0.0),
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
            info!("vox 场景加载完成：assets/ground.vox（4000 平方米，16 个独立模型）");
            *reported = true;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 检查水平面积、中心位置及竖直格高，防止场景缩放改变起伏幅度。
    #[test]
    fn scene_covers_a_centered_four_thousand_square_meters() {
        let transform = ground_transform();
        assert!(((SOURCE_SIDE * transform.scale.x).powi(2) - 4000.0).abs() < 0.001);
        assert_eq!(transform.scale.y, 1.0);
        assert_eq!(transform.scale.x, transform.scale.z);
        assert_eq!(transform.translation, Vec3::new(0.0, -0.1, 0.0));
        for (height, expected) in [(3, -0.1), (4, 0.0), (5, 0.1)] {
            assert!(
                ((height as f32 - 3.0) * 0.1 + transform.translation.y - expected).abs() < 1e-6
            );
        }
    }
}
