use std::fmt;

/// Variable types used in conditions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarType {
    /// Variable representing accumulated (elapsed) time.
    AccumulateTime,
    /// Variable representing base stats.
    BaseStat(StatKind),
    /// Variables related to order/rank.
    Order(OrderVar),
    /// Variables related to distance between runners.
    Distance(DistanceVar),
    /// Variables related to phases.
    Phase(PhaseVar),
    /// Variables related to course properties.
    Course(CourseVar),
    /// Variables related to spatial position (i.e. position on course or course features).
    Spatial(SpatialVar),
    /// Variables related to skill activation tracking.
    Activation(ActivationVar),
    /// Variables related to race parameters.
    RaceParam(RaceParamVar),
    /// Variables related to running style.
    RunningStyle(RunningStyleVar),
    /// Variables related to status/flags.
    Status(StatusVar),
    /// Variables whose value is determined randomly.
    Random(RandomVar),
    /// Variables related to blocking/lane position.
    Blocking(BlockingVar),
    /// Variables related to Rush (Temptation) mechanics.
    Temptation(TemptationVar),
    /// Special constant that is always true.
    Always,
    /// Fallback type for unknown variables.
    Unknown(String),
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
    RemainDistanceViewerId,
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
    RunningStyle,
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
        use VarType::*;
        match self {
            BaseStat(s) => write!(f, "base_{}", s),
            Order(o) => write!(f, "{}", o),
            Distance(d) => write!(f, "{}", d),
            Phase(p) => write!(f, "{}", p),
            Spatial(l) => write!(f, "{}", l),
            Course(c) => write!(f, "{}", c),
            Activation(a) => write!(f, "{}", a),
            RaceParam(r) => write!(f, "{}", r),
            RunningStyle(r) => write!(f, "{}", r),
            Status(s) => write!(f, "{}", s),
            Random(r) => write!(f, "{}", r),
            Blocking(b) => write!(f, "{}", b),
            Temptation(t) => write!(f, "{}", t),
            AccumulateTime => write!(f, "accumulatetime"),
            Always => write!(f, "always"),
            Unknown(s) => write!(f, "<unknown:{}>", s),
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
        use VarType::*;

        match s {
            // Base stats
            "base_speed" => BaseStat(StatKind::Speed),
            "base_stamina" => BaseStat(StatKind::Stamina),
            "base_power" => BaseStat(StatKind::Power),
            "base_guts" => BaseStat(StatKind::Guts),
            "base_wiz" => BaseStat(StatKind::Wiz),

            // Order/Rank
            "change_order_onetime" => Order(OrderVar::ChangeOrderOnetime),
            "change_order_up_end_after" => Order(OrderVar::ChangeOrderUpEndAfter),
            "change_order_up_finalcorner_after" => Order(OrderVar::ChangeOrderUpFinalcornerAfter),
            "change_order_up_middle" => Order(OrderVar::ChangeOrderUpMiddle),
            "near_count" => Order(OrderVar::NearCount),
            "near_infront_count" => Order(OrderVar::NearInfrontCount),
            "order" => Order(OrderVar::Order),
            "order_rate" => Order(OrderVar::OrderRate),
            "order_rate_in20_continue" => Order(OrderVar::OrderRateIn20Continue),
            "order_rate_in40_continue" => Order(OrderVar::OrderRateIn40Continue),
            "order_rate_in50_continue" => Order(OrderVar::OrderRateIn50Continue),
            "order_rate_in80_continue" => Order(OrderVar::OrderRateIn80Continue),
            "order_rate_out20_continue" => Order(OrderVar::OrderRateOut20Continue),
            "order_rate_out40_continue" => Order(OrderVar::OrderRateOut40Continue),
            "order_rate_out50_continue" => Order(OrderVar::OrderRateOut50Continue),
            "order_rate_out70_continue" => Order(OrderVar::OrderRateOut70Continue),
            "overtake_target_no_order_up_time" => Order(OrderVar::OvertakeTargetNoOrderUpTime),
            "overtake_target_time" => Order(OrderVar::OvertakeTargetTime),

            // Distance to other runners
            "bashin_diff_behind" => Distance(DistanceVar::BashinDiffBehind),
            "bashin_diff_infront" => Distance(DistanceVar::BashinDiffInfront),
            "distance_diff_rate" => Distance(DistanceVar::DistanceDiffRate),
            "distance_diff_top_float" => Distance(DistanceVar::DistanceDiffTopFloat),
            "distance_diff_top" => Distance(DistanceVar::DistanceDiffTop),

            // Time
            "accumulatetime" => AccumulateTime,

            // Phase
            "phase" => Phase(PhaseVar::Phase),
            "phase_corner_random" => Phase(PhaseVar::PhaseCornerRandom),
            "phase_first_half_straight_random" => Phase(PhaseVar::PhaseFirstHalfStraightRandom),
            "phase_firsthalf" => Phase(PhaseVar::PhaseFirsthalf),
            "phase_firsthalf_random" => Phase(PhaseVar::PhaseFirsthalfRandom),
            "phase_firstquarter" => Phase(PhaseVar::PhaseFirstquarter),
            "phase_firstquarter_random" => Phase(PhaseVar::PhaseFirstquarterRandom),
            "phase_laterhalf" => Phase(PhaseVar::PhaseLaterhalf),
            "phase_laterhalf_random" => Phase(PhaseVar::PhaseLaterhalfRandom),
            "phase_latter_half_straight_random" => Phase(PhaseVar::PhaseLatterHalfStraightRandom),
            "phase_random" => Phase(PhaseVar::PhaseRandom),
            "phase_straight_random" => Phase(PhaseVar::PhaseStraightRandom),

            // Course
            "corner_count" => Course(CourseVar::CornerCount),
            "course_distance" => Course(CourseVar::CourseDistance),
            "distance_type" => Course(CourseVar::DistanceType),
            "ground_type" => Course(CourseVar::GroundType),
            "is_basis_distance" => Course(CourseVar::IsBasisDistance),
            "is_dirtgrade" => Course(CourseVar::IsDirtgrade),
            "rotation" => Course(CourseVar::Rotation),
            "track_id" => Course(CourseVar::TrackId),

            // Spatial
            "all_corner_random" => Spatial(SpatialVar::AllCornerRandom),
            "corner" => Spatial(SpatialVar::Corner),
            "corner_random" => Spatial(SpatialVar::CornerRandom),
            "distance_rate" => Spatial(SpatialVar::DistanceRate),
            "distance_rate_after_random" => Spatial(SpatialVar::DistanceRateAfterRandom),
            "down_slope_random" => Spatial(SpatialVar::DownSlopeRandom),
            "furlong" => Spatial(SpatialVar::Furlong),
            "is_finalcorner" => Spatial(SpatialVar::IsFinalcorner),
            "is_finalcorner_laterhalf" => Spatial(SpatialVar::IsFinalcornerLaterhalf),
            "is_finalcorner_random" => Spatial(SpatialVar::IsFinalcornerRandom),
            "is_last_straight" => Spatial(SpatialVar::IsLastStraight),
            "is_last_straight_onetime" => Spatial(SpatialVar::IsLastStraightOnetime),
            "lane_type" => Spatial(SpatialVar::LaneType),
            "last_straight_random" => Spatial(SpatialVar::LastStraightRandom),
            "remain_distance" => Spatial(SpatialVar::RemainDistance),
            "remain_distance_viewer_id" => Spatial(SpatialVar::RemainDistanceViewerId),
            "slope" => Spatial(SpatialVar::Slope),
            "straight_front_type" => Spatial(SpatialVar::StraightFrontType),
            "straight_random" => Spatial(SpatialVar::StraightRandom),
            "up_slope_random" => Spatial(SpatialVar::UpSlopeRandom),
            "up_slope_random_later_half" => Spatial(SpatialVar::UpSlopeRandomLaterHalf),

            // Activation tracking
            "activate_count_all_team" => Activation(ActivationVar::ActivateCountAllTeam),
            "activate_count_all" => Activation(ActivationVar::ActivateCountAll),
            "activate_count_end_after" => Activation(ActivationVar::ActivateCountEndAfter),
            "activate_count_heal" => Activation(ActivationVar::ActivateCountHeal),
            "activate_count_later_half" => Activation(ActivationVar::ActivateCountLaterHalf),
            "activate_count_middle" => Activation(ActivationVar::ActivateCountMiddle),
            "activate_count_start" => Activation(ActivationVar::ActivateCountStart),
            "compete_fight_count" => Activation(ActivationVar::CompeteFightCount),
            "is_activate_any_skill" => Activation(ActivationVar::IsActivateAnySkill),
            "is_activate_heal_skill" => Activation(ActivationVar::IsActivateHealSkill),
            "is_activate_other_skill_detail" => {
                Activation(ActivationVar::IsActivateOtherSkillDetail)
            }
            "is_exist_skill_id" => Activation(ActivationVar::IsExistSkillId),
            "is_other_character_activate_advantage_skill" => {
                Activation(ActivationVar::IsOtherCharacterActivateAdvantageSkill)
            }
            "is_used_skill_id" => Activation(ActivationVar::IsUsedSkillId),

            // Race parameters
            "grade" => RaceParam(RaceParamVar::Grade),
            "ground_condition" => RaceParam(RaceParamVar::GroundCondition),
            "is_exist_chara_id" => RaceParam(RaceParamVar::IsExistCharaId),
            "popularity" => RaceParam(RaceParamVar::Popularity),
            "same_skill_horse_count" => RaceParam(RaceParamVar::SameSkillHorseCount),
            "season" => RaceParam(RaceParamVar::Season),
            "time" => RaceParam(RaceParamVar::Time),
            "visiblehorse" => RaceParam(RaceParamVar::Visiblehorse),
            "weather" => RaceParam(RaceParamVar::Weather),

            // Running style
            "running_style" => RunningStyle(RunningStyleVar::RunningStyle),
            "running_style_count_nige_otherself" => {
                RunningStyle(RunningStyleVar::RunningStyleCountNigeOtherself)
            }
            "running_style_count_oikomi_otherself" => {
                RunningStyle(RunningStyleVar::RunningStyleCountOikomiOtherself)
            }
            "running_style_count_sashi_otherself" => {
                RunningStyle(RunningStyleVar::RunningStyleCountSashiOtherself)
            }
            "running_style_count_senko_otherself" => {
                RunningStyle(RunningStyleVar::RunningStyleCountSenkoOtherself)
            }
            "running_style_count_same" => RunningStyle(RunningStyleVar::RunningStyleCountSame),
            "running_style_count_same_rate" => {
                RunningStyle(RunningStyleVar::RunningStyleCountSameRate)
            }
            "running_style_equal_popularity_one" => {
                RunningStyle(RunningStyleVar::RunningStyleEqualPopularityOne)
            }

            // Status
            "behind_near_lane_time" => Status(StatusVar::BehindNearLaneTime),
            "behind_near_lane_time_set1" => Status(StatusVar::BehindNearLaneTimeSet1),
            "hp_per" => Status(StatusVar::HpPer),
            "infront_near_lane_time" => Status(StatusVar::InfrontNearLaneTime),
            "is_badstart" => Status(StatusVar::IsBadstart),
            "is_behind_in" => Status(StatusVar::IsBehindIn),
            "is_hp_empty_onetime" => Status(StatusVar::IsHpEmptyOnetime),
            "is_lastspurt" => Status(StatusVar::IsLastspurt),
            "is_move_lane" => Status(StatusVar::IsMoveLane),
            "is_overtake" => Status(StatusVar::IsOvertake),
            "is_surrounded" => Status(StatusVar::IsSurrounded),
            "is_temptation" => Status(StatusVar::IsTemptation),
            "lastspurt" => Status(StatusVar::Lastspurt),
            "motivation" => Status(StatusVar::Motivation),
            "post_number" => Status(StatusVar::PostNumber),
            "temptation_count" => Status(StatusVar::TemptationCount),

            // Randoms
            "random_lot" => Random(RandomVar::RandomLot),

            // Blocking
            "blocked_all_continuetime" => Blocking(BlockingVar::BlockedAllContinuetime),
            "blocked_front" => Blocking(BlockingVar::BlockedFront),
            "blocked_front_continuetime" => Blocking(BlockingVar::BlockedFrontContinuetime),
            "blocked_side_continuetime" => Blocking(BlockingVar::BlockedSideContinuetime),

            // Rush/Temptation
            "running_style_temptation_count_nige" => {
                Temptation(TemptationVar::RunningStyleTemptationCountNige)
            }
            "running_style_temptation_count_oikomi" => {
                Temptation(TemptationVar::RunningStyleTemptationCountOikomi)
            }
            "running_style_temptation_count_sashi" => {
                Temptation(TemptationVar::RunningStyleTemptationCountSashi)
            }
            "running_style_temptation_count_senko" => {
                Temptation(TemptationVar::RunningStyleTemptationCountSenko)
            }
            "running_style_temptation_opponent_count_nige" => {
                Temptation(TemptationVar::RunningStyleTemptationOpponentCountNige)
            }
            "running_style_temptation_opponent_count_oikomi" => {
                Temptation(TemptationVar::RunningStyleTemptationOpponentCountOikomi)
            }
            "running_style_temptation_opponent_count_sashi" => {
                Temptation(TemptationVar::RunningStyleTemptationOpponentCountSashi)
            }
            "running_style_temptation_opponent_count_senko" => {
                Temptation(TemptationVar::RunningStyleTemptationOpponentCountSenko)
            }
            "temptation_count_behind" => Temptation(TemptationVar::TemptationCountBehind),
            "temptation_count_infront" => Temptation(TemptationVar::TemptationCountInfront),
            "temptation_opponent_count_behind" => {
                Temptation(TemptationVar::TemptationOpponentCountBehind)
            }
            "temptation_opponent_count_infront" => {
                Temptation(TemptationVar::TemptationOpponentCountInfront)
            }

            // Special
            "always" => Always,

            // Unknown variable - fallback
            _ => {
                #[cfg(debug_assertions)]
                eprintln!("warning: unknown variable '{}', evaluating to false", s);
                Unknown(s.to_string())
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
            Spatial(SpatialVar::RemainDistanceViewerId) => MonotonicGuard,

            // HP-based (decreasing)
            Status(StatusVar::HpPer) => MonotonicGuard,

            // HP-based one-time event (increasing; from 0 to 1)
            Status(StatusVar::IsHpEmptyOnetime) => MonotonicGuard,

            // Temptation count (increasing)
            Status(StatusVar::TemptationCount) => MonotonicGuard,

            // Activation counts (increasing)
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

            // ACTIVATION TRIGGERS - Fire when skill is activated
            Activation(ActivationVar::IsActivateAnySkill) => ActivationTrigger,
            Activation(ActivationVar::IsActivateHealSkill) => ActivationTrigger,

            // STATIC GUARDS - Never change during race
            // Race parameters
            RaceParam(_) => StaticGuard,

            // Course properties
            Course(_) => StaticGuard,

            // Base stats
            BaseStat(_) => StaticGuard,

            // Static status properties
            Status(StatusVar::Motivation) => StaticGuard,
            Status(StatusVar::PostNumber) => StaticGuard,
            Status(StatusVar::IsBadstart) => StaticGuard,

            // Running style
            // N.B. While running style can technically change, it can only do so
            // from Front Runner to Runaway. Since both styles are considered
            // the same for skill purposes, we can treat this type as static
            RunningStyle(_) => StaticGuard,

            // Special value - By definition, it never changes
            Always => StaticGuard,

            // DYNAMIC GUARDS - Can change during race
            // Status flags
            Status(StatusVar::IsLastspurt) => DynamicGuard,
            Status(StatusVar::Lastspurt) => DynamicGuard,
            Status(StatusVar::IsTemptation) => DynamicGuard,
            Status(StatusVar::IsOvertake) => DynamicGuard,
            Status(StatusVar::IsMoveLane) => DynamicGuard,
            Status(StatusVar::IsBehindIn) => DynamicGuard,
            Status(StatusVar::IsSurrounded) => DynamicGuard,
            Status(StatusVar::BehindNearLaneTime) => DynamicGuard,
            Status(StatusVar::BehindNearLaneTimeSet1) => DynamicGuard,
            Status(StatusVar::InfrontNearLaneTime) => DynamicGuard,

            // Skill activation tracking
            Activation(ActivationVar::IsActivateOtherSkillDetail) => DynamicGuard,
            Activation(ActivationVar::IsOtherCharacterActivateAdvantageSkill) => DynamicGuard,

            // Order/Rank
            Order(_) => DynamicGuard,

            // Distance between runners
            Distance(_) => DynamicGuard,

            // Blocking
            Blocking(_) => DynamicGuard,

            // Temptation/Rush
            Temptation(_) => DynamicGuard,

            // Lane
            Spatial(SpatialVar::LaneType) => DynamicGuard,

            // Unknown
            Unknown(_) => DynamicGuard,
        }
    }
}
