use std::io::{self, ErrorKind};

use crate::base64::{parse_args, write_wrapped};
use crate::{Ctx, Io};

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// `base32` is `base64` with the RFC 4648 base32 alphabet: same options,
/// same wrapping, same single input.
pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (decoding, wrap, files) = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "base32: {}", msg);
            ewln!(io, "Usage: base32 [-d] [-w COLS] [file]");
            return 1;
        }
    };
    io.each_input("base32", &files, |_, input, out| {
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

/// Standard base32 with `=` padding: every 5 bytes become 8 characters.
fn encode(data: &[u8]) -> String {
    let mut text = String::with_capacity(data.len().div_ceil(5) * 8);
    for chunk in data.chunks(5) {
        let group = (0..5).fold(0u64, |acc, i| {
            (acc << 8) | u64::from(chunk.get(i).copied().unwrap_or(0))
        });
        // A chunk of n bytes holds 8n bits, which take ceil(8n / 5)
        // characters; the rest of the 8 is padding.
        let used = (chunk.len() * 8).div_ceil(5);
        for i in 0..8 {
            if i < used {
                text.push(ALPHABET[((group >> (35 - 5 * i)) & 31) as usize] as char);
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// Decodes base32, skipping whitespace so wrapped and CRLF input both work.
/// Returns `None` on a character outside the alphabet, data after the
/// padding, or a final group too short to hold a whole byte.
fn decode(text: &[u8]) -> Option<Vec<u8>> {
    let mut data = Vec::with_capacity(text.len() / 8 * 5);
    let (mut acc, mut bits) = (0u32, 0);
    let mut padded = false;
    for &c in text {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'2'..=b'7' => c - b'2' + 26,
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
        acc = (acc << 5) | u32::from(value);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            data.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    // A final group of 1, 3 or 6 characters leaves 5 or more bits over:
    // a whole character that encodes no byte.
    (bits < 5).then_some(data)
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    /// The test vectors from RFC 4648, section 10.
    const VECTORS: [(&str, &str); 7] = [
        ("", ""),
        ("f", "MY======"),
        ("fo", "MZXQ===="),
        ("foo", "MZXW6==="),
        ("foob", "MZXW6YQ="),
        ("fooba", "MZXW6YTB"),
        ("foobar", "MZXW6YTBOI======"),
    ];

    #[test]
    fn encodes_the_rfc_vectors() {
        for (plain, encoded) in VECTORS {
            assert_eq!(
                super::encode(plain.as_bytes()),
                encoded,
                "input: {:?}",
                plain
            );
        }
        assert_eq!(run("base32", &[], "foobar").stdout, "MZXW6YTBOI======\n");
        assert_eq!(run("base32", &[], "").stdout, "");
    }

    #[test]
    fn decodes_the_rfc_vectors() {
        for (plain, encoded) in VECTORS {
            assert_eq!(
                super::decode(encoded.as_bytes()).unwrap(),
                plain.as_bytes(),
                "input: {:?}",
                encoded
            );
        }
        assert_eq!(
            run("base32", &["-d"], "MZXW6YTB\r\nOI======\r\n").stdout,
            "foobar"
        );
    }

    #[test]
    fn wraps_like_base64() {
        let out = run("base32", &[], &"x".repeat(50));
        let lines: Vec<&str> = out.stdout.lines().collect();
        assert_eq!(lines.iter().map(|l| l.len()).collect::<Vec<_>>(), [76, 4]);

        assert_eq!(
            run("base32", &["-w", "8"], "foobar").stdout,
            "MZXW6YTB\nOI======\n"
        );
        assert_eq!(run("base32", &["-w0"], "foobar").stdout, "MZXW6YTBOI======");
    }

    #[test]
    fn round_trips_every_byte_count() {
        let bytes: Vec<u8> = (0..=255).collect();
        for len in 0..12 {
            let encoded = super::encode(&bytes[250 - len..250]);
            assert_eq!(
                super::decode(encoded.as_bytes()).unwrap(),
                &bytes[250 - len..250]
            );
        }
    }

    #[test]
    fn rejects_invalid_input() {
        // Lower case, a base64-only digit, data after padding, and final
        // groups of 1, 3 and 6 characters.
        for bad in ["my======", "MY1=====", "MY======MY", "M", "MZX", "MZXW6Y"] {
            let out = run("base32", &["-d"], bad);
            assert_eq!(out.status, 1, "input: {:?}", bad);
            assert!(out.stderr.contains("invalid input"), "{}", out.stderr);
        }
    }

    #[test]
    fn takes_at_most_one_file() {
        let out = run("base32", &["a", "b"], "");
        assert_eq!(out.status, 1);
        assert!(
            out.stderr.starts_with("base32: extra operand 'b'"),
            "{}",
            out.stderr
        );
    }
}
