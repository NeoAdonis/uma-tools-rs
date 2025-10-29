use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    BaseStat(StatKind),            // Base stats
    Position(PositionVar),         // Position/Order related
    Distance(DistanceVar),         // Distance related
    Phase(PhaseVar),               // Phase/Time related
    Course(CourseVar),             // Course features
    Activation(ActivationVar),     // Skill activation tracking
    RaceParam(RaceParamVar),       // Race parameters
    RunningStyle(RunningStyleVar), // Running style related
    Status(StatusVar),             // Status/State flags
    Trigger(TriggerVar),           // Random/Trigger variables
    Blocking(BlockingVar),         // Blocking/Lane related
    Temptation(TemptationVar),     // Rush/Temptation related
    Always,                        // Special
    Unknown(String),               // Fallback for unknown variables
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatKind {
    Speed,
    Stamina,
    Power,
    Guts,
    Wiz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionVar {
    ChangeOrderOnetime,
    ChangeOrderUpEndAfter,
    ChangeOrderUpFinalcornerAfter,
    ChangeOrderUpMiddle,
    NearCount,
    NearInfrontCount,
    Order,
    OrderRate,
    OrderRateIn20Continue,
    OrderRateIn40Continue,
    OrderRateIn50Continue,
    OrderRateIn80Continue,
    OrderRateOut20Continue,
    OrderRateOut40Continue,
    OrderRateOut50Continue,
    OrderRateOut70Continue,
    OvertakeTargetNoOrderUpTime,
    OvertakeTargetTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceVar {
    BashinDiffBehind,
    BashinDiffInfront,
    CourseDistance,
    DistanceDiffRate,
    DistanceDiffTop,
    DistanceDiffTopFloat,
    DistanceRate,
    DistanceRateAfterRandom,
    Furlong,
    RemainDistance,
    RemainDistanceViewerId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseVar {
    AccumulateTime,
    IsFinalcorner,
    IsFinalcornerLaterhalf,
    IsFinalcornerRandom,
    Phase,
    PhaseCornerRandom,
    PhaseFirsthalf,
    PhaseFirsthalfRandom,
    PhaseFirstHalfStraightRandom,
    PhaseFirstquarter,
    PhaseFirstquarterRandom,
    PhaseLaterhalf,
    PhaseLaterhalfRandom,
    PhaseLatterHalfStraightRandom,
    PhaseRandom,
    PhaseStraightRandom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourseVar {
    AllCornerRandom,
    Corner,
    CornerCount,
    CornerRandom,
    DistanceType,
    DownSlopeRandom,
    GroundCondition,
    GroundType,
    IsBasisDistance,
    IsLastStraight,
    IsLastStraightOnetime,
    LaneType,
    LastStraightRandom,
    Rotation,
    Slope,
    StraightFrontType,
    StraightRandom,
    TrackId,
    UpSlopeRandom,
    UpSlopeRandomLaterHalf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationVar {
    ActivateCountAll,
    ActivateCountAllTeam,
    ActivateCountEndAfter,
    ActivateCountHeal,
    ActivateCountLaterHalf,
    ActivateCountMiddle,
    ActivateCountStart,
    CompeteFightCount,
    IsActivateAnySkill,
    IsActivateHealSkill,
    IsActivateOtherSkillDetail,
    IsExistCharaId,
    IsExistSkillId,
    IsOtherCharacterActivateAdvantageSkill,
    IsUsedSkillId,
    SameSkillHorseCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceParamVar {
    Grade,
    IsDirtgrade,
    Motivation,
    Popularity,
    PostNumber,
    Season,
    Time,
    Visiblehorse,
    Weather,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunningStyleVar {
    RunningStyle,
    RunningStyleCountNigeOtherself,
    RunningStyleCountOikomiOtherself,
    RunningStyleCountSame,
    RunningStyleCountSameRate,
    RunningStyleCountSashiOtherself,
    RunningStyleCountSenkoOtherself,
    RunningStyleEqualPopularityOne,
    RunningStyleTemptationCountNige,
    RunningStyleTemptationCountOikomi,
    RunningStyleTemptationCountSashi,
    RunningStyleTemptationCountSenko,
    RunningStyleTemptationOpponentCountNige,
    RunningStyleTemptationOpponentCountOikomi,
    RunningStyleTemptationOpponentCountSashi,
    RunningStyleTemptationOpponentCountSenko,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusVar {
    BehindNearLaneTime,
    BehindNearLaneTimeSet1,
    HpPer,
    InfrontNearLaneTime,
    IsBadstart,
    IsBehindIn,
    IsHpEmptyOnetime,
    IsLastspurt,
    IsMoveLane,
    IsOvertake,
    IsSurrounded,
    IsTemptation,
    Lastspurt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerVar {
    RandomLot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockingVar {
    BlockedAllContinuetime,
    BlockedFront,
    BlockedFrontContinuetime,
    BlockedSideContinuetime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemptationVar {
    TemptationCount,
    TemptationCountBehind,
    TemptationCountInfront,
    TemptationOpponentCountBehind,
    TemptationOpponentCountInfront,
}

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VarType::BaseStat(s) => write!(f, "base_{}", s),
            VarType::Position(p) => write!(f, "{}", p),
            VarType::Distance(d) => write!(f, "{}", d),
            VarType::Phase(p) => write!(f, "{}", p),
            VarType::Course(c) => write!(f, "{}", c),
            VarType::Activation(a) => write!(f, "{}", a),
            VarType::RaceParam(r) => write!(f, "{}", r),
            VarType::RunningStyle(r) => write!(f, "{}", r),
            VarType::Status(s) => write!(f, "{}", s),
            VarType::Trigger(t) => write!(f, "{}", t),
            VarType::Blocking(b) => write!(f, "{}", b),
            VarType::Temptation(t) => write!(f, "{}", t),
            VarType::Always => write!(f, "always"),
            VarType::Unknown(s) => write!(f, "<unknown:{}>", s),
        }
    }
}

impl fmt::Display for StatKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StatKind::Speed => write!(f, "speed"),
            StatKind::Stamina => write!(f, "stamina"),
            StatKind::Power => write!(f, "power"),
            StatKind::Guts => write!(f, "guts"),
            StatKind::Wiz => write!(f, "wiz"),
        }
    }
}

// Implement Display for other enums (using snake_case of variant name)
macro_rules! impl_display_snake_case {
    ($type:ty) => {
        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let s = format!("{:?}", self);
                let mut result = String::new();
                for (i, c) in s.chars().enumerate() {
                    if c.is_uppercase() && i > 0 {
                        result.push('_');
                    }
                    result.push(c.to_ascii_lowercase());
                }
                write!(f, "{}", result)
            }
        }
    };
}

impl_display_snake_case!(PositionVar);
impl_display_snake_case!(DistanceVar);
impl_display_snake_case!(PhaseVar);
impl_display_snake_case!(CourseVar);
impl_display_snake_case!(ActivationVar);
impl_display_snake_case!(RaceParamVar);
impl_display_snake_case!(RunningStyleVar);
impl_display_snake_case!(StatusVar);
impl_display_snake_case!(TriggerVar);
impl_display_snake_case!(BlockingVar);
impl_display_snake_case!(TemptationVar);

impl VarType {
    pub fn parse(s: &str) -> Self {
        match s {
            // Base stats
            "base_speed" => VarType::BaseStat(StatKind::Speed),
            "base_stamina" => VarType::BaseStat(StatKind::Stamina),
            "base_power" => VarType::BaseStat(StatKind::Power),
            "base_guts" => VarType::BaseStat(StatKind::Guts),
            "base_wiz" => VarType::BaseStat(StatKind::Wiz),

            // Position
            "change_order_onetime" => VarType::Position(PositionVar::ChangeOrderOnetime),
            "change_order_up_end_after" => VarType::Position(PositionVar::ChangeOrderUpEndAfter),
            "change_order_up_finalcorner_after" => {
                VarType::Position(PositionVar::ChangeOrderUpFinalcornerAfter)
            }
            "change_order_up_middle" => VarType::Position(PositionVar::ChangeOrderUpMiddle),
            "near_count" => VarType::Position(PositionVar::NearCount),
            "near_infront_count" => VarType::Position(PositionVar::NearInfrontCount),
            "order" => VarType::Position(PositionVar::Order),
            "order_rate" => VarType::Position(PositionVar::OrderRate),
            "order_rate_in20_continue" => VarType::Position(PositionVar::OrderRateIn20Continue),
            "order_rate_in40_continue" => VarType::Position(PositionVar::OrderRateIn40Continue),
            "order_rate_in50_continue" => VarType::Position(PositionVar::OrderRateIn50Continue),
            "order_rate_in80_continue" => VarType::Position(PositionVar::OrderRateIn80Continue),
            "order_rate_out20_continue" => VarType::Position(PositionVar::OrderRateOut20Continue),
            "order_rate_out40_continue" => VarType::Position(PositionVar::OrderRateOut40Continue),
            "order_rate_out50_continue" => VarType::Position(PositionVar::OrderRateOut50Continue),
            "order_rate_out70_continue" => VarType::Position(PositionVar::OrderRateOut70Continue),
            "overtake_target_no_order_up_time" => {
                VarType::Position(PositionVar::OvertakeTargetNoOrderUpTime)
            }
            "overtake_target_time" => VarType::Position(PositionVar::OvertakeTargetTime),

            // Distance
            "bashin_diff_behind" => VarType::Distance(DistanceVar::BashinDiffBehind),
            "bashin_diff_infront" => VarType::Distance(DistanceVar::BashinDiffInfront),
            "course_distance" => VarType::Distance(DistanceVar::CourseDistance),
            "distance_diff_rate" => VarType::Distance(DistanceVar::DistanceDiffRate),
            "distance_diff_top_float" => VarType::Distance(DistanceVar::DistanceDiffTopFloat),
            "distance_diff_top" => VarType::Distance(DistanceVar::DistanceDiffTop),
            "distance_rate" => VarType::Distance(DistanceVar::DistanceRate),
            "distance_rate_after_random" => VarType::Distance(DistanceVar::DistanceRateAfterRandom),
            "furlong" => VarType::Distance(DistanceVar::Furlong),
            "remain_distance" => VarType::Distance(DistanceVar::RemainDistance),
            "remain_distance_viewer_id" => VarType::Distance(DistanceVar::RemainDistanceViewerId),

            // Phase/Time
            "accumulatetime" => VarType::Phase(PhaseVar::AccumulateTime),
            "is_finalcorner" => VarType::Phase(PhaseVar::IsFinalcorner),
            "is_finalcorner_laterhalf" => VarType::Phase(PhaseVar::IsFinalcornerLaterhalf),
            "is_finalcorner_random" => VarType::Phase(PhaseVar::IsFinalcornerRandom),
            "phase" => VarType::Phase(PhaseVar::Phase),
            "phase_corner_random" => VarType::Phase(PhaseVar::PhaseCornerRandom),
            "phase_first_half_straight_random" => {
                VarType::Phase(PhaseVar::PhaseFirstHalfStraightRandom)
            }
            "phase_firsthalf" => VarType::Phase(PhaseVar::PhaseFirsthalf),
            "phase_firsthalf_random" => VarType::Phase(PhaseVar::PhaseFirsthalfRandom),
            "phase_firstquarter" => VarType::Phase(PhaseVar::PhaseFirstquarter),
            "phase_firstquarter_random" => VarType::Phase(PhaseVar::PhaseFirstquarterRandom),
            "phase_laterhalf" => VarType::Phase(PhaseVar::PhaseLaterhalf),
            "phase_laterhalf_random" => VarType::Phase(PhaseVar::PhaseLaterhalfRandom),
            "phase_latter_half_straight_random" => {
                VarType::Phase(PhaseVar::PhaseLatterHalfStraightRandom)
            }
            "phase_random" => VarType::Phase(PhaseVar::PhaseRandom),
            "phase_straight_random" => VarType::Phase(PhaseVar::PhaseStraightRandom),

            // Course
            "all_corner_random" => VarType::Course(CourseVar::AllCornerRandom),
            "corner" => VarType::Course(CourseVar::Corner),
            "corner_count" => VarType::Course(CourseVar::CornerCount),
            "corner_random" => VarType::Course(CourseVar::CornerRandom),
            "distance_type" => VarType::Course(CourseVar::DistanceType),
            "down_slope_random" => VarType::Course(CourseVar::DownSlopeRandom),
            "ground_condition" => VarType::Course(CourseVar::GroundCondition),
            "ground_type" => VarType::Course(CourseVar::GroundType),
            "is_basis_distance" => VarType::Course(CourseVar::IsBasisDistance),
            "is_last_straight" => VarType::Course(CourseVar::IsLastStraight),
            "is_last_straight_onetime" => VarType::Course(CourseVar::IsLastStraightOnetime),
            "lane_type" => VarType::Course(CourseVar::LaneType),
            "last_straight_random" => VarType::Course(CourseVar::LastStraightRandom),
            "rotation" => VarType::Course(CourseVar::Rotation),
            "slope" => VarType::Course(CourseVar::Slope),
            "straight_front_type" => VarType::Course(CourseVar::StraightFrontType),
            "straight_random" => VarType::Course(CourseVar::StraightRandom),
            "track_id" => VarType::Course(CourseVar::TrackId),
            "up_slope_random" => VarType::Course(CourseVar::UpSlopeRandom),
            "up_slope_random_later_half" => VarType::Course(CourseVar::UpSlopeRandomLaterHalf),

            // Activation tracking
            "activate_count_all_team" => VarType::Activation(ActivationVar::ActivateCountAllTeam),
            "activate_count_all" => VarType::Activation(ActivationVar::ActivateCountAll),
            "activate_count_end_after" => VarType::Activation(ActivationVar::ActivateCountEndAfter),
            "activate_count_heal" => VarType::Activation(ActivationVar::ActivateCountHeal),
            "activate_count_later_half" => {
                VarType::Activation(ActivationVar::ActivateCountLaterHalf)
            }
            "activate_count_middle" => VarType::Activation(ActivationVar::ActivateCountMiddle),
            "activate_count_start" => VarType::Activation(ActivationVar::ActivateCountStart),
            "compete_fight_count" => VarType::Activation(ActivationVar::CompeteFightCount),
            "is_activate_any_skill" => VarType::Activation(ActivationVar::IsActivateAnySkill),
            "is_activate_heal_skill" => VarType::Activation(ActivationVar::IsActivateHealSkill),
            "is_activate_other_skill_detail" => {
                VarType::Activation(ActivationVar::IsActivateOtherSkillDetail)
            }
            "is_exist_chara_id" => VarType::Activation(ActivationVar::IsExistCharaId),
            "is_exist_skill_id" => VarType::Activation(ActivationVar::IsExistSkillId),
            "is_other_character_activate_advantage_skill" => {
                VarType::Activation(ActivationVar::IsOtherCharacterActivateAdvantageSkill)
            }
            "is_used_skill_id" => VarType::Activation(ActivationVar::IsUsedSkillId),
            "same_skill_horse_count" => VarType::Activation(ActivationVar::SameSkillHorseCount),

            // Race parameters
            "grade" => VarType::RaceParam(RaceParamVar::Grade),
            "is_dirtgrade" => VarType::RaceParam(RaceParamVar::IsDirtgrade),
            "motivation" => VarType::RaceParam(RaceParamVar::Motivation),
            "popularity" => VarType::RaceParam(RaceParamVar::Popularity),
            "post_number" => VarType::RaceParam(RaceParamVar::PostNumber),
            "season" => VarType::RaceParam(RaceParamVar::Season),
            "time" => VarType::RaceParam(RaceParamVar::Time),
            "weather" => VarType::RaceParam(RaceParamVar::Weather),
            "visiblehorse" => VarType::RaceParam(RaceParamVar::Visiblehorse),

            // Running style
            "running_style" => VarType::RunningStyle(RunningStyleVar::RunningStyle),
            "running_style_count_nige_otherself" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleCountNigeOtherself)
            }
            "running_style_count_oikomi_otherself" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleCountOikomiOtherself)
            }
            "running_style_count_sashi_otherself" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleCountSashiOtherself)
            }
            "running_style_count_senko_otherself" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleCountSenkoOtherself)
            }
            "running_style_count_same" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleCountSame)
            }
            "running_style_count_same_rate" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleCountSameRate)
            }
            "running_style_equal_popularity_one" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleEqualPopularityOne)
            }
            "running_style_temptation_count_nige" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleTemptationCountNige)
            }
            "running_style_temptation_count_oikomi" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleTemptationCountOikomi)
            }
            "running_style_temptation_count_sashi" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleTemptationCountSashi)
            }
            "running_style_temptation_count_senko" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleTemptationCountSenko)
            }
            "running_style_temptation_opponent_count_nige" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleTemptationOpponentCountNige)
            }
            "running_style_temptation_opponent_count_oikomi" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleTemptationOpponentCountOikomi)
            }
            "running_style_temptation_opponent_count_sashi" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleTemptationOpponentCountSashi)
            }
            "running_style_temptation_opponent_count_senko" => {
                VarType::RunningStyle(RunningStyleVar::RunningStyleTemptationOpponentCountSenko)
            }

            // Status
            "behind_near_lane_time" => VarType::Status(StatusVar::BehindNearLaneTime),
            "behind_near_lane_time_set1" => VarType::Status(StatusVar::BehindNearLaneTimeSet1),
            "hp_per" => VarType::Status(StatusVar::HpPer),
            "infront_near_lane_time" => VarType::Status(StatusVar::InfrontNearLaneTime),
            "is_badstart" => VarType::Status(StatusVar::IsBadstart),
            "is_behind_in" => VarType::Status(StatusVar::IsBehindIn),
            "is_hp_empty_onetime" => VarType::Status(StatusVar::IsHpEmptyOnetime),
            "is_lastspurt" => VarType::Status(StatusVar::IsLastspurt),
            "is_move_lane" => VarType::Status(StatusVar::IsMoveLane),
            "is_overtake" => VarType::Status(StatusVar::IsOvertake),
            "is_surrounded" => VarType::Status(StatusVar::IsSurrounded),
            "is_temptation" => VarType::Status(StatusVar::IsTemptation),
            "lastspurt" => VarType::Status(StatusVar::Lastspurt),

            // Triggers
            "random_lot" => VarType::Trigger(TriggerVar::RandomLot),

            // Blocking
            "blocked_all_continuetime" => VarType::Blocking(BlockingVar::BlockedAllContinuetime),
            "blocked_front" => VarType::Blocking(BlockingVar::BlockedFront),
            "blocked_front_continuetime" => {
                VarType::Blocking(BlockingVar::BlockedFrontContinuetime)
            }
            "blocked_side_continuetime" => VarType::Blocking(BlockingVar::BlockedSideContinuetime),

            // Rush/Temptation
            "temptation_count" => VarType::Temptation(TemptationVar::TemptationCount),
            "temptation_count_behind" => VarType::Temptation(TemptationVar::TemptationCountBehind),
            "temptation_count_infront" => {
                VarType::Temptation(TemptationVar::TemptationCountInfront)
            }
            "temptation_opponent_count_behind" => {
                VarType::Temptation(TemptationVar::TemptationOpponentCountBehind)
            }
            "temptation_opponent_count_infront" => {
                VarType::Temptation(TemptationVar::TemptationOpponentCountInfront)
            }

            // Special
            "always" => VarType::Always,

            // Unknown variable - fallback
            _ => {
                #[cfg(debug_assertions)]
                eprintln!("warning: unknown variable '{}', evaluating to false", s);
                VarType::Unknown(s.to_string())
            }
        }
    }

    /// Returns the trigger classification for this variable
    /// This determines how the variable should be evaluated in the event-driven trigger system
    pub fn trigger_class(&self) -> TriggerClass {
        use VarType::*;
        use TriggerClass::*;

        match self {
            // STATIC - Evaluated once at race start
            BaseStat(_) => Static,
            RaceParam(_) => Static,
            Distance(DistanceVar::CourseDistance) => Static,
            Course(CourseVar::DistanceType) => Static,
            Course(CourseVar::GroundType) => Static,
            Course(CourseVar::GroundCondition) => Static,
            Course(CourseVar::TrackId) => Static,
            Course(CourseVar::Rotation) => Static,
            Course(CourseVar::IsBasisDistance) => Static,
            Course(CourseVar::CornerCount) => Static, // Under investigation: number of corners on course
            RunningStyle(RunningStyleVar::RunningStyle) => Static,
            Always => Static,

            // MONOTONIC INCREASING - Priority queue based
            Phase(PhaseVar::AccumulateTime) => MonotonicIncreasing,
            Distance(DistanceVar::DistanceRate) => MonotonicIncreasing,
            Distance(DistanceVar::Furlong) => MonotonicIncreasing, // Under investigation: distance in furlongs
            Activation(ActivationVar::ActivateCountAll) => MonotonicIncreasing,
            Activation(ActivationVar::ActivateCountStart) => MonotonicIncreasing,
            Activation(ActivationVar::ActivateCountMiddle) => MonotonicIncreasing,
            Activation(ActivationVar::ActivateCountLaterHalf) => MonotonicIncreasing,
            Activation(ActivationVar::ActivateCountEndAfter) => MonotonicIncreasing,
            Activation(ActivationVar::ActivateCountHeal) => MonotonicIncreasing,
            Activation(ActivationVar::ActivateCountAllTeam) => MonotonicIncreasing, // Multi-uma

            // MONOTONIC DECREASING - Priority queue based
            Distance(DistanceVar::RemainDistance) => MonotonicDecreasing,
            Distance(DistanceVar::RemainDistanceViewerId) => MonotonicDecreasing, // Multi-uma
            Status(StatusVar::HpPer) => MonotonicDecreasing, // Can increase via recovery

            // PHASE TRANSITIONS - Priority queue based
            Phase(PhaseVar::Phase) => PhaseTransition,
            Phase(PhaseVar::PhaseFirstquarter) => PhaseTransition, // Early phase
            Phase(PhaseVar::PhaseFirsthalf) => PhaseTransition,    // Early + Mid
            Phase(PhaseVar::PhaseLaterhalf) => PhaseTransition,    // Late + Last
            Phase(PhaseVar::IsFinalcorner) => PhaseTransition,     // Under investigation
            Phase(PhaseVar::IsFinalcornerLaterhalf) => PhaseTransition,

            // SPATIAL TRIGGERS - Course geometry based
            Course(CourseVar::Corner) => SpatialCorner,
            Course(CourseVar::Slope) => SpatialSlope,
            Course(CourseVar::IsLastStraight) => SpatialStraight,
            Course(CourseVar::StraightFrontType) => SpatialStraight,

            // ONE-TIME TRIGGERS
            Course(CourseVar::IsLastStraightOnetime) => Onetime,
            Position(PositionVar::ChangeOrderOnetime) => Onetime,
            Position(PositionVar::ChangeOrderUpEndAfter) => Onetime,    // Under investigation
            Position(PositionVar::ChangeOrderUpFinalcornerAfter) => Onetime,
            Position(PositionVar::ChangeOrderUpMiddle) => Onetime,
            Status(StatusVar::IsHpEmptyOnetime) => Onetime,

            // RANDOM TRIGGERS - Pre-rolled at race start
            Trigger(TriggerVar::RandomLot) => Random,
            Phase(PhaseVar::PhaseRandom) => Random,
            Phase(PhaseVar::PhaseFirsthalfRandom) => Random,
            Phase(PhaseVar::PhaseFirstquarterRandom) => Random,
            Phase(PhaseVar::PhaseLaterhalfRandom) => Random,
            Phase(PhaseVar::PhaseCornerRandom) => Random,
            Phase(PhaseVar::PhaseStraightRandom) => Random,
            Phase(PhaseVar::PhaseFirstHalfStraightRandom) => Random,
            Phase(PhaseVar::PhaseLatterHalfStraightRandom) => Random,
            Phase(PhaseVar::IsFinalcornerRandom) => Random,
            Course(CourseVar::CornerRandom) => Random,
            Course(CourseVar::AllCornerRandom) => Random,
            Course(CourseVar::StraightRandom) => Random,
            Course(CourseVar::LastStraightRandom) => Random,
            Course(CourseVar::UpSlopeRandom) => Random,
            Course(CourseVar::UpSlopeRandomLaterHalf) => Random,
            Course(CourseVar::DownSlopeRandom) => Random,
            Distance(DistanceVar::DistanceRateAfterRandom) => Random,

            // MULTI-UMA STUBS - Requires other umas (placeholder)
            Position(PositionVar::Order) => MultiUmaStub,
            Position(PositionVar::OrderRate) => MultiUmaStub,
            Position(PositionVar::NearCount) => MultiUmaStub,
            Position(PositionVar::NearInfrontCount) => MultiUmaStub,
            Position(PositionVar::OvertakeTargetTime) => MultiUmaStub,
            Position(PositionVar::OvertakeTargetNoOrderUpTime) => MultiUmaStub,
            Position(PositionVar::OrderRateIn20Continue) => MultiUmaStub,
            Position(PositionVar::OrderRateIn40Continue) => MultiUmaStub,
            Position(PositionVar::OrderRateIn50Continue) => MultiUmaStub,
            Position(PositionVar::OrderRateIn80Continue) => MultiUmaStub,
            Position(PositionVar::OrderRateOut20Continue) => MultiUmaStub,
            Position(PositionVar::OrderRateOut40Continue) => MultiUmaStub,
            Position(PositionVar::OrderRateOut50Continue) => MultiUmaStub,
            Position(PositionVar::OrderRateOut70Continue) => MultiUmaStub,
            Distance(DistanceVar::DistanceDiffTop) => MultiUmaStub,
            Distance(DistanceVar::DistanceDiffTopFloat) => MultiUmaStub,
            Distance(DistanceVar::DistanceDiffRate) => MultiUmaStub,
            Distance(DistanceVar::BashinDiffBehind) => MultiUmaStub,
            Distance(DistanceVar::BashinDiffInfront) => MultiUmaStub,
            Blocking(_) => MultiUmaStub,
            Status(StatusVar::IsSurrounded) => MultiUmaStub,
            Status(StatusVar::BehindNearLaneTime) => MultiUmaStub,
            Status(StatusVar::BehindNearLaneTimeSet1) => MultiUmaStub,
            Status(StatusVar::InfrontNearLaneTime) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleCountNigeOtherself) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleCountSenkoOtherself) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleCountSashiOtherself) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleCountOikomiOtherself) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleCountSame) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleCountSameRate) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleEqualPopularityOne) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleTemptationCountNige) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleTemptationCountSenko) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleTemptationCountSashi) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleTemptationCountOikomi) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleTemptationOpponentCountNige) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleTemptationOpponentCountSenko) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleTemptationOpponentCountSashi) => MultiUmaStub,
            RunningStyle(RunningStyleVar::RunningStyleTemptationOpponentCountOikomi) => MultiUmaStub,
            Activation(ActivationVar::IsExistCharaId) => MultiUmaStub,
            Activation(ActivationVar::IsExistSkillId) => MultiUmaStub,
            Activation(ActivationVar::IsOtherCharacterActivateAdvantageSkill) => MultiUmaStub,
            Activation(ActivationVar::SameSkillHorseCount) => MultiUmaStub,
            Temptation(_) => MultiUmaStub,

            // CONTINUOUS - Must check every step (minimized)
            Status(StatusVar::IsLastspurt) => Continuous,
            Status(StatusVar::Lastspurt) => Continuous,
            Status(StatusVar::IsTemptation) => Continuous,
            Status(StatusVar::IsBadstart) => Continuous,  // Under investigation
            Status(StatusVar::IsOvertake) => Continuous,  // Under investigation
            Status(StatusVar::IsMoveLane) => Continuous,
            Status(StatusVar::IsBehindIn) => Continuous, // Under investigation
            Activation(ActivationVar::CompeteFightCount) => Continuous, // Under investigation
            Activation(ActivationVar::IsActivateAnySkill) => Continuous,
            Activation(ActivationVar::IsActivateHealSkill) => Continuous,
            Activation(ActivationVar::IsActivateOtherSkillDetail) => Continuous,
            Activation(ActivationVar::IsUsedSkillId) => Continuous,
            Course(CourseVar::LaneType) => Continuous, // Under investigation

            // UNKNOWN
            VarType::Unknown(_) => TriggerClass::Unknown,
        }
    }
}

/// Classification of how a variable should be triggered/evaluated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerClass {
    /// Static values determined at race start (track type, distance, etc.)
    Static,
    /// Monotonically increasing values (time, distance covered, activation counts)
    MonotonicIncreasing,
    /// Monotonically decreasing values (remaining distance, HP)
    MonotonicDecreasing,
    /// Phase transitions (Early -> Mid -> Late -> Last)
    PhaseTransition,
    /// Corner entry/exit events
    SpatialCorner,
    /// Slope entry/exit events
    SpatialSlope,
    /// Straight entry/exit events
    SpatialStraight,
    /// One-time triggers that fire once then never again
    Onetime,
    /// Random triggers pre-rolled at race start
    Random,
    /// Multi-uma variables requiring other umas (placeholder/stub)
    MultiUmaStub,
    /// Continuous checks required every step (minimized)
    Continuous,
    /// Unknown variable type
    Unknown,
}

/// Classification for trigger-guard separation
/// Determines whether a variable acts as a trigger (determines WHEN to check)
/// or a guard (must be true AT activation time)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableRole {
    /// Random spatial gate - activates at pre-rolled location, checks guards there
    /// Examples: phase_random==1, is_finalcorner_random==1, corner_random==2
    RandomGate,

    /// Onetime event marker - state change that persists
    /// Examples: is_last_straight_onetime==1, change_order_onetime<0, is_hp_empty_onetime==1
    OnetimeEvent,

    /// Monotonic trigger - fires when threshold crossed
    /// Examples: accumulatetime>=5, hp_per<=50, distance_rate>=50
    MonotonicTrigger,

    /// Phase transition trigger - fires when entering new phase
    /// Example: phase>=2
    PhaseTrigger,

    /// Spatial trigger - fires when entering/exiting course features
    /// Examples: corner!=0, slope==1, is_last_straight==1
    SpatialTrigger,

    /// Guard condition - must be true at trigger activation time
    /// Examples: ground_type==2, motivation>=4, running_style==1
    Guard,

    /// Always true - special case
    AlwaysTrue,
}

impl VarType {
    /// Classify variable role for trigger-guard separation
    pub fn variable_role(&self) -> VariableRole {
        use VariableRole::*;
        use VarType::*;

        match self {
            // RANDOM GATES - Activate at pre-rolled location, check guards there
            Trigger(TriggerVar::RandomLot) => RandomGate,
            Phase(PhaseVar::PhaseRandom) => RandomGate,
            Phase(PhaseVar::PhaseFirsthalfRandom) => RandomGate,
            Phase(PhaseVar::PhaseFirstquarterRandom) => RandomGate,
            Phase(PhaseVar::PhaseLaterhalfRandom) => RandomGate,
            Phase(PhaseVar::PhaseCornerRandom) => RandomGate,
            Phase(PhaseVar::PhaseStraightRandom) => RandomGate,
            Phase(PhaseVar::PhaseFirstHalfStraightRandom) => RandomGate,
            Phase(PhaseVar::PhaseLatterHalfStraightRandom) => RandomGate,
            Phase(PhaseVar::IsFinalcornerRandom) => RandomGate,
            Course(CourseVar::CornerRandom) => RandomGate,
            Course(CourseVar::AllCornerRandom) => RandomGate,
            Course(CourseVar::StraightRandom) => RandomGate,
            Course(CourseVar::LastStraightRandom) => RandomGate,
            Course(CourseVar::UpSlopeRandom) => RandomGate,
            Course(CourseVar::UpSlopeRandomLaterHalf) => RandomGate,
            Course(CourseVar::DownSlopeRandom) => RandomGate,
            Distance(DistanceVar::DistanceRateAfterRandom) => RandomGate,

            // ONETIME EVENTS - State changes that persist
            Course(CourseVar::IsLastStraightOnetime) => OnetimeEvent,
            Position(PositionVar::ChangeOrderOnetime) => OnetimeEvent,
            Status(StatusVar::IsHpEmptyOnetime) => OnetimeEvent,

            // MONOTONIC TRIGGERS - Fire when threshold crossed
            // Time-based
            Phase(PhaseVar::AccumulateTime) => MonotonicTrigger,

            // Distance-based (increasing)
            Distance(DistanceVar::DistanceRate) => MonotonicTrigger,
            Distance(DistanceVar::Furlong) => MonotonicTrigger,

            // Distance-based (decreasing)
            Distance(DistanceVar::RemainDistance) => MonotonicTrigger,
            Distance(DistanceVar::RemainDistanceViewerId) => MonotonicTrigger,

            // HP-based (decreasing)
            Status(StatusVar::HpPer) => MonotonicTrigger,

            // PHASE TRIGGERS - Fire when entering new phase
            Phase(PhaseVar::Phase) => PhaseTrigger,

            // SPATIAL TRIGGERS - Fire when entering/exiting course features
            // Corners
            Course(CourseVar::Corner) => SpatialTrigger,
            Phase(PhaseVar::IsFinalcorner) => SpatialTrigger,
            Phase(PhaseVar::IsFinalcornerLaterhalf) => SpatialTrigger,

            // Slopes
            Course(CourseVar::Slope) => SpatialTrigger,

            // Straights
            Course(CourseVar::IsLastStraight) => SpatialTrigger,
            Course(CourseVar::StraightFrontType) => SpatialTrigger,

            // Phase subdivisions (spatial within phase)
            Phase(PhaseVar::PhaseFirsthalf) => SpatialTrigger,
            Phase(PhaseVar::PhaseLaterhalf) => SpatialTrigger,
            Phase(PhaseVar::PhaseFirstquarter) => SpatialTrigger,

            // ALWAYS TRUE
            VarType::Always => AlwaysTrue,

            // GUARD CONDITIONS - Must be true at activation time
            // All other variables are guards that must be checked when triggers fire
            // Static race parameters
            RaceParam(RaceParamVar::Grade) => Guard,
            RaceParam(RaceParamVar::IsDirtgrade) => Guard,
            RaceParam(RaceParamVar::Motivation) => Guard,
            RaceParam(RaceParamVar::Popularity) => Guard,
            RaceParam(RaceParamVar::PostNumber) => Guard,
            RaceParam(RaceParamVar::Season) => Guard,
            RaceParam(RaceParamVar::Time) => Guard,
            RaceParam(RaceParamVar::Weather) => Guard,
            RaceParam(RaceParamVar::Visiblehorse) => Guard,

            // Course properties
            Course(CourseVar::DistanceType) => Guard,
            Course(CourseVar::GroundCondition) => Guard,
            Course(CourseVar::GroundType) => Guard,
            Course(CourseVar::IsBasisDistance) => Guard,
            Course(CourseVar::Rotation) => Guard,
            Course(CourseVar::TrackId) => Guard,
            Course(CourseVar::CornerCount) => Guard,
            Distance(DistanceVar::CourseDistance) => Guard,

            // Base stats
            BaseStat(_) => Guard,

            // Running style
            RunningStyle(RunningStyleVar::RunningStyle) => Guard,
            RunningStyle(_) => Guard,  // All other running style vars are guards

            // Status flags (continuous checks, but act as guards)
            Status(StatusVar::IsLastspurt) => Guard,
            Status(StatusVar::Lastspurt) => Guard,
            Status(StatusVar::IsTemptation) => Guard,
            Status(StatusVar::IsBadstart) => Guard,
            Status(StatusVar::IsOvertake) => Guard,
            Status(StatusVar::IsMoveLane) => Guard,
            Status(StatusVar::IsBehindIn) => Guard,
            Status(StatusVar::IsSurrounded) => Guard,
            Status(StatusVar::BehindNearLaneTime) => Guard,
            Status(StatusVar::BehindNearLaneTimeSet1) => Guard,
            Status(StatusVar::InfrontNearLaneTime) => Guard,

            // Activation counts (can act as guards checked at trigger time)
            Activation(ActivationVar::ActivateCountAll) => Guard,
            Activation(ActivationVar::ActivateCountStart) => Guard,
            Activation(ActivationVar::ActivateCountMiddle) => Guard,
            Activation(ActivationVar::ActivateCountLaterHalf) => Guard,
            Activation(ActivationVar::ActivateCountEndAfter) => Guard,
            Activation(ActivationVar::ActivateCountHeal) => Guard,
            Activation(ActivationVar::ActivateCountAllTeam) => Guard,
            Activation(ActivationVar::IsActivateAnySkill) => Guard,
            Activation(ActivationVar::IsActivateHealSkill) => Guard,
            Activation(ActivationVar::IsActivateOtherSkillDetail) => Guard,
            Activation(ActivationVar::IsUsedSkillId) => Guard,
            Activation(ActivationVar::IsExistSkillId) => Guard,
            Activation(ActivationVar::CompeteFightCount) => Guard,
            Activation(_) => Guard,

            // Position/Order (multi-uma, but act as guards)
            Position(PositionVar::Order) => Guard,
            Position(PositionVar::OrderRate) => Guard,
            Position(PositionVar::NearCount) => Guard,
            Position(PositionVar::NearInfrontCount) => Guard,
            Position(PositionVar::ChangeOrderUpEndAfter) => Guard,
            Position(PositionVar::ChangeOrderUpFinalcornerAfter) => Guard,
            Position(PositionVar::ChangeOrderUpMiddle) => Guard,
            Position(PositionVar::OrderRateIn20Continue) => Guard,
            Position(PositionVar::OrderRateIn40Continue) => Guard,
            Position(PositionVar::OrderRateIn50Continue) => Guard,
            Position(PositionVar::OrderRateIn80Continue) => Guard,
            Position(PositionVar::OrderRateOut20Continue) => Guard,
            Position(PositionVar::OrderRateOut40Continue) => Guard,
            Position(PositionVar::OrderRateOut50Continue) => Guard,
            Position(PositionVar::OrderRateOut70Continue) => Guard,
            Position(PositionVar::OvertakeTargetTime) => Guard,
            Position(PositionVar::OvertakeTargetNoOrderUpTime) => Guard,

            // Distance (multi-uma, but act as guards)
            Distance(DistanceVar::DistanceDiffTop) => Guard,
            Distance(DistanceVar::DistanceDiffTopFloat) => Guard,
            Distance(DistanceVar::DistanceDiffRate) => Guard,
            Distance(DistanceVar::BashinDiffBehind) => Guard,
            Distance(DistanceVar::BashinDiffInfront) => Guard,

            // Blocking
            Blocking(_) => Guard,

            // Temptation/Rush
            Temptation(_) => Guard,

            // Lane
            Course(CourseVar::LaneType) => Guard,

            // Unknown
            VarType::Unknown(_) => Guard,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_variables_parse() {
        // Test a sample of known variables
        assert!(matches!(
            VarType::parse("base_speed"),
            VarType::BaseStat(StatKind::Speed)
        ));
        assert!(matches!(
            VarType::parse("accumulatetime"),
            VarType::Phase(PhaseVar::AccumulateTime)
        ));
        assert!(matches!(
            VarType::parse("order"),
            VarType::Position(PositionVar::Order)
        ));
        assert!(matches!(
            VarType::parse("remain_distance"),
            VarType::Distance(DistanceVar::RemainDistance)
        ));
        assert!(matches!(
            VarType::parse("corner"),
            VarType::Course(CourseVar::Corner)
        ));
        assert!(matches!(
            VarType::parse("random_lot"),
            VarType::Trigger(TriggerVar::RandomLot)
        ));
        assert!(matches!(VarType::parse("always"), VarType::Always));
    }

    #[test]
    fn test_unknown_variable_fallback() {
        let result = VarType::parse("this_var_does_not_exist");
        assert!(matches!(result, VarType::Unknown(_)));
        if let VarType::Unknown(s) = result {
            assert_eq!(s, "this_var_does_not_exist");
        }
    }

    #[test]
    fn test_display_formatting() {
        assert_eq!(
            format!("{}", VarType::BaseStat(StatKind::Speed)),
            "base_speed"
        );
        assert_eq!(format!("{}", VarType::Always), "always");
    }
}
