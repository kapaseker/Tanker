//! 玩家模型的作者化尺寸契约；生成器与运行时共同读取，单位为体素格。

use super::WheelSpec;

/// 三包共同参考画布；高度扩为 37 格（3.70 米）以容纳车顶观瞄、舱盖与天线，
/// 不再用 2.70 米的旧画布压缩附件。纵向碰撞仍见 COLLISION_MAX。
#[allow(dead_code)] // Python 作者源消费。
pub const CANVAS: (u32, u32, u32) = (144, 38, 37);
/// 场景安装轴心，单位参考格；Z 保持旧画布中心 13.5，使扩高后整车离地高度与
/// 共同原点不漂移（新增高度全部位于屋顶以上）。X/Y 仍为画布中心。
pub const CANVAS_PIVOT: (f32, f32, f32) = (72.0, 19.0, 13.5);
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

/// 左右各七个负重轮和两个端轮（诱导轮在前、主动轮在后），共十八个独立轮节点；
/// 生成器和运行时共享节点名及安装位置。轴高统一 4.2 参考格，轮胎恰好骑在下段履带上。
pub const WHEELS: [[WheelSpec; 9]; 2] = [
    [
        wheel("wheel_l_0", 44.5, 4.5, false),
        wheel("wheel_l_1", 53.4, 4.5, false),
        wheel("wheel_l_2", 62.3, 4.5, false),
        wheel("wheel_l_3", 71.2, 4.5, false),
        wheel("wheel_l_4", 80.1, 4.5, false),
        wheel("wheel_l_5", 89.0, 4.5, false),
        wheel("wheel_l_6", 97.9, 4.5, false),
        wheel("wheel_l_7", 36.8, 4.5, true),
        wheel("wheel_l_8", 107.0, 4.5, true),
    ],
    [
        wheel("wheel_r_0", 44.5, 33.5, false),
        wheel("wheel_r_1", 53.4, 33.5, false),
        wheel("wheel_r_2", 62.3, 33.5, false),
        wheel("wheel_r_3", 71.2, 33.5, false),
        wheel("wheel_r_4", 80.1, 33.5, false),
        wheel("wheel_r_5", 89.0, 33.5, false),
        wheel("wheel_r_6", 97.9, 33.5, false),
        wheel("wheel_r_7", 36.8, 33.5, true),
        wheel("wheel_r_8", 107.0, 33.5, true),
    ],
];

/// 轮组旋转包络的竖直开区间，单位参考格：上界 8.4 即顶段履带下表面（0.84 米），
/// 负重轮包络顶 8.3 保持净空；下界 0.05 低于下段履带上表面（0.2 格），
/// 允许包络角探入履带板带、名义轮胎底面恰好骑在履带面上。完整包络必须落在其间。
pub const WHEEL_CLEARANCE: (f32, f32) = (0.05, 8.4);

/// 区分负重轮与端轮的名义滚动半径和连续旋转净空；轴高统一 4.2 参考格。
const fn wheel(name: &'static str, x: f32, y: f32, end: bool) -> WheelSpec {
    WheelSpec {
        name,
        center: [x, y, 4.2],
        size: if end { (5, 4, 5) } else { (8, 4, 8) },
        radius: if end { 2.5 } else { 4.0 },
        envelope: if end { 2.6 } else { 4.1 },
    }
}
