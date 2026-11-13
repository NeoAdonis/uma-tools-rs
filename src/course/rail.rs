//! Shape of the inner rail (lane 0) of a course in the world, which sets the
//! extra world distance runners cover by running wide or changing lanes, and
//! so how much it slows them down.
//!
//! The game places runners in the world from 1001 keyframes per course, at
//! equal course distances from the gate to the goal, each with the position
//! of the rail and a rotation that points the lanes outwards. Both are
//! interpolated linearly between keyframes, and a runner is moved outwards
//! from the rail by her lane offset in meters. A rail keeps what world
//! distances depend on:
//!
//! - The base ratio: world distance covered along the rail over the first
//!   meter of the course, which every other ratio is compared against.
//! - The position of each keyframe on the horizontal (xz) plane.
//! - The outward direction of the lanes at each keyframe on that plane (the
//!   x and z components of the rotation's unit vector pointing outwards).
//!
//! The rail data (one file per course named after its ID, in `data/jp/rails/`
//! and `data/en/rails/` for the Japanese and English versions of the game)
//! stores them, with the course length the keyframes cover. The versions'
//! rails differ, and the English one only has the courses of its version.

use serde::Deserialize;

use crate::units::CourseWidths;

/// Shape of the inner rail of a course (see the [module docs](self)).
///
/// The default rail is straight, with a base ratio of 1, e.g. for synthetic
/// courses of any length.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(try_from = "RailData")]
pub struct Rail {
    /// World distance covered along the rail over the first meter.
    pub(crate) base_ratio: f64,
    /// Course length in meters that the keyframes cover.
    pub(crate) length: f64,
    /// Keyframes per course meter.
    frames_per_meter: f64,
    /// Keyframes, equally spaced from the gate (first) to the goal (last).
    keyframes: Box<[Keyframe]>,
}

/// Position of the rail and outward direction of the lanes at a keyframe, on
/// the horizontal plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Keyframe {
    pub(crate) position: [f64; 2],
    pub(crate) outward: [f64; 2],
}

/// Rail as stored in the rail data.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RailData {
    base_ratio: f64,
    length: f64,
    x: Vec<f64>,
    z: Vec<f64>,
    outward_x: Vec<f64>,
    outward_z: Vec<f64>,
}

impl TryFrom<RailData> for Rail {
    type Error = String;

    fn try_from(data: RailData) -> Result<Self, Self::Error> {
        if data.base_ratio <= 0.0 || data.length <= 0.0 {
            return Err(format!(
                "non-positive base ratio {} or length {}",
                data.base_ratio, data.length
            ));
        }
        let lengths = [
            data.x.len(),
            data.z.len(),
            data.outward_x.len(),
            data.outward_z.len(),
        ];
        if lengths.iter().any(|&len| len != lengths[0]) || lengths[0] < 2 {
            return Err(format!(
                "{lengths:?} x, z and outward keyframes, instead of the same \
                 number (at least 2)"
            ));
        }
        let keyframes: Vec<_> = (0..lengths[0])
            .map(|i| Keyframe {
                position: [data.x[i], data.z[i]],
                outward: [data.outward_x[i], data.outward_z[i]],
            })
            .collect();
        Ok(Rail::new(data.base_ratio, data.length, keyframes))
    }
}

impl Default for Rail {
    fn default() -> Self {
        // A single 1m segment, extended past its end (see `world_position`).
        let keyframe = |x| Keyframe {
            position: [x, 0.0],
            outward: [0.0, 1.0],
        };
        Rail::new(1.0, 1.0, vec![keyframe(0.0), keyframe(1.0)])
    }
}

impl Rail {
    /// Create a rail from its keyframes, equally spaced over `length` meters.
    pub(crate) fn new(base_ratio: f64, length: f64, keyframes: Vec<Keyframe>) -> Self {
        debug_assert!(base_ratio > 0.0 && length > 0.0 && keyframes.len() >= 2);
        Rail {
            base_ratio,
            length,
            frames_per_meter: (keyframes.len() - 1) as f64 / length,
            keyframes: keyframes.into_boxed_slice(),
        }
    }

    /// World position on the horizontal plane at `position` in meters and
    /// `lane`.
    ///
    /// Positions before the gate are clamped to it, and those past the goal
    /// extend the last segment in a straight line.
    // Inlined into its only call site per frame, in another module.
    #[inline]
    pub(crate) fn world_position(&self, position: f64, lane: CourseWidths) -> [f64; 2] {
        // `u32` rather than `usize` indices, which convert to and from floats
        // with fewer instructions.
        let segments = (self.keyframes.len() - 1) as u32;
        let frame = (position * self.frames_per_meter).max(0.0);
        let idx = (frame as u32).min(segments - 1);
        let t = frame - idx as f64;
        let idx = idx as usize;
        let (start, end) = (self.keyframes[idx], self.keyframes[idx + 1]);
        let offset = lane.meters();
        let lerp = |i: usize| {
            let rail = start.position[i] + t * (end.position[i] - start.position[i]);
            let outward = start.outward[i] + t * (end.outward[i] - start.outward[i]);
            rail + offset * outward
        };
        [lerp(0), lerp(1)]
    }
}

/// Rail of a course `length` meters long, from the excess world length (over
/// the base ratio) and the turn (positive towards the inner rail) of each of
/// its equally long segments, with lanes pointing straight outwards.
#[cfg(test)]
pub(crate) fn rail_from_turns(base_ratio: f64, length: f64, excess: &[f64], turn: &[f64]) -> Rail {
    let segment = length / turn.len() as f64;
    // Turning left, so the lanes point to the right.
    let keyframe = |position: [f64; 2], heading: f64| Keyframe {
        position,
        outward: [heading.sin(), -heading.cos()],
    };
    let (mut position, mut heading) = ([0.0, 0.0], 0.0);
    let mut keyframes = vec![keyframe(position, heading)];
    for (excess, turn) in excess.iter().zip(turn) {
        let mid = heading + turn / 2.0;
        let step = segment * base_ratio + excess;
        position = [
            position[0] + step * mid.cos(),
            position[1] + step * mid.sin(),
        ];
        heading += turn;
        keyframes.push(keyframe(position, heading));
    }
    Rail::new(base_ratio, length, keyframes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::assert_close;

    /// Rail of a 4m course: 2m straight east, then 2m straight north, with
    /// the lanes pointing south then east (so the course turns left).
    fn rail() -> Rail {
        let keyframe = |position, outward| Keyframe { position, outward };
        Rail::new(
            1.0,
            4.0,
            vec![
                keyframe([0.0, 0.0], [0.0, -1.0]),
                keyframe([2.0, 0.0], [0.0, -1.0]),
                keyframe([2.0, 2.0], [1.0, 0.0]),
            ],
        )
    }

    #[test]
    fn test_world_position_interpolates_keyframes() {
        let lane = CourseWidths(0.0);
        let cases = [
            (1.0, [1.0, 0.0]),
            (2.0, [2.0, 0.0]),
            (3.0, [2.0, 1.0]),
            (4.0, [2.0, 2.0]),
        ];
        for (position, expected) in cases {
            assert_eq!(
                rail().world_position(position, lane),
                expected,
                "{position}m"
            );
        }
    }

    #[test]
    fn test_world_position_moves_outwards_by_lane() {
        let lane = CourseWidths(1.0);
        let offset = lane.meters();
        assert_eq!(rail().world_position(1.0, lane), [1.0, -offset]);
        // Halfway through the turn of the lanes.
        let [x, z] = rail().world_position(3.0, lane);
        assert_close!(x, 2.0 + offset / 2.0);
        assert_close!(z, 1.0 - offset / 2.0);
    }

    #[test]
    fn test_world_position_clamps_gate_and_extends_goal() {
        let rail = rail();
        let lane = CourseWidths(0.0);
        assert_eq!(rail.world_position(-2.0, lane), [0.0, 0.0]);
        assert_eq!(rail.world_position(5.0, lane), [2.0, 3.0]);
    }

    #[test]
    fn test_default_rail_is_straight_for_any_length() {
        let rail = Rail::default();
        assert_eq!(rail.base_ratio, 1.0);
        let lane = CourseWidths(0.5);
        let [x, z] = rail.world_position(2400.0, lane);
        assert_close!(x, 2400.0);
        assert_close!(z, lane.meters());
    }

    #[test]
    fn test_rail_from_turns() {
        // A quarter turn over the second of two 2m segments.
        let rail = rail_from_turns(1.0, 4.0, &[0.0, 0.0], &[0.0, std::f64::consts::FRAC_PI_2]);
        let lane = CourseWidths(0.0);
        let [x, z] = rail.world_position(4.0, lane);
        assert_close!(x, 2.0 + 2f64.sqrt());
        assert_close!(z, 2f64.sqrt());
    }

    #[test]
    fn test_deserialize_rejects_invalid_rails() {
        let parse = |json| serde_json::from_str::<Rail>(json);
        let rail = parse(
            r#"{"baseRatio":1.05,"length":2,"x":[0,2],"z":[0,0],
                "outwardX":[0,0],"outwardZ":[1,1]}"#,
        )
        .unwrap();
        assert_eq!(rail.base_ratio, 1.05);
        assert_eq!(rail.world_position(1.0, CourseWidths(0.0)), [1.0, 0.0]);
        let cases = [
            (
                r#"{"baseRatio":0,"length":2,"x":[0,2],"z":[0,0],"outwardX":[0,0],"outwardZ":[1,1]}"#,
                "non-positive base ratio 0 or length 2",
            ),
            (
                r#"{"baseRatio":1,"length":0,"x":[0,2],"z":[0,0],"outwardX":[0,0],"outwardZ":[1,1]}"#,
                "non-positive base ratio 1 or length 0",
            ),
            (
                r#"{"baseRatio":1,"length":2,"x":[0,2],"z":[0],"outwardX":[0,0],"outwardZ":[1,1]}"#,
                "[2, 1, 2, 2] x, z and outward keyframes",
            ),
            (
                r#"{"baseRatio":1,"length":2,"x":[0],"z":[0],"outwardX":[0],"outwardZ":[1]}"#,
                "[1, 1, 1, 1] x, z and outward keyframes",
            ),
        ];
        for (json, reason) in cases {
            let error = parse(json).unwrap_err().to_string();
            assert!(error.contains(reason), "{json}: {error}");
        }
    }
}
