//! 与语言无关的通用符号；各语言文案模块可直接引用。

/// 操作方向符号。
pub(crate) const ARROW: &str = "→";

/// 键帽提示：光标上移。
pub(crate) const KEY_UP: &str = "↑";

/// 键帽提示：光标下移。
pub(crate) const KEY_DOWN: &str = "↓";

/// 键帽提示：确认。英文白名单内的按键标识，不随语言翻译。
pub(crate) const KEY_ENTER: &str = "ENTER";

/// 返回操作方向符号。
pub(crate) const BACK_ARROW: &str = "←";
/// 返回键帽标识，不随语言翻译。
pub(crate) const KEY_ESCAPE: &str = "ESC";
