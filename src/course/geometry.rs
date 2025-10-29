use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Clone, Debug, Deserialize)]
pub struct Corner {
    pub start: f64,  // in meters
    pub length: f64, // in meters
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SlopeType {
    Flat = 0,
    Uphill = 1,
    Downhill = 2,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Slope {
    pub start: f64,  // in meters
    pub length: f64, // in meters
    slope: i32,
}

impl Slope {
    /// Create a new Slope for testing purposes
    #[cfg(test)]
    pub fn new(start: f64, length: f64, slope: i32) -> Self {
        Slope { start, length, slope }
    }

    pub fn is_uphill(&self) -> bool {
        self.slope > 0
    }

    pub fn is_downhill(&self) -> bool {
        self.slope < 0
    }

    pub fn slope_percent(&self) -> f64 {
        self.slope as f64 / 10000.0
    }

    /// Get the slope type classification
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum FrontType {
    Audience = 1,
    Opposite = 2,
    Unknown = 3,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Straight {
    pub start: f64, // in meters
    pub end: f64,   // in meters
    pub front_type: FrontType,
}
