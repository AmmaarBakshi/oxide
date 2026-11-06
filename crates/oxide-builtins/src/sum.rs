use std::io::{self, BufRead};

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "rs") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "sum: {}", msg);
            ewln!(io, "Usage: sum [-r | -s] [file...]");
            return 1;
        }
    };
    // As with coreutils, whichever of -r and -s comes last wins.
    let sysv = flags.rfind('s') > flags.rfind('r');
    // BSD sum names files only when there's more than one; System V names
    // any file it was given.
    let named = if sysv {
        !files.is_empty()
    } else {
        files.len() > 1
    };
    io.each_input("sum", &files, |name, input, out| {
        let line = if sysv {
            let (sum, len) = sysv_sum(input)?;
            format!("{} {}", sum, len.div_ceil(512))
        } else {
            let (sum, len) = bsd_sum(input)?;
            format!("{:05} {:5}", sum, len.div_ceil(1024))
        };
        if named {
            writeln!(out, "{} {}", line, name)
        } else {
            writeln!(out, "{}", line)
        }
    })
}

/// Calls `f` on each buffer of `input` and returns how many bytes there were.
fn each_buf(input: &mut dyn BufRead, mut f: impl FnMut(&[u8])) -> io::Result<u64> {
    let mut len = 0u64;
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            return Ok(len);
        }
        f(buf);
        len += buf.len() as u64;
        let read = buf.len();
        input.consume(read);
    }
}

/// The BSD checksum: a 16-bit sum, rotated right a bit before each byte.
fn bsd_sum(input: &mut dyn BufRead) -> io::Result<(u16, u64)> {
    let mut sum = 0u16;
    let len = each_buf(input, |buf| {
        for &byte in buf {
            sum = sum.rotate_right(1).wrapping_add(byte as u16);
        }
    })?;
    Ok((sum, len))
}

/// The System V checksum: every byte added up, then folded to 16 bits.
fn sysv_sum(input: &mut dyn BufRead) -> io::Result<(u32, u64)> {
    let mut total = 0u32;
    let len = each_buf(input, |buf| {
        for &byte in buf {
            total = total.wrapping_add(byte as u32);
        }
    })?;
    let folded = (total & 0xffff) + (total >> 16);
    Ok(((folded & 0xffff) + (folded >> 16), len))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn bsd_by_default() {
        assert_eq!(run("sum", &[], "").stdout, "00000     0\n");
        assert_eq!(run("sum", &[], "hello\n").stdout, "36979     1\n");
        assert_eq!(run("sum", &["-r"], "123456789").stdout, "53615     1\n");
        assert_eq!(run("sum", &[], &"a".repeat(5000)).stdout, "41146     5\n");
    }

    #[test]
    fn dash_s_is_system_v() {
        assert_eq!(run("sum", &["-s"], "").stdout, "0 0\n");
        assert_eq!(run("sum", &["-s"], "hello\n").stdout, "542 1\n");
        assert_eq!(run("sum", &["-s"], &"a".repeat(1000)).stdout, "31465 2\n");
        // The last of -r and -s wins.
        assert_eq!(run("sum", &["-s", "-r"], "hello\n").stdout, "36979     1\n");
        assert_eq!(run("sum", &["-rs"], "hello\n").stdout, "542 1\n");
    }

    #[test]
    fn names_files_the_way_coreutils_does() {
        let dir = scratch_dir("sum_files");
        let file = dir.join("hello.txt");
        fs::write(&file, "hello\n").unwrap();
        let file = file.to_str().unwrap();

        assert_eq!(run("sum", &[file], "").stdout, "36979     1\n");
        assert_eq!(
            run("sum", &[file, "-"], "hello\n").stdout,
            format!("36979     1 {}\n36979     1 -\n", file)
        );
        assert_eq!(
            run("sum", &["-s", file], "").stdout,
            format!("542 1 {}\n", file)
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_reported() {
        let out = run("sum", &["no-such-file.txt"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.starts_with("sum: no-such-file.txt:"));
    }
}
