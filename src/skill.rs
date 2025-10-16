pub enum SkillType {
    SpeedUp = 1,
    StaminaUp = 2,
    PowerUp = 3,
    GutsUp = 4,
    WitUp = 5,
    Recovery = 9,
    MultiplyStartDelay = 10,
    SetStartDelay = 14,
    CurrentSpeed = 21,
    CurrentSpeedWithNaturalDeceleration = 22,
    TargetSpeed = 27,
    Acceleration = 31,
    ActivateRandomGold = 37,
    ExtendEvolvedDuration = 42,
}

pub enum SkillRarity {
    White = 1,
    Gold,
    Unique, // for some reason 1*/2* uniques, 1*/2* upgraded to 3*, and naturally 3* uniques all have different rarity (3, 4, 5 respectively)
    Evolution = 6,
}

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

pub enum Perspective {
    Oneself = 1,
    Other,
    Any,
}

pub struct SkillEffect {
    pub skill_type: SkillType,
    pub target: SkillTarget,
    pub modifier: f64,
}

pub struct Skill {
    pub rarity: SkillRarity,
    pub effects: Vec<SkillEffect>,
    pub perspective: Perspective,
    pub modifier: f64,
    pub duration: f64,
    pub cooldown: f64,
    pub condition: String,
    pub precondition: String,
}
