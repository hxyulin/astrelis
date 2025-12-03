//! Type-safe length and dimension types for UI styling.
//!
//! Provides enum-based alternatives to raw floats for better type safety
//! and clearer intent in style declarations.

use std::fmt;

/// Length value for UI dimensions.
///
/// Represents different ways to specify sizes in the UI system.
///
/// # Examples
/// ```
/// use astrelis_ui::Length;
///
/// let fixed = Length::Px(100.0);
/// let relative = Length::Percent(50.0);
/// let auto_size = Length::Auto;
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// Fixed pixel value
    Px(f32),
    /// Percentage of parent size (0.0 - 100.0)
    Percent(f32),
    /// Automatic sizing based on content
    Auto,
}

impl Length {
    /// Create a pixel length.
    pub fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Create a percentage length.
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Create an auto length.
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Convert to Taffy Dimension.
    pub fn to_dimension(self) -> taffy::Dimension {
        match self {
            Length::Px(v) => taffy::Dimension::Length(v),
            Length::Percent(v) => taffy::Dimension::Percent(v / 100.0),
            Length::Auto => taffy::Dimension::Auto,
        }
    }

    /// Convert from Taffy Dimension.
    pub fn from_dimension(dim: taffy::Dimension) -> Self {
        match dim {
            taffy::Dimension::Length(v) => Length::Px(v),
            taffy::Dimension::Percent(v) => Length::Percent(v * 100.0),
            taffy::Dimension::Auto => Length::Auto,
        }
    }

    /// Check if this is a fixed pixel value.
    pub fn is_px(&self) -> bool {
        matches!(self, Length::Px(_))
    }

    /// Check if this is a percentage value.
    pub fn is_percent(&self) -> bool {
        matches!(self, Length::Percent(_))
    }

    /// Check if this is auto.
    pub fn is_auto(&self) -> bool {
        matches!(self, Length::Auto)
    }
}

impl From<f32> for Length {
    fn from(value: f32) -> Self {
        Length::Px(value)
    }
}

impl From<Length> for taffy::Dimension {
    fn from(length: Length) -> Self {
        length.to_dimension()
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Length::Px(v) => write!(f, "{}px", v),
            Length::Percent(v) => write!(f, "{}%", v),
            Length::Auto => write!(f, "auto"),
        }
    }
}

/// Length value that can also be Auto (for margins/insets).
///
/// Similar to Length but allows automatic positioning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthAuto {
    /// Fixed pixel value
    Px(f32),
    /// Percentage of parent size (0.0 - 100.0)
    Percent(f32),
    /// Automatic positioning
    Auto,
}

impl LengthAuto {
    /// Create a pixel length.
    pub fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Create a percentage length.
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Create an auto length.
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Convert to Taffy LengthPercentageAuto.
    pub fn to_length_percentage_auto(self) -> taffy::LengthPercentageAuto {
        match self {
            LengthAuto::Px(v) => taffy::LengthPercentageAuto::Length(v),
            LengthAuto::Percent(v) => taffy::LengthPercentageAuto::Percent(v / 100.0),
            LengthAuto::Auto => taffy::LengthPercentageAuto::Auto,
        }
    }

    /// Convert from Taffy LengthPercentageAuto.
    pub fn from_length_percentage_auto(lpa: taffy::LengthPercentageAuto) -> Self {
        match lpa {
            taffy::LengthPercentageAuto::Length(v) => LengthAuto::Px(v),
            taffy::LengthPercentageAuto::Percent(v) => LengthAuto::Percent(v * 100.0),
            taffy::LengthPercentageAuto::Auto => LengthAuto::Auto,
        }
    }
}

impl From<f32> for LengthAuto {
    fn from(value: f32) -> Self {
        LengthAuto::Px(value)
    }
}

impl From<Length> for LengthAuto {
    fn from(length: Length) -> Self {
        match length {
            Length::Px(v) => LengthAuto::Px(v),
            Length::Percent(v) => LengthAuto::Percent(v),
            Length::Auto => LengthAuto::Auto,
        }
    }
}

impl From<LengthAuto> for taffy::LengthPercentageAuto {
    fn from(length: LengthAuto) -> Self {
        length.to_length_percentage_auto()
    }
}

impl fmt::Display for LengthAuto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthAuto::Px(v) => write!(f, "{}px", v),
            LengthAuto::Percent(v) => write!(f, "{}%", v),
            LengthAuto::Auto => write!(f, "auto"),
        }
    }
}

/// Length value without Auto (for padding/border).
///
/// Similar to Length but doesn't allow Auto sizing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LengthPercentage {
    /// Fixed pixel value
    Px(f32),
    /// Percentage of parent size (0.0 - 100.0)
    Percent(f32),
}

impl LengthPercentage {
    /// Create a pixel length.
    pub fn px(value: f32) -> Self {
        Self::Px(value)
    }

    /// Create a percentage length.
    pub fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    /// Convert to Taffy LengthPercentage.
    pub fn to_length_percentage(self) -> taffy::LengthPercentage {
        match self {
            LengthPercentage::Px(v) => taffy::LengthPercentage::Length(v),
            LengthPercentage::Percent(v) => taffy::LengthPercentage::Percent(v / 100.0),
        }
    }

    /// Convert from Taffy LengthPercentage.
    pub fn from_length_percentage(lp: taffy::LengthPercentage) -> Self {
        match lp {
            taffy::LengthPercentage::Length(v) => LengthPercentage::Px(v),
            taffy::LengthPercentage::Percent(v) => LengthPercentage::Percent(v * 100.0),
        }
    }
}

impl From<f32> for LengthPercentage {
    fn from(value: f32) -> Self {
        LengthPercentage::Px(value)
    }
}

impl From<LengthPercentage> for taffy::LengthPercentage {
    fn from(length: LengthPercentage) -> Self {
        length.to_length_percentage()
    }
}

impl fmt::Display for LengthPercentage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LengthPercentage::Px(v) => write!(f, "{}px", v),
            LengthPercentage::Percent(v) => write!(f, "{}%", v),
        }
    }
}

/// Helper function to create a Taffy length dimension.
pub fn length(value: f32) -> taffy::Dimension {
    taffy::Dimension::Length(value)
}

/// Helper function to create a Taffy percent dimension.
pub fn percent(value: f32) -> taffy::Dimension {
    taffy::Dimension::Percent(value / 100.0)
}

/// Helper function to create auto dimension.
pub fn auto() -> taffy::Dimension {
    taffy::Dimension::Auto
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length_px() {
        let len = Length::Px(100.0);
        assert!(len.is_px());
        assert!(!len.is_percent());
        assert!(!len.is_auto());
        assert_eq!(len.to_string(), "100px");
    }

    #[test]
    fn test_length_percent() {
        let len = Length::Percent(50.0);
        assert!(!len.is_px());
        assert!(len.is_percent());
        assert!(!len.is_auto());
        assert_eq!(len.to_string(), "50%");
    }

    #[test]
    fn test_length_auto() {
        let len = Length::Auto;
        assert!(!len.is_px());
        assert!(!len.is_percent());
        assert!(len.is_auto());
        assert_eq!(len.to_string(), "auto");
    }

    #[test]
    fn test_length_from_f32() {
        let len: Length = 100.0.into();
        assert_eq!(len, Length::Px(100.0));
    }

    #[test]
    fn test_length_to_dimension() {
        let len = Length::Px(100.0);
        let dim = len.to_dimension();
        assert!(matches!(dim, taffy::Dimension::Length(100.0)));

        let len = Length::Percent(50.0);
        let dim = len.to_dimension();
        assert!(matches!(dim, taffy::Dimension::Percent(0.5)));

        let len = Length::Auto;
        let dim = len.to_dimension();
        assert!(matches!(dim, taffy::Dimension::Auto));
    }

    #[test]
    fn test_length_percentage() {
        let lp = LengthPercentage::Px(100.0);
        assert_eq!(lp.to_string(), "100px");

        let lp = LengthPercentage::Percent(50.0);
        assert_eq!(lp.to_string(), "50%");
    }

    #[test]
    fn test_length_auto_conversion() {
        let la = LengthAuto::Px(100.0);
        assert_eq!(la.to_string(), "100px");

        let len: LengthAuto = Length::Percent(50.0).into();
        assert_eq!(len, LengthAuto::Percent(50.0));
    }

    #[test]
    fn test_helper_functions() {
        let dim = length(100.0);
        assert!(matches!(dim, taffy::Dimension::Length(100.0)));

        let dim = percent(50.0);
        assert!(matches!(dim, taffy::Dimension::Percent(0.5)));

        let dim = auto();
        assert!(matches!(dim, taffy::Dimension::Auto));
    }
}
