use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

/// Represents a corner on the course.
#[derive(Clone, Debug, Deserialize)]
pub struct Corner {
    /// Start position of the corner in meters.
    pub start: f64,
    /// Length of the corner in meters.
    pub length: f64,
}

/// Represents the slope type of a course segment.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SlopeType {
    Flat = 0,
    Uphill = 1,
    Downhill = 2,
}

/// Represents a slope segment on the course.
#[derive(Clone, Debug, Deserialize)]
pub struct Slope {
    /// Start position of the slope in meters.
    pub start: f64,
    /// Length of the slope in meters.
    pub length: f64,
    /// Slope value (in ten-thousandths).
    slope: i32,
}

impl Slope {
    /// Create a new Slope for testing purposes
    #[cfg(test)]
    pub fn new(start: f64, length: f64, slope: i32) -> Self {
        Slope { start, length, slope }
    }

    /// Check if the slope is uphill.
    pub fn is_uphill(&self) -> bool {
        self.slope > 0
    }

    /// Check if the slope is downhill.
    pub fn is_downhill(&self) -> bool {
        self.slope < 0
    }

    /// Get the slope as a percentage.
    pub fn slope_percent(&self) -> f64 {
        self.slope as f64 / 10000.0
    }

    /// Get the slope type classification.
    pub fn slope_type(&self) -> SlopeType {
        if self.slope > 0 {
            SlopeType::Uphill
        } else if self.slope < 0 {
            SlopeType::Downhill
        } else {
            SlopeType::Flat
        }
    }
}

/// Represents whether a straight segment is facing the audience or not.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum FrontType {
    /// Towards the audience.
    Audience = 1,
    /// Away from the audience.
    Opposite = 2,
    Unknown = 3,
}

/// Represents a straight segment on the course.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Straight {
    /// Start position of the straight in meters.
    pub start: f64,
    /// End position of the straight in meters.
    pub end: f64,
    /// Whether the straight is facing the audience or not.
    pub front_type: FrontType,
}
