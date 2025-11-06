use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    AccumulateTime,                // Elapsed time
    BaseStat(StatKind),            // Base stats
    Order(OrderVar),               // Order (rank) related
    Distance(DistanceVar),         // Distance relative to other runners
    Phase(PhaseVar),               // Phase/Time related
    Course(CourseVar),             // Course features
    Spatial(SpatialVar),           // Runner position relative to course features
    Activation(ActivationVar),     // Skill activation tracking
    RaceParam(RaceParamVar),       // Race parameters
    RunningStyle(RunningStyleVar), // Running style related
    Status(StatusVar),             // Status/State flags
    Random(RandomVar),             // Random activation
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
pub enum OrderVar {
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
    DistanceDiffRate,
    DistanceDiffTop,
    DistanceDiffTopFloat,
    RemainDistanceViewerId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseVar {
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
    CourseDistance,
    DistanceType,
    GroundType,
    IsBasisDistance,
    IsDirtgrade,
    Rotation,
    TrackId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialVar {
    AllCornerRandom,
    Corner,
    CornerRandom,
    DistanceRate,
    DistanceRateAfterRandom,
    DownSlopeRandom,
    Furlong,
    IsFinalcorner,
    IsFinalcornerLaterhalf,
    IsFinalcornerRandom,
    IsLastStraight,
    IsLastStraightOnetime,
    LaneType,
    LastStraightRandom,
    RemainDistance,
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
    GroundCondition,
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
pub enum RandomVar {
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

impl fmt::Display for VarType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VarType::BaseStat(s) => write!(f, "base_{}", s),
            VarType::Order(o) => write!(f, "{}", o),
            VarType::Distance(d) => write!(f, "{}", d),
            VarType::Phase(p) => write!(f, "{}", p),
            VarType::Spatial(l) => write!(f, "{}", l),
            VarType::Course(c) => write!(f, "{}", c),
            VarType::Activation(a) => write!(f, "{}", a),
            VarType::RaceParam(r) => write!(f, "{}", r),
            VarType::RunningStyle(r) => write!(f, "{}", r),
            VarType::Status(s) => write!(f, "{}", s),
            VarType::Random(r) => write!(f, "{}", r),
            VarType::Blocking(b) => write!(f, "{}", b),
            VarType::Temptation(t) => write!(f, "{}", t),
            VarType::AccumulateTime => write!(f, "accumulatetime"),
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

impl_display_snake_case!(OrderVar);
impl_display_snake_case!(DistanceVar);
impl_display_snake_case!(PhaseVar);
impl_display_snake_case!(SpatialVar);
impl_display_snake_case!(CourseVar);
impl_display_snake_case!(ActivationVar);
impl_display_snake_case!(RaceParamVar);
impl_display_snake_case!(RunningStyleVar);
impl_display_snake_case!(StatusVar);
impl_display_snake_case!(RandomVar);
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

            // Order/Rank
            "change_order_onetime" => VarType::Order(OrderVar::ChangeOrderOnetime),
            "change_order_up_end_after" => VarType::Order(OrderVar::ChangeOrderUpEndAfter),
            "change_order_up_finalcorner_after" => {
                VarType::Order(OrderVar::ChangeOrderUpFinalcornerAfter)
            }
            "change_order_up_middle" => VarType::Order(OrderVar::ChangeOrderUpMiddle),
            "near_count" => VarType::Order(OrderVar::NearCount),
            "near_infront_count" => VarType::Order(OrderVar::NearInfrontCount),
            "order" => VarType::Order(OrderVar::Order),
            "order_rate" => VarType::Order(OrderVar::OrderRate),
            "order_rate_in20_continue" => VarType::Order(OrderVar::OrderRateIn20Continue),
            "order_rate_in40_continue" => VarType::Order(OrderVar::OrderRateIn40Continue),
            "order_rate_in50_continue" => VarType::Order(OrderVar::OrderRateIn50Continue),
            "order_rate_in80_continue" => VarType::Order(OrderVar::OrderRateIn80Continue),
            "order_rate_out20_continue" => VarType::Order(OrderVar::OrderRateOut20Continue),
            "order_rate_out40_continue" => VarType::Order(OrderVar::OrderRateOut40Continue),
            "order_rate_out50_continue" => VarType::Order(OrderVar::OrderRateOut50Continue),
            "order_rate_out70_continue" => VarType::Order(OrderVar::OrderRateOut70Continue),
            "overtake_target_no_order_up_time" => {
                VarType::Order(OrderVar::OvertakeTargetNoOrderUpTime)
            }
            "overtake_target_time" => VarType::Order(OrderVar::OvertakeTargetTime),

            // Distance to other runners
            "bashin_diff_behind" => VarType::Distance(DistanceVar::BashinDiffBehind),
            "bashin_diff_infront" => VarType::Distance(DistanceVar::BashinDiffInfront),
            "distance_diff_rate" => VarType::Distance(DistanceVar::DistanceDiffRate),
            "distance_diff_top_float" => VarType::Distance(DistanceVar::DistanceDiffTopFloat),
            "distance_diff_top" => VarType::Distance(DistanceVar::DistanceDiffTop),
            "remain_distance_viewer_id" => VarType::Distance(DistanceVar::RemainDistanceViewerId),

            // Time
            "accumulatetime" => VarType::AccumulateTime,

            // Phase
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
            "course_distance" => VarType::Course(CourseVar::CourseDistance),
            "distance_type" => VarType::Course(CourseVar::DistanceType),
            "ground_type" => VarType::Course(CourseVar::GroundType),
            "is_basis_distance" => VarType::Course(CourseVar::IsBasisDistance),
            "is_dirtgrade" => VarType::Course(CourseVar::IsDirtgrade),
            "rotation" => VarType::Course(CourseVar::Rotation),
            "track_id" => VarType::Course(CourseVar::TrackId),

            // Spatial
            "all_corner_random" => VarType::Spatial(SpatialVar::AllCornerRandom),
            "corner" => VarType::Spatial(SpatialVar::Corner),
            "corner_random" => VarType::Spatial(SpatialVar::CornerRandom),
            "distance_rate" => VarType::Spatial(SpatialVar::DistanceRate),
            "distance_rate_after_random" => VarType::Spatial(SpatialVar::DistanceRateAfterRandom),
            "down_slope_random" => VarType::Spatial(SpatialVar::DownSlopeRandom),
            "furlong" => VarType::Spatial(SpatialVar::Furlong),
            "is_finalcorner" => VarType::Spatial(SpatialVar::IsFinalcorner),
            "is_finalcorner_laterhalf" => VarType::Spatial(SpatialVar::IsFinalcornerLaterhalf),
            "is_finalcorner_random" => VarType::Spatial(SpatialVar::IsFinalcornerRandom),
            "is_last_straight" => VarType::Spatial(SpatialVar::IsLastStraight),
            "is_last_straight_onetime" => VarType::Spatial(SpatialVar::IsLastStraightOnetime),
            "lane_type" => VarType::Spatial(SpatialVar::LaneType),
            "last_straight_random" => VarType::Spatial(SpatialVar::LastStraightRandom),
            "remain_distance" => VarType::Spatial(SpatialVar::RemainDistance),
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
            "ground_condition" => VarType::RaceParam(RaceParamVar::GroundCondition),
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

            // Randoms
            "random_lot" => VarType::Random(RandomVar::RandomLot),

            // Blocking
            "blocked_all_continuetime" => VarType::Blocking(BlockingVar::BlockedAllContinuetime),
            "blocked_front" => VarType::Blocking(BlockingVar::BlockedFront),
            "blocked_front_continuetime" => {
                VarType::Blocking(BlockingVar::BlockedFrontContinuetime)
            }
            "blocked_side_continuetime" => VarType::Blocking(BlockingVar::BlockedSideContinuetime),

            // Rush/Temptation
            "running_style_temptation_count_nige" => {
                VarType::Temptation(TemptationVar::RunningStyleTemptationCountNige)
            }
            "running_style_temptation_count_oikomi" => {
                VarType::Temptation(TemptationVar::RunningStyleTemptationCountOikomi)
            }
            "running_style_temptation_count_sashi" => {
                VarType::Temptation(TemptationVar::RunningStyleTemptationCountSashi)
            }
            "running_style_temptation_count_senko" => {
                VarType::Temptation(TemptationVar::RunningStyleTemptationCountSenko)
            }
            "running_style_temptation_opponent_count_nige" => {
                VarType::Temptation(TemptationVar::RunningStyleTemptationOpponentCountNige)
            }
            "running_style_temptation_opponent_count_oikomi" => {
                VarType::Temptation(TemptationVar::RunningStyleTemptationOpponentCountOikomi)
            }
            "running_style_temptation_opponent_count_sashi" => {
                VarType::Temptation(TemptationVar::RunningStyleTemptationOpponentCountSashi)
            }
            "running_style_temptation_opponent_count_senko" => {
                VarType::Temptation(TemptationVar::RunningStyleTemptationOpponentCountSenko)
            }
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

/// Classification for trigger-guard separation.
///
/// Determines whether a variable acts as a trigger (determines WHEN to check)
/// or a guard (must be true AT activation time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableRole {
    /// A trigger that fires at a random location. The activation point should be
    /// pre-determined before race start.
    ///
    /// Examples: phase_random==1, is_finalcorner_random==1, corner_random==2
    RandomTrigger,

    /// A variable whose value can only increase OR decrease, but not both.
    /// In most cases, it should be treated as a guard, but it can act as a trigger
    /// depending on context.
    ///
    /// - When paired with explicit triggers (Random/Spatial/Phase): acts as a GUARD.
    /// - When alone (no explicit triggers): if the condition starts as false and can
    ///   turn true later (e.g. accumulatetime>=5), it acts as a TRIGGER; if it's the
    ///   opposite (e.g. distance_rate<=50), it acts as CONTINUOUS, meaning that the
    ///   underlying condition must be continuously checked.
    ///
    /// Examples: hp_per<=50, accumulatetime>=5, distance_rate>=50
    MonotonicGuard,

    /// Phase transition trigger - fires when entering new phase.
    ///
    /// Example: phase>=2
    PhaseTrigger,

    /// Spatial trigger - fires when entering/exiting course features.
    ///
    /// Examples: corner!=0, slope==1, is_last_straight==1
    SpatialTrigger,

    /// Activation trigger - fires when a skill is activated.
    ///
    /// Examples: is_activate_any_skill==1, is_heal_skill_activated==1
    ActivationTrigger,

    /// A variable whose value never changes during race. Therefore,
    /// it can be evaluated just once at race start.
    ///
    /// Examples: ground_type==2, running_style==1, motivation>=4
    StaticGuard,

    /// A variable whose value can change during race.
    /// Must be checked when a trigger fires.
    ///
    /// Examples: order<=3, is_lastspurt==1, blocked_side_continuetime>=2
    DynamicGuard,
}

impl VarType {
    /// Classify variable role for trigger-guard separation
    pub fn variable_role(&self) -> VariableRole {
        use VarType::*;
        use VariableRole::*;

        match self {
            // RANDOM GATES - Activate at pre-rolled location, check guards there
            Random(RandomVar::RandomLot) => RandomTrigger,
            Phase(PhaseVar::PhaseRandom) => RandomTrigger,
            Phase(PhaseVar::PhaseFirsthalfRandom) => RandomTrigger,
            Phase(PhaseVar::PhaseFirstquarterRandom) => RandomTrigger,
            Phase(PhaseVar::PhaseLaterhalfRandom) => RandomTrigger,
            Phase(PhaseVar::PhaseCornerRandom) => RandomTrigger,
            Phase(PhaseVar::PhaseStraightRandom) => RandomTrigger,
            Phase(PhaseVar::PhaseFirstHalfStraightRandom) => RandomTrigger,
            Phase(PhaseVar::PhaseLatterHalfStraightRandom) => RandomTrigger,
            Spatial(SpatialVar::IsFinalcornerRandom) => RandomTrigger,
            Spatial(SpatialVar::CornerRandom) => RandomTrigger,
            Spatial(SpatialVar::AllCornerRandom) => RandomTrigger,
            Spatial(SpatialVar::StraightRandom) => RandomTrigger,
            Spatial(SpatialVar::LastStraightRandom) => RandomTrigger,
            Spatial(SpatialVar::UpSlopeRandom) => RandomTrigger,
            Spatial(SpatialVar::UpSlopeRandomLaterHalf) => RandomTrigger,
            Spatial(SpatialVar::DownSlopeRandom) => RandomTrigger,
            Spatial(SpatialVar::DistanceRateAfterRandom) => RandomTrigger,

            // MONOTONIC GUARDS - Can act as trigger or guard depending on context
            // Time-based
            AccumulateTime => MonotonicGuard,

            // Distance-based (increasing)
            Spatial(SpatialVar::DistanceRate) => MonotonicGuard,
            Spatial(SpatialVar::Furlong) => MonotonicGuard,

            // Distance-based (decreasing)
            Spatial(SpatialVar::RemainDistance) => MonotonicGuard,
            Distance(DistanceVar::RemainDistanceViewerId) => MonotonicGuard,

            // HP-based (decreasing)
            Status(StatusVar::HpPer) => MonotonicGuard,

            // HP-based one-time event (increasing; from 0 to 1)
            Status(StatusVar::IsHpEmptyOnetime) => MonotonicGuard,

            // Temptation count (increasing)
            Status(StatusVar::TemptationCount) => MonotonicGuard,

            // PHASE TRIGGERS - Fire when entering new phase
            Phase(PhaseVar::Phase) => PhaseTrigger,

            // SPATIAL TRIGGERS - Fire when entering/exiting course features
            // Corners
            Spatial(SpatialVar::Corner) => SpatialTrigger,
            Spatial(SpatialVar::IsFinalcorner) => SpatialTrigger,
            Spatial(SpatialVar::IsFinalcornerLaterhalf) => SpatialTrigger,

            // Slopes
            Spatial(SpatialVar::Slope) => SpatialTrigger,

            // Straights
            Spatial(SpatialVar::IsLastStraight) => SpatialTrigger,
            Spatial(SpatialVar::IsLastStraightOnetime) => SpatialTrigger,
            Spatial(SpatialVar::StraightFrontType) => SpatialTrigger,

            // Phase subdivisions (spatial within phase)
            Phase(PhaseVar::PhaseFirsthalf) => SpatialTrigger,
            Phase(PhaseVar::PhaseLaterhalf) => SpatialTrigger,
            Phase(PhaseVar::PhaseFirstquarter) => SpatialTrigger,

            // STATIC GUARDS - Never change during race
            // Race parameters
            RaceParam(_) => StaticGuard,

            // Course properties
            Course(_) => StaticGuard,

            // Base stats
            BaseStat(_) => StaticGuard,

            // Special value - By definition, it never changes
            Always => StaticGuard,

            // Static status properties
            Status(StatusVar::Motivation) => StaticGuard,
            Status(StatusVar::PostNumber) => StaticGuard,

            // Running style
            // N.B. While running style can technically change, it can only do so
            // from Front Runner to Runaway. Since both styles are considered
            // the same for skill purposes, we can treat this variable as static
            Status(StatusVar::RunningStyle) => StaticGuard,

            // DYNAMIC GUARDS - Can change during race
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
            Activation(ActivationVar::ActivateCountAll) => MonotonicGuard,
            Activation(ActivationVar::ActivateCountAllTeam) => MonotonicGuard,
            Activation(ActivationVar::ActivateCountEndAfter) => MonotonicGuard,
            Activation(ActivationVar::ActivateCountHeal) => MonotonicGuard,
            Activation(ActivationVar::ActivateCountLaterHalf) => MonotonicGuard,
            Activation(ActivationVar::ActivateCountMiddle) => MonotonicGuard,
            Activation(ActivationVar::ActivateCountStart) => MonotonicGuard,
            Activation(ActivationVar::CompeteFightCount) => MonotonicGuard,
            Activation(ActivationVar::IsExistSkillId) => MonotonicGuard,
            Activation(ActivationVar::IsUsedSkillId) => MonotonicGuard,

            // Activation triggers
            Activation(ActivationVar::IsActivateAnySkill) => ActivationTrigger,
            Activation(ActivationVar::IsActivateHealSkill) => ActivationTrigger,
            Activation(ActivationVar::IsActivateOtherSkillDetail) => DynamicGuard,
            Activation(ActivationVar::IsOtherCharacterActivateAdvantageSkill) => DynamicGuard,

            // Order/Rank (multi-uma, but act as guards)
            Order(OrderVar::Order) => DynamicGuard,
            Order(OrderVar::OrderRate) => DynamicGuard,
            Order(OrderVar::NearCount) => DynamicGuard,
            Order(OrderVar::NearInfrontCount) => DynamicGuard,
            Order(OrderVar::ChangeOrderOnetime) => DynamicGuard,
            Order(OrderVar::ChangeOrderUpEndAfter) => DynamicGuard,
            Order(OrderVar::ChangeOrderUpFinalcornerAfter) => DynamicGuard,
            Order(OrderVar::ChangeOrderUpMiddle) => DynamicGuard,
            Order(OrderVar::OrderRateIn20Continue) => DynamicGuard,
            Order(OrderVar::OrderRateIn40Continue) => DynamicGuard,
            Order(OrderVar::OrderRateIn50Continue) => DynamicGuard,
            Order(OrderVar::OrderRateIn80Continue) => DynamicGuard,
            Order(OrderVar::OrderRateOut20Continue) => DynamicGuard,
            Order(OrderVar::OrderRateOut40Continue) => DynamicGuard,
            Order(OrderVar::OrderRateOut50Continue) => DynamicGuard,
            Order(OrderVar::OrderRateOut70Continue) => DynamicGuard,
            Order(OrderVar::OvertakeTargetTime) => DynamicGuard,
            Order(OrderVar::OvertakeTargetNoOrderUpTime) => DynamicGuard,

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
