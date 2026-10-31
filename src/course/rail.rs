//! Shape of the inner rail (lane 0) of a course in the world, which sets the
//! extra world distance runners cover by running wide or changing lanes, and
//! so how much it slows them down.
//!
//! The game places runners in the world from 1001 keyframes per course, at
//! equal course distances from the gate to the goal, each with the position
//! of the rail and a rotation that points the lanes outwards. A rail is made
//! from those keyframes, and keeps only what world distances depend on:
//!
//! - The base ratio: world distance covered along the rail over the first
//!   meter of the course, which every other ratio is compared against.
//! - The world length of the rail (on the horizontal plane) between
//!   keyframes, beyond the base ratio.
//! - The turn of the rotation between keyframes: running off the rail where
//!   the course turns covers more world distance.
//!
//! The last two are kept as running totals from the gate, interpolated linearly
//! between keyframes, so the extra distance of any stretch is the difference
//! of the totals at both ends. The rail data (`data/rails/`, one file per
//! course named after its ID) stores them per segment between keyframes
//! instead.

use serde::Deserialize;

/// Shape of the inner rail of a course (see the [module docs](self)).
///
/// The default rail is straight, with a base ratio of 1 and no excess world
/// length, e.g. for synthetic courses.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(try_from = "RailData")]
pub struct Rail {
    /// World distance covered along the rail over the first meter.
    pub(crate) base_ratio: f64,
    /// Running totals at each keyframe, equally spaced from the gate (first)
    /// to the goal (last).
    keyframes: Box<[Totals]>,
}

/// Running totals of a rail from the gate to a keyframe or position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Totals {
    /// World length of the rail beyond `base_ratio` times the course
    /// distance, in meters (negative if shorter).
    pub(crate) excess: f64,
    /// Turn in radians, positive towards the inner rail (i.e. in the course
    /// direction).
    pub(crate) turn: f64,
}

/// Rail as stored in the rail data, per segment between keyframes.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RailData {
    base_ratio: f64,
    excess: Vec<f64>,
    turn: Vec<f64>,
}

impl TryFrom<RailData> for Rail {
    type Error = String;

    fn try_from(data: RailData) -> Result<Self, Self::Error> {
        if data.base_ratio <= 0.0 {
            return Err(format!("non-positive base ratio {}", data.base_ratio));
        }
        if data.excess.len() != data.turn.len() || data.turn.is_empty() {
            return Err(format!(
                "{} excess and {} turn segments, instead of the same number",
                data.excess.len(),
                data.turn.len()
            ));
        }
        Ok(Rail::new(data.base_ratio, &data.excess, &data.turn))
    }
}

impl Default for Rail {
    fn default() -> Self {
        Rail::new(1.0, &[0.0], &[0.0])
    }
}

impl Rail {
    /// Create a rail from the excess world length and turn of each of its
    /// equally long segments.
    pub(crate) fn new(base_ratio: f64, excess: &[f64], turn: &[f64]) -> Self {
        debug_assert!(base_ratio > 0.0 && excess.len() == turn.len() && !turn.is_empty());
        let mut total = Totals::default();
        let keyframes = std::iter::once(total)
            .chain(excess.iter().zip(turn).map(|(excess, turn)| {
                total.excess += excess;
                total.turn += turn;
                total
            }))
            .collect();
        Rail {
            base_ratio,
            keyframes,
        }
    }

    /// Running totals at `position` in meters, on a course `length` meters
    /// long, clamped to the course.
    pub(crate) fn totals_at(&self, position: f64, length: f64) -> Totals {
        // `u32` rather than `usize` indices, which convert to and from floats
        // with fewer instructions.
        let segments = (self.keyframes.len() - 1) as u32;
        let frame = (position * segments as f64 / length).clamp(0.0, segments as f64);
        let idx = (frame as u32).min(segments - 1);
        let t = frame - idx as f64;
        let idx = idx as usize;
        let (start, end) = (self.keyframes[idx], self.keyframes[idx + 1]);
        Totals {
            excess: start.excess + t * (end.excess - start.excess),
            turn: start.turn + t * (end.turn - start.turn),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_close;

    /// Rail of a 4m course, with two 2m segments.
    fn rail() -> Rail {
        Rail::new(1.0, &[0.1, -0.3], &[0.5, 0.0])
    }

    #[test]
    fn test_totals_at_interpolates_running_totals() {
        let cases = [
            (0.0, 0.0, 0.0),
            (1.0, 0.05, 0.25),
            (2.0, 0.1, 0.5),
            (3.0, -0.05, 0.5),
            (4.0, -0.2, 0.5),
        ];
        for (position, excess, turn) in cases {
            let totals = rail().totals_at(position, 4.0);
            assert_close!(totals.excess, excess, "excess at {position}m");
            assert_close!(totals.turn, turn, "turn at {position}m");
        }
    }

    #[test]
    fn test_totals_at_clamps_to_course() {
        let rail = rail();
        assert_eq!(rail.totals_at(-2.0, 4.0), Totals::default());
        assert_eq!(rail.totals_at(6.0, 4.0), rail.totals_at(4.0, 4.0));
    }

    #[test]
    fn test_default_rail_is_straight() {
        let rail = Rail::default();
        assert_eq!(rail.base_ratio, 1.0);
        assert_eq!(rail.totals_at(2000.0, 2000.0), Totals::default());
    }

    #[test]
    fn test_deserialize_rejects_invalid_rails() {
        let parse = |json| serde_json::from_str::<Rail>(json);
        let rail = parse(r#"{"baseRatio":1.05,"excess":[0.1],"turn":[0.2]}"#).unwrap();
        assert_eq!(rail, Rail::new(1.05, &[0.1], &[0.2]));
        let cases = [
            r#"{"baseRatio":0,"excess":[0.1],"turn":[0.2]}"#,
            r#"{"baseRatio":1,"excess":[0.1],"turn":[0.2,0.3]}"#,
            r#"{"baseRatio":1,"excess":[],"turn":[]}"#,
        ];
        for json in cases {
            assert!(parse(json).is_err(), "{json}");
        }
    }
}
