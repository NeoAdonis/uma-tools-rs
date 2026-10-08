use std::fmt;

/// Variable types used in conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarType {
    /// Variable representing accumulated (elapsed) time.
    AccumulateTime,
    /// Variable representing base stats.
    BaseStat(StatKind),
    /// Variables related to the runner's entry in the race (gate, popularity,
    /// etc.).
    Entry(EntryVar),
    /// Variables related to order/rank.
    Order(OrderVar),
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
    /// Variables related to skill activation tracking.
    Activation(ActivationVar),
    /// Variables related to race parameters.
    RaceParam(RaceParamVar),
    /// Variables related to the lineup (every runner entered in the race).
    Lineup(LineupVar),
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
    /// Special value that is always true.
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
pub enum EntryVar {
    FanCount,
    Motivation,
    Popularity,
    PostNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderVar {
    ChangeOrderOnetime,
    ChangeOrderUpEndAfter,
    ChangeOrderUpFinalcornerAfter,
    ChangeOrderUpMiddle,
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
    PhaseFirsthalf,
    PhaseFirstquarter,
    PhaseLaterhalf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomSpatialVar {
    AllCornerRandom,
    CornerRandom,
    DistanceRateAfterRandom,
    DownSlopeRandom,
    IsFinalcornerRandom,
    LastStraightRandom,
    /// Random point after Full-Throttle Sprint starts. Rolled during the race.
    RunAtFullSpeedRandom,
    StraightRandom,
    UpSlopeRandom,
    UpSlopeRandomLaterHalf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    IsOtherCharacterActivateAdvantageSkill,
    IsUsedSkillId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceParamVar {
    Grade,
    GroundCondition,
    Season,
    Time,
    Weather,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineupVar {
    IsExistCharaId,
    IsExistSkillId,
    SameSkillHorseCount,
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
    HpPer,
    IsBadstart,
    IsHpEmptyOnetime,
    IsLastspurt,
    IsMoveLane,
    IsOvertake,
    IsTemptation,
    Lastspurt,
    TemptationCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomVar {
    /// Lottery rolled every time it's evaluated.
    RandomLot,
    /// Lottery rolled once per skill, shared by all its alternatives.
    RandomLotShared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            Entry(e) => write!(f, "{}", e),
            Order(o) => write!(f, "{}", o),
            Distance(d) => write!(f, "{}", d),
            Phase(p) => write!(f, "{}", p),
            Spatial(l) => write!(f, "{}", l),
            Course(c) => write!(f, "{}", c),
            Activation(a) => write!(f, "{}", a),
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

// Implement Display for other enums (using snake_case of variant name).
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

impl_display_snake_case!(EntryVar);
impl_display_snake_case!(OrderVar);
impl_display_snake_case!(DistanceVar);
impl_display_snake_case!(PhaseVar);
impl_display_snake_case!(RandomPhaseVar);
impl_display_snake_case!(SpatialVar);
impl_display_snake_case!(RandomSpatialVar);
impl_display_snake_case!(CourseVar);
impl_display_snake_case!(ActivationVar);
impl_display_snake_case!(RaceParamVar);
impl_display_snake_case!(LineupVar);
impl_display_snake_case!(RunningStyleVar);
impl_display_snake_case!(StatusVar);
impl_display_snake_case!(RandomVar);
impl_display_snake_case!(BlockingVar);
impl_display_snake_case!(TemptationVar);

impl VarType {
    /// Parse a variable name string into a [`VarType`].
    pub fn parse(s: &str) -> Self {
        use VarType::*;

        match s {
            // Base stats.
            "base_speed" => BaseStat(StatKind::Speed),
            "base_stamina" => BaseStat(StatKind::Stamina),
            "base_power" => BaseStat(StatKind::Power),
            "base_guts" => BaseStat(StatKind::Guts),
            "base_wiz" => BaseStat(StatKind::Wiz),

            // Runner entry.
            "fan_count" => Entry(EntryVar::FanCount),
            "motivation" => Entry(EntryVar::Motivation),
            "popularity" => Entry(EntryVar::Popularity),
            "post_number" => Entry(EntryVar::PostNumber),

            // Order/Rank.
            "change_order_onetime" => Order(OrderVar::ChangeOrderOnetime),
            "change_order_up_end_after" => Order(OrderVar::ChangeOrderUpEndAfter),
            "change_order_up_finalcorner_after" => Order(OrderVar::ChangeOrderUpFinalcornerAfter),
            "change_order_up_middle" => Order(OrderVar::ChangeOrderUpMiddle),
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

            // Distance to other runners.
            "bashin_diff_behind" => Distance(DistanceVar::BashinDiffBehind),
            "bashin_diff_infront" => Distance(DistanceVar::BashinDiffInfront),
            "distance_diff_rate" => Distance(DistanceVar::DistanceDiffRate),
            "distance_diff_top_float" => Distance(DistanceVar::DistanceDiffTopFloat),
            "distance_diff_top" => Distance(DistanceVar::DistanceDiffTop),

            // Time.
            "accumulatetime" => AccumulateTime,

            // Phase.
            "phase" => Phase(PhaseVar::Phase),
            "phase_firsthalf" => Phase(PhaseVar::PhaseFirsthalf),
            "phase_firstquarter" => Phase(PhaseVar::PhaseFirstquarter),
            "phase_laterhalf" => Phase(PhaseVar::PhaseLaterhalf),

            // Random Phase.
            "phase_corner_random" => RandomPhase(RandomPhaseVar::PhaseCornerRandom),
            "phase_first_half_straight_random" => {
                RandomPhase(RandomPhaseVar::PhaseFirstHalfStraightRandom)
            }
            "phase_firsthalf_random" => RandomPhase(RandomPhaseVar::PhaseFirsthalfRandom),
            "phase_firstquarter_random" => RandomPhase(RandomPhaseVar::PhaseFirstquarterRandom),
            "phase_laterhalf_random" => RandomPhase(RandomPhaseVar::PhaseLaterhalfRandom),
            "phase_latter_half_straight_random" => {
                RandomPhase(RandomPhaseVar::PhaseLatterHalfStraightRandom)
            }
            "phase_random" => RandomPhase(RandomPhaseVar::PhaseRandom),
            "phase_straight_random" => RandomPhase(RandomPhaseVar::PhaseStraightRandom),

            // Course.
            "corner_count" => Course(CourseVar::CornerCount),
            "course_distance" => Course(CourseVar::CourseDistance),
            "distance_type" => Course(CourseVar::DistanceType),
            "ground_type" => Course(CourseVar::GroundType),
            "is_abroad" => Course(CourseVar::IsAbroad),
            "is_basis_distance" => Course(CourseVar::IsBasisDistance),
            "is_dirtgrade" => Course(CourseVar::IsDirtgrade),
            "is_tight_track" => Course(CourseVar::IsTightTrack),
            "rotation" => Course(CourseVar::Rotation),
            "track_id" => Course(CourseVar::TrackId),

            // Spatial.
            "corner" => Spatial(SpatialVar::Corner),
            "distance_rate" => Spatial(SpatialVar::DistanceRate),
            "furlong" => Spatial(SpatialVar::Furlong),
            "is_finalcorner" => Spatial(SpatialVar::IsFinalcorner),
            "is_finalcorner_laterhalf" => Spatial(SpatialVar::IsFinalcornerLaterhalf),
            "is_last_straight" => Spatial(SpatialVar::IsLastStraight),
            "is_last_straight_onetime" => Spatial(SpatialVar::IsLastStraightOnetime),
            "lane_type" => Spatial(SpatialVar::LaneType),
            "remain_distance" => Spatial(SpatialVar::RemainDistance),
            "remain_distance_viewer_id" => Spatial(SpatialVar::RemainDistanceViewerId),
            "slope" => Spatial(SpatialVar::Slope),
            "straight_front_type" => Spatial(SpatialVar::StraightFrontType),

            // Random Spatial.
            "all_corner_random" => RandomSpatial(RandomSpatialVar::AllCornerRandom),
            "corner_random" => RandomSpatial(RandomSpatialVar::CornerRandom),
            "distance_rate_after_random" => {
                RandomSpatial(RandomSpatialVar::DistanceRateAfterRandom)
            }
            "down_slope_random" => RandomSpatial(RandomSpatialVar::DownSlopeRandom),
            "is_finalcorner_random" => RandomSpatial(RandomSpatialVar::IsFinalcornerRandom),
            "last_straight_random" => RandomSpatial(RandomSpatialVar::LastStraightRandom),
            "run_at_full_speed_random" => RandomSpatial(RandomSpatialVar::RunAtFullSpeedRandom),
            "straight_random" => RandomSpatial(RandomSpatialVar::StraightRandom),
            "up_slope_random" => RandomSpatial(RandomSpatialVar::UpSlopeRandom),
            "up_slope_random_later_half" => RandomSpatial(RandomSpatialVar::UpSlopeRandomLaterHalf),

            // Activation tracking.
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
            "is_other_character_activate_advantage_skill" => {
                Activation(ActivationVar::IsOtherCharacterActivateAdvantageSkill)
            }
            "is_used_skill_id" => Activation(ActivationVar::IsUsedSkillId),

            // Race parameters.
            "grade" => RaceParam(RaceParamVar::Grade),
            "ground_condition" => RaceParam(RaceParamVar::GroundCondition),
            "season" => RaceParam(RaceParamVar::Season),
            "time" => RaceParam(RaceParamVar::Time),
            "weather" => RaceParam(RaceParamVar::Weather),

            // Lineup.
            "is_exist_chara_id" => Lineup(LineupVar::IsExistCharaId),
            "is_exist_skill_id" => Lineup(LineupVar::IsExistSkillId),
            "same_skill_horse_count" => Lineup(LineupVar::SameSkillHorseCount),

            // Running style.
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

            // Status.
            "hp_per" => Status(StatusVar::HpPer),
            "is_badstart" => Status(StatusVar::IsBadstart),
            "is_hp_empty_onetime" => Status(StatusVar::IsHpEmptyOnetime),
            "is_lastspurt" => Status(StatusVar::IsLastspurt),
            "is_move_lane" => Status(StatusVar::IsMoveLane),
            "is_overtake" => Status(StatusVar::IsOvertake),
            "is_temptation" => Status(StatusVar::IsTemptation),
            "lastspurt" => Status(StatusVar::Lastspurt),
            "temptation_count" => Status(StatusVar::TemptationCount),

            // Randoms.
            "random_lot" => Random(RandomVar::RandomLot),
            "random_lot_shared" => Random(RandomVar::RandomLotShared),

            // Blocking/Surrounding.
            "behind_near_lane_time_set1" => Blocking(BlockingVar::BehindNearLaneTimeSet1),
            "behind_near_lane_time" => Blocking(BlockingVar::BehindNearLaneTime),
            "blocked_all_continuetime" => Blocking(BlockingVar::BlockedAllContinuetime),
            "blocked_front_continuetime" => Blocking(BlockingVar::BlockedFrontContinuetime),
            "blocked_front" => Blocking(BlockingVar::BlockedFront),
            "blocked_side_continuetime" => Blocking(BlockingVar::BlockedSideContinuetime),
            "infront_near_lane_time" => Blocking(BlockingVar::InfrontNearLaneTime),
            "is_behind_in" => Blocking(BlockingVar::IsBehindIn),
            "is_surrounded" => Blocking(BlockingVar::IsSurrounded),
            "near_count" => Blocking(BlockingVar::NearCount),
            "near_infront_count" => Blocking(BlockingVar::NearInfrontCount),
            "overtake_target_no_order_up_time" => {
                Blocking(BlockingVar::OvertakeTargetNoOrderUpTime)
            }
            "overtake_target_time" => Blocking(BlockingVar::OvertakeTargetTime),
            "visiblehorse" => Blocking(BlockingVar::Visiblehorse),

            // Rush/Temptation.
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

            // Special.
            "always" => Always,

            // Unknown variable - fallback.
            _ => {
                #[cfg(debug_assertions)]
                eprintln!("warning: unknown variable '{}', evaluating to false", s);
                Unknown(s.to_string())
            }
        }
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
            Status(StatusVar::IsBadstart) => true,
            // N.B. While running style can technically change, it can only do so from Front
            // Runner to Runaway. Since both styles are considered the same for skill
            // purposes, we can treat this type as static.
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
    fn test_known_variables_parse() {
        // Test a sample of known variables.
        assert!(matches!(
            VarType::parse("base_speed"),
            VarType::BaseStat(StatKind::Speed)
        ));
        assert!(matches!(
            VarType::parse("accumulatetime"),
            VarType::AccumulateTime
        ));
        assert!(matches!(
            VarType::parse("order"),
            VarType::Order(OrderVar::Order)
        ));
        assert!(matches!(
            VarType::parse("bashin_diff_behind"),
            VarType::Distance(DistanceVar::BashinDiffBehind)
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
            VarType::Random(RandomVar::RandomLot)
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
