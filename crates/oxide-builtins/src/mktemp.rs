use std::collections::hash_map::RandomState;
use std::env;
use std::fs::{self, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use crate::{Ctx, Io};

const DEFAULT_TEMPLATE: &str = "tmp.XXXXXXXXXX";

/// The fewest `X`s a template may have. Fewer leave so few names that
/// guessing or colliding with one becomes easy.
const MIN_XS: usize = 3;

/// Names to try before giving up, in case others keep taking them.
const ATTEMPTS: usize = 100;

const NAME_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

struct Options<'a> {
    directory: bool,
    parent: Option<PathBuf>,
    template: Option<&'a str>,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let opts = match parse_args(args) {
        Ok(opts) => opts,
        Err(msg) => {
            ewln!(io, "mktemp: {}", msg);
            ewln!(io, "Usage: mktemp [-d] [-t | -p DIR] [TEMPLATE]");
            return 1;
        }
    };

    // An explicit template is taken from the current directory, as in
    // coreutils, unless -p or -t says where; the default goes in the
    // system's temporary directory.
    let template = opts.template.unwrap_or(DEFAULT_TEMPLATE);
    let path = match opts.parent {
        Some(parent) => parent.join(template),
        None => PathBuf::from(template),
    };
    match create(&path, opts.directory) {
        Ok(created) => {
            wln!(io, "{}", created.display());
            0
        }
        Err(e) => {
            ewln!(io, "mktemp: {}: {}", template, e);
            1
        }
    }
}

fn parse_args(args: &[String]) -> Result<Options<'_>, String> {
    let mut opts = Options {
        directory: false,
        parent: None,
        template: None,
    };
    let mut in_tmpdir = false;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-d" => opts.directory = true,
            "-t" => in_tmpdir = true,
            "-p" => {
                let dir = iter.next().ok_or("option requires an argument -- 'p'")?;
                opts.parent = Some(PathBuf::from(dir));
            }
            other if other.len() > 1 && other.starts_with('-') => {
                return Err(format!("invalid option '{}'", other));
            }
            template => {
                if let Some(first) = opts.template {
                    return Err(format!(
                        "too many templates: '{}' and '{}'",
                        first, template
                    ));
                }
                opts.template = Some(template);
            }
        }
    }
    if opts.parent.is_none() && (in_tmpdir || opts.template.is_none()) {
        opts.parent = Some(env::temp_dir());
    }
    Ok(opts)
}

/// Creates a new file (or directory) named by `template` with its trailing
/// `X`s replaced by random letters and digits, and returns its path.
///
/// Creation is exclusive, so a name that already exists is never reused;
/// another is tried instead.
fn create(template: &Path, directory: bool) -> io::Result<PathBuf> {
    let name = template
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let xs = name.len() - name.trim_end_matches('X').len();
    if xs < MIN_XS {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!("too few X's in template (need at least {})", MIN_XS),
        ));
    }
    let prefix = &name[..name.len() - xs];

    let mut rng = SplitMix64(RandomState::new().build_hasher().finish());
    for _ in 0..ATTEMPTS {
        let random: String = (0..xs)
            .map(|_| NAME_CHARS[(rng.next_u64() % NAME_CHARS.len() as u64) as usize] as char)
            .collect();
        let path = template.with_file_name(format!("{}{}", prefix, random));
        let result = if directory {
            fs::create_dir(&path)
        } else {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map(drop)
        };
        match result {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        "could not find an unused name",
    ))
}

/// A small, fast generator. Seeded from `RandomState`, which std keys from
/// the OS's random source, it is unpredictable enough for picking names:
/// exclusive creation, not secrecy, is what keeps two callers apart.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::test_support::{run, scratch_dir};

    fn created(args: &[&str]) -> PathBuf {
        let out = run("mktemp", args, "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        PathBuf::from(out.stdout.trim_end())
    }

    fn name(path: &Path) -> &str {
        path.file_name().unwrap().to_str().unwrap()
    }

    #[test]
    fn makes_a_new_file_in_the_temp_dir_by_default() {
        let path = created(&[]);
        assert!(path.is_file());
        assert_eq!(path.parent().unwrap(), std::env::temp_dir().as_path());
        assert!(name(&path).starts_with("tmp."));
        assert_eq!(name(&path).len(), "tmp.XXXXXXXXXX".len());

        let other = created(&[]);
        assert_ne!(path, other);
        fs::remove_file(path).unwrap();
        fs::remove_file(other).unwrap();
    }

    #[test]
    fn dash_d_makes_a_directory() {
        let path = created(&["-d"]);
        assert!(path.is_dir());
        fs::remove_dir(path).unwrap();
    }

    #[test]
    fn fills_in_a_template_where_dash_p_says() {
        let dir = scratch_dir("mktemp_template");
        let path = created(&["-p", dir.to_str().unwrap(), "log.XXXX"]);
        assert!(path.is_file());
        assert_eq!(path.parent().unwrap(), dir.as_path());
        assert!(name(&path).starts_with("log."));
        assert!(!name(&path).ends_with("XXXX"));

        // A template that is already a path is used as it stands.
        let path = created(&[dir.join("dXXX").to_str().unwrap()]);
        assert_eq!(path.parent().unwrap(), dir.as_path());
        assert_eq!(name(&path).len(), 4);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_a_template_with_too_few_xs() {
        let out = run("mktemp", &["-t", "fileXX"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("too few X's"), "{}", out.stderr);
    }
}
