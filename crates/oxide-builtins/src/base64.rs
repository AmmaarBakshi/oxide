use std::io::{self, ErrorKind, Write};

use crate::{Ctx, Io};

const DEFAULT_WRAP: usize = 76;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (decoding, wrap, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "base64: {}", msg);
            ewln!(io, "Usage: base64 [-d] [-w COLS] [file]");
            return 1;
        }
    };
    io.each_input("base64", &files, |_, input, out| {
        let mut data = Vec::new();
        input.read_to_end(&mut data)?;
        if decoding {
            let bytes = decode(&data)
                .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "invalid input"))?;
            out.write_all(&bytes)
        } else {
            write_wrapped(out, &encode(&data), wrap)
        }
    })
}

fn parse_args(args: &[String]) -> Result<(bool, usize, Vec<&str>), String> {
    let mut decoding = false;
    let mut wrap = DEFAULT_WRAP;
    let mut files = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = match arg.as_str() {
            "--" => {
                files.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-d" => {
                decoding = true;
                continue;
            }
            "-w" => iter
                .next()
                .map(String::as_str)
                .ok_or("option requires an argument -- 'w'")?,
            other => match other.strip_prefix("-w") {
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
        wrap = value
            .parse()
            .map_err(|_| format!("invalid wrap size: '{}'", value))?;
    }
    // One input, as in coreutils: the output is a single encoding, and
    // concatenating two of them would not decode back to the two files.
    if let Some(extra) = files.get(1) {
        return Err(format!("extra operand '{}'", extra));
    }
    Ok((decoding, wrap, files))
}

/// Standard base64 with `=` padding.
fn encode(data: &[u8]) -> String {
    let mut text = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let byte = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let group = (byte(0) << 16) | (byte(1) << 8) | byte(2);
        for i in 0..4 {
            // A chunk of n bytes fills n + 1 characters; the rest is padding.
            if i <= chunk.len() {
                text.push(ALPHABET[((group >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// Decodes base64, skipping whitespace so wrapped and CRLF input both work.
/// Returns `None` on a character outside the alphabet, data after the
/// padding, or a dangling final character.
fn decode(text: &[u8]) -> Option<Vec<u8>> {
    let mut data = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    let mut padded = false;
    for &c in text {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                padded = true;
                continue;
            }
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        if padded {
            return None;
        }
        acc = (acc << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            data.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    // Six bits left over means a group had a single character, which can't
    // encode a whole byte.
    (bits != 6).then_some(data)
}

/// Writes `text` in lines of `cols` characters. A width of 0 means one
/// unbroken run with no newline at all, which is what makes `-w 0` handy
/// for putting an encoding inside another command.
fn write_wrapped(out: &mut dyn Write, text: &str, cols: usize) -> io::Result<()> {
    if cols == 0 {
        return out.write_all(text.as_bytes());
    }
    for line in text.as_bytes().chunks(cols) {
        out.write_all(line)?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn encodes_with_padding() {
        assert_eq!(run("base64", &[], "hello").stdout, "aGVsbG8=\n");
        assert_eq!(run("base64", &[], "a").stdout, "YQ==\n");
        assert_eq!(run("base64", &[], "ab").stdout, "YWI=\n");
        assert_eq!(run("base64", &[], "abc").stdout, "YWJj\n");
        assert_eq!(run("base64", &[], "").stdout, "");
    }

    #[test]
    fn wraps_at_76_columns_unless_told_otherwise() {
        let out = run("base64", &[], &"x".repeat(60));
        let lines: Vec<&str> = out.stdout.lines().collect();
        assert_eq!(lines.iter().map(|l| l.len()).collect::<Vec<_>>(), [76, 4]);

        assert_eq!(run("base64", &["-w", "4"], "hello").stdout, "aGVs\nbG8=\n");
        assert_eq!(run("base64", &["-w0"], "hello").stdout, "aGVsbG8=");
    }

    #[test]
    fn dash_d_decodes_wrapped_and_crlf_input() {
        assert_eq!(run("base64", &["-d"], "aGVsbG8=\n").stdout, "hello");
        assert_eq!(run("base64", &["-d"], "aGVs\r\nbG8=\r\n").stdout, "hello");
        assert_eq!(run("base64", &["-d"], "YWI").stdout, "ab");
    }

    #[test]
    fn round_trips_every_byte_count() {
        for len in 0..10 {
            let text: String = "oxide-shell".chars().take(len).collect();
            let encoded = super::encode(text.as_bytes());
            assert_eq!(super::decode(encoded.as_bytes()).unwrap(), text.as_bytes());
        }
    }

    #[test]
    fn rejects_invalid_input() {
        for bad in ["a!bc", "YQ==YQ", "Y"] {
            let out = run("base64", &["-d"], bad);
            assert_eq!(out.status, 1, "input: {:?}", bad);
            assert!(out.stderr.contains("invalid input"), "{}", out.stderr);
        }
    }

    #[test]
    fn takes_at_most_one_file() {
        let out = run("base64", &["a", "b"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("extra operand 'b'"));
    }
}
