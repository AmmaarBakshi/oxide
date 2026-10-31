use std::cmp::Ordering;
use std::fmt;

use crate::{Ctx, Io};

/// The exit status for a malformed expression or a failed operation,
/// leaving 0 and 1 to say whether the result was null or zero.
const INVALID: i32 = 2;

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "expr: missing operand");
        return INVALID;
    }
    let mut parser = Parser {
        tokens: args,
        pos: 0,
    };
    let result = parser.or().and_then(|value| match args.get(parser.pos) {
        Some(extra) => Err(format!("syntax error: unexpected argument '{}'", extra)),
        None => Ok(value),
    });
    match result {
        Ok(value) => {
            wln!(io, "{}", value);
            if value.is_null() {
                1
            } else {
                0
            }
        }
        Err(msg) => {
            ewln!(io, "expr: {}", msg);
            INVALID
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Int(i64),
    Str(String),
}

impl Value {
    /// Any argument that reads as an integer is one, so `expr 2 + 3`
    /// adds, while `expr abc \< abd` compares text.
    fn from_token(token: &str) -> Self {
        match token.parse() {
            Ok(n) if !token.starts_with('+') => Value::Int(n),
            _ => Value::Str(token.to_string()),
        }
    }

    /// Null (the empty string) and zero are false; everything else true.
    fn is_null(&self) -> bool {
        match self {
            Value::Int(n) => *n == 0,
            Value::Str(s) => s.is_empty() || Value::from_token(s) == Value::Int(0),
        }
    }

    fn int(&self) -> Result<i64, String> {
        match self {
            Value::Int(n) => Ok(*n),
            Value::Str(s) => match Value::from_token(s) {
                Value::Int(n) => Ok(n),
                Value::Str(_) => Err(format!("non-integer argument: '{}'", s)),
            },
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Str(s) => f.write_str(s),
        }
    }
}

/// A recursive-descent parser over the arguments, from the loosest
/// binding operator (`|`) to the tightest (`*`, `/`, `%`).
struct Parser<'a> {
    tokens: &'a [String],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(String::as_str)
    }

    /// Consumes the next token if it is one of `ops`.
    fn take(&mut self, ops: &[&str]) -> Option<String> {
        let op = self.peek().filter(|token| ops.contains(token))?.to_string();
        self.pos += 1;
        Some(op)
    }

    /// The next argument, which has to be there: an operator or function
    /// can't be the last thing in the expression.
    fn operand(&mut self) -> Result<String, String> {
        let Some(token) = self.peek().map(String::from) else {
            // Out of arguments, so the last one is what wanted another.
            let last = self.tokens.last().map_or("", String::as_str);
            return Err(format!("syntax error: missing argument after '{}'", last));
        };
        self.pos += 1;
        Ok(token)
    }

    /// `A | B`: A unless it is null or zero, else B (or 0 if B is too).
    fn or(&mut self) -> Result<Value, String> {
        let mut left = self.and()?;
        while self.take(&["|"]).is_some() {
            let right = self.and()?;
            if left.is_null() {
                left = if right.is_null() {
                    Value::Int(0)
                } else {
                    right
                };
            }
        }
        Ok(left)
    }

    /// `A & B`: A if neither is null or zero, else 0.
    fn and(&mut self) -> Result<Value, String> {
        let mut left = self.comparison()?;
        while self.take(&["&"]).is_some() {
            let right = self.comparison()?;
            if left.is_null() || right.is_null() {
                left = Value::Int(0);
            }
        }
        Ok(left)
    }

    /// `= != < <= > >=`: 1 or 0. Integers compare as numbers when both
    /// sides are integers, and as text otherwise.
    fn comparison(&mut self) -> Result<Value, String> {
        let mut left = self.sum()?;
        while let Some(op) = self.take(&["=", "!=", "<", "<=", ">", ">="]) {
            let right = self.sum()?;
            let order = match (left.int(), right.int()) {
                (Ok(a), Ok(b)) => a.cmp(&b),
                _ => left.to_string().cmp(&right.to_string()),
            };
            let holds = match op.as_str() {
                "=" => order == Ordering::Equal,
                "!=" => order != Ordering::Equal,
                "<" => order == Ordering::Less,
                "<=" => order != Ordering::Greater,
                ">" => order == Ordering::Greater,
                _ => order != Ordering::Less,
            };
            left = Value::Int(i64::from(holds));
        }
        Ok(left)
    }

    fn sum(&mut self) -> Result<Value, String> {
        let mut left = self.product()?;
        while let Some(op) = self.take(&["+", "-"]) {
            let (a, b) = (left.int()?, self.product()?.int()?);
            let result = if op == "+" {
                a.checked_add(b)
            } else {
                a.checked_sub(b)
            };
            left = Value::Int(result.ok_or("integer result out of range")?);
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Value, String> {
        let mut left = self.matching()?;
        while let Some(op) = self.take(&["*", "/", "%"]) {
            let (a, b) = (left.int()?, self.matching()?.int()?);
            if op != "*" && b == 0 {
                return Err("division by zero".to_string());
            }
            let result = match op.as_str() {
                "*" => a.checked_mul(b),
                "/" => a.checked_div(b),
                _ => a.checked_rem(b),
            };
            left = Value::Int(result.ok_or("integer result out of range")?);
        }
        Ok(left)
    }

    /// `STRING : REGEX`, which binds tighter than any other operator. It
    /// needs a regular-expression engine, so for now it's refused outright
    /// rather than half-done.
    fn matching(&mut self) -> Result<Value, String> {
        let left = self.primary()?;
        if self.take(&[":"]).is_some() {
            return Err("regular expression matching is not supported".to_string());
        }
        Ok(left)
    }

    /// A parenthesized expression, a string function, or a plain argument.
    fn primary(&mut self) -> Result<Value, String> {
        let token = self.operand()?;
        match token.as_str() {
            "(" => {
                let value = self.or()?;
                match self.take(&[")"]) {
                    Some(_) => Ok(value),
                    None => Err("syntax error: expecting ')'".to_string()),
                }
            }
            "length" => {
                let s = self.operand()?;
                Ok(Value::Int(s.chars().count() as i64))
            }
            // `substr STRING POS LEN`: LEN characters from POS, counting
            // from 1. Out-of-range positions give an empty string.
            "substr" => {
                let s = self.operand()?;
                let pos = Value::from_token(&self.operand()?).int()?;
                let len = Value::from_token(&self.operand()?).int()?;
                let piece: String = if pos < 1 || len < 1 {
                    String::new()
                } else {
                    s.chars()
                        .skip(pos as usize - 1)
                        .take(len as usize)
                        .collect()
                };
                Ok(Value::Str(piece))
            }
            // `index STRING CHARS`: where the first of CHARS appears in
            // STRING, counting from 1, or 0.
            "index" => {
                let s = self.operand()?;
                let chars = self.operand()?;
                let at = s
                    .chars()
                    .position(|c| chars.contains(c))
                    .map_or(0, |i| i + 1);
                Ok(Value::Int(at as i64))
            }
            ":" | "match" => Err("regular expression matching is not supported".to_string()),
            // `+ TOKEN` takes TOKEN literally, even if it's an operator.
            "+" => Ok(Value::Str(self.operand()?)),
            ")" | "|" | "&" | "=" | "!=" | "<" | "<=" | ">" | ">=" | "-" | "*" | "/" | "%" => {
                Err(format!("syntax error: unexpected argument '{}'", token))
            }
            _ => Ok(Value::from_token(&token)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{run, Output};

    fn expr(args: &[&str]) -> Output {
        run("expr", args, "")
    }

    fn value(args: &[&str]) -> String {
        expr(args).stdout.trim_end().to_string()
    }

    #[test]
    fn does_integer_arithmetic_with_precedence() {
        assert_eq!(value(&["1", "+", "2", "*", "3"]), "7");
        assert_eq!(value(&["(", "1", "+", "2", ")", "*", "3"]), "9");
        assert_eq!(value(&["7", "/", "2"]), "3");
        assert_eq!(value(&["7", "%", "3"]), "1");
        assert_eq!(value(&["-5", "+", "2"]), "-3");
        assert_eq!(value(&["10", "-", "4", "-", "3"]), "3");
    }

    #[test]
    fn compares_numbers_as_numbers_and_text_as_text() {
        assert_eq!(value(&["10", ">", "9"]), "1");
        assert_eq!(value(&["abc", "<", "abd"]), "1");
        assert_eq!(value(&["10", "<", "9a"]), "1");
        assert_eq!(value(&["2", "=", "02"]), "1");
        assert_eq!(value(&["a", "!=", "a"]), "0");
    }

    #[test]
    fn or_and_and_pick_values() {
        assert_eq!(value(&["0", "|", "5"]), "5");
        assert_eq!(value(&["", "|", "0"]), "0");
        assert_eq!(value(&["3", "&", "4"]), "3");
        assert_eq!(value(&["3", "&", "0"]), "0");
    }

    #[test]
    fn string_functions() {
        assert_eq!(value(&["length", "hello"]), "5");
        assert_eq!(value(&["substr", "hello", "2", "3"]), "ell");
        assert_eq!(value(&["substr", "hello", "9", "3"]), "");
        assert_eq!(value(&["index", "hello", "ol"]), "3");
        assert_eq!(value(&["index", "hello", "z"]), "0");
        assert_eq!(value(&["+", "length"]), "length");
    }

    #[test]
    fn a_null_or_zero_result_exits_one() {
        let out = expr(&["1", "-", "1"]);
        assert_eq!((out.status, out.stdout.as_str()), (1, "0\n"));
        assert_eq!(expr(&["5"]).status, 0);
    }

    #[test]
    fn errors_exit_two() {
        for (args, message) in [
            (&["1", "/", "0"][..], "division by zero"),
            (&["1", "+"], "missing argument after '+'"),
            (&["a", "+", "1"], "non-integer argument: 'a'"),
            (&["1", "2"], "unexpected argument '2'"),
            (&["(", "1"], "expecting ')'"),
            (&["9223372036854775807", "+", "1"], "out of range"),
            (&["abc", ":", "a"], "not supported"),
            (&[], "missing operand"),
        ] {
            let out = expr(args);
            assert_eq!(out.status, 2, "{:?}", args);
            assert!(out.stderr.contains(message), "{:?}: {}", args, out.stderr);
        }
    }
}
