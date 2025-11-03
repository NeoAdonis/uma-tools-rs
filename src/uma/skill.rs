use serde::Deserialize;
use serde_repr::Deserialize_repr;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum SkillType {
    SpeedUp = 1,
    StaminaUp = 2,
    PowerUp = 3,
    GutsUp = 4,
    WizUp = 5,
    ChangeStyleOonige = 6,
    Fov = 8,
    Recovery = 9,
    MultiplyStartDelay = 10,
    RushTime = 13,
    SetStartDelay = 14,
    CurrentSpeed = 21,
    CurrentSpeedWithNaturalDeceleration = 22,
    TargetSpeed = 27,
    LaneMovementSpeed = 28,
    RushChance = 29,
    Acceleration = 31,
    AllStatsUp = 32,
    LaneChange = 35,
    ActivateRandomGold = 37,
    ActivateSympathy = 41,
    ExtendEvolvedDuration = 42,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum SkillRarity {
    White = 1,
    Gold = 2,
    Unique = 3,         // 1/2 stars unique
    UniqueUpgraded = 4, // 3+ stars unique upgraded from 1/2 stars unique
    UniqueNatural = 5,  // 3+ stars unique with no 1/2 star equivalent
    Evolution = 6,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum SkillTarget {
    Oneself = 1,
    All = 2,
    InFov = 4,
    AheadOfPosition = 7,
    AheadOfSelf = 9,
    BehindSelf = 10,
    AllAllies = 11,
    EnemyStrategy = 18,
    RushedAhead = 19,
    RushedBehind = 20,
    RushedStrategy = 21,
    UmaId = 22,
    UsedRecovery = 23,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum Perspective {
    Oneself = 1,
    Other = 2,
    Any = 3,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEffect {
    #[serde(rename = "type")]
    pub skill_type: SkillType,
    pub target: SkillTarget,
    pub modifier: i32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAlternative {
    pub base_duration: i32,
    pub condition: String,
    pub effects: Vec<SkillEffect>,
    pub precondition: String,
}

#[derive(Clone, Debug)]
pub struct Skill {
    pub id: String,
    data: SkillData,
}

impl std::ops::Deref for Skill {
    type Target = SkillData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillData {
    pub rarity: SkillRarity,
    pub alternatives: Vec<SkillAlternative>,
}

impl Skill {
    /// Create a new Skill from ID and SkillData
    pub fn new(id: String, data: SkillData) -> Self {
        Skill { id, data }
    }

    /// Check if this skill is a "green" skill (stat boosting)
    pub fn is_green(self: &Skill) -> bool {
        for alternative in &self.alternatives {
            for effect in &alternative.effects {
                match effect.skill_type {
                    SkillType::SpeedUp
                    | SkillType::StaminaUp
                    | SkillType::PowerUp
                    | SkillType::GutsUp
                    | SkillType::WizUp
                    | SkillType::AllStatsUp => {
                        return true;
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// Check if this skill is a unique skill (i.e. only available to an specific Uma)
    pub fn is_unique(self: &Skill) -> bool {
        matches!(
            self.rarity,
            SkillRarity::Unique | SkillRarity::UniqueUpgraded | SkillRarity::UniqueNatural
        )
    }

    /// Check if this skill should activate out of gate (before race begins)
    pub fn should_activate_out_of_gate(&self) -> bool {
        // Green skills (stat boosts) always activate at start
        if self.is_green() {
            return true;
        }

        // Unique skills always activate at start
        if self.is_unique() {
            return true;
        }

        // Skills with always==1 condition activate at start
        if let Some(alt) = self.alternatives.first() {
            if alt.condition == "always==1" {
                return true;
            }

            // Start delay modification skills activate at start
            if alt.effects.iter().any(|ef| {
                matches!(
                    ef.skill_type,
                    SkillType::MultiplyStartDelay | SkillType::SetStartDelay
                )
            }) {
                return true;
            }
        }

        false
    }

    /// Load all skills from a JSON file into a HashMap
    ///
    /// # Arguments
    /// * `json_path` - Path to the JSON file containing skill data
    ///
    /// # Returns
    /// * `Ok(HashMap<String, Skill>)` - A map of skill IDs to Skill objects
    /// * `Err` - If the file can't be read or JSON is invalid
    pub fn load_skills_from_file(
        json_path: &str,
    ) -> Result<HashMap<String, Skill>, Box<dyn std::error::Error>> {
        use std::fs;

        let json_content = fs::read_to_string(json_path)?;
        let skills_data: HashMap<String, SkillData> = serde_json::from_str(&json_content)?;

        let mut skills_map = HashMap::new();
        for (id_str, data) in skills_data {
            skills_map.insert(id_str.clone(), Skill::new(id_str, data));
        }

        Ok(skills_map)
    }
}

/// Check if an effect type is a stat boost (applied permanently)
pub fn is_stat_boost(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::SpeedUp
            | SkillType::StaminaUp
            | SkillType::PowerUp
            | SkillType::GutsUp
            | SkillType::WizUp
            | SkillType::AllStatsUp
    )
}

/// Check if an effect type is a start delay modification
pub fn is_start_delay_modification(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::MultiplyStartDelay | SkillType::SetStartDelay
    )
}

/// Check if an effect type is a style change
pub fn is_style_change(effect_type: &SkillType) -> bool {
    matches!(effect_type, SkillType::ChangeStyleOonige)
}

/// Check if an effect type is a timed effect
pub fn is_timed_effect(effect_type: &SkillType) -> bool {
    matches!(
        effect_type,
        SkillType::Acceleration
            | SkillType::TargetSpeed
            | SkillType::CurrentSpeed
            | SkillType::CurrentSpeedWithNaturalDeceleration
            | SkillType::ExtendEvolvedDuration
    )
}
