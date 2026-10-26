use std::io::{self, BufRead, Write};

use crate::args::parse_flags;
use crate::{Ctx, Io};

/// What to do with each character, worked out from the flags and sets.
struct Plan {
    /// Characters dropped outright (`-d`).
    delete: Vec<char>,
    /// `from[i]` becomes `to[i]`.
    from: Vec<char>,
    to: Vec<char>,
    /// Runs of these collapse to one (`-s`), checked after translation.
    squeeze: Vec<char>,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let plan = match parse_flags(args, "ds").and_then(|(flags, sets)| plan(&flags, &sets)) {
        Ok(plan) => plan,
        Err(msg) => {
            ewln!(io, "tr: {}", msg);
            ewln!(io, "Usage: tr [-ds] SET1 [SET2]");
            return 1;
        }
    };
    match translate(io.stdin, io.stdout, &plan) {
        Ok(()) => 0,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => 1,
        Err(e) => {
            ewln!(io, "tr: {}", e);
            1
        }
    }
}

/// Checks the sets make sense for the flags given, the way coreutils does:
/// `-d` takes one set (two with `-s`, the second being what to squeeze),
/// plain translation takes two, and `-s` alone takes one.
fn plan(flags: &str, sets: &[&str]) -> Result<Plan, String> {
    let (delete, squeeze) = (flags.contains('d'), flags.contains('s'));
    let wanted = match (delete, squeeze, sets.len()) {
        (false, true, 1) => 1,
        (true, false, _) => 1,
        _ => 2,
    };
    match sets.len().cmp(&wanted) {
        std::cmp::Ordering::Less if sets.is_empty() => return Err("missing operand".into()),
        std::cmp::Ordering::Less => {
            return Err(format!("missing operand after '{}'", sets[0]));
        }
        std::cmp::Ordering::Greater => {
            return Err(format!("extra operand '{}'", sets[wanted]));
        }
        std::cmp::Ordering::Equal => {}
    }

    let first = expand(sets[0])?;
    let second = match sets.get(1) {
        Some(set) => expand(set)?,
        None => Vec::new(),
    };
    let mut plan = Plan {
        delete: Vec::new(),
        from: Vec::new(),
        to: Vec::new(),
        squeeze: Vec::new(),
    };
    if delete {
        plan.delete = first;
        plan.squeeze = second;
    } else if sets.len() == 1 {
        plan.squeeze = first;
    } else {
        // A short SET2 is padded with its last character, so `tr a-z X`
        // turns every letter into X.
        let Some(&last) = second.last() else {
            return Err("when not truncating set1, string2 must be non-empty".into());
        };
        plan.to = (0..first.len())
            .map(|i| second.get(i).copied().unwrap_or(last))
            .collect();
        plan.from = first;
        if squeeze {
            plan.squeeze = second;
        }
    }
    Ok(plan)
}

/// Expands ranges (`a-z`), classes (`[:digit:]`) and backslash escapes
/// (`\n`, `\t`, `\\`) into the characters they stand for.
fn expand(set: &str) -> Result<Vec<char>, String> {
    let chars: Vec<char> = set.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' && chars.get(i + 1) == Some(&':') {
            let rest: String = chars[i + 2..].iter().collect();
            if let Some(end) = rest.find(":]") {
                out.extend(class(&rest[..end])?);
                i += 2 + rest[..end].chars().count() + 2;
                continue;
            }
        }
        let (start, used) = literal(&chars[i..]);
        i += used;
        if chars.get(i) == Some(&'-') && i + 1 < chars.len() {
            let (end, used) = literal(&chars[i + 1..]);
            if end < start {
                return Err(format!(
                    "range-endpoints of '{}-{}' are in reverse collating sequence order",
                    start, end
                ));
            }
            out.extend(start..=end);
            i += 1 + used;
        } else {
            out.push(start);
        }
    }
    Ok(out)
}

/// The character at the front of `chars`, decoding a backslash escape, and
/// how many characters it took up.
fn literal(chars: &[char]) -> (char, usize) {
    match chars {
        ['\\', 'n', ..] => ('\n', 2),
        ['\\', 't', ..] => ('\t', 2),
        ['\\', 'r', ..] => ('\r', 2),
        ['\\', c, ..] => (*c, 2),
        [c, ..] => (*c, 1),
        [] => unreachable!("called with at least one character"),
    }
}

fn class(name: &str) -> Result<Vec<char>, String> {
    let test: fn(&char) -> bool = match name {
        "alnum" => |c| c.is_ascii_alphanumeric(),
        "alpha" => |c| c.is_ascii_alphabetic(),
        "digit" => |c| c.is_ascii_digit(),
        "lower" => |c| c.is_ascii_lowercase(),
        "upper" => |c| c.is_ascii_uppercase(),
        "space" => |c| c.is_ascii_whitespace() || *c == '\x0b',
        "blank" => |c| *c == ' ' || *c == '\t',
        "punct" => |c| c.is_ascii_punctuation(),
        _ => return Err(format!("invalid character class '{}'", name)),
    };
    Ok((0u8..128).map(char::from).filter(test).collect())
}

/// Streams `input` through the plan line by line. The last character
/// written is carried across lines so a squeezed run can span them.
fn translate(input: &mut dyn BufRead, out: &mut dyn Write, plan: &Plan) -> io::Result<()> {
    let mut previous = None;
    let mut line = Vec::new();
    let mut buf = String::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        buf.clear();
        for c in String::from_utf8_lossy(&line).chars() {
            if plan.delete.contains(&c) {
                continue;
            }
            let c = match plan.from.iter().rposition(|&f| f == c) {
                Some(i) => plan.to[i],
                None => c,
            };
            if previous == Some(c) && plan.squeeze.contains(&c) {
                continue;
            }
            previous = Some(c);
            buf.push(c);
        }
        out.write_all(buf.as_bytes())?;
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn translates_ranges() {
        assert_eq!(
            run("tr", &["a-z", "A-Z"], "Hello, world\n").stdout,
            "HELLO, WORLD\n"
        );
        assert_eq!(run("tr", &["a-y", "b-z"], "hello\n").stdout, "ifmmp\n");
    }

    #[test]
    fn pads_a_short_second_set_with_its_last_character() {
        assert_eq!(run("tr", &["abc", "x"], "aabbccd\n").stdout, "xxxxxxd\n");
    }

    #[test]
    fn deletes_characters_and_classes() {
        assert_eq!(run("tr", &["-d", "[:digit:]"], "a1b22c\n").stdout, "abc\n");
        assert_eq!(run("tr", &["-d", "\\n"], "a\nb\n").stdout, "ab");
    }

    #[test]
    fn squeezes_runs() {
        assert_eq!(run("tr", &["-s", " "], "a   b    c\n").stdout, "a b c\n");
        // Translation first, then the runs of SET2 collapse.
        assert_eq!(run("tr", &["-s", "a-c", "x"], "aabbcc d\n").stdout, "x d\n");
    }

    #[test]
    fn checks_the_operands() {
        let out = run("tr", &["abc"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("missing operand after 'abc'"));
        let out = run("tr", &["-d", "a", "b"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'b'"));
        let out = run("tr", &["z-a", "x"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("reverse collating sequence order"));
    }
}
