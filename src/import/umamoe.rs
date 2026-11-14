//! Import of races and runners from uma.moe build codes.
//!
//! uma.moe shares a race setup as a build code: JSON compressed with gzip and
//! encoded in base64. [`UmaMoeBuild`] reads one, ignoring the fields that the
//! simulation doesn't use (runner names, team colors, number of runs, etc.).
//!
//! A build code holds:
//!
//! - The course ID and race conditions (track condition, weather, season and
//!   time of day), with the game's values. There's no grade, so races are G1,
//!   as in Champions Meeting.
//! - The runners of the race field (`page.field`), with their teams, mood and
//!   skills. Older build codes, or those without a field, only have two runners
//!   (`uma1` and `uma2`), with no team.
//! - The gate order, and the gates of some runners (see
//!   [`UmaMoeBuild::configure_gates`] and [`UmaMoeRunner::gate`]).
//!
//! Runners have aptitudes for every distance and track surface, so racing with
//! them takes the ones to use (see [`UmaMoeRunner::to_uma_data`], or
//! [`UmaMoeBuild::entrants`] for the course of the race). Their fan count is
//! optional.
//!
//! uma.moe allows the Runaway style, which the game doesn't have: it's the
//! effect of the Runaway skill on a Front runner. So runners with that style
//! are imported as Front runners with the skill, added if missing.
//!
//! # Examples
//!
//! Race the field of a build code, with its teams and gates:
//!
//! ```
//! use uma_tools::import::umamoe::UmaMoeBuild;
//! use uma_tools::{Configuration, Course, RaceSolver, SkillData};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let path = "tests/data/umamoe_build.txt";
//! let build = UmaMoeBuild::from_build_code_file(path)?;
//! let parameters = build.race_parameters();
//! let (courses, rails) = ("data/course_data.json", "data/jp/rails");
//! let course_id = &parameters.course_id;
//! let course = Course::from_json_files(courses, rails, course_id)?;
//! let all_skills = SkillData::from_json_file("data/jp/skill_data.json")?;
//!
//! let mut config = Configuration::default();
//! build.configure_gates(&mut config);
//! let teams = build.entrants(&course, &all_skills)?;
//! let mut solver = RaceSolver::new(course, parameters.race, config);
//! for team in teams {
//!     solver.add_team(team);
//! }
//! let mut solver = solver.ready_runners();
//! solver.run();
//! assert_eq!(solver.runners().len(), 9);
//! # Ok(())
//! # }
//! ```

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use flate2::read::GzDecoder;
use serde::de::{Error, IntoDeserializer};
use serde::{Deserialize, Deserializer};
use serde_with::{TryFromInto, serde_as};
use std::io::Read;
use std::path::Path;

use crate::configuration::{Assignment, Configuration, DefaultGate};
use crate::course::{Course, DistanceType, Track};
use crate::race::parameters::*;
use crate::race::solver::{Entrant, MAX_TEAM_SIZE};
use crate::skill::{DEFAULT_SKILL_LEVEL, SkillData};
use crate::uma::*;

/// Only supported version of the build code format.
const VERSION: u8 = 1;

/// A race setup from an uma.moe build code.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "RawBuild")]
pub struct UmaMoeBuild {
    course_id: u16,
    track_condition: TrackCondition,
    weather: Weather,
    season: Season,
    time: Time,
    /// From 1 to [`MAX_GATES`] runners, in field order.
    runners: Vec<UmaMoeRunner>,
    gate_order: Option<RawGateOrder>,
}

/// A runner from an uma.moe build code.
///
/// Converts it with [`UmaMoeRunner::to_uma_data`] to race with it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UmaMoeRunner {
    /// See [`Uma::character_id`].
    character_id: Option<u32>,
    speed: u16,
    stamina: u16,
    power: u16,
    guts: u16,
    wit: u16,
    style: BaseStyle,
    /// Short, Mile, Medium and Long aptitudes.
    distance_aptitudes: [Aptitude; 4],
    /// Front, Pace, Late and End aptitudes.
    style_aptitudes: [Aptitude; 4],
    /// Turf and Dirt aptitudes.
    track_aptitudes: [Aptitude; 2],
    mood: Mood,
    /// See [`Uma::fan_count`].
    fan_count: Option<u32>,
    skills: Vec<UmaSkill>,
    /// Team ID in the build code, if any.
    team: Option<u8>,
    /// Gate (1-indexed) set in the build code, if any.
    gate: Option<u8>,
}

/// Build code as it is in JSON.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBuild {
    version: u8,
    course_id: u16,
    racedef: RawRaceDef,
    uma1: Option<RawUma>,
    uma2: Option<RawUma>,
    page: Option<RawPage>,
}

/// Race conditions, with the game's values.
#[serde_as]
#[derive(Deserialize)]
struct RawRaceDef {
    #[serde_as(as = "TryFromInto<u8>")]
    ground: TrackCondition,
    #[serde_as(as = "TryFromInto<u8>")]
    weather: Weather,
    #[serde_as(as = "TryFromInto<u8>")]
    season: Season,
    #[serde_as(as = "TryFromInto<u8>")]
    time: Time,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPage {
    field: Option<Vec<RawFieldUma>>,
    gate_order: Option<RawGateOrder>,
}

/// Gate order, as in the build code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
enum RawGateOrder {
    /// Gates as set for each runner, the rest in field order.
    #[serde(rename = "sent")]
    Sent,
    /// Random gates for every runner, even those with one set.
    #[serde(rename = "randomized")]
    Randomized,
}

/// Runner of the race field.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFieldUma {
    trainee: RawTrainee,
    skills: Vec<RawSkill>,
    card_id: Option<u32>,
    team_id: Option<u8>,
    fan_count: Option<u32>,
    /// Gate (1-indexed).
    frame_order: Option<u8>,
}

/// Stats, style, mood and aptitudes of a runner of the race field, with stats
/// and mood as strings.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTrainee {
    #[serde(deserialize_with = "deserialize_string_stat")]
    speed: u16,
    #[serde(deserialize_with = "deserialize_string_stat")]
    stamina: u16,
    #[serde(deserialize_with = "deserialize_string_stat")]
    power: u16,
    #[serde(deserialize_with = "deserialize_string_stat")]
    guts: u16,
    #[serde(deserialize_with = "deserialize_string_stat")]
    wisdom: u16,
    #[serde(deserialize_with = "deserialize_style")]
    running_style: (BaseStyle, bool),
    #[serde(deserialize_with = "deserialize_motivation")]
    motivation: Mood,
    distance_aptitudes: [Aptitude; 4],
    style_aptitudes: [Aptitude; 4],
    ground_aptitudes: [Aptitude; 2],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSkill {
    skill_id: i32,
    level: u8,
}

/// Runner from `uma1` or `uma2`, with skill IDs as strings and the level of
/// the unique skill apart.
#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawUma {
    outfit_id: Option<String>,
    #[serde(deserialize_with = "deserialize_stat")]
    speed: u16,
    #[serde(deserialize_with = "deserialize_stat")]
    stamina: u16,
    #[serde(deserialize_with = "deserialize_stat")]
    power: u16,
    #[serde(deserialize_with = "deserialize_stat")]
    guts: u16,
    #[serde(deserialize_with = "deserialize_stat")]
    wisdom: u16,
    #[serde(deserialize_with = "deserialize_style")]
    strategy: (BaseStyle, bool),
    /// Distance, style and track aptitudes, in the same order as the race
    /// field's.
    aptitudes: [Aptitude; 10],
    skills: Vec<String>,
    unique_lv: u8,
    #[serde_as(as = "TryFromInto<i8>")]
    mood: Mood,
    fans: Option<u32>,
}

/// ID of the Runaway (大逃げ) skill, which makes a Front runner a Runaway.
const RUNAWAY_SKILL_ID: i32 = 202051;

/// Deserializes a running style, which uma.moe also allows to be Runaway
/// (`Oonige`).
///
/// Returns the basic style (Front for Runaway) and whether it was Runaway.
fn deserialize_style<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<(BaseStyle, bool), D::Error> {
    let name = String::deserialize(deserializer)?;
    if name == "Oonige" {
        return Ok((BaseStyle::Front, true));
    }
    let style = BaseStyle::deserialize(name.into_deserializer())?;
    Ok((style, false))
}

/// Adds the Runaway skill to `skills` if `runaway` and it's missing.
///
/// Runaway is the effect of that skill on a Front runner.
fn add_runaway_skill(skills: &mut Vec<UmaSkill>, runaway: bool) {
    if runaway && !skills.iter().any(|skill| skill.id == RUNAWAY_SKILL_ID) {
        skills.push(UmaSkill {
            id: RUNAWAY_SKILL_ID,
            level: DEFAULT_SKILL_LEVEL,
        });
    }
}

/// Deserializes a stat from a string, as [`deserialize_stat`] does from a
/// number.
fn deserialize_string_stat<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u16, D::Error> {
    let value = String::deserialize(deserializer)?;
    let stat: u16 = value
        .parse()
        .map_err(|_| D::Error::custom(format!("invalid stat {value:?}: expected a number")))?;
    deserialize_stat(stat.into_deserializer())
}

/// Deserializes a mood from a string, from `"1"` (Awful) to `"5"` (Great).
fn deserialize_motivation<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Mood, D::Error> {
    let value = String::deserialize(deserializer)?;
    value
        .parse::<i8>()
        .ok()
        .and_then(Mood::from_motivation)
        .ok_or_else(|| D::Error::custom(format!("invalid motivation {value:?}: expected 1 to 5")))
}

impl From<RawFieldUma> for UmaMoeRunner {
    fn from(raw: RawFieldUma) -> Self {
        let trainee = raw.trainee;
        let (style, runaway) = trainee.running_style;
        let mut skills: Vec<_> = raw
            .skills
            .into_iter()
            .map(|skill| UmaSkill {
                id: skill.skill_id,
                level: skill.level,
            })
            .collect();
        add_runaway_skill(&mut skills, runaway);
        UmaMoeRunner {
            character_id: raw.card_id,
            speed: trainee.speed,
            stamina: trainee.stamina,
            power: trainee.power,
            guts: trainee.guts,
            wit: trainee.wisdom,
            style,
            distance_aptitudes: trainee.distance_aptitudes,
            style_aptitudes: trainee.style_aptitudes,
            track_aptitudes: trainee.ground_aptitudes,
            mood: trainee.motivation,
            fan_count: raw.fan_count,
            skills,
            team: raw.team_id,
            gate: raw.frame_order,
        }
    }
}

impl TryFrom<RawUma> for UmaMoeRunner {
    type Error = String;

    /// Converts a runner from `uma1` or `uma2`, with `unique_lv` as the level of
    /// its unique skills (6-digit IDs starting with 1), and level 1 for the rest.
    fn try_from(raw: RawUma) -> Result<Self, String> {
        let character_id = match raw.outfit_id.as_deref() {
            None | Some("") => None,
            Some(id) => Some(
                id.parse()
                    .map_err(|_| format!("invalid outfit ID {id:?}: expected a number"))?,
            ),
        };
        let mut skills = raw
            .skills
            .iter()
            .map(|id| {
                let id: i32 = id
                    .parse()
                    .map_err(|_| format!("invalid skill ID {id:?}: expected a number"))?;
                let unique = (100000..200000).contains(&id);
                let level = if unique { raw.unique_lv } else { 1 };
                Ok(UmaSkill { id, level })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (style, runaway) = raw.strategy;
        add_runaway_skill(&mut skills, runaway);
        let [a0, a1, a2, a3, a4, a5, a6, a7, a8, a9] = raw.aptitudes;
        Ok(UmaMoeRunner {
            character_id,
            speed: raw.speed,
            stamina: raw.stamina,
            power: raw.power,
            guts: raw.guts,
            wit: raw.wisdom,
            style,
            distance_aptitudes: [a0, a1, a2, a3],
            style_aptitudes: [a4, a5, a6, a7],
            track_aptitudes: [a8, a9],
            mood: raw.mood,
            fan_count: raw.fans,
            skills,
            team: None,
            gate: None,
        })
    }
}

impl TryFrom<RawBuild> for UmaMoeBuild {
    type Error = String;

    fn try_from(raw: RawBuild) -> Result<Self, String> {
        if raw.version != VERSION {
            return Err(format!(
                "unsupported build code version {}: expected {VERSION}",
                raw.version
            ));
        }
        let (field, gate_order) = match raw.page {
            Some(page) => (page.field.unwrap_or_default(), page.gate_order),
            None => (Vec::new(), None),
        };
        // Fall back to `uma1` and `uma2` without a race field.
        let runners: Vec<UmaMoeRunner> = if field.is_empty() {
            [raw.uma1, raw.uma2]
                .into_iter()
                .flatten()
                .map(UmaMoeRunner::try_from)
                .collect::<Result<_, _>>()?
        } else {
            field.into_iter().map(UmaMoeRunner::from).collect()
        };

        if !(1..=MAX_GATES as usize).contains(&runners.len()) {
            return Err(format!(
                "invalid number of runners {}: expected 1 to {MAX_GATES}",
                runners.len()
            ));
        }
        let gates = runners.len();
        for gate in runners.iter().filter_map(|runner| runner.gate) {
            if !(1..=gates).contains(&(gate as usize)) {
                return Err(format!("invalid gate {gate}: expected 1 to {gates}"));
            }
        }
        for runner in &runners {
            let Some(team) = runner.team else { continue };
            let size = runners
                .iter()
                .filter(|other| other.team == Some(team))
                .count();
            if size > MAX_TEAM_SIZE {
                return Err(format!(
                    "invalid team {team} of {size} runners: expected up to \
                     {MAX_TEAM_SIZE}"
                ));
            }
        }

        let racedef = raw.racedef;
        Ok(UmaMoeBuild {
            course_id: raw.course_id,
            track_condition: racedef.ground,
            weather: racedef.weather,
            season: racedef.season,
            time: racedef.time,
            runners,
            gate_order,
        })
    }
}

impl UmaMoeBuild {
    /// Parses `UmaMoeBuild` from a build code.
    ///
    /// Surrounding whitespace is ignored.
    ///
    /// # Errors
    ///
    /// Returns an error if the build code isn't valid base64 or gzip, or for
    /// the same reasons as [`UmaMoeBuild::from_json`].
    pub fn from_build_code(code: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let compressed = BASE64.decode(code.trim())?;
        let mut json = String::new();
        GzDecoder::new(compressed.as_slice()).read_to_string(&mut json)?;
        Ok(Self::from_json(&json)?)
    }

    /// Parses `UmaMoeBuild` from a file with a build code.
    ///
    /// # Errors
    ///
    /// Returns an error if the file can't be read, or for the same reasons as
    /// [`UmaMoeBuild::from_build_code`].
    ///
    /// See the [module docs](self) for an example.
    pub fn from_build_code_file(
        path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        Self::from_build_code(&content)
    }

    /// Parses `UmaMoeBuild` from the JSON of a decoded build code.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON is invalid, the format version isn't 1, a
    /// value is out of range (e.g. a stat is zero), there are no runners or
    /// more than [`MAX_GATES`], or a team has more than [`MAX_TEAM_SIZE`].
    ///
    /// # Examples
    ///
    /// Only the fields used by the simulation are required:
    ///
    /// ```
    /// use uma_tools::configuration::DefaultGate;
    /// use uma_tools::import::umamoe::UmaMoeBuild;
    /// use uma_tools::Configuration;
    ///
    /// # fn main() -> Result<(), serde_json::Error> {
    /// let build = UmaMoeBuild::from_json(
    ///     r#"{
    ///         "version": 1, "courseId": 10506,
    ///         "racedef": { "ground": 1, "weather": 1, "season": 1, "time": 2 },
    ///         "page": {
    ///             "gateOrder": "sent",
    ///             "field": [{
    ///                 "trainee": {
    ///                     "speed": "1200", "stamina": "800", "power": "1000",
    ///                     "guts": "600", "wisdom": "1100",
    ///                     "runningStyle": "Sashi", "motivation": "4",
    ///                     "distanceAptitudes": ["E", "B", "A", "S"],
    ///                     "styleAptitudes": ["G", "C", "A", "B"],
    ///                     "groundAptitudes": ["A", "F"]
    ///                 },
    ///                 "skills": [{ "skillId": 100211, "level": 5 }]
    ///             }]
    ///         }
    ///     }"#,
    /// )?;
    /// assert_eq!(build.runners().len(), 1);
    /// let mut config = Configuration::default();
    /// build.configure_gates(&mut config);
    /// assert_eq!(config.default_gate, DefaultGate::InOrder);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Returns the race parameters and course ID, as a G1 race with one gate per
    /// runner.
    pub fn race_parameters(&self) -> RaceParametersData {
        RaceParametersData {
            race: RaceParameters {
                track_condition: self.track_condition,
                weather: self.weather,
                season: self.season,
                time: self.time,
                grade: Grade::G1,
                gates: self.runners.len() as u8,
                career: false,
            },
            course_id: self.course_id.to_string(),
        }
    }

    /// Returns the runners, in field order.
    pub fn runners(&self) -> &[UmaMoeRunner] {
        &self.runners
    }

    /// Returns the runners grouped by team, in order of first appearance in the
    /// field.
    ///
    /// Runners without a team are alone in theirs.
    ///
    /// Teams have at most [`MAX_TEAM_SIZE`] runners, so that each can be added
    /// with [`RaceSolver::add_team`].
    ///
    /// [`RaceSolver::add_team`]: crate::race::solver::RaceSolver::add_team
    pub fn teams(&self) -> Vec<Vec<&UmaMoeRunner>> {
        let mut teams: Vec<Vec<&UmaMoeRunner>> = Vec::new();
        for runner in &self.runners {
            let existing = runner
                .team
                .and_then(|id| teams.iter_mut().find(|team| team[0].team == Some(id)));
            match existing {
                Some(team) => team.push(runner),
                None => teams.push(vec![runner]),
            }
        }
        teams
    }

    /// Returns the runners as [`Entrant`]s to race on `course`, grouped by team as
    /// in [`Self::teams`].
    ///
    /// Each team can be added with [`RaceSolver::add_team`].
    ///
    /// Each runner takes her aptitudes for the course (see
    /// [`UmaMoeRunner::to_uma_data`]), her mood and her gate (see
    /// [`UmaMoeRunner::gate`]), and her skills are resolved from `all_skills`
    /// (see [`UmaData::to_entrant`]).
    ///
    /// # Errors
    ///
    /// Returns an error if a runner's skills can't be resolved.
    ///
    /// [`RaceSolver::add_team`]: crate::race::solver::RaceSolver::add_team
    pub fn entrants(
        &self,
        course: &Course,
        all_skills: &SkillData,
    ) -> Result<Vec<Vec<Entrant>>, ResolveSkillsError> {
        let (distance, track) = (course.distance_type(), course.surface);
        self.teams()
            .into_iter()
            .map(|team| {
                team.into_iter()
                    .map(|runner| {
                        let entrant = runner.to_uma_data(distance, track).to_entrant(all_skills)?;
                        Ok(Entrant {
                            gate: runner.gate,
                            ..entrant
                        })
                    })
                    .collect()
            })
            .collect()
    }

    /// Configures `config` to assign gates as the build code says, if it does.
    ///
    /// Gates are either as sent ([`Assignment::Unset`] and
    /// [`DefaultGate::InOrder`]), or random ones for every runner
    /// ([`Assignment::All`] and [`DefaultGate::Random`]).
    ///
    /// As sent, runners without a gate (see [`UmaMoeRunner::gate`]) take the
    /// free ones in field order. The solver gives them in the order runners are
    /// added, so they match if runners are added in field order.
    pub fn configure_gates(&self, config: &mut Configuration) {
        let (assignment, default_gate) = match self.gate_order {
            None => return,
            Some(RawGateOrder::Sent) => (Assignment::Unset, DefaultGate::InOrder),
            Some(RawGateOrder::Randomized) => (Assignment::All, DefaultGate::Random),
        };
        config.gate_assignment = assignment;
        config.default_gate = default_gate;
    }
}

impl UmaMoeRunner {
    /// Returns the team ID in the build code, if any.
    pub fn team(&self) -> Option<u8> {
        self.team
    }

    /// Returns the gate (1-indexed) set in the build code, if any (see
    /// [`Entrant::gate`]).
    pub fn gate(&self) -> Option<u8> {
        self.gate
    }

    /// Converts `self` to [`UmaData`] for a race of the given distance type and
    /// track surface.
    ///
    /// The distance and track aptitudes are the ones for `distance` and
    /// `track`, and the style aptitude the one for the runner's style.
    pub fn to_uma_data(&self, distance: DistanceType, track: Track) -> UmaData {
        let style_index = match self.style {
            BaseStyle::Front => 0,
            BaseStyle::Pace => 1,
            BaseStyle::Late => 2,
            BaseStyle::End => 3,
        };
        let distance_index = match distance {
            DistanceType::Short => 0,
            DistanceType::Mile => 1,
            DistanceType::Medium => 2,
            DistanceType::Long => 3,
        };
        let track_index = match track {
            Track::Turf => 0,
            Track::Dirt => 1,
        };
        UmaData {
            uma: Uma {
                speed: self.speed,
                stamina: self.stamina,
                power: self.power,
                guts: self.guts,
                wit: self.wit,
                style: self.style,
                distance_aptitude: self.distance_aptitudes[distance_index],
                track_aptitude: self.track_aptitudes[track_index],
                style_aptitude: self.style_aptitudes[style_index],
                fan_count: self.fan_count,
                character_id: self.character_id,
            },
            mood: Some(self.mood),
            skills: self.skills.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::Skill;
    use std::sync::Arc;

    /// Path to a build code from uma.moe, with 9 runners in 3 teams.
    const BUILD_PATH: &str = "tests/data/umamoe_build.txt";

    /// JSON for the race field of a build code, with two runners of team 1
    /// with distinct aptitudes, and the given gate order.
    fn field_json(gate_order: &str) -> String {
        let runner = |card_id: u32, style: &str| {
            format!(
                r#"{{
                    "trainee": {{
                        "speed": "1201", "stamina": "802", "power": "1003",
                        "guts": "604", "wisdom": "1105",
                        "runningStyle": "{style}", "motivation": "2",
                        "distanceAptitudes": ["S", "A", "B", "C"],
                        "styleAptitudes": ["D", "E", "F", "G"],
                        "groundAptitudes": ["A", "B"]
                    }},
                    "skills": [
                        {{ "skillId": 100011, "level": 4 }},
                        {{ "skillId": 200012, "level": 1 }}
                    ],
                    "cardId": {card_id}, "teamId": 1, "fanCount": 123456
                }}"#
            )
        };
        format!(
            r#"{{
                "version": 1, "courseId": 10101,
                "racedef": {{ "ground": 3, "weather": 4, "season": 5, "time": 4 }},
                "page": {{
                    {gate_order}
                    "field": [{}, {}]
                }}
            }}"#,
            runner(100101, "Nige"),
            runner(100201, "Senko"),
        )
    }

    /// JSON for a build code without a race field, with `uma1` and `uma2`.
    const UMA1_UMA2_JSON: &str = r#"{
        "version": 1, "courseId": 10506,
        "racedef": { "ground": 1, "weather": 1, "season": 1, "time": 1 },
        "uma1": {
            "outfitId": "100201", "speed": 664, "stamina": 544, "power": 473,
            "guts": 451, "wisdom": 1189, "strategy": "Oonige",
            "aptitudes": ["A", "A", "A", "E", "S", "B", "C", "D", "A", "G"],
            "skills": ["100021", "200132", "910071"],
            "uniqueLv": 5, "mood": -1, "fans": 654321
        },
        "uma2": {
            "outfitId": "", "speed": 713, "stamina": 608, "power": 473,
            "guts": 463, "wisdom": 1238, "strategy": "Oikomi",
            "aptitudes": ["A", "A", "A", "B", "A", "A", "A", "E", "A", "A"],
            "skills": [], "uniqueLv": 6, "mood": 2
        },
        "page": { "tab": "compare" }
    }"#;

    #[test]
    fn test_from_build_code_file() {
        let build = UmaMoeBuild::from_build_code_file(BUILD_PATH).unwrap();
        assert_eq!(
            build.race_parameters(),
            RaceParametersData {
                race: RaceParameters {
                    track_condition: TrackCondition::Good,
                    weather: Weather::Cloudy,
                    season: Season::Winter,
                    time: Time::Noon,
                    grade: Grade::G1,
                    gates: 9,
                    career: false,
                },
                course_id: "10506".to_string(),
            }
        );
        assert_eq!(build.gate_order, Some(RawGateOrder::Sent));
        assert!(build.runners().iter().all(|runner| runner.gate().is_none()));

        let uma_data = build.runners()[0].to_uma_data(DistanceType::Long, Track::Turf);
        let uma = uma_data.uma;
        assert_eq!(
            (uma.speed, uma.stamina, uma.power, uma.guts, uma.wit),
            (664, 544, 473, 451, 1189)
        );
        assert_eq!(uma.style, BaseStyle::Front);
        assert_eq!(
            (
                uma.distance_aptitude,
                uma.track_aptitude,
                uma.style_aptitude
            ),
            (Aptitude::E, Aptitude::A, Aptitude::A)
        );
        assert_eq!(uma.fan_count, Some(999999));
        assert_eq!(uma.character_id, Some(100201));
        assert_eq!(uma_data.mood, Some(Mood::Great));
        assert_eq!(uma_data.skills.len(), 21);
        assert_eq!(
            uma_data.skills[..2],
            [
                UmaSkill {
                    id: 100021,
                    level: 5
                },
                UmaSkill {
                    id: 200132,
                    level: 1
                },
            ]
        );
        // Only the first runner has a fan count.
        let uma = build.runners()[1].to_uma_data(DistanceType::Long, Track::Turf);
        assert_eq!(uma.uma.fan_count, None);

        let teams: Vec<Vec<Option<u32>>> = build
            .teams()
            .iter()
            .map(|team| team.iter().map(|runner| runner.character_id).collect())
            .collect();
        assert_eq!(
            teams,
            [
                [Some(100201), Some(103201), Some(106801)],
                [Some(100103), Some(100701), Some(101101)],
                [Some(100602), Some(102101), Some(106001)],
            ]
        );
    }

    #[test]
    fn test_from_field() {
        let build = UmaMoeBuild::from_json(&field_json("")).unwrap();
        let parameters = build.race_parameters();
        assert_eq!(parameters.course_id, "10101");
        assert_eq!(
            parameters.race,
            RaceParameters {
                track_condition: TrackCondition::Soft,
                weather: Weather::Snowy,
                season: Season::Sakura,
                time: Time::Night,
                grade: Grade::G1,
                gates: 2,
                career: false,
            }
        );
        let expected = UmaData {
            uma: Uma {
                speed: 1201,
                stamina: 802,
                power: 1003,
                guts: 604,
                wit: 1105,
                style: BaseStyle::Front,
                distance_aptitude: Aptitude::S,
                track_aptitude: Aptitude::A,
                style_aptitude: Aptitude::D,
                fan_count: Some(123456),
                character_id: Some(100101),
            },
            mood: Some(Mood::Bad),
            skills: vec![
                UmaSkill {
                    id: 100011,
                    level: 4,
                },
                UmaSkill {
                    id: 200012,
                    level: 1,
                },
            ],
        };
        let runner = &build.runners()[0];
        assert_eq!(runner.team(), Some(1));
        assert_eq!(
            runner.to_uma_data(DistanceType::Short, Track::Turf),
            expected
        );
        assert_eq!(build.teams().len(), 1);
    }

    #[test]
    fn test_to_uma_data_picks_aptitudes() {
        let build = UmaMoeBuild::from_json(&field_json("")).unwrap();
        let cases = [
            (DistanceType::Short, Track::Turf, Aptitude::S, Aptitude::A),
            (DistanceType::Mile, Track::Dirt, Aptitude::A, Aptitude::B),
            (DistanceType::Medium, Track::Turf, Aptitude::B, Aptitude::A),
            (DistanceType::Long, Track::Dirt, Aptitude::C, Aptitude::B),
        ];
        for (distance, track, distance_aptitude, track_aptitude) in cases {
            let uma = build.runners()[0].to_uma_data(distance, track).uma;
            assert_eq!(uma.distance_aptitude, distance_aptitude, "{distance:?}");
            assert_eq!(uma.track_aptitude, track_aptitude, "{track:?}");
        }
    }

    #[test]
    fn test_runaway_style_adds_the_runaway_skill_once() {
        // (skill replacing the second one of the first runner, if any, runner
        // style, resulting skills of the first runner).
        let cases = [
            (None, "Oonige", vec![(100011, 4), (200012, 1), (202051, 1)]),
            (Some((202051, 3)), "Oonige", vec![(100011, 4), (202051, 3)]),
            (None, "Nige", vec![(100011, 4), (200012, 1)]),
        ];
        for (replacement, style, expected) in cases {
            let mut json = field_json("").replacen(
                r#""runningStyle": "Nige""#,
                &format!(r#""runningStyle": "{style}""#),
                1,
            );
            if let Some((id, level)) = replacement {
                json = json.replacen(
                    r#""skillId": 200012, "level": 1"#,
                    &format!(r#""skillId": {id}, "level": {level}"#),
                    1,
                );
            }
            let build = UmaMoeBuild::from_json(&json).unwrap();
            let uma_data = build.runners()[0].to_uma_data(DistanceType::Mile, Track::Turf);
            let skills: Vec<_> = uma_data.skills.iter().map(|s| (s.id, s.level)).collect();
            assert_eq!(skills, expected, "{style} {replacement:?}");
        }
    }

    #[test]
    fn test_style_aptitude_follows_running_style() {
        let cases = [
            ("Oonige", BaseStyle::Front, Aptitude::D),
            ("Nige", BaseStyle::Front, Aptitude::D),
            ("Senko", BaseStyle::Pace, Aptitude::E),
            ("Sashi", BaseStyle::Late, Aptitude::F),
            ("Oikomi", BaseStyle::End, Aptitude::G),
        ];
        for (name, style, aptitude) in cases {
            let json = field_json("").replacen(
                r#""runningStyle": "Nige""#,
                &format!(r#""runningStyle": "{name}""#),
                1,
            );
            let uma = UmaMoeBuild::from_json(&json).unwrap().runners()[0]
                .to_uma_data(DistanceType::Mile, Track::Turf)
                .uma;
            assert_eq!((uma.style, uma.style_aptitude), (style, aptitude), "{name}");
        }
    }

    #[test]
    fn test_configure_gates() {
        // Different from both orders, so that changes show.
        let base = Configuration {
            gate_assignment: Assignment::All,
            default_gate: DefaultGate::InOrder,
            ..Configuration::default()
        };
        let cases = [
            ("", base),
            (
                r#""gateOrder": "randomized","#,
                Configuration {
                    default_gate: DefaultGate::Random,
                    ..base
                },
            ),
            (
                r#""gateOrder": "sent","#,
                Configuration {
                    gate_assignment: Assignment::Unset,
                    ..base
                },
            ),
        ];
        for (gate_order, expected) in cases {
            let build = UmaMoeBuild::from_json(&field_json(gate_order)).unwrap();
            let mut config = base;
            build.configure_gates(&mut config);
            assert_eq!(config, expected, "{gate_order}");
        }
    }

    #[test]
    fn test_entrants() {
        // The second runner in a team of her own, with gate 1.
        let json = field_json("").replacen(
            r#""cardId": 100201, "teamId": 1,"#,
            r#""cardId": 100201, "teamId": 2, "frameOrder": 1,"#,
            1,
        );
        let build = UmaMoeBuild::from_json(&json).unwrap();
        let skill = |id| {
            Arc::new(Skill {
                id,
                group: id,
                ..crate::test_utils::synthetic_skill(vec![])
            })
        };
        let all_skills = SkillData::from_iter([skill(100011), skill(200012)]);
        let course = crate::test_utils::load_course(10101);
        let (distance, track) = (course.distance_type(), course.surface);

        let teams = build.entrants(&course, &all_skills).unwrap();
        let sizes: Vec<_> = teams.iter().map(Vec::len).collect();
        assert_eq!(sizes, [1, 1]);
        let (first, second) = (&teams[0][0], &teams[1][0]);
        assert_eq!(
            first.uma,
            build.runners()[0].to_uma_data(distance, track).uma
        );
        assert_eq!((first.mood, first.gate), (Some(Mood::Bad), None));
        assert_eq!((second.mood, second.gate), (Some(Mood::Bad), Some(1)));
        // Skills keep the levels from the build code.
        let levels: Vec<_> = first
            .skills
            .iter()
            .map(|skill| (skill.id, skill.level))
            .collect();
        assert_eq!(levels, [(100011, 4), (200012, 1)]);
    }

    #[test]
    fn test_gates() {
        // Set for the first runner only.
        let with_gate = |gate: u8| {
            let json = field_json("").replacen(
                r#""teamId": 1,"#,
                &format!(r#""teamId": 1, "frameOrder": {gate},"#),
                1,
            );
            UmaMoeBuild::from_json(&json)
        };
        let build = with_gate(2).unwrap();
        let gates: Vec<_> = build.runners().iter().map(UmaMoeRunner::gate).collect();
        assert_eq!(gates, [Some(2), None]);

        // Only as many gates as runners.
        for gate in [0, 3] {
            let error = with_gate(gate).unwrap_err().to_string();
            assert!(
                error.contains(&format!("invalid gate {gate}: expected 1 to 2")),
                "{error}"
            );
        }
    }

    #[test]
    fn test_from_uma1_uma2() {
        let build = UmaMoeBuild::from_json(UMA1_UMA2_JSON).unwrap();
        assert_eq!(build.race_parameters().race.gates, 2);
        assert_eq!(build.gate_order, None);
        assert_eq!(build.teams().len(), 2);

        let runners = build.runners();
        assert_eq!(runners[0].team(), None);
        let uma_data = runners[0].to_uma_data(DistanceType::Long, Track::Dirt);
        let uma = uma_data.uma;
        assert_eq!(uma.character_id, Some(100201));
        assert_eq!(uma.fan_count, Some(654321));
        assert_eq!(uma.style, BaseStyle::Front);
        assert_eq!(
            (
                uma.distance_aptitude,
                uma.track_aptitude,
                uma.style_aptitude
            ),
            (Aptitude::E, Aptitude::G, Aptitude::S)
        );
        assert_eq!(uma_data.mood, Some(Mood::Bad));
        // Only the unique skill gets the unique level, not inherited ones.
        let levels: Vec<_> = uma_data.skills.iter().map(|s| (s.id, s.level)).collect();
        // Runaway is a Front runner with the Runaway skill.
        assert_eq!(levels, [(100021, 5), (200132, 1), (910071, 1), (202051, 1)]);

        let uma_data = runners[1].to_uma_data(DistanceType::Long, Track::Turf);
        assert_eq!(uma_data.uma.character_id, None);
        assert_eq!(uma_data.uma.fan_count, None);
        assert_eq!(uma_data.uma.style_aptitude, Aptitude::E);
        assert_eq!(uma_data.mood, Some(Mood::Great));
    }

    #[test]
    fn test_empty_field_falls_back_to_uma1_uma2() {
        let json = UMA1_UMA2_JSON.replace(r#""tab": "compare""#, r#""field": []"#);
        let build = UmaMoeBuild::from_json(&json).unwrap();
        assert_eq!(build.runners().len(), 2);
    }

    #[test]
    fn test_from_invalid_json() {
        let field = field_json("");
        let stat_zero = "invalid stat 0: expected at least 1";
        // (JSON, replaced text, replacement, reason it's rejected).
        let cases = [
            (
                &*field,
                r#""version": 1"#,
                r#""version": 2"#,
                "unsupported build code version 2: expected 1",
            ),
            (&field, r#""guts": "604""#, r#""guts": "0""#, stat_zero),
            (
                &field,
                r#""guts": "604""#,
                r#""guts": "x""#,
                r#"invalid stat "x": expected a number"#,
            ),
            (
                &field,
                r#""motivation": "2""#,
                r#""motivation": "0""#,
                r#"invalid motivation "0": expected 1 to 5"#,
            ),
            (
                &field,
                r#""motivation": "2""#,
                r#""motivation": "6""#,
                r#"invalid motivation "6": expected 1 to 5"#,
            ),
            (UMA1_UMA2_JSON, r#""guts": 451"#, r#""guts": 0"#, stat_zero),
            (
                UMA1_UMA2_JSON,
                r#""mood": 2"#,
                r#""mood": 3"#,
                "enum `Mood` matches the value `3`",
            ),
            (
                UMA1_UMA2_JSON,
                r#""200132""#,
                r#""skill""#,
                r#"invalid skill ID "skill": expected a number"#,
            ),
            (
                UMA1_UMA2_JSON,
                r#""outfitId": "100201""#,
                r#""outfitId": "x""#,
                r#"invalid outfit ID "x": expected a number"#,
            ),
        ];
        for (json, from, to, reason) in cases {
            assert!(json.contains(from), "{from}");
            let json = json.replacen(from, to, 1);
            let error = UmaMoeBuild::from_json(&json).unwrap_err().to_string();
            assert!(error.contains(reason), "{to}: {error}");
        }
    }

    /// Decoded JSON of the build code at [`BUILD_PATH`].
    fn build_json() -> serde_json::Value {
        let code = std::fs::read_to_string(BUILD_PATH).unwrap();
        let compressed = BASE64.decode(code.trim()).unwrap();
        serde_json::from_reader(GzDecoder::new(compressed.as_slice())).unwrap()
    }

    #[test]
    fn test_runner_count() {
        let no_runners = r#"{
            "version": 1, "courseId": 10506,
            "racedef": { "ground": 1, "weather": 1, "season": 1, "time": 1 }
        }"#;
        let error = UmaMoeBuild::from_json(no_runners).unwrap_err();
        let reason = format!("invalid number of runners 0: expected 1 to {MAX_GATES}");
        assert!(error.to_string().contains(&reason), "{error}");

        // Give every runner a team of its own, then fill every gate.
        let mut json = build_json();
        let field = json["page"]["field"].as_array_mut().unwrap();
        for (i, runner) in field.iter_mut().enumerate() {
            runner["teamId"] = i.into();
        }
        let mut extra = field[0].clone();
        extra["teamId"] = serde_json::Value::Null;
        field.resize(MAX_GATES as usize, extra.clone());
        let full = UmaMoeBuild::from_json(&json.to_string()).unwrap();
        assert_eq!(full.race_parameters().race.gates, MAX_GATES);
        assert_eq!(full.teams().len(), MAX_GATES as usize);

        json["page"]["field"].as_array_mut().unwrap().push(extra);
        let error = UmaMoeBuild::from_json(&json.to_string()).unwrap_err();
        let reason = format!("invalid number of runners {}", MAX_GATES + 1);
        assert!(error.to_string().contains(&reason), "{error}");
    }

    #[test]
    fn test_team_size() {
        // Moving the second runner of team 2 to team 1 makes it too large.
        let mut json = build_json();
        json["page"]["field"][4]["teamId"] = 1.into();
        let error = UmaMoeBuild::from_json(&json.to_string()).unwrap_err();
        let reason = format!("invalid team 1 of 4 runners: expected up to {MAX_TEAM_SIZE}");
        assert!(error.to_string().contains(&reason), "{error}");
    }
}
