use crate::condition::variables::VarType;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Variable(String),
    Value(i32),
    CompOp(CompOp),
    BinOp(BinOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub comp_tree: ComparisonTree,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonTree {
    Single(Comparison),
    Binary {
        left: Box<ComparisonTree>,
        op: BinOp,
        right: Box<ComparisonTree>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub var: VarType,
    pub op: CompOp,
    pub value: i32,
}

/// A flat list of comparisons that must all be true (AND conjunction).
///
/// After splitting OR branches, each branch's guards are guaranteed to be
/// a simple AND conjunction of comparisons, which can be represented as a list.
pub type ComparisonList = Vec<Comparison>;

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.comp_tree)
    }
}

impl fmt::Display for ComparisonTree {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ComparisonTree::Single(comp) => write!(f, "{}", comp),
            ComparisonTree::Binary { left, op, right } => {
                // Add parentheses for clarity when displaying nested expressions
                match (&**left, &**right) {
                    (ComparisonTree::Single(_), ComparisonTree::Single(_)) => {
                        write!(f, "{} {} {}", left, op, right)
                    }
                    _ => write!(f, "({}) {} ({})", left, op, right),
                }
            }
        }
    }
}

impl fmt::Display for Comparison {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.var, self.op, self.value)
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
                        || (c == '-' && self.peek(1).is_some_and(|p| p.is_ascii_digit())) =>
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
    ExpectedCompOp,
    ExpectedVariable,
    ExpectedValue,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken(msg, pos) => {
                write!(f, "Unexpected token '{}' at position {}", msg, pos)
            }
            ParseError::ExpectedCompOp => write!(f, "Expected comparison operator"),
            ParseError::ExpectedVariable => write!(f, "Expected variable"),
            ParseError::ExpectedValue => write!(f, "Expected value"),
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
        let comp_tree = self.parse_or_expression()?;

        if self.current().is_some() {
            return Err(ParseError::UnexpectedToken(
                format!("{:?}", self.current().unwrap()),
                self.pos,
            ));
        }

        Ok(Condition { comp_tree })
    }

    fn parse_or_expression(&mut self) -> Result<ComparisonTree, ParseError> {
        let mut left = self.parse_and_expression()?;

        while let Some(Token::BinOp(BinOp::Or)) = self.current() {
            self.advance();
            let right = self.parse_and_expression()?;
            left = ComparisonTree::Binary {
                left: Box::new(left),
                op: BinOp::Or,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and_expression(&mut self) -> Result<ComparisonTree, ParseError> {
        let mut left = self.parse_primary()?;

        while let Some(Token::BinOp(BinOp::And)) = self.current() {
            self.advance();
            let right = self.parse_primary()?;
            left = ComparisonTree::Binary {
                left: Box::new(left),
                op: BinOp::And,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<ComparisonTree, ParseError> {
        let comp = self.parse_comp()?;
        Ok(ComparisonTree::Single(comp))
    }

    fn parse_comp(&mut self) -> Result<Comparison, ParseError> {
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

        Ok(Comparison { var, op, value })
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
mod tests;
