use std::fs::File;
use std::io::{self, BufRead, BufReader};

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    run_sum::<Sha256>("sha256sum", args, io)
}

/// A hash that a `*sum` builtin prints and checks. sha256sum and its
/// siblings differ only in this; the listing, `-c` and the messages are
/// all [`run_sum`].
pub(crate) trait Hash {
    /// The digest length in bytes.
    const LEN: usize;
    fn new() -> Self;
    fn update(&mut self, data: &[u8]);
    fn finish(self) -> Vec<u8>;
}

/// Runs `cmd`, a `*sum` builtin using the hash `H`: prints `HASH  NAME` for
/// each file, or with `-c` checks a list of such lines.
pub(crate) fn run_sum<H: Hash>(cmd: &str, args: &[String], io: &mut Io<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "c") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "{}: {}", cmd, msg);
            ewln!(io, "Usage: {} [-c] [file...]", cmd);
            return 1;
        }
    };
    if flags.contains('c') {
        return check::<H>(cmd, io, &files);
    }
    io.each_input(cmd, &files, |name, input, out| {
        writeln!(out, "{}  {}", hex(&digest::<H>(input)?), name)
    })
}

/// `-c`: reads `HASH  NAME` lines, as `cmd` prints them, and says whether
/// each named file still has that hash.
fn check<H: Hash>(cmd: &str, io: &mut Io<'_>, lists: &[&str]) -> i32 {
    let mut text = String::new();
    let status = io.each_input(cmd, lists, |_, input, _| {
        input.read_to_string(&mut text).map(drop)
    });
    if status != 0 {
        return status;
    }

    let (mut checked, mut failed, mut unreadable, mut malformed) = (0, 0, 0, 0);
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        // Two spaces, or a space and `*` for files hashed in binary mode,
        // which is the same thing here.
        let entry = line
            .split_once("  ")
            .or_else(|| line.split_once(" *"))
            .filter(|(hash, _)| {
                hash.len() == 2 * H::LEN && hash.bytes().all(|b| b.is_ascii_hexdigit())
            });
        let Some((expected, name)) = entry else {
            malformed += 1;
            continue;
        };
        checked += 1;
        match File::open(name).and_then(|file| digest::<H>(&mut BufReader::new(file))) {
            Ok(actual) if hex(&actual).eq_ignore_ascii_case(expected) => wln!(io, "{}: OK", name),
            Ok(_) => {
                failed += 1;
                wln!(io, "{}: FAILED", name);
            }
            Err(e) => {
                unreadable += 1;
                ewln!(io, "{}: {}: {}", cmd, name, e);
                wln!(io, "{}: FAILED open or read", name);
            }
        }
    }

    if checked == 0 {
        ewln!(io, "{}: no properly formatted checksum lines found", cmd);
        return 1;
    }
    for (count, one, many) in [
        (
            malformed,
            "line is improperly formatted",
            "lines are improperly formatted",
        ),
        (
            unreadable,
            "listed file could not be read",
            "listed files could not be read",
        ),
        (
            failed,
            "computed checksum did NOT match",
            "computed checksums did NOT match",
        ),
    ] {
        if count > 0 {
            let what = if count == 1 { one } else { many };
            ewln!(io, "{}: WARNING: {} {}", cmd, count, what);
        }
    }
    if failed + unreadable > 0 {
        1
    } else {
        0
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// The hash of everything in `input`, read a buffer at a time.
fn digest<H: Hash>(input: &mut dyn BufRead) -> io::Result<Vec<u8>> {
    let mut hasher = H::new();
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            return Ok(hasher.finish());
        }
        hasher.update(buf);
        let read = buf.len();
        input.consume(read);
    }
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The message buffering MD5, SHA-1 and SHA-256 share: input is cut into
/// 64-byte blocks for a compression function, and the end is padded out to
/// a whole block that carries the message length.
pub(crate) struct Blocks {
    block: [u8; 64],
    filled: usize,
    total: u64,
}

impl Blocks {
    pub(crate) fn new() -> Self {
        Self {
            block: [0; 64],
            filled: 0,
            total: 0,
        }
    }

    /// Adds `data`, passing each block it completes to `compress`.
    pub(crate) fn update(&mut self, mut data: &[u8], mut compress: impl FnMut(&[u8; 64])) {
        self.total += data.len() as u64;
        while !data.is_empty() {
            let take = (64 - self.filled).min(data.len());
            self.block[self.filled..self.filled + take].copy_from_slice(&data[..take]);
            self.filled += take;
            data = &data[take..];
            if self.filled == 64 {
                compress(&self.block);
                self.filled = 0;
            }
        }
    }

    /// Pads the message and compresses what is left: a 1 bit, zeros up to 8
    /// bytes short of a block, then the length in bits, in the byte order
    /// `length` gives it.
    pub(crate) fn finish(
        mut self,
        length: fn(u64) -> [u8; 8],
        mut compress: impl FnMut(&[u8; 64]),
    ) {
        let bits = self.total.wrapping_mul(8);
        self.update(&[0x80], &mut compress);
        while self.filled != 56 {
            self.update(&[0], &mut compress);
        }
        self.update(&length(bits), &mut compress);
    }
}

/// SHA-256, as FIPS 180-4 defines it.
pub(crate) struct Sha256 {
    state: [u32; 8],
    blocks: Blocks,
}

impl Sha256 {
    /// SHA-256's compression from another initial state, which is all that
    /// SHA-224 changes besides cutting the digest short.
    pub(crate) fn starting_from(state: [u32; 8]) -> Self {
        Self {
            state,
            blocks: Blocks::new(),
        }
    }
}

impl Hash for Sha256 {
    const LEN: usize = 32;

    fn new() -> Self {
        Self::starting_from([
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ])
    }

    fn update(&mut self, data: &[u8]) {
        let state = &mut self.state;
        self.blocks.update(data, |block| compress(state, block));
    }

    fn finish(self) -> Vec<u8> {
        let Self { mut state, blocks } = self;
        blocks.finish(u64::to_be_bytes, |block| compress(&mut state, block));
        state.iter().flat_map(|word| word.to_be_bytes()).collect()
    }
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (word, bytes) in w.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    for t in 16..64 {
        let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
        let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
        w[t] = w[t - 16]
            .wrapping_add(s0)
            .wrapping_add(w[t - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for (&k, &word) in K.iter().zip(&w) {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choose = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(choose)
            .wrapping_add(k)
            .wrapping_add(word);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(majority);
        (h, g, f, e) = (g, f, e, d.wrapping_add(t1));
        (d, c, b, a) = (c, b, a, t1.wrapping_add(t2));
    }
    for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *slot = slot.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn hash(input: &str) -> String {
        let out = run("sha256sum", &[], input);
        assert_eq!(out.status, 0, "{}", out.stderr);
        out.stdout.strip_suffix("  -\n").unwrap().to_string()
    }

    #[test]
    fn matches_the_standard_test_vectors() {
        assert_eq!(hash(""), EMPTY);
        assert_eq!(hash("abc"), ABC);
        assert_eq!(
            hash("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            hash(&"a".repeat(1_000_000)),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn dash_c_verifies_a_checksum_list() {
        let dir = scratch_dir("sha256sum_check");
        let (good, bad) = (dir.join("good.txt"), dir.join("bad.txt"));
        fs::write(&good, "abc").unwrap();
        fs::write(&bad, "changed").unwrap();
        let (good, bad) = (good.to_str().unwrap(), bad.to_str().unwrap());

        let list = format!("{}  {}\n{} *{}\n", ABC, good, ABC, bad);
        let out = run("sha256sum", &["-c"], &list);
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, format!("{}: OK\n{}: FAILED\n", good, bad));
        assert!(out.stderr.contains("1 computed checksum did NOT match"));

        let out = run(
            "sha256sum",
            &["-c"],
            &format!("{}  {}\n", ABC.to_uppercase(), good),
        );
        assert_eq!((out.status, out.stderr.as_str()), (0, ""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_c_reports_missing_files_and_bad_lines() {
        let list = format!("{}  no-such-file.txt\nnot a checksum line\n", EMPTY);
        let out = run("sha256sum", &["-c"], &list);
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, "no-such-file.txt: FAILED open or read\n");
        assert!(out.stderr.contains("1 line is improperly formatted"));
        assert!(out.stderr.contains("1 listed file could not be read"));
    }

    #[test]
    fn dash_c_needs_at_least_one_valid_line() {
        let out = run("sha256sum", &["-c"], "nothing here\n");
        assert_eq!(out.status, 1);
        assert!(out
            .stderr
            .contains("no properly formatted checksum lines found"));
    }
}
