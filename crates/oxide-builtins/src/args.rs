//! Option parsing for builtins that take single-letter flags.

/// Splits `args` into the flags present and the operands.
///
/// Flags may be bundled (`-rn` is `-r -n`), and `--` ends option parsing so a
/// file named like a flag can still be passed. A lone `-` is an operand,
/// conventionally meaning stdin. Any flag not listed in `allowed` is an
/// error, returned as a message ready to print after the command name.
pub fn parse_flags<'a>(
    args: &'a [String],
    allowed: &str,
) -> Result<(String, Vec<&'a str>), String> {
    let mut flags = String::new();
    let mut operands = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            operands.extend(iter.by_ref().map(String::as_str));
            break;
        }
        match arg.strip_prefix('-') {
            Some(long) if long.starts_with('-') => {
                return Err(format!("unrecognized option '{}'", arg));
            }
            Some(bundle) if !bundle.is_empty() => {
                for flag in bundle.chars() {
                    if !allowed.contains(flag) {
                        return Err(format!("invalid option -- '{}'", flag));
                    }
                    flags.push(flag);
                }
            }
            _ => operands.push(arg.as_str()),
        }
    }
    Ok((flags, operands))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn separates_bundled_flags_from_operands() {
        let args = strings(&["-rn", "a.txt", "-u", "-", "b.txt"]);
        let (flags, operands) = parse_flags(&args, "rnu").unwrap();
        assert_eq!(flags, "rnu");
        assert_eq!(operands, ["a.txt", "-", "b.txt"]);
    }

    #[test]
    fn double_dash_ends_options() {
        let args = strings(&["-r", "--", "-n", "--"]);
        let (flags, operands) = parse_flags(&args, "rn").unwrap();
        assert_eq!(flags, "r");
        assert_eq!(operands, ["-n", "--"]);
    }

    #[test]
    fn rejects_unknown_flags() {
        let args = strings(&["-rx"]);
        assert_eq!(parse_flags(&args, "r").unwrap_err(), "invalid option -- 'x'");
        let args = strings(&["--reverse"]);
        assert_eq!(parse_flags(&args, "r").unwrap_err(), "unrecognized option '--reverse'");
    }
}
