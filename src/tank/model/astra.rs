//! astraH 整车安装契约；参考格为 0.1 米，资产格为 0.02 米。

use super::WheelSpec;

/// 运行时整车资源：分文件只服务活动层级，不代表可换装部件。
pub const BODY_PATH: &str = "tank/player/astra/body.vox";
/// 含舱盖、观瞄、机枪、天线和烟幕的炮塔视觉。
pub const TURRET_PATH: &str = "tank/player/astra/turret.vox";
/// 绕炮轴俯仰的炮管及防盾。
pub const BARREL_PATH: &str = "tank/player/astra/barrel.vox";
/// 左右三相位与十八个独立轮节点。
pub const TRACK_PATH: &str = "tank/player/astra/track.vox";
/// 简化非炮管碰撞尺寸（长、高、宽），单位米；不以天线等附件扩高。
pub const COLLISION_SIZE: [f32; 3] = [7.68, 2.68, 3.76];
/// 车根在作者坐标中的轴心，单位参考格；地面 Z=0，因此车根落地高为 1.34 米。
pub const CANVAS_PIVOT: (f32, f32, f32) = (38.4, 0.0, 13.4);
/// 炮塔座圈轴心相对于车根，单位游戏米制坐标（−X 车头，+Y 上）。
pub const TURRET_POSITION: [f32; 3] = [0.0, 0.26, 0.0];
/// 炮管耳轴相对于炮塔轴心，单位游戏米制坐标。
pub const BARREL_POSITION: [f32; 3] = [-1.72, 0.48, 0.0];
/// 炮口端面相对于炮轴；弹丸中心可再沿发射方向偏移其半长。
pub const MUZZLE_POSITION: [f32; 3] = [-5.42, 0.0, 0.0];
/// 三相位近似原履带 7 个资产格的板距，单位米；不是全局参考格大小。
pub const TRACK_PHASE_STEP: f32 = 0.14 / 3.0;
/// 轮组旋转包络的竖直开区间，单位参考格；下界为地面，上界为履带外沿高度。
pub const WHEEL_CLEARANCE: (f32, f32) = (0.0, 9.4);

/// 每侧七个负重轮、后主动轮、前诱导轮；l 沿用引擎负宽轴侧，不代表作者车型左侧。
pub const WHEELS: [[WheelSpec; 9]; 2] = [
    [
        wheel("wheel_l_0", 11.6, -15.4, 0),
        wheel("wheel_l_1", 20.6, -15.4, 0),
        wheel("wheel_l_2", 29.4, -15.4, 0),
        wheel("wheel_l_3", 38.4, -15.4, 0),
        wheel("wheel_l_4", 47.2, -15.4, 0),
        wheel("wheel_l_5", 56.2, -15.4, 0),
        wheel("wheel_l_6", 65.0, -15.4, 0),
        wheel("wheel_l_7", 3.6, -15.4, 1),
        wheel("wheel_l_8", 73.0, -15.4, 2),
    ],
    [
        wheel("wheel_r_0", 11.6, 15.4, 0),
        wheel("wheel_r_1", 20.6, 15.4, 0),
        wheel("wheel_r_2", 29.4, 15.4, 0),
        wheel("wheel_r_3", 38.4, 15.4, 0),
        wheel("wheel_r_4", 47.2, 15.4, 0),
        wheel("wheel_r_5", 56.2, 15.4, 0),
        wheel("wheel_r_6", 65.0, 15.4, 0),
        wheel("wheel_r_7", 3.6, 15.4, 1),
        wheel("wheel_r_8", 73.0, 15.4, 2),
    ],
];

/// 轮径与完整体素顶点包络分别记录；端轮齿尖不冒充名义滚动半径。
const fn wheel(name: &'static str, x: f32, y: f32, kind: u8) -> WheelSpec {
    let (diameter, radius, envelope) = match kind {
        0 => (8.0, 4.0, 4.124),
        1 => (5.6, 2.5, 3.001),
        _ => (4.8, 2.5, 2.608),
    };
    WheelSpec {
        name,
        center: [x, y, 4.2],
        size: (diameter, 4.0, diameter),
        radius,
        envelope,
    }
}
