//! The formulas: a small language of whole numbers, turned into the program that the plugin runs.
//!
//! A formula is a text such as `(2 * base + iv + ev / 4) * level / 100 + 5`. It has numbers, variables, the operators `+ - * / %`, the
//! comparisons `==`, `<` and `>` (they give 1 or 0), brackets, and three functions: `min(a, b)`, `max(a, b)` and `if(condition, then, else)`.
//! The program has the instructions of `lib/include/core/formula.h` of the plugin: the studio and the plugin must agree, and the tests of
//! both sides use the same formulas.

/// The variables of the formulas of the statistics: the name that the user writes, and what it means.
pub const VARIABLES: [(&str, &str); 5] = [
    ("base", "The base value of the species."),
    ("iv", "The IV of the Pokémon (0 to 31)."),
    ("ev", "The EV of the Pokémon (0 to 252)."),
    ("level", "The level of the Pokémon."),
    ("species", "The species of the Pokémon (the HP formula only: the others get 0)."),
];

/// The most bytes of a program, and the most numbers on the stack (the limits of the plugin).
pub const MAX_SIZE: usize = 120;
pub const STACK_SIZE: usize = 16;

const CONSTANT: u8 = 0x01;
const VARIABLE: u8 = 0x02;
const ADD: u8 = 0x10;
const SUBTRACT: u8 = 0x11;
const MULTIPLY: u8 = 0x12;
const DIVIDE: u8 = 0x13;
const REMAINDER: u8 = 0x14;
const NEGATE: u8 = 0x15;
const MINIMUM: u8 = 0x16;
const MAXIMUM: u8 = 0x17;
const EQUAL: u8 = 0x18;
const LESS: u8 = 0x19;
const GREATER: u8 = 0x1A;
const CHOOSE: u8 = 0x1B;

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(i64),
    Name(String),
    Symbol(char),
    Equal,
}

fn tokens(text: &str) -> Result<Vec<(usize, Token)>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() {
            at += 1;
        } else if c.is_ascii_digit() {
            let start = at;
            while at < chars.len() && chars[at].is_ascii_digit() {
                at += 1;
            }
            let digits: String = chars[start..at].iter().collect();
            let value: i64 = digits.parse().map_err(|_| format!("The number {digits} is too big."))?;
            if value > i64::from(i32::MAX) {
                return Err(format!("The number {digits} is too big (at most {}).", i32::MAX));
            }
            out.push((start, Token::Number(value)));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            out.push((start, Token::Name(chars[start..at].iter().collect::<String>().to_lowercase())));
        } else if c == '=' && chars.get(at + 1) == Some(&'=') {
            out.push((at, Token::Equal));
            at += 2;
        } else if "+-*/%()<>,".contains(c) {
            out.push((at, Token::Symbol(c)));
            at += 1;
        } else {
            return Err(format!("The character {c} is not allowed (position {}).", at + 1));
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<(usize, Token)>,
    at: usize,
    code: Vec<u8>,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at).map(|t| &t.1)
    }

    fn place(&self) -> usize {
        self.tokens.get(self.at).map_or(usize::MAX, |t| t.0 + 1)
    }

    fn fail<T>(&self, message: &str) -> Result<T, String> {
        match self.tokens.get(self.at) {
            Some((at, _)) => Err(format!("{message} (position {}).", at + 1)),
            None => Err(format!("{message} (at the end of the formula).")),
        }
    }

    fn eat(&mut self, symbol: char) -> bool {
        if self.peek() == Some(&Token::Symbol(symbol)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, symbol: char) -> Result<(), String> {
        if self.eat(symbol) {
            Ok(())
        } else {
            self.fail(&format!("A {symbol} is missing"))
        }
    }

    fn constant(&mut self, value: i64) {
        self.code.push(CONSTANT);
        self.code.extend_from_slice(&(value as i32).to_le_bytes());
    }

    fn comparison(&mut self) -> Result<(), String> {
        self.sum()?;
        let op = match self.peek() {
            Some(Token::Equal) => EQUAL,
            Some(Token::Symbol('<')) => LESS,
            Some(Token::Symbol('>')) => GREATER,
            _ => return Ok(()),
        };
        self.at += 1;
        self.sum()?;
        self.code.push(op);
        Ok(())
    }

    fn sum(&mut self) -> Result<(), String> {
        self.term()?;
        loop {
            let op = match self.peek() {
                Some(Token::Symbol('+')) => ADD,
                Some(Token::Symbol('-')) => SUBTRACT,
                _ => return Ok(()),
            };
            self.at += 1;
            self.term()?;
            self.code.push(op);
        }
    }

    fn term(&mut self) -> Result<(), String> {
        self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Symbol('*')) => MULTIPLY,
                Some(Token::Symbol('/')) => DIVIDE,
                Some(Token::Symbol('%')) => REMAINDER,
                _ => return Ok(()),
            };
            self.at += 1;
            self.unary()?;
            self.code.push(op);
        }
    }

    fn unary(&mut self) -> Result<(), String> {
        if self.eat('-') {
            // A minus in front of a number is the number itself, so that -5 is one instruction.
            if let Some(Token::Number(value)) = self.peek().cloned() {
                self.at += 1;
                self.constant(-value);
                return Ok(());
            }
            self.unary()?;
            self.code.push(NEGATE);
            return Ok(());
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<(), String> {
        match self.peek().cloned() {
            Some(Token::Number(value)) => {
                self.at += 1;
                self.constant(value);
                Ok(())
            }
            Some(Token::Symbol('(')) => {
                self.at += 1;
                self.comparison()?;
                self.expect(')')
            }
            Some(Token::Name(name)) => {
                let here = self.place();
                self.at += 1;
                if self.peek() == Some(&Token::Symbol('(')) {
                    self.at += 1;
                    let (arguments, op): (usize, Option<u8>) = match name.as_str() {
                        "min" => (2, Some(MINIMUM)),
                        "max" => (2, Some(MAXIMUM)),
                        "if" => (3, Some(CHOOSE)),
                        _ => return Err(format!("The function {name} does not exist (position {here}): use min, max or if.")),
                    };
                    for k in 0..arguments {
                        if k > 0 {
                            self.expect(',')?;
                        }
                        self.comparison()?;
                    }
                    self.expect(')')?;
                    self.code.extend(op);
                    Ok(())
                } else if let Some(index) = VARIABLES.iter().position(|(v, _)| *v == name) {
                    self.code.push(VARIABLE);
                    self.code.push(index as u8);
                    Ok(())
                } else {
                    let names: Vec<&str> = VARIABLES.iter().map(|v| v.0).collect();
                    Err(format!("The word {name} is not known (position {here}): the variables are {}.", names.join(", ")))
                }
            }
            _ => self.fail("A number, a variable or a bracket is expected"),
        }
    }
}

/// Turns the text of a formula into a program. The error tells what is wrong and where.
pub fn compile(text: &str) -> Result<Vec<u8>, String> {
    let tokens = tokens(text)?;
    if tokens.is_empty() {
        return Err("The formula is empty.".to_string());
    }
    let mut parser = Parser { tokens, at: 0, code: Vec::new() };
    parser.comparison()?;
    if parser.at < parser.tokens.len() {
        return parser.fail("This part of the formula cannot be read");
    }
    if parser.code.len() > MAX_SIZE {
        return Err(format!("The formula is too long: {} bytes, at most {MAX_SIZE}. Write it shorter.", parser.code.len()));
    }
    if stack_height(&parser.code) > STACK_SIZE {
        return Err(format!("The formula is too deep: it needs more than {STACK_SIZE} numbers at the same time. Use fewer brackets."));
    }
    Ok(parser.code)
}

/// The most numbers that a program keeps on the stack.
fn stack_height(code: &[u8]) -> usize {
    let (mut depth, mut top, mut at) = (0usize, 0usize, 0usize);
    while at < code.len() {
        let (pops, length) = match code[at] {
            CONSTANT => (0, 5),
            VARIABLE => (0, 2),
            NEGATE => (1, 1),
            CHOOSE => (3, 1),
            _ => (2, 1),
        };
        depth = depth + 1 - pops;
        top = top.max(depth);
        at += length;
    }
    top
}

/// Runs a program like the plugin: 64-bit numbers that wrap around, and a division by 0 that gives 0. The tests use it to check the
/// programs against the formulas of the game.
#[cfg(test)]
fn evaluate(program: &[u8], variables: &[i32]) -> i64 {
    let mut stack: Vec<i64> = Vec::new();
    let mut at = 0;
    while at < program.len() {
        match program[at] {
            CONSTANT => {
                stack.push(i64::from(i32::from_le_bytes([program[at + 1], program[at + 2], program[at + 3], program[at + 4]])));
                at += 5;
                continue;
            }
            VARIABLE => {
                stack.push(variables.get(program[at + 1] as usize).map_or(0, |v| i64::from(*v)));
                at += 2;
                continue;
            }
            NEGATE => {
                let a = stack.pop().unwrap_or(0);
                stack.push(a.wrapping_neg());
            }
            CHOOSE => {
                let otherwise = stack.pop().unwrap_or(0);
                let then = stack.pop().unwrap_or(0);
                let condition = stack.pop().unwrap_or(0);
                stack.push(if condition != 0 { then } else { otherwise });
            }
            op => {
                let b = stack.pop().unwrap_or(0);
                let a = stack.pop().unwrap_or(0);
                stack.push(match op {
                    ADD => a.wrapping_add(b),
                    SUBTRACT => a.wrapping_sub(b),
                    MULTIPLY => a.wrapping_mul(b),
                    DIVIDE => {
                        if b == 0 {
                            0
                        } else {
                            a.wrapping_div(b)
                        }
                    }
                    REMAINDER => {
                        if b == 0 || b == -1 {
                            0
                        } else {
                            a % b
                        }
                    }
                    MINIMUM => a.min(b),
                    MAXIMUM => a.max(b),
                    EQUAL => i64::from(a == b),
                    LESS => i64::from(a < b),
                    GREATER => i64::from(a > b),
                    _ => 0,
                });
            }
        }
        at += 1;
    }
    stack.pop().unwrap_or(0)
}

#[cfg(test)]
mod test {
    use super::*;

    fn run(text: &str, variables: &[i32]) -> i64 {
        evaluate(&compile(text).unwrap(), variables)
    }

    #[test]
    fn the_operators_follow_the_usual_order() {
        assert_eq!(run("1 + 2 * 3", &[]), 7);
        assert_eq!(run("(1 + 2) * 3", &[]), 9);
        assert_eq!(run("10 - 4 - 3", &[]), 3);
        assert_eq!(run("100 / 7 % 5", &[]), 4);
        assert_eq!(run("-5 + 2", &[]), -3);
        assert_eq!(run("-(2 + 3)", &[]), -5);
        assert_eq!(run("7 / 0", &[]), 0);
        assert_eq!(run("min(4, 9) + max(4, 9)", &[]), 13);
        assert_eq!(run("if(2 > 1, 10, 20)", &[]), 10);
        assert_eq!(run("if(2 == 1, 10, 20)", &[]), 20);
        assert_eq!(run("3 < 4", &[]), 1);
    }

    #[test]
    fn the_variables_have_their_numbers() {
        let program = compile("base").unwrap();
        assert_eq!(program, vec![VARIABLE, 0]);
        assert_eq!(compile("species").unwrap(), vec![VARIABLE, 4]);
        assert_eq!(run("BASE + Level", &[7, 0, 0, 3]), 10);
    }

    #[test]
    fn a_wrong_formula_tells_what_is_wrong() {
        assert!(compile("").unwrap_err().contains("empty"));
        assert!(compile("2 +").unwrap_err().contains("end"));
        assert!(compile("2 $ 3").unwrap_err().contains("not allowed"));
        assert!(compile("foo + 1").unwrap_err().contains("not known"));
        assert!(compile("sqrt(4)").unwrap_err().contains("does not exist"));
        assert!(compile("(1 + 2").unwrap_err().contains(")"));
        assert!(compile("1 2").unwrap_err().contains("cannot be read"));
        assert!(compile("99999999999").unwrap_err().contains("too big"));
        let long = (0..60).map(|_| "level").collect::<Vec<_>>().join(" + ");
        assert!(compile(&long).unwrap_err().contains("too long"));
        let deep = format!("{}1{}", "1 + (".repeat(20), ")".repeat(20));
        assert!(compile(&deep).is_err());
    }

    /// The formulas of the game, written as text: they must give the numbers of the game.
    #[test]
    fn the_formulas_of_the_game_give_the_numbers_of_the_game() {
        let stat = compile(crate::defaults::STAT).unwrap();
        let hp = compile(crate::defaults::HP).unwrap();
        for base in (5..=255).step_by(10) {
            for level in 1..=100 {
                for (iv, ev) in [(0, 0), (31, 0), (31, 252), (15, 100)] {
                    let variables = [base, iv, ev, level, 25];
                    let game = ((2 * base + iv + ev / 4) * level / 100 + 5) as i64;
                    assert_eq!(evaluate(&stat, &variables), game);
                    let game_hp = ((2 * base + iv + ev / 4) * level / 100 + level + 10) as i64;
                    assert_eq!(evaluate(&hp, &variables), game_hp);
                }
            }
        }
        // The Pokémon with 1 HP.
        assert_eq!(evaluate(&hp, &[1, 31, 0, 50, 292]), 1);
    }
}

/// The programs of the formulas of the game, byte by byte. The tests of the plugin (`tests/test_formula.cc`) have the same bytes: if one
/// side changes, the other one must change.
#[cfg(test)]
mod golden {
    pub const STAT: &[u8] = &[
        0x01, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x12, 0x02, 0x01, 0x10, 0x02, 0x02, 0x01, 0x04, 0x00, 0x00, 0x00, 0x13, 0x10, 0x02, 0x03, 0x12, 0x01, 0x64,
        0x00, 0x00, 0x00, 0x13, 0x01, 0x05, 0x00, 0x00, 0x00, 0x10,
    ];
    pub const HP: &[u8] = &[
        0x02, 0x04, 0x01, 0x24, 0x01, 0x00, 0x00, 0x18, 0x01, 0x01, 0x00, 0x00, 0x00, 0x01, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x12, 0x02, 0x01, 0x10, 0x02,
        0x02, 0x01, 0x04, 0x00, 0x00, 0x00, 0x13, 0x10, 0x02, 0x03, 0x12, 0x01, 0x64, 0x00, 0x00, 0x00, 0x13, 0x02, 0x03, 0x10, 0x01, 0x0A, 0x00, 0x00, 0x00,
        0x10, 0x1B,
    ];

    #[test]
    fn the_programs_of_the_game_have_the_bytes_of_the_plugin_tests() {
        assert_eq!(super::compile(crate::defaults::STAT).unwrap(), STAT);
        assert_eq!(super::compile(crate::defaults::HP).unwrap(), HP);
    }
}
