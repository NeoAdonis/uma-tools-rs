use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    BaseStat(StatKind),            // Base stats
    Position(PositionVar),         // Position/Order related
    Distance(DistanceVar),         // Distance related
    Phase(PhaseVar),               // Phase/Time related
    Course(CourseVar),             // Course features
    Spatial(SpatialVar),           // Spatial related
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
    CornerCount,
    DistanceType,
    GroundCondition,
    GroundType,
    IsBasisDistance,
    Rotation,
    TrackId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialVar {
    AllCornerRandom,
    Corner,
    CornerRandom,
    DownSlopeRandom,
    IsLastStraight,
    IsLastStraightOnetime,
    LaneType,
    LastStraightRandom,
    Slope,
    StraightFrontType,
    StraightRandom,
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
    IsExistSkillId,
    IsOtherCharacterActivateAdvantageSkill,
    IsUsedSkillId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceParamVar {
    Grade,
    IsDirtgrade,
    IsExistCharaId,
    Popularity,
    SameSkillHorseCount,
    Season,
    Time,
    Visiblehorse,
    Weather,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunningStyleVar {
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
    Motivation,
    PostNumber,
    TemptationCount,
    RunningStyle,
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
            VarType::Spatial(l) => write!(f, "{}", l),
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
impl_display_snake_case!(SpatialVar);
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
            "corner_count" => VarType::Course(CourseVar::CornerCount),
            "distance_type" => VarType::Course(CourseVar::DistanceType),
            "ground_condition" => VarType::Course(CourseVar::GroundCondition),
            "ground_type" => VarType::Course(CourseVar::GroundType),
            "is_basis_distance" => VarType::Course(CourseVar::IsBasisDistance),
            "rotation" => VarType::Course(CourseVar::Rotation),
            "track_id" => VarType::Course(CourseVar::TrackId),

            // Spatial
            "all_corner_random" => VarType::Spatial(SpatialVar::AllCornerRandom),
            "corner" => VarType::Spatial(SpatialVar::Corner),
            "corner_random" => VarType::Spatial(SpatialVar::CornerRandom),
            "down_slope_random" => VarType::Spatial(SpatialVar::DownSlopeRandom),
            "is_last_straight" => VarType::Spatial(SpatialVar::IsLastStraight),
            "is_last_straight_onetime" => VarType::Spatial(SpatialVar::IsLastStraightOnetime),
            "lane_type" => VarType::Spatial(SpatialVar::LaneType),
            "last_straight_random" => VarType::Spatial(SpatialVar::LastStraightRandom),
            "slope" => VarType::Spatial(SpatialVar::Slope),
            "straight_front_type" => VarType::Spatial(SpatialVar::StraightFrontType),
            "straight_random" => VarType::Spatial(SpatialVar::StraightRandom),
            "up_slope_random" => VarType::Spatial(SpatialVar::UpSlopeRandom),
            "up_slope_random_later_half" => VarType::Spatial(SpatialVar::UpSlopeRandomLaterHalf),

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
            "is_exist_skill_id" => VarType::Activation(ActivationVar::IsExistSkillId),
            "is_other_character_activate_advantage_skill" => {
                VarType::Activation(ActivationVar::IsOtherCharacterActivateAdvantageSkill)
            }
            "is_used_skill_id" => VarType::Activation(ActivationVar::IsUsedSkillId),

            // Race parameters
            "grade" => VarType::RaceParam(RaceParamVar::Grade),
            "is_dirtgrade" => VarType::RaceParam(RaceParamVar::IsDirtgrade),
            "is_exist_chara_id" => VarType::RaceParam(RaceParamVar::IsExistCharaId),
            "popularity" => VarType::RaceParam(RaceParamVar::Popularity),
            "same_skill_horse_count" => VarType::RaceParam(RaceParamVar::SameSkillHorseCount),
            "season" => VarType::RaceParam(RaceParamVar::Season),
            "time" => VarType::RaceParam(RaceParamVar::Time),
            "visiblehorse" => VarType::RaceParam(RaceParamVar::Visiblehorse),
            "weather" => VarType::RaceParam(RaceParamVar::Weather),

            // Running style
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
            "running_style" => VarType::Status(StatusVar::RunningStyle),
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
            "motivation" => VarType::Status(StatusVar::Motivation),
            "post_number" => VarType::Status(StatusVar::PostNumber),
            "temptation_count" => VarType::Status(StatusVar::TemptationCount),

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

    /// Monotonic guard - can act as trigger OR guard depending on context
    /// When paired with explicit triggers (RandomGate/Spatial/Phase): acts as GUARD
    /// When alone (no explicit triggers): acts as TRIGGER
    /// Examples: hp_per<=50, accumulatetime>=5, distance_rate>=50
    MonotonicGuard,

    /// Phase transition trigger - fires when entering new phase
    /// Example: phase>=2
    PhaseTrigger,

    /// Spatial trigger - fires when entering/exiting course features
    /// Examples: corner!=0, slope==1, is_last_straight==1
    SpatialTrigger,

    /// Static guard - never changes during race, evaluated once at race start
    /// Examples: ground_type==2, distance_type==4, running_style==1, motivation>=4
    StaticGuard,

    /// Dynamic guard - can change during race, must be checked when trigger fires
    /// Examples: order<=3, is_lastspurt==1, blocked_side_continuetime>=2
    DynamicGuard,

    /// Always true - special case
    AlwaysTrue,
}

impl VarType {
    /// Classify variable role for trigger-guard separation
    pub fn variable_role(&self) -> VariableRole {
        use VarType::*;
        use VariableRole::*;

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
            Spatial(SpatialVar::CornerRandom) => RandomGate,
            Spatial(SpatialVar::AllCornerRandom) => RandomGate,
            Spatial(SpatialVar::StraightRandom) => RandomGate,
            Spatial(SpatialVar::LastStraightRandom) => RandomGate,
            Spatial(SpatialVar::UpSlopeRandom) => RandomGate,
            Spatial(SpatialVar::UpSlopeRandomLaterHalf) => RandomGate,
            Spatial(SpatialVar::DownSlopeRandom) => RandomGate,
            Distance(DistanceVar::DistanceRateAfterRandom) => RandomGate,

            // ONETIME EVENTS - State changes that persist
            Spatial(SpatialVar::IsLastStraightOnetime) => OnetimeEvent,
            Position(PositionVar::ChangeOrderOnetime) => OnetimeEvent,
            Status(StatusVar::IsHpEmptyOnetime) => OnetimeEvent,

            // MONOTONIC GUARDS - Can act as trigger or guard depending on context
            // Time-based
            Phase(PhaseVar::AccumulateTime) => MonotonicGuard,

            // Distance-based (increasing)
            Distance(DistanceVar::DistanceRate) => MonotonicGuard,
            Distance(DistanceVar::Furlong) => MonotonicGuard,

            // Distance-based (decreasing)
            Distance(DistanceVar::RemainDistance) => MonotonicGuard,
            Distance(DistanceVar::RemainDistanceViewerId) => MonotonicGuard,

            // HP-based (decreasing)
            Status(StatusVar::HpPer) => MonotonicGuard,

            // Temptation count (increasing)
            Status(StatusVar::TemptationCount) => MonotonicGuard,

            // PHASE TRIGGERS - Fire when entering new phase
            Phase(PhaseVar::Phase) => PhaseTrigger,

            // SPATIAL TRIGGERS - Fire when entering/exiting course features
            // Corners
            Spatial(SpatialVar::Corner) => SpatialTrigger,
            Phase(PhaseVar::IsFinalcorner) => SpatialTrigger,
            Phase(PhaseVar::IsFinalcornerLaterhalf) => SpatialTrigger,

            // Slopes
            Spatial(SpatialVar::Slope) => SpatialTrigger,

            // Straights
            Spatial(SpatialVar::IsLastStraight) => SpatialTrigger,
            Spatial(SpatialVar::StraightFrontType) => SpatialTrigger,

            // Phase subdivisions (spatial within phase)
            Phase(PhaseVar::PhaseFirsthalf) => SpatialTrigger,
            Phase(PhaseVar::PhaseLaterhalf) => SpatialTrigger,
            Phase(PhaseVar::PhaseFirstquarter) => SpatialTrigger,

            // ALWAYS TRUE
            VarType::Always => AlwaysTrue,

            // STATIC GUARDS - Never change during race
            // Static race parameters
            RaceParam(RaceParamVar::Grade) => StaticGuard,
            RaceParam(RaceParamVar::IsDirtgrade) => StaticGuard,
            RaceParam(RaceParamVar::IsExistCharaId) => StaticGuard,
            RaceParam(RaceParamVar::Popularity) => StaticGuard,
            RaceParam(RaceParamVar::SameSkillHorseCount) => StaticGuard,
            RaceParam(RaceParamVar::Season) => StaticGuard,
            RaceParam(RaceParamVar::Time) => StaticGuard,
            RaceParam(RaceParamVar::Visiblehorse) => StaticGuard,
            RaceParam(RaceParamVar::Weather) => StaticGuard,

            // Static status properties
            Status(StatusVar::Motivation) => StaticGuard,
            Status(StatusVar::PostNumber) => StaticGuard,
            Status(StatusVar::RunningStyle) => StaticGuard, // TODO: treat as static for now

            // Course properties
            Course(CourseVar::DistanceType) => StaticGuard,
            Course(CourseVar::GroundCondition) => StaticGuard,
            Course(CourseVar::GroundType) => StaticGuard,
            Course(CourseVar::IsBasisDistance) => StaticGuard,
            Course(CourseVar::Rotation) => StaticGuard,
            Course(CourseVar::TrackId) => StaticGuard,
            Course(CourseVar::CornerCount) => StaticGuard,
            Distance(DistanceVar::CourseDistance) => StaticGuard,

            // Base stats
            BaseStat(_) => StaticGuard,

            // Running style
            RunningStyle(_) => DynamicGuard, // Other running style vars are dynamic (multi-uma)

            // Status flags (continuous checks, but act as guards)
            Status(StatusVar::IsLastspurt) => DynamicGuard,
            Status(StatusVar::Lastspurt) => DynamicGuard,
            Status(StatusVar::IsTemptation) => DynamicGuard,
            Status(StatusVar::IsBadstart) => DynamicGuard,
            Status(StatusVar::IsOvertake) => DynamicGuard,
            Status(StatusVar::IsMoveLane) => DynamicGuard,
            Status(StatusVar::IsBehindIn) => DynamicGuard,
            Status(StatusVar::IsSurrounded) => DynamicGuard,
            Status(StatusVar::BehindNearLaneTime) => DynamicGuard,
            Status(StatusVar::BehindNearLaneTimeSet1) => DynamicGuard,
            Status(StatusVar::InfrontNearLaneTime) => DynamicGuard,

            // Activation counts (can act as guards checked at trigger time)
            Activation(ActivationVar::ActivateCountAll) => DynamicGuard,
            Activation(ActivationVar::ActivateCountStart) => DynamicGuard,
            Activation(ActivationVar::ActivateCountMiddle) => DynamicGuard,
            Activation(ActivationVar::ActivateCountLaterHalf) => DynamicGuard,
            Activation(ActivationVar::ActivateCountEndAfter) => DynamicGuard,
            Activation(ActivationVar::ActivateCountHeal) => DynamicGuard,
            Activation(ActivationVar::ActivateCountAllTeam) => DynamicGuard,
            Activation(ActivationVar::IsActivateAnySkill) => DynamicGuard,
            Activation(ActivationVar::IsActivateHealSkill) => DynamicGuard,
            Activation(ActivationVar::IsActivateOtherSkillDetail) => DynamicGuard,
            Activation(ActivationVar::IsUsedSkillId) => DynamicGuard,
            Activation(ActivationVar::IsExistSkillId) => DynamicGuard,
            Activation(ActivationVar::CompeteFightCount) => DynamicGuard,
            Activation(ActivationVar::IsOtherCharacterActivateAdvantageSkill) => DynamicGuard,

            // Position/Order (multi-uma, but act as guards)
            Position(PositionVar::Order) => DynamicGuard,
            Position(PositionVar::OrderRate) => DynamicGuard,
            Position(PositionVar::NearCount) => DynamicGuard,
            Position(PositionVar::NearInfrontCount) => DynamicGuard,
            Position(PositionVar::ChangeOrderUpEndAfter) => DynamicGuard,
            Position(PositionVar::ChangeOrderUpFinalcornerAfter) => DynamicGuard,
            Position(PositionVar::ChangeOrderUpMiddle) => DynamicGuard,
            Position(PositionVar::OrderRateIn20Continue) => DynamicGuard,
            Position(PositionVar::OrderRateIn40Continue) => DynamicGuard,
            Position(PositionVar::OrderRateIn50Continue) => DynamicGuard,
            Position(PositionVar::OrderRateIn80Continue) => DynamicGuard,
            Position(PositionVar::OrderRateOut20Continue) => DynamicGuard,
            Position(PositionVar::OrderRateOut40Continue) => DynamicGuard,
            Position(PositionVar::OrderRateOut50Continue) => DynamicGuard,
            Position(PositionVar::OrderRateOut70Continue) => DynamicGuard,
            Position(PositionVar::OvertakeTargetTime) => DynamicGuard,
            Position(PositionVar::OvertakeTargetNoOrderUpTime) => DynamicGuard,

            // Distance (multi-uma, but act as guards)
            Distance(DistanceVar::DistanceDiffTop) => DynamicGuard,
            Distance(DistanceVar::DistanceDiffTopFloat) => DynamicGuard,
            Distance(DistanceVar::DistanceDiffRate) => DynamicGuard,
            Distance(DistanceVar::BashinDiffBehind) => DynamicGuard,
            Distance(DistanceVar::BashinDiffInfront) => DynamicGuard,

            // Blocking
            Blocking(_) => DynamicGuard,

            // Temptation/Rush
            Temptation(_) => DynamicGuard,

            // Lane
            Spatial(SpatialVar::LaneType) => DynamicGuard,

            // Unknown
            VarType::Unknown(_) => DynamicGuard,
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
            VarType::parse("corner_count"),
            VarType::Course(CourseVar::CornerCount)
        ));
        assert!(matches!(
            VarType::parse("corner"),
            VarType::Spatial(SpatialVar::Corner)
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
