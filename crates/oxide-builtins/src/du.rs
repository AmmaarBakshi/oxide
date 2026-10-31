use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};

use crate::args::parse_flags;
use crate::free::human;
use crate::{Ctx, Io};

struct Options {
    /// `-a`: list files as well as directories.
    all: bool,
    /// `-b`: count bytes in files rather than space allocated on disk.
    apparent: bool,
    /// `-h`: readable sizes rather than 1K blocks.
    human: bool,
    /// `-s`: one line per operand, not one per directory.
    summarize: bool,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "abchs") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "du: {}", msg);
            ewln!(io, "Usage: du [-abchs] [FILE...]");
            return 1;
        }
    };
    let opts = Options {
        all: flags.contains('a'),
        apparent: flags.contains('b'),
        human: flags.contains('h'),
        summarize: flags.contains('s'),
    };
    let files = if files.is_empty() { vec!["."] } else { files };

    let mut walk = Walk {
        io,
        opts,
        status: 0,
    };
    let mut grand_total = 0;
    for file in files {
        match walk.usage(Path::new(file), 0) {
            Ok(size) => grand_total += size,
            // Stdout is gone; nothing more can be reported.
            Err(_) => return 1,
        }
    }
    if flags.contains('c') && walk.print(grand_total, Path::new("total")).is_err() {
        return 1;
    }
    walk.status
}

struct Walk<'a, 'b> {
    io: &'a mut Io<'b>,
    opts: Options,
    status: i32,
}

impl Walk<'_, '_> {
    /// Totals the space under `path`, printing a line for it and, unless
    /// summarizing, for every directory below. Problems reading part of the
    /// tree are reported and leave that part out; only a failure to write
    /// the report is an error.
    fn usage(&mut self, path: &Path, depth: usize) -> io::Result<u64> {
        let meta = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(e) => {
                self.complain(path, e);
                return Ok(0);
            }
        };
        // Links aren't followed, so a link to a directory is just a small
        // file and a link back up the tree can't loop.
        if !meta.is_dir() {
            let size = self.size_of(&meta);
            if depth == 0 || (self.opts.all && !self.opts.summarize) {
                self.print(size, path)?;
            }
            return Ok(size);
        }

        let mut total = 0;
        match fs::read_dir(path) {
            Ok(entries) => {
                let mut children: Vec<PathBuf> = Vec::new();
                for entry in entries {
                    match entry {
                        Ok(entry) => children.push(entry.path()),
                        Err(e) => self.complain(path, e),
                    }
                }
                children.sort();
                for child in children {
                    total += self.usage(&child, depth + 1)?;
                }
            }
            Err(e) => self.complain(path, e),
        }
        if depth == 0 || !self.opts.summarize {
            self.print(total, path)?;
        }
        Ok(total)
    }

    fn size_of(&self, meta: &Metadata) -> u64 {
        if self.opts.apparent {
            meta.len()
        } else {
            allocated(meta)
        }
    }

    fn print(&mut self, bytes: u64, path: &Path) -> io::Result<()> {
        let size = if self.opts.human {
            human(bytes)
        } else if self.opts.apparent {
            bytes.to_string()
        } else {
            bytes.div_ceil(1024).to_string()
        };
        writeln!(self.io.stdout, "{}\t{}", size, path.display())
    }

    fn complain(&mut self, path: &Path, e: io::Error) {
        ewln!(self.io, "du: {}: {}", path.display(), e);
        self.status = 1;
    }
}

/// Space the file takes on disk, in whole blocks.
#[cfg(unix)]
fn allocated(meta: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks() * 512
}

/// Std has no portable way to ask Windows for a file's allocation, so its
/// length stands in.
#[cfg(not(unix))]
fn allocated(meta: &Metadata) -> u64 {
    meta.len()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use crate::test_support::{run, scratch_dir};

    /// `dir/a` (3 bytes) and `dir/sub/b` (5 bytes).
    fn tree(test: &str) -> PathBuf {
        let dir = scratch_dir(test);
        fs::create_dir(dir.join("sub")).unwrap();
        fs::write(dir.join("a"), "abc").unwrap();
        fs::write(dir.join("sub").join("b"), "bbbbb").unwrap();
        dir
    }

    fn line(size: &str, path: &Path) -> String {
        format!("{}\t{}\n", size, path.display())
    }

    #[test]
    fn totals_each_directory_deepest_first() {
        let dir = tree("du_tree");
        let out = run("du", &["-b", dir.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout, line("5", &dir.join("sub")) + &line("8", &dir));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_a_lists_files_too() {
        let dir = tree("du_all");
        let out = run("du", &["-ab", dir.to_str().unwrap()], "");
        let expected = [
            line("3", &dir.join("a")),
            line("5", &dir.join("sub").join("b")),
            line("5", &dir.join("sub")),
            line("8", &dir),
        ];
        assert_eq!(out.stdout, expected.concat());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_s_and_dash_c_summarize() {
        let dir = tree("du_summary");
        let a = dir.join("a");
        let args = ["-bsc", dir.to_str().unwrap(), a.to_str().unwrap()];
        let out = run("du", &args, "");
        let expected = line("8", &dir) + &line("3", &a) + &line("11", Path::new("total"));
        assert_eq!(out.stdout, expected);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dash_h_prints_readable_sizes() {
        let dir = scratch_dir("du_human");
        fs::write(dir.join("big"), vec![0u8; 2048]).unwrap();
        let out = run("du", &["-bsh", dir.to_str().unwrap()], "");
        assert_eq!(out.stdout, line("2.0K", &dir));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn counts_disk_blocks_by_default() {
        let dir = tree("du_blocks");
        let out = run("du", &["-s", dir.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        let (size, path) = out.stdout.trim_end().split_once('\t').unwrap();
        assert!(size.parse::<u64>().is_ok(), "{}", size);
        assert_eq!(path, dir.display().to_string());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_missing_file_is_reported() {
        let out = run("du", &["no-such-file.txt"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.starts_with("du: no-such-file.txt:"));
    }
}
