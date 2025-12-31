//! Global layout parameters for invalidation tracking.

use crate::state::app_state::WrapMode;

/// Global parameters that affect entry layout.
///
/// Used for invalidation: if current params != last layout params,
/// full relayout may be needed.
///
/// Note: Per-entry state (expanded, wrap_override) is stored in EntryView,
/// not here. This struct only tracks global parameters.
///
/// # Equality Semantics
/// Two LayoutParams are equal if they would produce identical layouts
/// (assuming per-entry state unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutParams {
    /// Viewport width in columns.
    pub width: u16,
    /// Global wrap mode.
    pub global_wrap: WrapMode,
}

impl LayoutParams {
    /// Create new layout params.
    pub fn new(width: u16, global_wrap: WrapMode) -> Self {
        Self { width, global_wrap }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equality_same_params() {
        let params1 = LayoutParams::new(80, WrapMode::Wrap);
        let params2 = LayoutParams::new(80, WrapMode::Wrap);
        assert_eq!(params1, params2);
    }

    #[test]
    fn test_inequality_different_width() {
        let params1 = LayoutParams::new(80, WrapMode::Wrap);
        let params2 = LayoutParams::new(120, WrapMode::Wrap);
        assert_ne!(params1, params2);
    }

    #[test]
    fn test_inequality_different_wrap_mode() {
        let params1 = LayoutParams::new(80, WrapMode::Wrap);
        let params2 = LayoutParams::new(80, WrapMode::NoWrap);
        assert_ne!(params1, params2);
    }

    #[test]
    fn test_inequality_both_different() {
        let params1 = LayoutParams::new(80, WrapMode::Wrap);
        let params2 = LayoutParams::new(120, WrapMode::NoWrap);
        assert_ne!(params1, params2);
    }
}
