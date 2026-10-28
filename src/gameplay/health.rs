//! 玩家车体生命值组件；表现层（HUD）只读本组件，写入方后续由战斗系统承担。

use bevy::prelude::*;

/// 车体生命值：current 在 0..=max 内变化，单位为“点”。
/// HUD 等表现层只读；当前没有伤害系统，current 固定等于 max。
/// ponytail: 暂无任何伤害来源，接入受击逻辑后由 combat 系统写入 current。
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Health {
    /// 当前生命点数，0 表示损毁。
    current: u32,
    /// 生命上限点数，生成时确定，运行期不变。
    max: u32,
}

impl Health {
    /// 满血状态；上限取 100 点，与设计稿百分比进度条一一对应。
    pub(crate) fn full(max: u32) -> Self {
        Self { current: max, max }
    }

    /// 生命比例 0..=1，供 HUD 进度条直接换算填充宽度；max 为 0 按空血处理。
    pub(crate) fn ratio(&self) -> f32 {
        if self.max == 0 {
            return 0.0;
        }
        (self.current.min(self.max) as f32) / (self.max as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 满血比例恒为 1；越界与零上限不产生 NaN 或超界比例。
    #[test]
    fn ratio_is_clamped_between_zero_and_one() {
        assert_eq!(Health::full(100).ratio(), 1.0);
        assert_eq!(Health { current: 30, max: 100 }.ratio(), 0.3);
        // current 超过 max 时钳制到 1。
        assert_eq!(Health { current: 150, max: 100 }.ratio(), 1.0);
        // 零上限按空血处理，避免除零。
        assert_eq!(Health { current: 0, max: 0 }.ratio(), 0.0);
    }
}
