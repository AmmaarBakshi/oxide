use crate::sha256sum::{run_sum, Hash, Sha256};
use crate::{Ctx, Io};

/// `sha224sum` is `sha256sum` with SHA-224: the same output and the same
/// `-c`.
pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    run_sum::<Sha224>("sha224sum", args, io)
}

/// SHA-224, as FIPS 180-4 defines it: SHA-256 started from a different
/// state, with the last of the eight output words dropped.
struct Sha224(Sha256);

impl Hash for Sha224 {
    const LEN: usize = 28;

    fn new() -> Self {
        Self(Sha256::starting_from([
            0xc1059ed8, 0x367cd507, 0x3070dd17, 0xf70e5939, 0xffc00b31, 0x68581511, 0x64f98fa7,
            0xbefa4fa4,
        ]))
    }

    fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    fn finish(self) -> Vec<u8> {
        let mut digest = self.0.finish();
        digest.truncate(Self::LEN);
        digest
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::test_support::{run, scratch_dir};

    const ABC: &str = "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7";

    fn hash(input: &str) -> String {
        let out = run("sha224sum", &[], input);
        assert_eq!(out.status, 0, "{}", out.stderr);
        out.stdout.strip_suffix("  -\n").unwrap().to_string()
    }

    #[test]
    fn matches_the_standard_test_vectors() {
        assert_eq!(
            hash(""),
            "d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f"
        );
        assert_eq!(hash("abc"), ABC);
        assert_eq!(
            hash("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "75388b16512776cc5dba5da1fd890150b0c6455cb4f58b1952522525"
        );
        assert_eq!(
            hash(&"a".repeat(1_000_000)),
            "20794655980c91d8bbb4c1ea97618a4bf03f42581948b2ee4ee7ad67"
        );
    }

    #[test]
    fn is_not_a_truncated_sha256() {
        // The different initial state changes every word, so SHA-224 is not
        // just the front of the SHA-256 digest.
        let sha256 = run("sha256sum", &[], "abc").stdout;
        assert!(!sha256.starts_with(ABC));
    }

    #[test]
    fn dash_c_verifies_a_checksum_list() {
        let dir = scratch_dir("sha224sum_check");
        let (good, bad) = (dir.join("good.txt"), dir.join("bad.txt"));
        fs::write(&good, "abc").unwrap();
        fs::write(&bad, "changed").unwrap();
        let (good, bad) = (good.to_str().unwrap(), bad.to_str().unwrap());

        let list = format!("{}  {}\n{}  {}\n", ABC, good, ABC, bad);
        let out = run("sha224sum", &["-c"], &list);
        assert_eq!(out.status, 1);
        assert_eq!(out.stdout, format!("{}: OK\n{}: FAILED\n", good, bad));
        assert!(out
            .stderr
            .starts_with("sha224sum: WARNING: 1 computed checksum did NOT match"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_c_takes_only_sha224_length_hashes() {
        // 64 hex digits is a SHA-256 hash, too long to be a SHA-224 one.
        let sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let out = run("sha224sum", &["-c"], &format!("{}  x\n", sha256));
        assert_eq!(out.status, 1);
        assert_eq!(
            out.stderr,
            "sha224sum: no properly formatted checksum lines found\n"
        );
    }
}
