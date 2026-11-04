//! 与语言无关的通用符号；各语言文案模块可直接引用。

/// 操作方向符号。
pub(crate) const ARROW: &str = "→";

/// 键帽提示：光标上移。
pub(crate) const KEY_UP: &str = "↑";

/// 键帽提示：光标下移。
pub(crate) const KEY_DOWN: &str = "↓";

/// 炮管仰俯与炮塔左右旋转的键帽标识，不随语言翻译。
pub(crate) const KEY_W: &str = "W";
pub(crate) const KEY_S: &str = "S";
pub(crate) const KEY_A: &str = "A";
pub(crate) const KEY_D: &str = "D";

/// 炮塔回正与炮管回正的键帽标识，不随语言翻译。
pub(crate) const KEY_R: &str = "R";
pub(crate) const KEY_F: &str = "F";

/// 键帽提示：确认。英文白名单内的按键标识，不随语言翻译。
pub(crate) const KEY_ENTER: &str = "ENTER";

/// 装备参数单位：米/秒；英文缩写，不随语言翻译。
pub(crate) const UNIT_METERS_PER_SECOND: &str = "m/s";

/// 装备参数单位：发/分钟；英文缩写，不随语言翻译。
pub(crate) const UNIT_ROUNDS_PER_MINUTE: &str = "rpm";
/// 炮塔运动规格的角度符号；与数字放在同一文本中以保持右上位置。
pub(crate) const UNIT_DEGREES: &str = "°";
/// 每秒后缀；角度符号已跟随主数值排版。
pub(crate) const UNIT_PER_SECOND: &str = "/s";

/// 返回操作方向符号。
pub(crate) const BACK_ARROW: &str = "←";
/// 返回键帽标识，不随语言翻译。
pub(crate) const KEY_ESCAPE: &str = "ESC";
/// 退出弹框键帽：确认与取消按钮的快捷键标识，不随语言翻译。
pub(crate) const KEY_Y: &str = "Y";
pub(crate) const KEY_N: &str = "N";
