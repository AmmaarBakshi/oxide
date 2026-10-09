//! `test EXPR` and `[ EXPR ]`: check files, strings and numbers, and say
//! the answer through the exit status (0 true, 1 false, 2 error).

use std::fs;
use std::path::Path;

use crate::{Ctx, Io};

pub fn run_test(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    evaluate("test", args, io)
}

pub fn run_bracket(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    match args.split_last() {
        Some((last, expr)) if last == "]" => evaluate("[", expr, io),
        _ => {
            ewln!(io, "oxide: [: missing ']'");
            2
        }
    }
}

fn evaluate(name: &str, args: &[String], io: &mut Io<'_>) -> i32 {
    let mut parser = Expr { args, pos: 0 };
    // No expression at all is false, as in every shell.
    if args.is_empty() {
        return 1;
    }
    let result = parser
        .or()
        .and_then(|value| match parser.args.get(parser.pos) {
            None => Ok(value),
            Some(extra) => Err(format!("{}: unexpected argument", extra)),
        });
    match result {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(msg) => {
            ewln!(io, "oxide: {}: {}", name, msg);
            2
        }
    }
}

/// Recursive descent over the arguments: `-o` binds loosest, then `-a`,
/// then `!`, then the primaries.
struct Expr<'a> {
    args: &'a [String],
    pos: usize,
}

const UNARY: &[&str] = &[
    "-e", "-f", "-d", "-r", "-w", "-x", "-s", "-L", "-h", "-z", "-n",
];
const BINARY: &[&str] = &[
    "=", "==", "!=", "<", ">", "-eq", "-ne", "-lt", "-le", "-gt", "-ge", "-nt", "-ot",
];

impl Expr<'_> {
    fn peek(&self, offset: usize) -> Option<&str> {
        self.args.get(self.pos + offset).map(String::as_str)
    }

    fn take(&mut self) -> Result<&str, String> {
        let arg = self
            .args
            .get(self.pos)
            .map(String::as_str)
            .ok_or_else(|| "argument expected".to_string())?;
        self.pos += 1;
        Ok(arg)
    }

    fn or(&mut self) -> Result<bool, String> {
        let mut value = self.and()?;
        while self.peek(0) == Some("-o") {
            self.pos += 1;
            let right = self.and()?;
            value = value || right;
        }
        Ok(value)
    }

    fn and(&mut self) -> Result<bool, String> {
        let mut value = self.not()?;
        while self.peek(0) == Some("-a") {
            self.pos += 1;
            let right = self.not()?;
            value = value && right;
        }
        Ok(value)
    }

    fn not(&mut self) -> Result<bool, String> {
        // A `!` with nothing after it is just the string "!", which is true.
        if self.peek(0) == Some("!") && self.peek(1).is_some() {
            self.pos += 1;
            return Ok(!self.not()?);
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<bool, String> {
        if self.peek(0) == Some("(") && self.peek(1).is_some() {
            self.pos += 1;
            let value = self.or()?;
            if self.take()? != ")" {
                return Err("')' expected".to_string());
            }
            return Ok(value);
        }
        // `a OP b` wins over a unary reading, so `[ -n = -n ]` compares.
        if let (Some(op), Some(_)) = (self.peek(1), self.peek(2)) {
            if BINARY.contains(&op) {
                let left = self.take()?.to_string();
                let op = self.take()?.to_string();
                let right = self.take()?.to_string();
                return binary(&left, &op, &right);
            }
        }
        if let (Some(op), Some(_)) = (self.peek(0), self.peek(1)) {
            if UNARY.contains(&op) {
                let op = self.take()?.to_string();
                let operand = self.take()?.to_string();
                return Ok(unary(&op, &operand));
            }
        }
        // A lone string: true when it isn't empty.
        Ok(!self.take()?.is_empty())
    }
}

fn unary(op: &str, operand: &str) -> bool {
    let path = Path::new(operand);
    match op {
        "-z" => operand.is_empty(),
        "-n" => !operand.is_empty(),
        "-e" => path.exists(),
        "-f" => path.is_file(),
        "-d" => path.is_dir(),
        "-s" => fs::metadata(path).is_ok_and(|m| m.len() > 0),
        "-L" | "-h" => fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()),
        "-r" => fs::File::open(path).is_ok() || path.is_dir(),
        "-w" => fs::metadata(path).is_ok_and(|m| !m.permissions().readonly()),
        "-x" => is_executable(path),
        _ => false,
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

/// Windows has no execute bit: directories, and files with a runnable
/// extension, count.
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    let runnable = ["exe", "bat", "cmd", "com", "ps1"];
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| runnable.contains(&e.to_ascii_lowercase().as_str()))
}

fn binary(left: &str, op: &str, right: &str) -> Result<bool, String> {
    let number = |s: &str| {
        s.trim()
            .parse::<i64>()
            .map_err(|_| format!("{}: integer expression expected", s))
    };
    let modified = |s: &str| fs::metadata(s).and_then(|m| m.modified()).ok();
    Ok(match op {
        "=" | "==" => left == right,
        "!=" => left != right,
        "<" => left < right,
        ">" => left > right,
        "-eq" => number(left)? == number(right)?,
        "-ne" => number(left)? != number(right)?,
        "-lt" => number(left)? < number(right)?,
        "-le" => number(left)? <= number(right)?,
        "-gt" => number(left)? > number(right)?,
        "-ge" => number(left)? >= number(right)?,
        "-nt" => matches!((modified(left), modified(right)), (Some(a), Some(b)) if a > b),
        "-ot" => matches!((modified(left), modified(right)), (Some(a), Some(b)) if a < b),
        _ => unreachable!("{op} is listed in BINARY"),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    fn test(args: &[&str]) -> i32 {
        run("test", args, "").status
    }

    #[test]
    fn strings() {
        assert_eq!(test(&["abc"]), 0);
        assert_eq!(test(&[""]), 1);
        assert_eq!(test(&[]), 1);
        assert_eq!(test(&["-z", ""]), 0);
        assert_eq!(test(&["-n", ""]), 1);
        assert_eq!(test(&["a", "=", "a"]), 0);
        assert_eq!(test(&["a", "!=", "a"]), 1);
        assert_eq!(test(&["-n", "=", "-n"]), 0);
    }

    #[test]
    fn numbers() {
        assert_eq!(test(&["10", "-gt", "9"]), 0);
        assert_eq!(test(&["10", "-lt", "9"]), 1);
        assert_eq!(test(&["-3", "-le", "-3"]), 0);
        let out = run("test", &["x", "-eq", "1"], "");
        assert_eq!(out.status, 2);
        assert_eq!(out.stderr, "oxide: test: x: integer expression expected\n");
    }

    #[test]
    fn negation_and_connectives() {
        assert_eq!(test(&["!", "a", "=", "b"]), 0);
        assert_eq!(test(&["a", "-a", ""]), 1);
        assert_eq!(test(&["", "-o", "b"]), 0);
        assert_eq!(test(&["(", "a", "-o", "", ")", "-a", "b"]), 0);
        assert_eq!(test(&["!"]), 0);
    }

    #[test]
    fn files() {
        let dir = scratch_dir("test_files");
        let file = dir.join("data.txt");
        fs::write(&file, "x").unwrap();
        let empty = dir.join("empty.txt");
        fs::write(&empty, "").unwrap();
        let (dir_s, file_s, empty_s) = (
            dir.to_str().unwrap(),
            file.to_str().unwrap(),
            empty.to_str().unwrap(),
        );
        assert_eq!(test(&["-d", dir_s]), 0);
        assert_eq!(test(&["-f", dir_s]), 1);
        assert_eq!(test(&["-f", file_s]), 0);
        assert_eq!(test(&["-e", &format!("{}.nope", file_s)]), 1);
        assert_eq!(test(&["-s", file_s]), 0);
        assert_eq!(test(&["-s", empty_s]), 1);
        assert_eq!(test(&["-r", file_s]), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn bracket_needs_its_closing_bracket() {
        assert_eq!(run("[", &["a", "=", "a", "]"], "").status, 0);
        assert_eq!(run("[", &["]"], "").status, 1);
        let out = run("[", &["a", "=", "a"], "");
        assert_eq!(out.status, 2);
        assert_eq!(out.stderr, "oxide: [: missing ']'\n");
    }

    #[test]
    fn leftover_arguments_are_an_error() {
        assert_eq!(run("test", &["a", "b"], "").status, 2);
    }
}
