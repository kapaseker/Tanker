//! 敌方原创现代坦克的作者化契约；Python 生成器读取这些字面量，单位为体素格。
use super::WheelSpec;

/// 三包共同画布；车体中心 X=57，尾部留白使炮管零偏移安装。
pub const CANVAS: (u32, u32, u32) = (114, 34, 26);
/// 非炮管底盘范围，半开区间；长度66格。
pub const HULL_X: (i32, i32) = (24, 90);
/// 炮塔正面至炮口，半开区间；整车视觉长度90格。
pub const BARREL_X: (i32, i32) = (76, 114);
/// 敌方炮管轴线离地高度；独立于玩家车型。
pub const BORE_HEIGHT: f32 = 20.0;
/// 敌方非炮管几何边界；由本车型画布定义，不随玩家改造变化。
pub const COLLISION_MIN: [i32; 3] = [HULL_X.0, 0, 0];
/// 宽度与高度来自共同画布，炮管不扩大碰撞框。
pub const COLLISION_MAX: [i32; 3] = [HULL_X.1, CANVAS.1 as i32, CANVAS.2 as i32];

/// 非炮管碰撞尺寸（长、高、宽），单位：米。长度与宽度由本车型碰撞边界推导，
/// 高度同样来自敌方边界；运行时驾驶刚体、履带规格与敌人生成共同读取，
/// 避免在业务模块复制常量。
pub const COLLISION_SIZE: [f32; 3] = [
    (COLLISION_MAX[0] - COLLISION_MIN[0]) as f32 * super::REFERENCE_VOXEL_SIZE,
    (COLLISION_MAX[2] - COLLISION_MIN[2]) as f32 * super::REFERENCE_VOXEL_SIZE,
    (COLLISION_MAX[1] - COLLISION_MIN[1]) as f32 * super::REFERENCE_VOXEL_SIZE,
];

/// 履带内腔的竖直净空，供轮组完整旋转包络校验。
pub const WHEEL_CLEARANCE: (f32, f32) = (1.0, 12.0);
/// 每侧四个负重轮和前后两个端轮；名字与包内节点一致。
pub const WHEELS: [[WheelSpec; 6]; 2] = [
    [
        wheel("wheel_l_0", 37.5, 4.0, false),
        wheel("wheel_l_1", 50.5, 4.0, false),
        wheel("wheel_l_2", 63.5, 4.0, false),
        wheel("wheel_l_3", 76.5, 4.0, false),
        wheel("wheel_l_4", 28.5, 4.0, true),
        wheel("wheel_l_5", 85.5, 4.0, true),
    ],
    [
        wheel("wheel_r_0", 37.5, 30.0, false),
        wheel("wheel_r_1", 50.5, 30.0, false),
        wheel("wheel_r_2", 63.5, 30.0, false),
        wheel("wheel_r_3", 76.5, 30.0, false),
        wheel("wheel_r_4", 28.5, 30.0, true),
        wheel("wheel_r_5", 85.5, 30.0, true),
    ],
];

/// 大轮直径1米、端轮0.6米；旋转包络含体素顶点，避免转到斜角时穿插。
const fn wheel(name: &'static str, x: f32, y: f32, end: bool) -> WheelSpec {
    WheelSpec {
        name,
        center: [x, y, if end { 7.0 } else { 6.5 }],
        size: if end { (6, 4, 6) } else { (10, 4, 10) },
        radius: if end { 3.0 } else { 5.0 },
        envelope: if end { 3.3 } else { 5.3 },
    }
}
