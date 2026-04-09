//! System theme types.

/// The system theme preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    /// Light appearance.
    Light,
    /// Dark appearance.
    Dark,
}
