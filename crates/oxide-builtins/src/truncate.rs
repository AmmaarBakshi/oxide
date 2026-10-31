use std::fs::OpenOptions;
use std::io::{self, ErrorKind};

use crate::{Ctx, Io};

/// How a `-s` size relates to the file's current one.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Adjust {
    /// `N`: exactly this size.
    Set,
    /// `+N`: this much longer.
    Grow,
    /// `-N`: this much shorter, stopping at empty.
    Shrink,
    /// `<N`: no larger than this.
    AtMost,
    /// `>N`: no smaller than this.
    AtLeast,
}

/// A parsed `-s` operand: how to adjust, and by how many bytes.
type Size = (Adjust, u64);

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (size, no_create, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "truncate: {}", msg);
            ewln!(io, "Usage: truncate [-c] -s [+-<>]SIZE FILE...");
            return 1;
        }
    };

    let mut status = 0;
    for file in files {
        if let Err(e) = resize(file, size, no_create) {
            ewln!(io, "truncate: {}: {}", file, e);
            status = 1;
        }
    }
    status
}

fn parse_args(args: &[String]) -> Result<(Size, bool, Vec<&str>), String> {
    let mut size = None;
    let mut no_create = false;
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = match arg.as_str() {
            "--" => {
                files.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-c" => {
                no_create = true;
                continue;
            }
            // Taken whole, so `-s -5` is a size and not another option.
            "-s" => iter
                .next()
                .map(String::as_str)
                .ok_or("option requires an argument -- 's'")?,
            other => match other.strip_prefix("-s") {
                Some(value) => value,
                None if other.len() > 1 && other.starts_with('-') => {
                    return Err(format!("invalid option '{}'", other));
                }
                None => {
                    files.push(other);
                    continue;
                }
            },
        };
        size = Some(parse_size(value).ok_or_else(|| format!("invalid size: '{}'", value))?);
    }
    let size = size.ok_or("you must give a size with -s")?;
    if files.is_empty() {
        return Err("missing file operand".to_string());
    }
    Ok((size, no_create, files))
}

/// Parses `[+-<>]N[SUFFIX]`. `K`, `M`, `G` and `T` (or `KiB` and so on)
/// are powers of 1024; `KB`, `MB`, `GB` and `TB` are powers of 1000.
fn parse_size(text: &str) -> Option<Size> {
    let (adjust, rest) = match text.chars().next()? {
        '+' => (Adjust::Grow, &text[1..]),
        '-' => (Adjust::Shrink, &text[1..]),
        '<' => (Adjust::AtMost, &text[1..]),
        '>' => (Adjust::AtLeast, &text[1..]),
        _ => (Adjust::Set, text),
    };
    let split = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let (digits, suffix) = rest.split_at(split);
    let n: u64 = digits.parse().ok()?;

    let multiplier = if suffix.is_empty() {
        1
    } else {
        let power = match suffix.as_bytes()[0].to_ascii_uppercase() {
            b'K' => 1,
            b'M' => 2,
            b'G' => 3,
            b'T' => 4,
            _ => return None,
        };
        let base: u64 = match &suffix[1..] {
            "" | "iB" => 1024,
            "B" => 1000,
            _ => return None,
        };
        base.pow(power)
    };
    Some((adjust, n.checked_mul(multiplier)?))
}

/// Sets `file` to the size `-s` asks for, creating it first unless
/// `no_create`, in which case a missing file is quietly left alone.
fn resize(file: &str, (adjust, n): Size, no_create: bool) -> io::Result<()> {
    let handle = match OpenOptions::new()
        .write(true)
        .create(!no_create)
        .truncate(false)
        .open(file)
    {
        Err(e) if no_create && e.kind() == ErrorKind::NotFound => return Ok(()),
        result => result?,
    };
    let current = handle.metadata()?.len();
    let target = match adjust {
        Adjust::Set => n,
        Adjust::Grow => current
            .checked_add(n)
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "size too large"))?,
        Adjust::Shrink => current.saturating_sub(n),
        Adjust::AtMost => current.min(n),
        Adjust::AtLeast => current.max(n),
    };
    handle.set_len(target)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{parse_size, Adjust};
    use crate::test_support::{run, scratch_dir};

    #[test]
    fn creates_or_cuts_a_file_to_size() {
        let dir = scratch_dir("truncate_set");
        let (new, old) = (dir.join("new.bin"), dir.join("old.txt"));
        fs::write(&old, "hello world").unwrap();

        let out = run("truncate", &["-s", "10", new.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(fs::read(&new).unwrap(), [0; 10]);

        let out = run("truncate", &["-s5", old.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(fs::read_to_string(&old).unwrap(), "hello");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn relative_sizes_start_from_the_current_one() {
        let dir = scratch_dir("truncate_relative");
        let file = dir.join("f");
        fs::write(&file, "12345").unwrap();
        let path = file.to_str().unwrap();
        let size_after = |size: &str| {
            let out = run("truncate", &["-s", size, path], "");
            assert_eq!(out.status, 0, "{}", out.stderr);
            fs::metadata(&file).unwrap().len()
        };

        assert_eq!(size_after("+3"), 8);
        assert_eq!(size_after("-2"), 6);
        assert_eq!(size_after("<4"), 4);
        assert_eq!(size_after("<9"), 4);
        assert_eq!(size_after(">7"), 7);
        assert_eq!(size_after("-100"), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn suffixes_scale_by_1024_or_1000() {
        assert_eq!(parse_size("1K"), Some((Adjust::Set, 1024)));
        assert_eq!(parse_size("2KiB"), Some((Adjust::Set, 2048)));
        assert_eq!(parse_size("+1KB"), Some((Adjust::Grow, 1000)));
        assert_eq!(parse_size("3m"), Some((Adjust::Set, 3 << 20)));
        assert_eq!(parse_size("1GB"), Some((Adjust::Set, 1_000_000_000)));
        for bad in ["", "+", "1X", "1KX", "x1", "99999999999T"] {
            assert_eq!(parse_size(bad), None, "{:?}", bad);
        }
    }

    #[test]
    fn dash_c_does_not_create_missing_files() {
        let dir = scratch_dir("truncate_no_create");
        let missing = dir.join("missing");
        let out = run(
            "truncate",
            &["-c", "-s", "4", missing.to_str().unwrap()],
            "",
        );
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(!missing.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn needs_a_size_and_a_file() {
        let out = run("truncate", &["f"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("you must give a size with -s"));

        let out = run("truncate", &["-s", "1"], "");
        assert!(out.stderr.contains("missing file operand"));

        let out = run("truncate", &["-s", "big", "f"], "");
        assert!(out.stderr.contains("invalid size: 'big'"));
    }
}
