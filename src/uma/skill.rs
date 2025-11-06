use serde::Deserialize;
use serde_repr::Deserialize_repr;
use std::collections::HashMap;

/// Represents the type of skill effect.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum SkillType {
    /// Modify speed stat.
    SpeedUp = 1,
    /// Modify stamina stat.
    StaminaUp = 2,
    /// Modify power stat.
    PowerUp = 3,
    /// Modify guts stat.
    GutsUp = 4,
    /// Modify wisdom stat.
    WizUp = 5,
    /// Change running style to Runaway.
    ChangeStyleRunaway = 6,
    /// Modify field of view.
    Fov = 8,
    /// Recover (heal) some HP.
    Recovery = 9,
    /// Multiply start delay.
    MultiplyStartDelay = 10,
    /// Modify how long a runner can stay rushed.
    RushTimeUp = 13,
    /// Set start delay to a fixed value.
    SetStartDelay = 14,
    /// Modify current speed directly.
    CurrentSpeed = 21,
    /// Modify current speed with natural deceleration.
    CurrentSpeedWithNaturalDeceleration = 22,
    /// Modify target speed.
    TargetSpeed = 27,
    /// Modify lane movement speed.
    LaneMovementSpeed = 28,
    /// Modify chance of becoming rushed.
    RushChanceUp = 29,
    /// Modify acceleration.
    Acceleration = 31,
    /// Modify all stats at once.
    AllStatsUp = 32,
    /// Change target lane.
    LaneChange = 35,
    /// Activate a random gold skill.
    ActivateRandomGold = 37,
    /// Activate "Sympathy" skill.
    ActivateSympathy = 41,
    /// Extend evolved skill duration.
    ExtendEvolvedDuration = 42,
}

/// Represents the rarity of a skill.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum SkillRarity {
    /// Common skill.
    White = 1,
    /// Non-unique gold skill; stronger than white skills.
    Gold = 2,
    /// Skill unique to a specific Uma (1/2 stars).
    Unique = 3,
    /// Skill unique to a specific Uma (3+ stars) after upgrade from 1/2 star unique.
    UniqueUpgraded = 4,
    /// Skill unique to a specific Uma (3+ stars) with no 1/2 star equivalent.
    UniqueNatural = 5,
    /// Skills evolved from gold skills.
    Evolution = 6,
}

/// Represents the target of a skill effect.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize_repr)]
#[repr(u8)]
pub enum SkillTarget {
    /// Skill affects oneself.
    Oneself = 1,
    /// Skill affects all runners.
    All = 2,
    /// Skill affects only runners inside field of view.
    InFov = 4,
    /// Skill affects only the next 4 runners ahead.
    AheadOfPosition = 7,
    /// Skill affects only the next 2 runners ahead.
    AheadOfSelf = 9,
    /// Skill affects only the previous 2 runners behind.
    BehindSelf = 10,
    /// Skill affects all team allies.
    AllAllies = 11,
    /// Skill affects all opponents with a specific running style.
    EnemyStrategy = 18,
    /// Skill affects runners ahead that are rushed.
    RushedAhead = 19,
    /// Skill affects runners behind that are rushed.
    RushedBehind = 20,
    /// Skill affects runners with a specific running style that are rushed.
    RushedStrategy = 21,
    /// (Unknown)
    UmaId = 22,
    /// Skill affects runners that have used recovery skills.
    UsedRecovery = 23,
}

/// Represents a single effect of a skill.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEffect {
    #[serde(rename = "type")]
    pub skill_type: SkillType,
    pub target: SkillTarget,
    pub modifier: i32,
}

/// Represents an alternative version of a skill, with its own conditions and effects.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillAlternative {
    pub base_duration: i32,
    pub condition: String,
    pub effects: Vec<SkillEffect>,
    pub precondition: String,
}

/// Represents a skill with its ID and data.
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

/// Represents the data of a skill.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillData {
    pub rarity: SkillRarity,
    pub alternatives: Vec<SkillAlternative>,
}

impl Skill {
    /// Create a new Skill from ID and SkillData.
    pub fn new(id: String, data: SkillData) -> Self {
        Skill { id, data }
    }

    /// Check if this skill is a "green" (stat boosting) skill.
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

    /// Determine whether a skill effect is positive or negative.
    fn is_effect_positive(effect: &SkillEffect) -> bool {
        match effect.skill_type {
            // Purely positive effects
            SkillType::ActivateRandomGold
            | SkillType::ActivateSympathy
            | SkillType::ChangeStyleRunaway => true,
            // When a start delay is set, it's always to a bad start
            SkillType::SetStartDelay => false,
            // Multiplier of less than or equal to 1.0 is a reduction in delay
            SkillType::MultiplyStartDelay => effect.modifier <= 10000,
            // Less chance or shorter rush time is good
            SkillType::RushChanceUp | SkillType::RushTimeUp => effect.modifier <= 0,
            // Positive modifier is good for other effects
            _ => effect.modifier >= 0,
        }
    }

    /// Check if a skill affects you positively (buff).
    ///
    /// N.B. Because a skill can have both positive and negative effects,
    /// a skill can be both a buff and a nerf.
    pub fn is_buff(self: &Skill) -> bool {
        for alternative in &self.alternatives {
            for effect in &alternative.effects {
                if effect.target == SkillTarget::Oneself && Skill::is_effect_positive(effect) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a skill affects you negatively (nerf).
    ///
    /// N.B. Because a skill can have both positive and negative effects,
    /// a skill can be both a buff and a nerf.
    pub fn is_nerf(self: &Skill) -> bool {
        for alternative in &self.alternatives {
            for effect in &alternative.effects {
                if effect.target == SkillTarget::Oneself && !Skill::is_effect_positive(effect) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a skill affects others negatively (debuff).
    pub fn is_debuff(self: &Skill) -> bool {
        for alternative in &self.alternatives {
            for effect in &alternative.effects {
                if effect.target != SkillTarget::Oneself && !Skill::is_effect_positive(effect) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if this skill is a unique skill (i.e. only available to a specific Uma).
    pub fn is_unique(self: &Skill) -> bool {
        matches!(
            self.rarity,
            SkillRarity::Unique | SkillRarity::UniqueUpgraded | SkillRarity::UniqueNatural
        )
    }

    /// Check if this skill should activate out of gate (before race begins).
    pub fn should_activate_out_of_gate(&self) -> bool {
        // Green skills (stat boosts) always activate at start
        if self.is_green() {
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

    /// Load all skills from a JSON file into a HashMap.
    ///
    /// # Arguments
    ///
    /// - `json_path` - Path to the JSON file containing skill data.
    ///
    /// # Returns
    ///
    /// - `Ok(HashMap<String, Skill>)` - A map of skill IDs to Skill objects.
    /// - `Err` - If the file can't be read or JSON is invalid.
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
