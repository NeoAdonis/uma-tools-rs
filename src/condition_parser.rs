use crate::condition_variables::VarType;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Variable(String),
    Value(i32),
    CompOp(CompOp),
    BinOp(BinOp),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompOp {
    Equal,        // "=="
    NotEqual,     // "!="
    Greater,      // ">"
    GreaterEqual, // ">="
    Less,         // "<"
    LessEqual,    // "<="
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    And, // "&"
    Or,  // "@"
}

impl fmt::Display for CompOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompOp::Equal => write!(f, "=="),
            CompOp::NotEqual => write!(f, "!="),
            CompOp::Greater => write!(f, ">"),
            CompOp::GreaterEqual => write!(f, ">="),
            CompOp::Less => write!(f, "<"),
            CompOp::LessEqual => write!(f, "<="),
        }
    }
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BinOp::And => write!(f, "&"),
            BinOp::Or => write!(f, "@"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub comp_list: CompList,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompList {
    Single(Comp),
    Binary {
        left: Box<CompList>,
        op: BinOp,
        right: Box<CompList>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Comp {
    pub var: VarType,
    pub op: CompOp,
    pub value: i32,
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.comp_list)
    }
}

impl fmt::Display for CompList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CompList::Single(comp) => write!(f, "{}", comp),
            CompList::Binary { left, op, right } => {
                // Add parentheses for clarity when displaying nested expressions
                match (&**left, &**right) {
                    (CompList::Single(_), CompList::Single(_)) => {
                        write!(f, "{} {} {}", left, op, right)
                    }
                    _ => write!(f, "({}) {} ({})", left, op, right),
                }
            }
        }
    }
}

impl fmt::Display for Comp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.var, self.op, self.value)
    }
}

impl CompList {
    /// Visit all comparisons in the tree, calling the provided function for each
    ///
    /// This is useful for extracting all variables from a condition without
    /// evaluating the boolean logic.
    ///
    /// # Arguments
    /// * `visitor` - Function to call for each Comp in the tree
    ///
    /// # Example
    /// ```ignore
    /// condition.comp_list.visit_all_comps(|comp| {
    ///     println!("Variable: {}, Operator: {}, Value: {}", comp.var, comp.op, comp.value);
    /// });
    /// ```
    pub fn visit_all_comps<F>(&self, visitor: &mut F)
    where
        F: FnMut(&Comp),
    {
        match self {
            CompList::Single(comp) => visitor(comp),
            CompList::Binary { left, right, .. } => {
                left.visit_all_comps(visitor);
                right.visit_all_comps(visitor);
            }
        }
    }
}

#[derive(Debug)]
pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

#[derive(Debug)]
pub enum LexError {
    UnexpectedChar(char, usize),
    UnexpectedEnd,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LexError::UnexpectedChar(c, pos) => {
                write!(f, "Unexpected character '{}' at position {}", c, pos)
            }
            LexError::UnexpectedEnd => write!(f, "Unexpected end of input"),
        }
    }
}

impl std::error::Error for LexError {}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn current(&self) -> Option<char> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }

    fn peek(&self, offset: usize) -> Option<char> {
        let pos = self.pos + offset;
        if pos < self.input.len() {
            Some(self.input[pos])
        } else {
            None
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.current() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        loop {
            // No condition string actually contains whitespace, but just in case
            self.skip_whitespace();

            match self.current() {
                None => break,
                Some('=') => {
                    if self.peek(1) == Some('=') {
                        tokens.push(Token::CompOp(CompOp::Equal));
                        self.advance();
                        self.advance();
                    } else {
                        return Err(LexError::UnexpectedChar('=', self.pos));
                    }
                }
                Some('!') => {
                    if self.peek(1) == Some('=') {
                        tokens.push(Token::CompOp(CompOp::NotEqual));
                        self.advance();
                        self.advance();
                    } else {
                        return Err(LexError::UnexpectedChar('!', self.pos));
                    }
                }
                Some('>') => {
                    if self.peek(1) == Some('=') {
                        tokens.push(Token::CompOp(CompOp::GreaterEqual));
                        self.advance();
                        self.advance();
                    } else {
                        tokens.push(Token::CompOp(CompOp::Greater));
                        self.advance();
                    }
                }
                Some('<') => {
                    if self.peek(1) == Some('=') {
                        tokens.push(Token::CompOp(CompOp::LessEqual));
                        self.advance();
                        self.advance();
                    } else {
                        tokens.push(Token::CompOp(CompOp::Less));
                        self.advance();
                    }
                }
                Some('&') => {
                    tokens.push(Token::BinOp(BinOp::And));
                    self.advance();
                }
                Some('@') => {
                    tokens.push(Token::BinOp(BinOp::Or));
                    self.advance();
                }
                Some(c)
                    if c.is_ascii_digit()
                        || (c == '-' && self.peek(1).map_or(false, |p| p.is_ascii_digit())) =>
                {
                    tokens.push(self.read_number()?);
                }
                Some(c) if c.is_alphabetic() || c == '_' => {
                    tokens.push(self.read_variable());
                }
                Some(c) => {
                    return Err(LexError::UnexpectedChar(c, self.pos));
                }
            }
        }

        Ok(tokens)
    }

    fn read_number(&mut self) -> Result<Token, LexError> {
        let start = self.pos;

        // Handle negative sign
        if self.current() == Some('-') {
            self.advance();
        }

        while let Some(c) = self.current() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        let num_str: String = self.input[start..self.pos].iter().collect();
        let value = num_str
            .parse::<i32>()
            .map_err(|_| LexError::UnexpectedEnd)?;
        Ok(Token::Value(value))
    }

    fn read_variable(&mut self) -> Token {
        let start = self.pos;

        while let Some(c) = self.current() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let var_name: String = self.input[start..self.pos].iter().collect();
        Token::Variable(var_name)
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken(String, usize),
    UnexpectedEnd,
    ExpectedCompOp,
    ExpectedVariable,
    ExpectedValue,
    ExpectedBinOp,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken(msg, pos) => {
                write!(f, "Unexpected token '{}' at position {}", msg, pos)
            }
            ParseError::UnexpectedEnd => write!(f, "Unexpected end of tokens"),
            ParseError::ExpectedCompOp => write!(f, "Expected comparison operator"),
            ParseError::ExpectedVariable => write!(f, "Expected variable"),
            ParseError::ExpectedValue => write!(f, "Expected value"),
            ParseError::ExpectedBinOp => write!(f, "Expected binary operator"),
        }
    }
}

impl std::error::Error for ParseError {}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn current(&self) -> Option<&Token> {
        if self.pos < self.tokens.len() {
            Some(&self.tokens[self.pos])
        } else {
            None
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    pub fn parse(&mut self) -> Result<Condition, ParseError> {
        let comp_list = self.parse_or_expression()?;

        if self.current().is_some() {
            return Err(ParseError::UnexpectedToken(
                format!("{:?}", self.current().unwrap()),
                self.pos,
            ));
        }

        Ok(Condition { comp_list })
    }

    fn parse_or_expression(&mut self) -> Result<CompList, ParseError> {
        let mut left = self.parse_and_expression()?;

        while let Some(Token::BinOp(BinOp::Or)) = self.current() {
            self.advance();
            let right = self.parse_and_expression()?;
            left = CompList::Binary {
                left: Box::new(left),
                op: BinOp::Or,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and_expression(&mut self) -> Result<CompList, ParseError> {
        let mut left = self.parse_primary()?;

        while let Some(Token::BinOp(BinOp::And)) = self.current() {
            self.advance();
            let right = self.parse_primary()?;
            left = CompList::Binary {
                left: Box::new(left),
                op: BinOp::And,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<CompList, ParseError> {
        let comp = self.parse_comp()?;
        Ok(CompList::Single(comp))
    }

    fn parse_comp(&mut self) -> Result<Comp, ParseError> {
        // Parse variable
        let var_str = match self.current() {
            Some(Token::Variable(v)) => v.clone(),
            _ => return Err(ParseError::ExpectedVariable),
        };
        self.advance();

        // Convert string to VarType enum
        let var = VarType::parse(&var_str);

        // Parse comparison operator
        let op = match self.current() {
            Some(Token::CompOp(op)) => *op,
            _ => return Err(ParseError::ExpectedCompOp),
        };
        self.advance();

        // Parse value
        let value = match self.current() {
            Some(Token::Value(v)) => *v,
            _ => return Err(ParseError::ExpectedValue),
        };
        self.advance();

        Ok(Comp { var, op, value })
    }
}

pub fn parse_condition(input: &str) -> Result<Condition, Box<dyn std::error::Error>> {
    let mut lexer = Lexer::new(input);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let condition = parser.parse()?;
    Ok(condition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_condition() {
        let result = parse_condition("hp_per == 100").unwrap();
        match result.comp_list {
            CompList::Single(comp) => {
                assert!(matches!(comp.var, VarType::Status(_)));
                assert_eq!(comp.op, CompOp::Equal);
                assert_eq!(comp.value, 100);
            }
            _ => panic!("Expected Single"),
        }
    }

    #[test]
    fn test_binary_and() {
        let result = parse_condition("hp_per > 50 & base_stamina >= 30").unwrap();
        match result.comp_list {
            CompList::Binary { left, op, right } => {
                // Left should be a Single comparison
                match *left {
                    CompList::Single(ref comp) => {
                        assert!(matches!(comp.var, VarType::Status(_)));
                        assert_eq!(comp.op, CompOp::Greater);
                        assert_eq!(comp.value, 50);
                    }
                    _ => panic!("Expected Single on left"),
                }
                assert_eq!(op, BinOp::And);
                // Right should be a Single comparison
                match *right {
                    CompList::Single(ref comp) => {
                        assert!(matches!(comp.var, VarType::BaseStat(_)));
                        assert_eq!(comp.op, CompOp::GreaterEqual);
                        assert_eq!(comp.value, 30);
                    }
                    _ => panic!("Expected Single on right"),
                }
            }
            _ => panic!("Expected Binary"),
        }
    }

    #[test]
    fn test_multiple_conditions() {
        // With new precedence: order == 1 & phase != 2 @ corner < 3 parses as (order == 1 & phase != 2) @ corner < 3
        let result = parse_condition("order == 1 & phase != 2 @ corner < 3").unwrap();
        match result.comp_list {
            CompList::Binary { left, op, right } => {
                assert_eq!(op, BinOp::Or); // Top level is OR
                // Left side should be (order == 1 & phase != 2)
                match *left {
                    CompList::Binary {
                        left: ref inner_left,
                        op: inner_op,
                        right: ref inner_right,
                    } => {
                        assert_eq!(inner_op, BinOp::And);
                        match **inner_left {
                            CompList::Single(ref comp) => {
                                assert!(matches!(comp.var, VarType::Position(_)));
                                assert_eq!(comp.value, 1);
                            }
                            _ => panic!("Expected Single"),
                        }
                        match **inner_right {
                            CompList::Single(ref comp) => {
                                assert!(matches!(comp.var, VarType::Phase(_)));
                                assert_eq!(comp.value, 2);
                            }
                            _ => panic!("Expected Single"),
                        }
                    }
                    _ => panic!("Expected Binary on left"),
                }
                // Right side should be corner < 3
                match *right {
                    CompList::Single(ref comp) => {
                        assert!(matches!(comp.var, VarType::Course(_)));
                        assert_eq!(comp.value, 3);
                    }
                    _ => panic!("Expected Single on right"),
                }
            }
            _ => panic!("Expected Binary"),
        }
    }

    #[test]
    fn test_all_comparison_operators() {
        assert!(parse_condition("x == 5").is_ok());
        assert!(parse_condition("x != 5").is_ok());
        assert!(parse_condition("x > 5").is_ok());
        assert!(parse_condition("x >= 5").is_ok());
        assert!(parse_condition("x < 5").is_ok());
        assert!(parse_condition("x <= 5").is_ok());
    }

    #[test]
    fn test_negative_numbers() {
        let result = parse_condition("x > -10").unwrap();
        match result.comp_list {
            CompList::Single(comp) => {
                assert_eq!(comp.value, -10);
            }
            _ => panic!("Expected Single"),
        }
    }

    #[test]
    fn test_whitespace_handling() {
        let result1 = parse_condition("hp==100").unwrap();
        let result2 = parse_condition("  hp  ==  100  ").unwrap();
        let result3 = parse_condition("hp == 100").unwrap();

        assert_eq!(result1.comp_list, result2.comp_list);
        assert_eq!(result2.comp_list, result3.comp_list);
    }

    #[test]
    fn test_variable_names() {
        assert!(parse_condition("simple == 1").is_ok());
        assert!(parse_condition("with_underscore == 1").is_ok());
        assert!(parse_condition("camelCase == 1").is_ok());
        assert!(parse_condition("PascalCase == 1").is_ok());
        assert!(parse_condition("var123 == 1").is_ok());
    }

    #[test]
    fn test_display_trait() {
        let condition = parse_condition("hp_per > 50 & base_stamina >= 30").unwrap();
        let display = format!("{}", condition);
        assert!(display.contains("hp_per") || display.contains("hp"));
        assert!(display.contains("50"));
        assert!(display.contains("stamina"));
        assert!(display.contains("30"));
    }

    #[test]
    fn test_error_cases() {
        assert!(parse_condition("").is_err());
        assert!(parse_condition("hp").is_err());
        assert!(parse_condition("hp ==").is_err());
        assert!(parse_condition("== 100").is_err());
        assert!(parse_condition("hp = 100").is_err()); // single = not allowed
        assert!(parse_condition("hp > 50 &").is_err()); // incomplete
    }

    #[test]
    fn test_and_precedence_over_or() {
        // a & b @ c & d should parse as (a & b) @ (c & d)
        let result = parse_condition("a == 1 & b == 2 @ c == 3 & d == 4").unwrap();
        match result.comp_list {
            CompList::Binary { left, op, right } => {
                assert_eq!(op, BinOp::Or); // Top level is OR
                // Both sides should be AND expressions
                assert!(matches!(*left, CompList::Binary { op: BinOp::And, .. }));
                assert!(matches!(*right, CompList::Binary { op: BinOp::And, .. }));
            }
            _ => panic!("Expected Binary with OR at top level"),
        }
    }

    #[test]
    fn test_left_associativity_and() {
        // a & b & c should parse as (a & b) & c
        let result = parse_condition("a == 1 & b == 2 & c == 3").unwrap();
        match result.comp_list {
            CompList::Binary { left, op, right } => {
                assert_eq!(op, BinOp::And);
                // Left should be another AND expression
                assert!(matches!(*left, CompList::Binary { op: BinOp::And, .. }));
                // Right should be a single comparison
                assert!(matches!(*right, CompList::Single(_)));
            }
            _ => panic!("Expected Binary"),
        }
    }

    #[test]
    fn test_left_associativity_or() {
        // a @ b @ c should parse as (a @ b) @ c
        let result = parse_condition("a == 1 @ b == 2 @ c == 3").unwrap();
        match result.comp_list {
            CompList::Binary { left, op, right } => {
                assert_eq!(op, BinOp::Or);
                // Left should be another OR expression
                assert!(matches!(*left, CompList::Binary { op: BinOp::Or, .. }));
                // Right should be a single comparison
                assert!(matches!(*right, CompList::Single(_)));
            }
            _ => panic!("Expected Binary"),
        }
    }

    #[test]
    fn test_complex_precedence() {
        // a @ b & c @ d should parse as a @ (b & c) @ d
        let result = parse_condition("order == 1 @ phase == 2 & corner == 3 @ grade == 4").unwrap();
        match result.comp_list {
            CompList::Binary {
                left: left1,
                op: op1,
                right: right1,
            } => {
                assert_eq!(op1, BinOp::Or);
                // Left should be another OR expression
                match *left1 {
                    CompList::Binary {
                        left: ref left2,
                        op: op2,
                        right: ref right2,
                    } => {
                        assert_eq!(op2, BinOp::Or);
                        // left2 should be 'order == 1'
                        assert!(matches!(**left2, CompList::Single(_)));
                        // right2 should be 'phase == 2 & corner == 3'
                        assert!(matches!(**right2, CompList::Binary { op: BinOp::And, .. }));
                    }
                    _ => panic!("Expected nested OR on left"),
                }
                // Right should be 'grade == 4'
                assert!(matches!(*right1, CompList::Single(_)));
            }
            _ => panic!("Expected Binary"),
        }
    }

    #[test]
    fn test_unknown_variable() {
        // Unknown variables should be parsed as VarType::Unknown
        let result = parse_condition("this_var_does_not_exist == 42").unwrap();
        match result.comp_list {
            CompList::Single(comp) => {
                assert!(matches!(comp.var, VarType::Unknown(_)));
                if let VarType::Unknown(s) = comp.var {
                    assert_eq!(s, "this_var_does_not_exist");
                }
                assert_eq!(comp.value, 42);
            }
            _ => panic!("Expected Single"),
        }
    }

    #[test]
    fn test_known_variables() {
        // Test a few known variables to ensure they parse correctly
        let result = parse_condition("always == 1").unwrap();
        match result.comp_list {
            CompList::Single(comp) => {
                assert!(matches!(comp.var, VarType::Always));
            }
            _ => panic!("Expected Single"),
        }

        let result = parse_condition("random_lot == 30").unwrap();
        match result.comp_list {
            CompList::Single(comp) => {
                assert!(matches!(comp.var, VarType::Trigger(_)));
            }
            _ => panic!("Expected Single"),
        }
    }
}
