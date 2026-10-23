//! 玩家模型的作者化尺寸契约；生成器与运行时共同读取，单位为体素格。

use super::WheelSpec;

/// 每包共用的对称画布；车体中心在 X=67.5，车尾空白保证加载后零偏移对齐。
pub const CANVAS: (u32, u32, u32) = (135, 36, 26);
/// 底盘后缘（包含）与前缘（不包含），不包含炮管。
pub const HULL_X: (i32, i32) = (30, 105);
/// 炮塔正面与炮口端面；二者之间为独立炮管，长度 45 格。
pub const BARREL_X: (i32, i32) = (90, 135);
/// 炮管轴线距地高度：20 格；宽度轴线在画布正中。
pub const BORE_HEIGHT: f32 = 20.0;

/// 非炮管几何的 VOX 边界（最小点包含、最大点不包含），由生成器测试核对。
pub const COLLISION_MIN: [i32; 3] = [HULL_X.0, 0, 0];
/// 碰撞框上界；区别于含大量空白和炮管的统一画布尺寸。
pub const COLLISION_MAX: [i32; 3] = [HULL_X.1, 36, 26];

/// 左右各五个负重轮和两个端轮；生成器和运行时共享节点名及安装位置。
pub const WHEELS: [[WheelSpec; 7]; 2] = [
    [
        wheel("wheel_l_0", 43.5, 4.0, false),
        wheel("wheel_l_1", 55.5, 4.0, false),
        wheel("wheel_l_2", 67.5, 4.0, false),
        wheel("wheel_l_3", 79.5, 4.0, false),
        wheel("wheel_l_4", 91.5, 4.0, false),
        wheel("wheel_l_5", 34.0, 4.0, true),
        wheel("wheel_l_6", 101.0, 4.0, true),
    ],
    [
        wheel("wheel_r_0", 43.5, 32.0, false),
        wheel("wheel_r_1", 55.5, 32.0, false),
        wheel("wheel_r_2", 67.5, 32.0, false),
        wheel("wheel_r_3", 79.5, 32.0, false),
        wheel("wheel_r_4", 91.5, 32.0, false),
        wheel("wheel_r_5", 34.0, 32.0, true),
        wheel("wheel_r_6", 101.0, 32.0, true),
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
