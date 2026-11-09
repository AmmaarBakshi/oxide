use crate::sha256sum::{run_sum, Blocks, Hash};
use crate::{Ctx, Io};

/// `sha1sum` is `sha256sum` with SHA-1: the same output and the same `-c`.
pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    run_sum::<Sha1>("sha1sum", args, io)
}

/// SHA-1, as FIPS 180-4 defines it. It is no longer collision resistant,
/// but plenty of published checksum lists still use it.
struct Sha1 {
    state: [u32; 5],
    blocks: Blocks,
}

impl Hash for Sha1 {
    const LEN: usize = 20;

    fn new() -> Self {
        Self {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0],
            blocks: Blocks::new(),
        }
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

fn compress(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for (word, bytes) in w.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    for t in 16..80 {
        w[t] = (w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16]).rotate_left(1);
    }

    let [mut a, mut b, mut c, mut d, mut e] = *state;
    for (t, &word) in w.iter().enumerate() {
        let (f, k) = match t {
            0..=19 => ((b & c) | (!b & d), 0x5a827999),
            20..=39 => (b ^ c ^ d, 0x6ed9eba1),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
            _ => (b ^ c ^ d, 0xca62c1d6),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(word);
        (e, d, c, b, a) = (d, c, b.rotate_left(30), a, temp);
    }
    for (slot, value) in state.iter_mut().zip([a, b, c, d, e]) {
        *slot = slot.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    const ABC: &str = "a9993e364706816aba3e25717850c26c9cd0d89d";

    fn hash(input: &str) -> String {
        let out = run("sha1sum", &[], input);
        assert_eq!(out.status, 0, "{}", out.stderr);
        out.stdout.strip_suffix("  -\n").unwrap().to_string()
    }

    #[test]
    fn matches_the_standard_test_vectors() {
        assert_eq!(hash(""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(hash("abc"), ABC);
        assert_eq!(
            hash("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            hash(&"a".repeat(1_000_000)),
            "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
        );
    }

    #[test]
    fn dash_c_verifies_a_checksum_list() {
        let dir = scratch_dir("sha1sum_check");
        let (good, bad) = (dir.join("good.txt"), dir.join("bad.txt"));
        fs::write(&good, "abc").unwrap();
        fs::write(&bad, "changed").unwrap();
        let (good, bad) = (good.to_str().unwrap(), bad.to_str().unwrap());

        let list = format!("{}  {}\n{}  {}\n", ABC, good, ABC, bad);
        let out = run("sha1sum", &["-c"], &list);
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, format!("{}: OK\n{}: FAILED\n", good, bad));
        assert!(out
            .stderr
            .starts_with("sha1sum: WARNING: 1 computed checksum did NOT match"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_c_takes_only_sha1_length_hashes() {
        // A SHA-256 line is well formed for sha256sum, not for sha1sum.
        let sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let out = run("sha1sum", &["-c"], &format!("{}  x\n", sha256));
        assert_eq!(out.status, 1);
        assert_eq!(
            out.stderr,
            "sha1sum: no properly formatted checksum lines found\n"
        );
    }

    #[test]
    fn rejects_unknown_flags() {
        let out = run("sha1sum", &["-x"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.starts_with("sha1sum: invalid option -- 'x'"));
    }
}
