//! Error types shared by several modules.

use std::fmt;

/// A number that doesn't match any variant of the enum it was converted to,
/// returned by the enums' `TryFrom` impls from their game values.
///
/// The number is widened to `i16`, which holds the values of both `u8` and
/// `i8` enums.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InvalidValueError(i16);

impl InvalidValueError {
    pub(crate) fn new(value: impl Into<i16>) -> Self {
        Self(value.into())
    }

    /// The number that matched no variant.
    pub fn value(&self) -> i16 {
        self.0
    }
}

impl fmt::Display for InvalidValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid value {}", self.0)
    }
}

impl std::error::Error for InvalidValueError {}
