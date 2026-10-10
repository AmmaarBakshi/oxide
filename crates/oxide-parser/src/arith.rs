//! Shell arithmetic: the integer expressions inside `$(( ))` and `let`.
//!
//! Numbers are 64-bit signed integers. The operators are C's, from loosest
//! to tightest: `=` and the compound assignments, `?:`, `||`, `&&`, `|`,
//! `^`, `&`, `==` `!=`, `<` `<=` `>` `>=`, `<<` `>>`, `+` `-`, `*` `/` `%`,
//! `**`, then the unary `!` `~` `-` `+` and `++`/`--` before or after a
//! name. A bare name reads that variable; unset or empty counts as 0.

/// The variables an expression reads and assigns.
pub trait Vars {
    fn get(&self, name: &str) -> Option<String>;
    fn set(&mut self, name: &str, value: i64);
}

/// Evaluates `expr`, assigning through `vars` as it goes.
pub fn evaluate(expr: &str, vars: &mut dyn Vars) -> Result<i64, String> {
    let tokens = tokenize(expr)?;
    if tokens.is_empty() {
        return Ok(0);
    }
    let mut parser = Parser {
        tokens,
        pos: 0,
        vars,
    };
    let value = parser.assignment()?;
    match parser.tokens.get(parser.pos) {
        None => Ok(value),
        Some(token) => Err(format!("syntax error near '{}'", token.text())),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(i64),
    Name(String),
    Op(&'static str),
}

impl Tok {
    fn text(&self) -> String {
        match self {
            Tok::Num(n) => n.to_string(),
            Tok::Name(name) => name.clone(),
            Tok::Op(op) => op.to_string(),
        }
    }
}

/// Every operator, longest first so `<<=` wins over `<<` and `<`.
const OPERATORS: &[&str] = &[
    "<<=", ">>=", "**", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<", ">>",
    "<=", ">=", "==", "!=", "&&", "||", "+", "-", "*", "/", "%", "<", ">", "=", "!", "~", "&", "|",
    "^", "?", ":", "(", ")",
];

fn tokenize(expr: &str) -> Result<Vec<Tok>, String> {
    let mut tokens = Vec::new();
    let mut rest = expr;
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            rest = &rest[c.len_utf8()..];
        } else if c.is_ascii_digit() {
            let end = rest
                .find(|c: char| !c.is_ascii_alphanumeric())
                .unwrap_or(rest.len());
            tokens.push(Tok::Num(number(&rest[..end])?));
            rest = &rest[end..];
        } else if c.is_alphabetic() || c == '_' {
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            tokens.push(Tok::Name(rest[..end].to_string()));
            rest = &rest[end..];
        } else if let Some(op) = OPERATORS.iter().find(|op| rest.starts_with(**op)) {
            tokens.push(Tok::Op(op));
            rest = &rest[op.len()..];
        } else {
            return Err(format!("syntax error: invalid character '{}'", c));
        }
    }
    Ok(tokens)
}

/// A literal: decimal, `0x` hex, or `0`-prefixed octal.
fn number(text: &str) -> Result<i64, String> {
    let parsed = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16)
    } else if text.len() > 1 && text.starts_with('0') {
        i64::from_str_radix(&text[1..], 8)
    } else {
        text.parse()
    };
    parsed.map_err(|_| format!("'{}': invalid number", text))
}

struct Parser<'v> {
    tokens: Vec<Tok>,
    pos: usize,
    vars: &'v mut dyn Vars,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn eat(&mut self, op: &str) -> bool {
        if matches!(self.peek(), Some(Tok::Op(found)) if *found == op) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, op: &str) -> Result<(), String> {
        if self.eat(op) {
            Ok(())
        } else {
            Err(format!("'{}' expected", op))
        }
    }

    fn read(&self, name: &str) -> Result<i64, String> {
        match self.vars.get(name) {
            None => Ok(0),
            Some(value) if value.trim().is_empty() => Ok(0),
            Some(value) => {
                let value = value.trim();
                let (negative, digits) = match value.strip_prefix('-') {
                    Some(rest) => (true, rest),
                    None => (false, value),
                };
                let n =
                    number(digits).map_err(|_| format!("{}: '{}' is not a number", name, value))?;
                Ok(if negative { -n } else { n })
            }
        }
    }

    fn assignment(&mut self) -> Result<i64, String> {
        if let (Some(Tok::Name(name)), Some(Tok::Op(op))) =
            (self.tokens.get(self.pos), self.tokens.get(self.pos + 1))
        {
            let op = *op;
            if matches!(
                op,
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "|=" | "^="
            ) {
                let name = name.clone();
                self.pos += 2;
                let right = self.assignment()?;
                let value = if op == "=" {
                    right
                } else {
                    let current = self.read(&name)?;
                    apply(&op[..op.len() - 1], current, right)?
                };
                self.vars.set(&name, value);
                return Ok(value);
            }
        }
        self.ternary()
    }

    fn ternary(&mut self) -> Result<i64, String> {
        let condition = self.binary(0)?;
        if !self.eat("?") {
            return Ok(condition);
        }
        let then = self.assignment()?;
        self.expect(":")?;
        let otherwise = self.assignment()?;
        Ok(if condition != 0 { then } else { otherwise })
    }

    /// Binary operators by precedence level, loosest first.
    fn binary(&mut self, level: usize) -> Result<i64, String> {
        const LEVELS: &[&[&str]] = &[
            &["||"],
            &["&&"],
            &["|"],
            &["^"],
            &["&"],
            &["==", "!="],
            &["<", "<=", ">", ">="],
            &["<<", ">>"],
            &["+", "-"],
            &["*", "/", "%"],
        ];
        if level == LEVELS.len() {
            return self.power();
        }
        let mut left = self.binary(level + 1)?;
        while let Some(Tok::Op(op)) = self.peek() {
            let op = *op;
            if !LEVELS[level].contains(&op) {
                break;
            }
            self.pos += 1;
            let right = self.binary(level + 1)?;
            left = apply(op, left, right)?;
        }
        Ok(left)
    }

    /// `**` binds right to left: `2 ** 3 ** 2` is `2 ** 9`.
    fn power(&mut self) -> Result<i64, String> {
        let base = self.unary()?;
        if self.eat("**") {
            let exponent = self.power()?;
            return apply("**", base, exponent);
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<i64, String> {
        for (op, f) in [
            ("!", (|n: i64| (n == 0) as i64) as fn(i64) -> i64),
            ("~", |n| !n),
            ("-", |n: i64| n.wrapping_neg()),
            ("+", |n| n),
        ] {
            if self.eat(op) {
                return Ok(f(self.unary()?));
            }
        }
        for (op, delta) in [("++", 1), ("--", -1)] {
            if self.eat(op) {
                let Some(Tok::Name(name)) = self.peek().cloned() else {
                    return Err(format!("'{}' needs a variable", op));
                };
                self.pos += 1;
                let value = self.read(&name)?.wrapping_add(delta);
                self.vars.set(&name, value);
                return Ok(value);
            }
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<i64, String> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Ok(n)
            }
            Some(Tok::Name(name)) => {
                self.pos += 1;
                let value = self.read(&name)?;
                for (op, delta) in [("++", 1), ("--", -1)] {
                    if self.eat(op) {
                        self.vars.set(&name, value.wrapping_add(delta));
                    }
                }
                Ok(value)
            }
            Some(Tok::Op("(")) => {
                self.pos += 1;
                let value = self.assignment()?;
                self.expect(")")?;
                Ok(value)
            }
            Some(token) => Err(format!("syntax error near '{}'", token.text())),
            None => Err("syntax error: expression ends too soon".to_string()),
        }
    }
}

fn apply(op: &str, a: i64, b: i64) -> Result<i64, String> {
    Ok(match op {
        "+" => a.wrapping_add(b),
        "-" => a.wrapping_sub(b),
        "*" => a.wrapping_mul(b),
        "/" | "%" if b == 0 => return Err("division by zero".to_string()),
        "/" => a.wrapping_div(b),
        "%" => a.wrapping_rem(b),
        "**" if b < 0 => return Err("negative exponent".to_string()),
        "**" => a.wrapping_pow(u32::try_from(b).unwrap_or(u32::MAX)),
        "<<" => a.wrapping_shl(b as u32),
        ">>" => a.wrapping_shr(b as u32),
        "&" => a & b,
        "|" => a | b,
        "^" => a ^ b,
        "==" => (a == b) as i64,
        "!=" => (a != b) as i64,
        "<" => (a < b) as i64,
        "<=" => (a <= b) as i64,
        ">" => (a > b) as i64,
        ">=" => (a >= b) as i64,
        "&&" => (a != 0 && b != 0) as i64,
        "||" => (a != 0 || b != 0) as i64,
        _ => return Err(format!("unknown operator '{}'", op)),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct Map(HashMap<String, String>);

    impl Vars for Map {
        fn get(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }

        fn set(&mut self, name: &str, value: i64) {
            self.0.insert(name.to_string(), value.to_string());
        }
    }

    fn eval(expr: &str) -> Result<i64, String> {
        evaluate(expr, &mut Map::default())
    }

    #[test]
    fn precedence_follows_c() {
        assert_eq!(eval("1 + 2 * 3"), Ok(7));
        assert_eq!(eval("(1 + 2) * 3"), Ok(9));
        assert_eq!(eval("2 ** 3 ** 2"), Ok(512));
        assert_eq!(eval("-2 ** 2"), Ok(4));
        assert_eq!(eval("7 / 2 + 7 % 2"), Ok(4));
        assert_eq!(eval("1 < 2 && 3 > 4 || 5 == 5"), Ok(1));
        assert_eq!(eval("1 << 4 | 1"), Ok(17));
        assert_eq!(eval("!0 + ~0"), Ok(0));
    }

    #[test]
    fn literals_in_other_bases() {
        assert_eq!(eval("0x1F + 010"), Ok(39));
    }

    #[test]
    fn ternary_picks_a_side() {
        assert_eq!(eval("3 > 2 ? 10 : 20"), Ok(10));
        assert_eq!(eval("0 ? 1 : 0 ? 2 : 3"), Ok(3));
    }

    #[test]
    fn variables_are_read_and_assigned() {
        let mut vars = Map::default();
        vars.0.insert("x".into(), "5".into());
        vars.0.insert("blank".into(), "".into());
        assert_eq!(evaluate("x * 2 + unset + blank", &mut vars), Ok(10));
        assert_eq!(evaluate("y = x += 3", &mut vars), Ok(8));
        assert_eq!(vars.0["x"], "8");
        assert_eq!(vars.0["y"], "8");
        assert_eq!(evaluate("x++ + ++x", &mut vars), Ok(18));
        assert_eq!(vars.0["x"], "10");
    }

    #[test]
    fn errors_are_reported() {
        assert_eq!(eval("1 / 0"), Err("division by zero".to_string()));
        assert!(eval("1 +").is_err());
        assert!(eval("2 $ 3").is_err());
        assert!(eval("(1").is_err());
        let mut vars = Map::default();
        vars.0.insert("word".into(), "abc".into());
        assert!(evaluate("word + 1", &mut vars).is_err());
    }

    #[test]
    fn an_empty_expression_is_zero() {
        assert_eq!(eval("  "), Ok(0));
    }
}
