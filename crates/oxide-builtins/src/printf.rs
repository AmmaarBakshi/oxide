use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let Some((format, args)) = args.split_first() else {
        ewln!(io, "Usage: printf <format> [arguments...]");
        return 2;
    };

    let mut printer = Printer {
        args,
        next: 0,
        out: String::new(),
        errors: Vec::new(),
    };
    // As in other shells, the format is used again while arguments remain.
    loop {
        let before = printer.next;
        printer.render(format);
        if printer.next >= args.len() || printer.next == before {
            break;
        }
    }

    for error in &printer.errors {
        ewln!(io, "oxide: printf: {}", error);
    }
    if io.stdout.write_all(printer.out.as_bytes()).is_err() {
        return 1;
    }
    if printer.errors.is_empty() {
        0
    } else {
        1
    }
}

struct Printer<'a> {
    args: &'a [String],
    /// Index of the next argument a conversion will take.
    next: usize,
    out: String,
    errors: Vec<String>,
}

/// One `%` conversion's flags, width and precision.
#[derive(Default)]
struct Spec {
    left: bool,
    zero: bool,
    plus: bool,
    space: bool,
    width: usize,
    precision: Option<usize>,
}

impl Printer<'_> {
    fn arg(&mut self) -> Option<&str> {
        let arg = self.args.get(self.next).map(String::as_str);
        self.next += 1;
        arg
    }

    fn render(&mut self, format: &str) {
        let mut chars = format.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' => unescape_one(&mut chars, &mut self.out),
                '%' => self.conversion(&mut chars),
                _ => self.out.push(c),
            }
        }
    }

    fn conversion(&mut self, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
        let mut spec = Spec::default();
        while let Some(&flag) = chars.peek() {
            match flag {
                '-' => spec.left = true,
                '0' => spec.zero = true,
                '+' => spec.plus = true,
                ' ' => spec.space = true,
                '#' => {}
                _ => break,
            }
            chars.next();
        }
        spec.width = self.number(chars);
        if chars.peek() == Some(&'.') {
            chars.next();
            spec.precision = Some(self.number(chars));
        }

        let Some(kind) = chars.next() else {
            self.out.push('%');
            return;
        };
        let text = match kind {
            '%' => {
                self.out.push('%');
                return;
            }
            's' => {
                let arg = self.arg().unwrap_or("").to_string();
                match spec.precision {
                    Some(n) => arg.chars().take(n).collect(),
                    None => arg,
                }
            }
            'b' => {
                let arg = self.arg().unwrap_or("").to_string();
                let mut text = String::new();
                let mut chars = arg.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        unescape_one(&mut chars, &mut text);
                    } else {
                        text.push(c);
                    }
                }
                text
            }
            'c' => self
                .arg()
                .and_then(|a| a.chars().next())
                .map(String::from)
                .unwrap_or_default(),
            'd' | 'i' => {
                let n = self.integer();
                let digits = n.unsigned_abs().to_string();
                let digits = match spec.precision {
                    Some(p) if p > digits.len() => {
                        format!("{}{}", "0".repeat(p - digits.len()), digits)
                    }
                    _ => digits,
                };
                signed(n < 0, digits, &spec)
            }
            'u' => self.integer().to_string(),
            'x' => format!("{:x}", self.integer()),
            'X' => format!("{:X}", self.integer()),
            'o' => format!("{:o}", self.integer()),
            'f' | 'F' => {
                let n = self.float();
                let text = format!("{:.*}", spec.precision.unwrap_or(6), n.abs());
                signed(n.is_sign_negative() && n != 0.0, text, &spec)
            }
            'e' | 'E' => {
                let n = self.float();
                let text = exponent(n.abs(), spec.precision.unwrap_or(6), kind == 'E');
                signed(n.is_sign_negative() && n != 0.0, text, &spec)
            }
            'g' | 'G' => {
                let n = self.float();
                signed(n.is_sign_negative(), format!("{}", n.abs()), &spec)
            }
            other => {
                self.errors.push(format!("%{}: invalid conversion", other));
                return;
            }
        };
        let numeric = matches!(
            kind,
            'd' | 'i' | 'u' | 'x' | 'X' | 'o' | 'f' | 'F' | 'e' | 'E' | 'g' | 'G'
        );
        self.out.push_str(&pad(text, &spec, numeric));
    }

    /// A width or precision: digits, or `*` to take it from an argument.
    fn number(&mut self, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> usize {
        if chars.peek() == Some(&'*') {
            chars.next();
            return self.integer().max(0) as usize;
        }
        let mut n = 0;
        while let Some(d) = chars.peek().and_then(|c| c.to_digit(10)) {
            n = n * 10 + d as usize;
            chars.next();
        }
        n
    }

    /// The next argument as an integer. A missing one is 0; `'c` is the
    /// code of the character c; anything unreadable is an error and 0.
    fn integer(&mut self) -> i64 {
        let Some(arg) = self.arg().map(str::to_string) else {
            return 0;
        };
        let trimmed = arg.trim();
        if let Some(rest) = trimmed
            .strip_prefix('\'')
            .or_else(|| trimmed.strip_prefix('"'))
        {
            return rest.chars().next().map_or(0, |c| c as i64);
        }
        let (negative, digits) = match trimmed.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
        };
        let parsed = match digits
            .strip_prefix("0x")
            .or_else(|| digits.strip_prefix("0X"))
        {
            Some(hex) => i64::from_str_radix(hex, 16),
            None => digits.parse::<i64>(),
        };
        match parsed {
            Ok(n) if negative => -n,
            Ok(n) => n,
            Err(_) => {
                self.errors.push(format!("'{}': invalid number", arg));
                0
            }
        }
    }

    fn float(&mut self) -> f64 {
        let Some(arg) = self.arg().map(str::to_string) else {
            return 0.0;
        };
        arg.trim().parse().unwrap_or_else(|_| {
            self.errors.push(format!("'{}': invalid number", arg));
            0.0
        })
    }
}

/// Puts the sign (or the `+`/space flag) in front of an unsigned number.
fn signed(negative: bool, digits: String, spec: &Spec) -> String {
    let sign = if negative {
        "-"
    } else if spec.plus {
        "+"
    } else if spec.space {
        " "
    } else {
        ""
    };
    format!("{}{}", sign, digits)
}

/// `n` in C's `%e` style: one digit, the fraction, then `e+NN`.
fn exponent(n: f64, precision: usize, upper: bool) -> String {
    let text = format!("{:.*e}", precision, n);
    let (mantissa, exp) = text.split_once('e').unwrap_or((&text, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let e = if upper { 'E' } else { 'e' };
    format!(
        "{}{}{}{:02}",
        mantissa,
        e,
        if exp < 0 { '-' } else { '+' },
        exp.abs()
    )
}

/// Pads `text` to the spec's width: left-aligned with `-`, with zeros after
/// the sign for a number with `0`, otherwise with spaces on the left.
fn pad(text: String, spec: &Spec, numeric: bool) -> String {
    let len = text.chars().count();
    if len >= spec.width {
        return text;
    }
    let fill = spec.width - len;
    if spec.left {
        format!("{}{}", text, " ".repeat(fill))
    } else if spec.zero && numeric {
        let sign_len = usize::from(text.starts_with(['-', '+', ' ']));
        format!(
            "{}{}{}",
            &text[..sign_len],
            "0".repeat(fill),
            &text[sign_len..]
        )
    } else {
        format!("{}{}", " ".repeat(fill), text)
    }
}

/// Writes the character a backslash escape stands for (the backslash itself
/// already read): `\n`, `\t`, `\\`, `\0NNN` octal, and the rest of C's set.
fn unescape_one(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, out: &mut String) {
    let Some(c) = chars.next() else {
        out.push('\\');
        return;
    };
    let escaped = match c {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        'a' => '\x07',
        'b' => '\x08',
        'f' => '\x0c',
        'v' => '\x0b',
        'e' => '\x1b',
        '\\' => '\\',
        '"' => '"',
        '\'' => '\'',
        '0' => {
            let mut value = 0u32;
            for _ in 0..3 {
                match chars.peek().and_then(|c| c.to_digit(8)) {
                    Some(d) => {
                        value = value * 8 + d;
                        chars.next();
                    }
                    None => break,
                }
            }
            char::from_u32(value).unwrap_or('\0')
        }
        other => {
            out.push('\\');
            other
        }
    };
    out.push(escaped);
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    fn printf(args: &[&str]) -> String {
        let out = run("printf", args, "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        out.stdout
    }

    #[test]
    fn strings_and_escapes() {
        assert_eq!(printf(&["%s-%s\\n", "a", "b"]), "a-b\n");
        assert_eq!(printf(&["no newline"]), "no newline");
        assert_eq!(printf(&["tab\\there \\\\ 100%%\\n"]), "tab\there \\ 100%\n");
        assert_eq!(printf(&["%b", "a\\tb"]), "a\tb");
    }

    #[test]
    fn the_format_repeats_for_extra_arguments() {
        assert_eq!(printf(&["[%s]", "a", "b", "c"]), "[a][b][c]");
        assert_eq!(printf(&["%s=%s ", "k1", "v1", "k2"]), "k1=v1 k2= ");
    }

    #[test]
    fn integers_with_width_flags_and_bases() {
        assert_eq!(
            printf(&["%5d|%-5d|%05d", "42", "42", "-42"]),
            "   42|42   |-0042"
        );
        assert_eq!(printf(&["%+d % d %.3d", "7", "7", "7"]), "+7  7 007");
        assert_eq!(printf(&["%x %X %o", "255", "255", "8"]), "ff FF 10");
        assert_eq!(printf(&["%d", "'A"]), "65");
    }

    #[test]
    fn floats_and_precision() {
        assert_eq!(printf(&["%.2f %f", "3.14159", "1"]), "3.14 1.000000");
        assert_eq!(printf(&["%8.3f|", "-2.5"]), "  -2.500|");
        assert_eq!(printf(&["%.2e", "1234.5"]), "1.23e+03");
        assert_eq!(printf(&["%.3s|%*s", "abcdef", "4", "x"]), "abc|   x");
    }

    #[test]
    fn bad_numbers_are_reported_but_printing_goes_on() {
        let out = run("printf", &["%d %d\\n", "x", "2"], "");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, "0 2\n");
        assert_eq!(out.stderr, "oxide: printf: 'x': invalid number\n");
    }

    #[test]
    fn missing_format_is_a_usage_error() {
        assert_eq!(run("printf", &[], "").status, 2);
    }
}
