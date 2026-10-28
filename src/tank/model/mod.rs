//! 坦克模型的作者化契约：体素尺度与各车型的尺寸、轮位数据。
//! 纯标准库数据；运行时与 `scripts/` 下的生成器共同读取，不依赖引擎类型。

pub(crate) mod enemy;
pub(crate) mod player;

/// 作者化参考网格的单格边长，单位：米。仅用于旧尺寸/轮位契约换算米制值，
/// 不是资产加载单位——后者来自每个 `.vox` 旁的 `.vox.meta`（voxel_size 字段）。
/// 阶段 A 二者同为 0.1；阶段 B 细分资产为 0.02 后本常量仍表示参考格。
pub(crate) const VOXEL_SIZE: f32 = 0.1;

/// 单轮安装契约；VOX 坐标用体素格，允许半格轴心。
#[derive(Clone, Copy)]
pub struct WheelSpec {
    /// 包内命名节点，左右侧顺序与履带里程一致。
    pub name: &'static str,
    /// VOX 全局轴心（长、宽、高），加载时映射为（−X、Z、Y）。
    pub center: [f32; 3],
    /// 局部画布：静止外包络直径、轮宽、直径。
    pub size: (u32, u32, u32),
    /// 名义滚动半径，单位为体素格。
    pub radius: f32,
    /// 全部体素顶点旋转后的最大半径，单位为体素格。
    pub envelope: f32,
}
