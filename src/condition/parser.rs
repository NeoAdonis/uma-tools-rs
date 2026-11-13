//! # Condition Parser
//!
//! This module parses skill condition strings, such as
//! `distance_type==1&ground_type==1@distance_type==2&ground_type==1`.
//!
//! ## Syntax
//!
//! A condition is made of comparisons, each one consisting of a variable, an
//! operator ([`CompOp`]) and an integer value. For example, `accumulatetime>=5`
//! is true when the race has been ongoing for at least 5 seconds, and
//! `accumulatetime==5` only when it has been ongoing for exactly 5 seconds.
//! The meaning of each variable is described in the
//! [variable reference](crate::condition::variable_reference).
//!
//! Comparisons are linked by `&` (AND) and `@` (OR), with no parentheses. A
//! condition is therefore an OR of AND chains, and it's stored as such in
//! [`Condition::branches`]. The condition above is read as follows (spaces and
//! line breaks added for readability):
//!
//! ```text
//! distance_type==1 & ground_type==1   // Sprint race on turf...
//!                  @                  // ...or...
//! distance_type==2 & ground_type==1   // ...mile race on turf.
//! ```
//!
//! A few variables are sets of values, whose comparisons test membership (e.g.
//! `is_used_skill_id==202051` checks whether skill 202051 has been activated),
//! and some are flags. These variables only support some operators or values;
//! any other comparison is rejected with [`ParseError::UnsupportedComparison`],
//! as is any comparison on a variable that isn't implemented.
//!
//! ## Examples
//!
//! Check a hand-written condition, e.g. for a custom skill, before running
//! races with it:
//!
//! ```
//! use uma_tools::ParseError;
//! use uma_tools::condition::parser::Condition;
//!
//! # fn main() -> Result<(), ParseError> {
//! let condition: Condition = "phase>=2&order<=3@is_overtake==1".parse()?;
//! assert_eq!(condition.branches.len(), 2);
//!
//! let typo = "phase>=2&oder<=3".parse::<Condition>();
//! assert_eq!(typo, Err(ParseError::UnknownVariable("oder".into())));
//! # Ok(())
//! # }
//! ```

use crate::condition::variables::{
    ActivationQueryVar, ActivationVar, LineupVar, RandomSpatialVar, SpatialVar, VarType,
};
use crate::skill::SkillType;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompOp {
    Equal,        // "=="
    NotEqual,     // "!="
    Greater,      // ">"
    GreaterEqual, // ">="
    Less,         // "<"
    LessEqual,    // "<="
}

impl CompOp {
    /// All operators, with the two-character ones first so that the first
    /// operator whose symbol prefixes a string is the right one.
    const ALL: [CompOp; 6] = [
        CompOp::Equal,
        CompOp::NotEqual,
        CompOp::GreaterEqual,
        CompOp::LessEqual,
        CompOp::Greater,
        CompOp::Less,
    ];

    /// Symbol of the operator in condition strings.
    pub fn symbol(self) -> &'static str {
        match self {
            CompOp::Equal => "==",
            CompOp::NotEqual => "!=",
            CompOp::Greater => ">",
            CompOp::GreaterEqual => ">=",
            CompOp::Less => "<",
            CompOp::LessEqual => "<=",
        }
    }
}

impl fmt::Display for CompOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

/// A condition in disjunctive normal form, e.g. `a==1&b>2@c<3`.
///
/// It is true if all comparisons of any of its branches are true.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// OR branches (separated by `@`), each one being the list of comparisons
    /// of an AND chain (separated by `&`), from left to right.
    pub branches: Vec<ComparisonList>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Comparison {
    pub var: VarType,
    pub op: CompOp,
    pub value: i32,
}

/// A flat list of comparisons that must all be true (AND conjunction).
pub type ComparisonList = Vec<Comparison>;

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, branch) in self.branches.iter().enumerate() {
            if i > 0 {
                write!(f, " @ ")?;
            }
            for (j, comp) in branch.iter().enumerate() {
                if j > 0 {
                    write!(f, " & ")?;
                }
                write!(f, "{comp}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Comparison {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.var, self.op, self.value)
    }
}

/// Error parsing a condition, holding the offending comparison or variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParseError {
    /// A comparison lacks a valid operator after the variable name.
    ExpectedOperator(String),
    /// A comparison uses a variable that isn't known.
    UnknownVariable(String),
    /// A comparison is missing a valid value after the operator.
    InvalidValue(String),
    /// A comparison isn't supported by the evaluator (e.g., a set check which
    /// operator isn't `==` or `!=`), or has a known yet unsupported variable.
    UnsupportedComparison(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::ExpectedOperator(comp) => {
                write!(f, "expected comparison operator in '{comp}'")
            }
            ParseError::UnknownVariable(var) => write!(f, "unknown variable '{var}'"),
            ParseError::InvalidValue(comp) => write!(f, "invalid value in '{comp}'"),
            ParseError::UnsupportedComparison(comp) => {
                write!(f, "unsupported comparison '{comp}'")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a single comparison, such as `order<=3`.
fn parse_comparison(input: &str) -> Result<Comparison, ParseError> {
    let expected_operator = || ParseError::ExpectedOperator(input.into());

    let op_pos = input
        .find(['=', '!', '<', '>'])
        .ok_or_else(expected_operator)?;
    let (name, rest) = input.split_at(op_pos);
    let op = CompOp::ALL
        .into_iter()
        .find(|op| rest.starts_with(op.symbol()))
        .ok_or_else(expected_operator)?;

    let name = name.trim();
    let var = VarType::parse(name).ok_or_else(|| ParseError::UnknownVariable(name.into()))?;
    let value = rest[op.symbol().len()..]
        .trim()
        .parse()
        .map_err(|_| ParseError::InvalidValue(input.into()))?;

    let comp = Comparison { var, op, value };
    if !comp.is_supported() {
        return Err(ParseError::UnsupportedComparison(input.into()));
    }
    Ok(comp)
}

impl Comparison {
    /// Whether the comparison is on a random variable, rolled when building the
    /// trigger scheduler (see [`crate::trigger::random`]).
    pub(crate) fn is_random(&self) -> bool {
        use VarType::*;

        matches!(self.var, Random(_) | RandomSpatial(_) | RandomPhase(_))
    }

    /// Whether the comparison is an event: only true at specific points (random
    /// segments, entering the last straight, skill activations), so it's the
    /// trigger of its branch (see [`crate::trigger::skill_activation`]).
    pub(crate) fn is_event(&self) -> bool {
        use VarType::*;

        match (&self.var, self.op, self.value) {
            (RandomSpatial(_) | RandomPhase(_), _, _)
            | (Spatial(SpatialVar::IsLastStraightOnetime), _, _) => true,
            (
                Activation(ActivationVar::IsActivateAnySkill | ActivationVar::IsActivateHealSkill),
                CompOp::Equal,
                1,
            ) => true,
            // The value is the effect type of the advantage skill.
            (
                ActivationQuery(
                    ActivationQueryVar::IsOtherCharacterActivateAdvantageSkill
                    | ActivationQueryVar::IsPopularityTopCharacterActivateAdvantageSkill,
                ),
                CompOp::Equal,
                _,
            ) => true,
            _ => false,
        }
    }

    /// Whether the evaluator can evaluate this comparison.
    ///
    /// Variables that use the comparison value as parameter (e.g. a skill ID),
    /// and flags such as `always`, only support some operators or values.
    fn is_supported(&self) -> bool {
        use VarType::*;
        let is_equal = self.op == CompOp::Equal;
        match self.var {
            ActivationQuery(
                ActivationQueryVar::IsUsedSkillId | ActivationQueryVar::IsUsedSkillIdWithDetailOne,
            )
            | Lineup(LineupVar::IsExistSkillId) => is_equal || self.op == CompOp::NotEqual,
            // The value is the type of the activated buff.
            ActivationQuery(ActivationQueryVar::IsOtherCharacterActivateAdvantageSkill) => {
                is_equal && SkillType::from_id(self.value).is_some()
            }
            // Same, or -1 for any skill.
            ActivationQuery(ActivationQueryVar::IsPopularityTopCharacterActivateAdvantageSkill) => {
                is_equal && (self.value == -1 || SkillType::from_id(self.value).is_some())
            }
            // The value is the chance of success, as a percentage.
            Random(_) => is_equal && (0..=100).contains(&self.value),
            // For special flags, any other comparison is considered malformed.
            Always | RandomSpatial(RandomSpatialVar::AllCornerRandom) => {
                is_equal && self.value == 1
            }
            // The value selects where the random point is (e.g. which phase).
            RandomSpatial(_) | RandomPhase(_) => is_equal,
            // Not implemented, as they're only used outside PvP races.
            Spatial(SpatialVar::RemainDistanceViewerId) | Lineup(LineupVar::IsExistCharaId) => {
                false
            }
            _ => true,
        }
    }
}

impl FromStr for Condition {
    type Err = ParseError;

    /// Parse a condition string, such as `phase>=2&order<=3@is_overtake==1`.
    ///
    /// Whitespace is allowed around variables and values.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if any comparison lacks an operator, uses an
    /// unknown variable or an invalid value, or can't be evaluated.
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let branches = input
            .split('@')
            .map(|branch| branch.split('&').map(parse_comparison).collect())
            .collect::<Result<_, _>>()?;
        Ok(Condition { branches })
    }
}

#[cfg(test)]
mod tests;
