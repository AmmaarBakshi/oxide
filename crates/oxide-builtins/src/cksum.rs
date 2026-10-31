use std::io::{self, BufRead};

use crate::args::parse_flags;
use crate::{Ctx, Io};

/// The CRC-32 polynomial POSIX specifies for `cksum`.
const POLY: u32 = 0x04c1_1db7;

const TABLE: [u32; 256] = table();

const fn table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = (i as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ POLY
            } else {
                crc << 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, files) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "cksum: {}", msg);
            ewln!(io, "Usage: cksum [file...]");
            return 1;
        }
    };
    // Reading stdin by default, there's no name to print.
    let named = !files.is_empty();
    io.each_input("cksum", &files, |name, input, out| {
        let (crc, len) = checksum(input)?;
        if named {
            writeln!(out, "{} {} {}", crc, len, name)
        } else {
            writeln!(out, "{} {}", crc, len)
        }
    })
}

/// The POSIX checksum of everything in `input`, and its length in bytes.
/// The length is folded into the CRC too, so inputs differing only in
/// trailing zero bytes still get different sums.
fn checksum(input: &mut dyn BufRead) -> io::Result<(u32, u64)> {
    let mut crc = 0;
    let mut len = 0u64;
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            break;
        }
        for &byte in buf {
            crc = update(crc, byte);
        }
        len += buf.len() as u64;
        let read = buf.len();
        input.consume(read);
    }
    let mut rest = len;
    while rest > 0 {
        crc = update(crc, rest as u8);
        rest >>= 8;
    }
    Ok((!crc, len))
}

fn update(crc: u32, byte: u8) -> u32 {
    (crc << 8) ^ TABLE[((crc >> 24) as u8 ^ byte) as usize]
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn matches_coreutils() {
        assert_eq!(run("cksum", &[], "").stdout, "4294967295 0\n");
        assert_eq!(run("cksum", &[], "hello\n").stdout, "3015617425 6\n");
        assert_eq!(run("cksum", &[], "123456789").stdout, "930766865 9\n");
        assert_eq!(
            run("cksum", &[], &"a".repeat(1000)).stdout,
            "145108180 1000\n"
        );
    }

    #[test]
    fn names_each_file_given() {
        let dir = scratch_dir("cksum_files");
        let file = dir.join("hello.txt");
        fs::write(&file, "hello\n").unwrap();
        let file = file.to_str().unwrap();

        let out = run("cksum", &[file, "-"], "123456789");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(
            out.stdout,
            format!("3015617425 6 {}\n930766865 9 -\n", file)
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_reported() {
        let out = run("cksum", &["no-such-file.txt"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.starts_with("cksum: no-such-file.txt:"));
    }
}
