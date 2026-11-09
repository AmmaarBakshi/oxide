use crate::sha256sum::{run_sum, Blocks, Hash};
use crate::{Ctx, Io};

/// `md5sum` is `sha256sum` with MD5: the same output and the same `-c`.
pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    run_sum::<Md5>("md5sum", args, io)
}

/// The per-step left rotations, four per round.
const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, //
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, //
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, //
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// The per-step constants, floor(abs(sin(i + 1)) * 2^32).
const K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// MD5, as RFC 1321 defines it. It is long broken for security, but it is
/// still the checksum most download pages and package lists publish.
struct Md5 {
    state: [u32; 4],
    blocks: Blocks,
}

impl Hash for Md5 {
    const LEN: usize = 16;

    fn new() -> Self {
        Self {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            blocks: Blocks::new(),
        }
    }

    fn update(&mut self, data: &[u8]) {
        let state = &mut self.state;
        self.blocks.update(data, |block| compress(state, block));
    }

    fn finish(self) -> Vec<u8> {
        // MD5 is little-endian throughout, the length included, where the
        // SHA family is big-endian.
        let Self { mut state, blocks } = self;
        blocks.finish(u64::to_le_bytes, |block| compress(&mut state, block));
        state.iter().flat_map(|word| word.to_le_bytes()).collect()
    }
}

fn compress(state: &mut [u32; 4], block: &[u8; 64]) {
    let mut m = [0u32; 16];
    for (word, bytes) in m.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }

    let [mut a, mut b, mut c, mut d] = *state;
    for (i, (&k, &s)) in K.iter().zip(&S).enumerate() {
        // Each round mixes with its own function and visits the message
        // words in its own order.
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let rotated = a
            .wrapping_add(f)
            .wrapping_add(k)
            .wrapping_add(m[g])
            .rotate_left(s);
        (a, b, c, d) = (d, b.wrapping_add(rotated), b, c);
    }
    for (slot, value) in state.iter_mut().zip([a, b, c, d]) {
        *slot = slot.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    fn hash(input: &str) -> String {
        let out = run("md5sum", &[], input);
        assert_eq!(out.status, 0, "{}", out.stderr);
        out.stdout.strip_suffix("  -\n").unwrap().to_string()
    }

    #[test]
    fn matches_the_rfc_1321_test_suite() {
        assert_eq!(hash(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hash("a"), "0cc175b9c0f1b6a831c399e269772661");
        assert_eq!(hash("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hash("message digest"), "f96b697d7cb7938d525a2f31aaf161d0");
        assert_eq!(
            hash("abcdefghijklmnopqrstuvwxyz"),
            "c3fcd3d76192e4007dfb496cca67e13b"
        );
        assert_eq!(
            hash(&"1234567890".repeat(8)),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
        assert_eq!(
            hash(&"a".repeat(1_000_000)),
            "7707d6ae4e027c70eea2a935c2296f21"
        );
    }

    #[test]
    fn names_each_file_it_hashes() {
        let dir = scratch_dir("md5sum_files");
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "abc").unwrap();
        let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());

        let out = run("md5sum", &[a, b], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(
            out.stdout,
            format!(
                "0cc175b9c0f1b6a831c399e269772661  {}\n\
                 900150983cd24fb0d6963f7d28e17f72  {}\n",
                a, b
            )
        );

        let out = run("md5sum", &["-c"], &out.stdout);
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, format!("{}: OK\n{}: OK\n", a, b));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reports_an_unreadable_file_and_carries_on() {
        let out = run("md5sum", &["no-such-file.txt", "-"], "abc");
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, "900150983cd24fb0d6963f7d28e17f72  -\n");
        assert!(out.stderr.starts_with("md5sum: no-such-file.txt: "));
    }
}
