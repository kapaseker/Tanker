//! 玩家模型的作者化尺寸契约；生成器与运行时共同读取，单位为体素格。

use super::WheelSpec;

/// 三包共同参考画布；body 根 X=72，碰撞中心与画布中心重合。
pub const CANVAS: (u32, u32, u32) = (144, 38, 27);
/// 主要几何向外量化后的纵向碰撞边界；视觉边界另见 VISUAL_HULL_X。
pub const HULL_X: (i32, i32) = (33, 111);
/// 炮盾前面至炮口端面，单位参考格；允许 0.02 米的五分之一参考格。
pub const BARREL_X: (f32, f32) = (95.0, 143.6);
/// 炮管轴线距地高度，单位参考格；外观比例估算，非实车测绘。
pub const BORE_HEIGHT: f32 = 20.8;
/// 视觉车体后缘和前缘，单位 0.02 米资产格，半开区间。
pub const VISUAL_HULL_X: (i32, i32) = (169, 552);
/// 含前楔的炮塔纵向边界，单位资产格，不含独立炮管。
#[allow(dead_code)] // Python 作者源消费。
pub const TURRET_X: (i32, i32) = (245, 475);
/// 炮塔主体屋顶高度，单位资产格，排除舱盖等附件。
#[allow(dead_code)] // Python 作者源消费。
pub const TURRET_TOP: i32 = 132;
/// 主要车体、炮塔及履带的简化碰撞下界，单位参考格。
pub const COLLISION_MIN: [i32; 3] = [HULL_X.0, 0, 0];
/// 主要几何向外取 0.1 米后的碰撞上界；不以附件最高点扩高。
pub const COLLISION_MAX: [i32; 3] = [HULL_X.1, 38, 27];

/// 左右各五个负重轮和两个端轮；生成器和运行时共享节点名及安装位置。
pub const WHEELS: [[WheelSpec; 7]; 2] = [
    [
        wheel("wheel_l_0", 45.0, 4.5, false),
        wheel("wheel_l_1", 58.5, 4.5, false),
        wheel("wheel_l_2", 72.0, 4.5, false),
        wheel("wheel_l_3", 85.5, 4.5, false),
        wheel("wheel_l_4", 99.0, 4.5, false),
        wheel("wheel_l_5", 37.0, 4.5, true),
        wheel("wheel_l_6", 107.0, 4.5, true),
    ],
    [
        wheel("wheel_r_0", 45.0, 33.5, false),
        wheel("wheel_r_1", 58.5, 33.5, false),
        wheel("wheel_r_2", 72.0, 33.5, false),
        wheel("wheel_r_3", 85.5, 33.5, false),
        wheel("wheel_r_4", 99.0, 33.5, false),
        wheel("wheel_r_5", 37.0, 33.5, true),
        wheel("wheel_r_6", 107.0, 33.5, true),
    ],
];

/// 区分负重轮与端轮的名义半径和连续旋转净空。
const fn wheel(name: &'static str, x: f32, y: f32, end: bool) -> WheelSpec {
    WheelSpec {
        name,
        center: [x, y, if end { 6.5 } else { 6.0 }],
        size: if end { (5, 4, 5) } else { (8, 4, 8) },
        radius: if end { 2.5 } else { 4.0 },
        envelope: if end { 2.8 } else { 4.3 },
    }
}
