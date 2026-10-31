use std::fs;
use std::path::{Path, PathBuf};

use sysinfo::Disks;

use crate::args::parse_flags;
use crate::free::human;
use crate::realpath::display;
use crate::{Ctx, Io};

/// One mounted file system, sizes in bytes.
struct Row {
    name: String,
    kind: String,
    mount: PathBuf,
    total: u64,
    available: u64,
}

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (flags, files) = match parse_flags(args, "hT") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "df: {}", msg);
            ewln!(io, "Usage: df [-hT] [FILE...]");
            return 1;
        }
    };

    let mut rows: Vec<Row> = Disks::new_with_refreshed_list()
        .list()
        .iter()
        .map(|disk| {
            let mount = disk.mount_point().to_path_buf();
            // Windows volumes often have no label; the drive is the name
            // anyone would look for.
            let name = match disk.name().to_string_lossy() {
                name if name.is_empty() => mount.display().to_string(),
                name => name.into_owned(),
            };
            Row {
                name,
                kind: disk.file_system().to_string_lossy().into_owned(),
                mount,
                total: disk.total_space(),
                available: disk.available_space(),
            }
        })
        .collect();
    rows.sort_by(|a, b| a.mount.cmp(&b.mount));

    // With operands, show just the file system each one lives on.
    let mut status = 0;
    let shown: Vec<&Row> = if files.is_empty() {
        rows.iter().collect()
    } else {
        let mut shown = Vec::new();
        for file in files {
            match fs::canonicalize(file) {
                Ok(path) => match containing(&rows, Path::new(&display(path))) {
                    Some(row) => shown.push(row),
                    None => {
                        ewln!(io, "df: {}: no file system found", file);
                        status = 1;
                    }
                },
                Err(e) => {
                    ewln!(io, "df: {}: {}", file, e);
                    status = 1;
                }
            }
        }
        shown
    };

    if !shown.is_empty() {
        for line in table(&shown, flags.contains('h'), flags.contains('T')) {
            wln!(io, "{}", line);
        }
    }
    status
}

/// The file system `path` is on: the one mounted deepest above it.
fn containing<'a>(rows: &'a [Row], path: &Path) -> Option<&'a Row> {
    rows.iter()
        .filter(|row| path.starts_with(&row.mount))
        .max_by_key(|row| row.mount.components().count())
}

/// Lines of the report, columns padded to fit: names and mount points to
/// the left, figures to the right.
fn table(rows: &[&Row], human_sizes: bool, with_kind: bool) -> Vec<String> {
    let size = |bytes: u64| {
        if human_sizes {
            human(bytes)
        } else {
            // 1K blocks, rounding up as coreutils does.
            bytes.div_ceil(1024).to_string()
        }
    };
    let mut cells: Vec<Vec<String>> = vec![[
        "Filesystem",
        "Type",
        if human_sizes { "Size" } else { "1K-blocks" },
        "Used",
        if human_sizes { "Avail" } else { "Available" },
        "Use%",
        "Mounted on",
    ]
    .map(String::from)
    .to_vec()];
    for row in rows {
        let used = row.total.saturating_sub(row.available);
        let percent = match row.total {
            0 => "-".to_string(),
            total => format!("{}%", (used * 100).div_ceil(total)),
        };
        cells.push(vec![
            row.name.clone(),
            row.kind.clone(),
            size(row.total),
            size(used),
            size(row.available),
            percent,
            row.mount.display().to_string(),
        ]);
    }
    if !with_kind {
        for line in &mut cells {
            line.remove(1);
        }
    }

    let columns = cells[0].len();
    let widths: Vec<usize> = (0..columns)
        .map(|col| {
            cells
                .iter()
                .map(|line| line[col].chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    cells
        .iter()
        .map(|line| {
            let padded: Vec<String> = line
                .iter()
                .enumerate()
                .map(|(col, cell)| match col {
                    // The last column is left as it is: no trailing blanks.
                    _ if col == columns - 1 => cell.clone(),
                    _ if col == 0 || (with_kind && col == 1) => {
                        format!("{:<width$}", cell, width = widths[col])
                    }
                    _ => format!("{:>width$}", cell, width = widths[col]),
                })
                .collect();
            padded.join(" ")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{containing, table, Row};
    use crate::test_support::run;

    const GIB: u64 = 1 << 30;

    fn row(name: &str, mount: &str, total: u64, available: u64) -> Row {
        Row {
            name: name.to_string(),
            kind: "ext4".to_string(),
            mount: PathBuf::from(mount),
            total,
            available,
        }
    }

    #[test]
    fn lays_out_a_report_with_aligned_columns() {
        let disks = [
            row("/dev/sda1", "/", 100 * GIB, 25 * GIB),
            row("tmpfs", "/tmp", 0, 0),
        ];
        let refs: Vec<&Row> = disks.iter().collect();
        assert_eq!(
            table(&refs, true, false),
            [
                "Filesystem Size Used Avail Use% Mounted on",
                "/dev/sda1  100G  75G   25G  75% /",
                "tmpfs         0    0     0    - /tmp",
            ]
        );
    }

    #[test]
    fn counts_in_1k_blocks_unless_dash_h() {
        // A third used is shown as 34%: the percentage rounds up too.
        let disks = [row("d", "/", 4096, 1024), row("e", "/e", 3072, 2048)];
        let refs: Vec<&Row> = disks.iter().collect();
        assert_eq!(
            table(&refs, false, true),
            [
                "Filesystem Type 1K-blocks Used Available Use% Mounted on",
                "d          ext4         4    3         1  75% /",
                "e          ext4         3    1         2  34% /e",
            ]
        );
    }

    #[test]
    fn a_file_belongs_to_the_deepest_mount_above_it() {
        let disks = [
            row("root", "/", 1, 0),
            row("home", "/home", 1, 0),
            row("data", "/home/data", 1, 0),
        ];
        let find = |path: &str| containing(&disks, Path::new(path)).map(|row| row.name.as_str());
        assert_eq!(find("/home/data/x"), Some("data"));
        assert_eq!(find("/home/database"), Some("home"));
        assert_eq!(find("/etc"), Some("root"));
    }

    #[test]
    fn reports_the_file_system_of_a_path() {
        let out = run("df", &["."], "");
        if sysinfo::Disks::new_with_refreshed_list().list().is_empty() {
            // Some sandboxes expose no disks at all.
            return;
        }
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert_eq!(out.stdout.lines().count(), 2, "{}", out.stdout);
        assert!(out.stdout.starts_with("Filesystem"));
    }

    #[test]
    fn a_missing_file_is_reported() {
        let out = run("df", &["no-such-file.txt"], "");
        assert_eq!(out.status, 1);
        assert!(out.stderr.starts_with("df: no-such-file.txt:"));
    }
}
