use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

use crate::{Ctx, Io};

const DEFAULT_LINES: u64 = 1000;
const DEFAULT_SUFFIX_LEN: usize = 2;

/// How each piece is measured.
#[derive(Clone, Copy)]
enum Chunk {
    Lines(u64),
    Bytes(u64),
}

struct Options<'a> {
    chunk: Chunk,
    suffix_len: usize,
    numeric: bool,
    input: &'a str,
    prefix: &'a str,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let opts = match parse_args(args) {
        Ok(opts) => opts,
        Err(msg) => {
            ewln!(io, "split: {}", msg);
            ewln!(
                io,
                "Usage: split [-l LINES | -b SIZE] [-a LEN] [-d] [FILE [PREFIX]]"
            );
            return 1;
        }
    };
    let mut file;
    let input: &mut dyn BufRead = if opts.input == "-" {
        &mut *io.stdin
    } else {
        match File::open(opts.input) {
            Ok(opened) => {
                file = BufReader::new(opened);
                &mut file
            }
            Err(e) => {
                ewln!(io, "split: {}: {}", opts.input, e);
                return 1;
            }
        }
    };

    let mut pieces = Pieces {
        opts: &opts,
        index: 0,
        current: None,
    };
    let result = match opts.chunk {
        Chunk::Lines(n) => pieces.by_lines(input, n),
        Chunk::Bytes(n) => pieces.by_bytes(input, n),
    }
    .and_then(|()| pieces.close());
    match result {
        Ok(()) => 0,
        Err(msg) => {
            ewln!(io, "split: {}", msg);
            1
        }
    }
}

fn parse_args(args: &[String]) -> Result<Options<'_>, String> {
    let mut opts = Options {
        chunk: Chunk::Lines(DEFAULT_LINES),
        suffix_len: DEFAULT_SUFFIX_LEN,
        numeric: false,
        input: "-",
        prefix: "x",
    };
    let mut operands = Vec::new();

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let arg = arg.as_str();
        let option = match arg {
            "--" => {
                operands.extend(iter.by_ref().map(String::as_str));
                break;
            }
            "-d" => {
                opts.numeric = true;
                continue;
            }
            "-l" | "-b" | "-a" => arg,
            other if other.len() > 1 && other.starts_with('-') => {
                return Err(format!("invalid option '{}'", other));
            }
            operand => {
                operands.push(operand);
                continue;
            }
        };
        let value = iter
            .next()
            .ok_or_else(|| format!("option requires an argument -- '{}'", &option[1..]))?;
        let n = parse_size(value)
            .filter(|&n| n > 0)
            .ok_or_else(|| format!("invalid number: '{}'", value))?;
        match option {
            "-l" => opts.chunk = Chunk::Lines(n),
            "-b" => opts.chunk = Chunk::Bytes(n),
            _ => opts.suffix_len = n as usize,
        }
    }
    match operands[..] {
        [] => {}
        [input] => opts.input = input,
        [input, prefix] => (opts.input, opts.prefix) = (input, prefix),
        [_, _, extra, ..] => return Err(format!("extra operand '{}'", extra)),
    }
    Ok(opts)
}

/// A count, optionally with a `K`, `M` or `G` suffix (powers of 1024).
fn parse_size(text: &str) -> Option<u64> {
    let (digits, scale) = match text.as_bytes().last()?.to_ascii_uppercase() {
        b'K' => (&text[..text.len() - 1], 1 << 10),
        b'M' => (&text[..text.len() - 1], 1 << 20),
        b'G' => (&text[..text.len() - 1], 1 << 30),
        _ => (text, 1),
    };
    digits.parse::<u64>().ok()?.checked_mul(scale)
}

/// The suffix of piece number `n`: `aa`, `ab`, ... `zz`, or `00`, `01`,
/// ... with `numeric`. `None` once `len` characters can't count that high.
fn suffix(mut n: usize, len: usize, numeric: bool) -> Option<String> {
    let digits: &[u8] = if numeric {
        b"0123456789"
    } else {
        b"abcdefghijklmnopqrstuvwxyz"
    };
    let mut text = vec![digits[0]; len];
    for slot in text.iter_mut().rev() {
        *slot = digits[n % digits.len()];
        n /= digits.len();
    }
    (n == 0).then(|| String::from_utf8(text).expect("suffix digits are ASCII"))
}

/// The output files, opened one after another as each fills up. Nothing
/// is created until there's something to write, so empty input makes no
/// files at all.
struct Pieces<'a> {
    opts: &'a Options<'a>,
    index: usize,
    current: Option<BufWriter<File>>,
}

impl Pieces<'_> {
    fn open_next(&mut self) -> Result<&mut BufWriter<File>, String> {
        self.close()?;
        let suffix = suffix(self.index, self.opts.suffix_len, self.opts.numeric)
            .ok_or("output file suffixes exhausted")?;
        self.index += 1;
        let path = format!("{}{}", self.opts.prefix, suffix);
        let file = File::create(&path).map_err(|e| format!("{}: {}", path, e))?;
        Ok(self.current.insert(BufWriter::new(file)))
    }

    fn close(&mut self) -> Result<(), String> {
        match self.current.take() {
            Some(mut file) => file.flush().map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    fn by_lines(&mut self, input: &mut dyn BufRead, per_piece: u64) -> Result<(), String> {
        let mut line = Vec::new();
        let mut in_piece = per_piece;
        loop {
            line.clear();
            if read(input.read_until(b'\n', &mut line))? == 0 {
                return Ok(());
            }
            if in_piece == per_piece {
                self.open_next()?;
                in_piece = 0;
            }
            let out = self.current.as_mut().expect("a piece is open");
            out.write_all(&line).map_err(|e| e.to_string())?;
            in_piece += 1;
        }
    }

    fn by_bytes(&mut self, input: &mut dyn BufRead, per_piece: u64) -> Result<(), String> {
        let mut left = 0;
        loop {
            let buf = read(input.fill_buf())?;
            if buf.is_empty() {
                return Ok(());
            }
            if left == 0 {
                self.open_next()?;
                left = per_piece;
            }
            let take = buf.len().min(usize::try_from(left).unwrap_or(usize::MAX));
            let out = self.current.as_mut().expect("a piece is open");
            out.write_all(&buf[..take]).map_err(|e| e.to_string())?;
            input.consume(take);
            left -= take as u64;
        }
    }
}

fn read<T>(result: io::Result<T>) -> Result<T, String> {
    result.map_err(|e| format!("read error: {}", e))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::suffix;
    use crate::test_support::{run, scratch_dir};

    fn pieces(dir: &Path) -> Vec<(String, String)> {
        let mut found: Vec<(String, String)> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_str().unwrap().to_string();
                (name, fs::read_to_string(&path).unwrap())
            })
            .collect();
        found.sort();
        found
    }

    fn piece(name: &str, text: &str) -> (String, String) {
        (name.to_string(), text.to_string())
    }

    #[test]
    fn splits_every_n_lines() {
        let dir = scratch_dir("split_lines");
        let prefix = dir.join("part");
        let out = run(
            "split",
            &["-l", "2", "-", prefix.to_str().unwrap()],
            "1\n2\n3\n4\n5\n",
        );
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(
            pieces(&dir),
            [
                piece("partaa", "1\n2\n"),
                piece("partab", "3\n4\n"),
                piece("partac", "5\n")
            ]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn splits_every_n_bytes_with_numeric_suffixes() {
        let dir = scratch_dir("split_bytes");
        let prefix = dir.join("p");
        let args = ["-b", "4", "-d", "-a", "3", "-", prefix.to_str().unwrap()];
        let out = run("split", &args, "abcdefghij");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(
            pieces(&dir),
            [
                piece("p000", "abcd"),
                piece("p001", "efgh"),
                piece("p002", "ij")
            ]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn empty_input_makes_no_files() {
        let dir = scratch_dir("split_empty");
        let prefix = dir.join("x");
        let out = run("split", &["-", prefix.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(pieces(&dir).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn suffixes_count_up_until_they_run_out() {
        assert_eq!(suffix(0, 2, false).unwrap(), "aa");
        assert_eq!(suffix(27, 2, false).unwrap(), "bb");
        assert_eq!(suffix(12, 3, true).unwrap(), "012");
        assert_eq!(suffix(26, 1, false), None);
    }

    #[test]
    fn rejects_bad_counts() {
        let out = run("split", &["-l", "0"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("invalid number: '0'"));
    }
}
