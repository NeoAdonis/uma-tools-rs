//! # Condition Variables
//!
//! This module defines the variables used in skill conditions, such as
//! `hp_per` or `phase_random`, and classifies them.
//!
//! ## Categories
//!
//! Every variable is a [`VarType`], grouped by category: most categories are a
//! variant holding their own enum (e.g. [`VarType::Status`] holds a
//! [`StatusVar`]). Categories gather variables that are evaluated from the
//! same data (course, runner status, lineup...), or that are handled in the
//! same way, such as random variables, which are rolled instead of being read
//! from the race state.
//!
//! The meaning of each variable is described in the
//! [variable reference](crate::condition::variable_reference).
//!
//! ## Names
//!
//! Enum variants mirror the variable names in condition strings, keeping their
//! original spelling (e.g. `RunningStyleTemptationCountNige` for
//! `running_style_temptation_count_nige`). [`VarType::parse`] turns a name into
//! a variable, and [`Display`](fmt::Display) turns a variable back into its
//! name. Both convert the variant name to snake case, without splitting digits
//! (e.g. `order_rate_in20_continue`), so a new variant needs no separate
//! mapping.
//!
//! ## Classification
//!
//! Some properties of a variable are known without evaluating it:
//!
//! - [`VarType::is_static`]: whether its value never changes during a race, so
//!   that comparisons on it can be evaluated once before the race starts.
//! - [`VarType::is_popularity_guard`]: whether an unmet comparison on it
//!   prevents a skill from adding popularity points.
//!
//! See the [`condition`](super) module for an example.

use std::fmt::{self, Write as _};

/// Define a category enum, whose variants mirror the variable names in
/// condition strings.
///
/// Names are the variants' in snake case (see [`snake_case`]): `Display`
/// writes them, and `from_name` parses them.
macro_rules! category {
    (
        $(#[$attr:meta])*
        pub enum $name:ident {
            $($(#[$variant_attr:meta])* $variant:ident),* $(,)?
        }
    ) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$variant_attr])* $variant),*
        }

        impl $name {
            /// Variant named `name`, or `None` if there's none.
            fn from_name(name: &str) -> Option<Self> {
                [$(Self::$variant),*]
                    .into_iter()
                    .find(|variant| snake_case(variant.ident()).eq(name.chars()))
            }

            /// Name of the variant in Rust.
            fn ident(&self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant)),*
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                snake_case(self.ident()).try_for_each(|c| f.write_char(c))
            }
        }
    };
}

/// Convert a variant name to snake case. Digits aren't split (e.g.
/// `OrderRateIn20Continue` becomes `order_rate_in20_continue`).
fn snake_case(ident: &str) -> impl Iterator<Item = char> + '_ {
    ident.chars().enumerate().flat_map(|(i, c)| {
        let underscore = (i > 0 && c.is_ascii_uppercase()).then_some('_');
        underscore.into_iter().chain([c.to_ascii_lowercase()])
    })
}

/// Variable types used in conditions.
///
/// Variables are grouped by category. The meaning of each one is described in
/// the [variable reference](crate::condition::variable_reference).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VarType {
    /// Variable representing the elapsed (accumulated) time in a race.
    AccumulateTime,
    /// Variable representing base stats.
    BaseStat(BaseStatVar),
    /// Variables related to the runner's entry in the race (gate, popularity...).
    Entry(EntryVar),
    /// Variables related to order/rank.
    Order(OrderVar),
    /// Variables related to order rates (order relative to the runner count,
    /// e.g. "you are in the top X%").
    OrderRate(OrderRateVar),
    /// Variables related to distance between runners.
    Distance(DistanceVar),
    /// Variables related to phases.
    Phase(PhaseVar),
    /// Variables related to random phase aspects.
    RandomPhase(RandomPhaseVar),
    /// Variables related to course properties.
    Course(CourseVar),
    /// Variables related to position (distance or course features).
    Spatial(SpatialVar),
    /// Variables related to random positional aspects.
    RandomSpatial(RandomSpatialVar),
    /// Variables related to skill activation counts and status.
    Activation(ActivationVar),
    /// Variables related to skill activation tracking of sets of skill IDs or types
    /// (see [`Value::Set`](crate::condition::evaluator::Value)).
    ActivationQuery(ActivationQueryVar),
    /// Variables related to race parameters.
    RaceParam(RaceParamVar),
    /// Variables related to the lineup (every runner entered in the race).
    Lineup(LineupVar),
    /// Variables related to running styles.
    RunningStyle(RunningStyleVar),
    /// Variables related to a runner's status.
    Status(StatusVar),
    /// Variables whose value is determined through a random lottery.
    Random(RandomVar),
    /// Variables related to blocking or lane position.
    Blocking(BlockingVar),
    /// Variables related to rush (temptation) mechanics.
    Temptation(TemptationVar),
    /// Special value that is always true.
    Always,
}

category! {
    pub enum BaseStatVar {
        BaseSpeed,
        BaseStamina,
        BasePower,
        BaseGuts,
        BaseWiz,
    }
}

category! {
    pub enum EntryVar {
        FanCount,
        Motivation,
        Popularity,
        PostNumber,
    }
}

category! {
    pub enum OrderVar {
        ChangeOrderOnetime,
        ChangeOrderUpEndAfter,
        ChangeOrderUpFinalcornerAfter,
        ChangeOrderUpMiddle,
        Order,
    }
}

category! {
    pub enum OrderRateVar {
        OrderRate,
        OrderRateIn20Continue,
        OrderRateIn40Continue,
        OrderRateIn50Continue,
        OrderRateIn80Continue,
        OrderRateOut20Continue,
        OrderRateOut40Continue,
        OrderRateOut50Continue,
        OrderRateOut70Continue,
    }
}

category! {
    pub enum DistanceVar {
        BashinDiffBehind,
        BashinDiffInfront,
        DistanceDiffRate,
        DistanceDiffTop,
        DistanceDiffTopFloat,
    }
}

category! {
    pub enum PhaseVar {
        Phase,
        PhaseFirsthalf,
        PhaseFirstquarter,
        PhaseLaterhalf,
    }
}

category! {
    pub enum CourseVar {
        CornerCount,
        CourseDistance,
        DistanceType,
        GroundType,
        IsAbroad,
        IsBasisDistance,
        IsDirtgrade,
        IsTightTrack,
        Rotation,
        TrackId,
    }
}

category! {
    pub enum SpatialVar {
        Corner,
        DistanceRate,
        Furlong,
        IsFinalcorner,
        IsFinalcornerLaterhalf,
        IsLastStraight,
        IsLastStraightOnetime,
        LaneType,
        RemainDistance,
        RemainDistanceViewerId,
        Slope,
        StraightFrontType,
    }
}

category! {
    pub enum RandomSpatialVar {
        AllCornerRandom,
        CornerRandom,
        DistanceRateAfterRandom,
        DownSlopeRandom,
        IsFinalcornerRandom,
        LastStraightRandom,
        RunAtFullSpeedRandom,
        StraightRandom,
        UpSlopeRandom,
        UpSlopeRandomLaterHalf,
    }
}

category! {
    pub enum RandomPhaseVar {
        PhaseCornerRandom,
        PhaseFirsthalfRandom,
        PhaseFirstHalfStraightRandom,
        PhaseFirstquarterRandom,
        PhaseLaterhalfRandom,
        PhaseLatterHalfStraightRandom,
        PhaseRandom,
        PhaseStraightRandom,
    }
}

category! {
    pub enum ActivationVar {
        ActivateCountAll,
        ActivateCountAllTeam,
        ActivateCountEndAfter,
        ActivateCountHeal,
        ActivateCountLaterHalf,
        ActivateCountMiddle,
        ActivateCountParamSkill,
        ActivateCountStart,
        CompeteFightCount,
        IsActivateAnySkill,
        IsActivateHealSkill,
        IsActivateOtherSkillDetail,
    }
}

category! {
    pub enum ActivationQueryVar {
        IsOtherCharacterActivateAdvantageSkill,
        IsPopularityTopCharacterActivateAdvantageSkill,
        IsUsedSkillId,
        IsUsedSkillIdWithDetailOne,
    }
}

category! {
    pub enum RaceParamVar {
        Grade,
        GroundCondition,
        Season,
        Time,
        Weather,
    }
}

category! {
    pub enum LineupVar {
        IsExistCharaId,
        IsExistSkillId,
        SameSkillHorseCount,
    }
}

category! {
    pub enum RunningStyleVar {
        RunningStyle,
        RunningStyleCountNigeOtherself,
        RunningStyleCountOikomiOtherself,
        RunningStyleCountSame,
        RunningStyleCountSameRate,
        RunningStyleCountSashiOtherself,
        RunningStyleCountSenkoOtherself,
        RunningStyleEqualPopularityOne,
    }
}

category! {
    pub enum StatusVar {
        HpPer,
        IsBadstart,
        IsGoodstart,
        IsHpEmptyOnetime,
        IsLastspurt,
        IsMoveLane,
        IsOvertake,
        IsTemptation,
        Lastspurt,
        SuccessionSkillCount,
        TemptationCount,
    }
}

category! {
    pub enum RandomVar {
        RandomLot,
        RandomLotShared,
    }
}

category! {
    pub enum BlockingVar {
        BehindNearLaneTime,
        BehindNearLaneTimeSet1,
        BlockedAllContinuetime,
        BlockedFront,
        BlockedFrontContinuetime,
        BlockedSideContinuetime,
        InfrontNearLaneTime,
        IsBehindIn,
        IsSurrounded,
        NearCount,
        NearInfrontCount,
        OvertakeTargetNoOrderUpTime,
        OvertakeTargetTime,
        Visiblehorse,
    }
}

category! {
    pub enum TemptationVar {
        RunningStyleTemptationCountNige,
        RunningStyleTemptationCountOikomi,
        RunningStyleTemptationCountSashi,
        RunningStyleTemptationCountSenko,
        RunningStyleTemptationOpponentCountNige,
        RunningStyleTemptationOpponentCountOikomi,
        RunningStyleTemptationOpponentCountSashi,
        RunningStyleTemptationOpponentCountSenko,
        TemptationCountBehind,
        TemptationCountInfront,
        TemptationOpponentCountBehind,
        TemptationOpponentCountInfront,
    }
}

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use VarType::*;
        match self {
            BaseStat(s) => write!(f, "{}", s),
            Entry(e) => write!(f, "{}", e),
            Order(o) => write!(f, "{}", o),
            OrderRate(o) => write!(f, "{}", o),
            Distance(d) => write!(f, "{}", d),
            Phase(p) => write!(f, "{}", p),
            Spatial(l) => write!(f, "{}", l),
            Course(c) => write!(f, "{}", c),
            Activation(a) => write!(f, "{}", a),
            ActivationQuery(a) => write!(f, "{}", a),
            RaceParam(r) => write!(f, "{}", r),
            Lineup(l) => write!(f, "{}", l),
            RunningStyle(r) => write!(f, "{}", r),
            Status(s) => write!(f, "{}", s),
            Random(r) => write!(f, "{}", r),
            Blocking(b) => write!(f, "{}", b),
            Temptation(t) => write!(f, "{}", t),
            AccumulateTime => write!(f, "accumulatetime"),
            Always => write!(f, "always"),
            RandomSpatial(rs) => write!(f, "{}", rs),
            RandomPhase(rp) => write!(f, "{}", rp),
        }
    }
}

impl VarType {
    /// Parse a variable name string into a [`VarType`], or `None` if the
    /// variable is unknown.
    pub fn parse(s: &str) -> Option<Self> {
        use VarType::*;

        let var = match s {
            "accumulatetime" => AccumulateTime,
            "always" => Always,
            _ => {
                return BaseStatVar::from_name(s)
                    .map(BaseStat)
                    .or_else(|| EntryVar::from_name(s).map(Entry))
                    .or_else(|| OrderVar::from_name(s).map(Order))
                    .or_else(|| OrderRateVar::from_name(s).map(OrderRate))
                    .or_else(|| DistanceVar::from_name(s).map(Distance))
                    .or_else(|| PhaseVar::from_name(s).map(Phase))
                    .or_else(|| RandomPhaseVar::from_name(s).map(RandomPhase))
                    .or_else(|| CourseVar::from_name(s).map(Course))
                    .or_else(|| SpatialVar::from_name(s).map(Spatial))
                    .or_else(|| RandomSpatialVar::from_name(s).map(RandomSpatial))
                    .or_else(|| ActivationVar::from_name(s).map(Activation))
                    .or_else(|| ActivationQueryVar::from_name(s).map(ActivationQuery))
                    .or_else(|| RaceParamVar::from_name(s).map(RaceParam))
                    .or_else(|| LineupVar::from_name(s).map(Lineup))
                    .or_else(|| RunningStyleVar::from_name(s).map(RunningStyle))
                    .or_else(|| StatusVar::from_name(s).map(Status))
                    .or_else(|| RandomVar::from_name(s).map(Random))
                    .or_else(|| BlockingVar::from_name(s).map(Blocking))
                    .or_else(|| TemptationVar::from_name(s).map(Temptation));
            }
        };
        Some(var)
    }
}

impl VarType {
    /// Whether the value never changes during a race.
    ///
    /// Determines if a comparison can be evaluated once before the race starts
    /// (e.g. `ground_type`, `running_style`, `motivation`).
    pub fn is_static(&self) -> bool {
        use VarType::*;

        match self {
            RaceParam(_) | Lineup(_) | Course(_) | BaseStat(_) | Entry(_) => true,
            Status(
                StatusVar::IsBadstart | StatusVar::IsGoodstart | StatusVar::SuccessionSkillCount,
            ) => true,
            // Conditions are based off the base style, which is a static variable.
            RunningStyle(_) => true,
            // Special value - By definition, it never changes.
            Always => true,
            _ => false,
        }
    }

    /// Whether an unmet condition on this variable prevents a skill from adding
    /// popularity points.
    ///
    /// These are the variables known before the race about the race itself and
    /// the runner's strategy and gate. Conditions on any other variable are
    /// assumed to be satisfiable.
    pub fn is_popularity_guard(&self) -> bool {
        use VarType::*;

        matches!(
            self,
            Course(
                CourseVar::DistanceType
                    | CourseVar::GroundType
                    | CourseVar::Rotation
                    | CourseVar::TrackId
                    | CourseVar::IsBasisDistance
            ) | RaceParam(
                RaceParamVar::GroundCondition
                    | RaceParamVar::Season
                    | RaceParamVar::Weather
                    | RaceParamVar::Time
                    | RaceParamVar::Grade
            ) | RunningStyle(RunningStyleVar::RunningStyle)
                | Entry(EntryVar::PostNumber)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_static() {
        let static_vars = [
            "ground_condition",
            "is_exist_skill_id",
            "same_skill_horse_count",
            "course_distance",
            "base_speed",
            "motivation",
            "popularity",
            "is_badstart",
            "is_goodstart",
            "succession_skill_count",
            "running_style",
            "running_style_count_same",
            "always",
        ];
        let dynamic_vars = [
            "accumulatetime",
            "hp_per",
            "phase",
            "corner",
            "order",
            "distance_diff_top",
            "is_used_skill_id",
            "activate_count_all",
            "is_lastspurt",
            "blocked_front",
            "temptation_count_infront",
            "random_lot",
            "phase_random",
            "corner_random",
        ];
        for name in static_vars {
            assert!(VarType::parse(name).unwrap().is_static(), "{name}");
        }
        for name in dynamic_vars {
            assert!(!VarType::parse(name).unwrap().is_static(), "{name}");
        }
    }
}
