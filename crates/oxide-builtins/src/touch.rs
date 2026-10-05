use std::fs::OpenOptions;
use std::time::SystemTime;

use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        ewln!(io, "touch: missing file operand");
        return 1;
    }
    for path in args {
        // Creates the file if it's missing; an existing one keeps its contents
        // but gets a fresh modification time, which is what build tools see.
        let result = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .and_then(|file| file.set_modified(SystemTime::now()));
        if let Err(e) = result {
            ewln!(io, "touch: cannot touch '{}': {}", path, e);
            return 1;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime};

    use crate::test_support::{run, scratch_dir};

    #[test]
    fn creates_missing_files_and_bumps_existing_ones() {
        let dir = scratch_dir("touch_mtime");
        let (old, new) = (dir.join("old.txt"), dir.join("new.txt"));
        fs::write(&old, "keep").unwrap();
        let an_hour_ago = SystemTime::now() - Duration::from_secs(3600);
        fs::File::options().write(true).open(&old).unwrap().set_modified(an_hour_ago).unwrap();

        let out = run("touch", &[old.to_str().unwrap(), new.to_str().unwrap()], "");
        assert_eq!(out.status, 0, "{}", out.stderr);
        assert!(new.is_file());
        assert_eq!(fs::read_to_string(&old).unwrap(), "keep");
        assert!(fs::metadata(&old).unwrap().modified().unwrap() > an_hour_ago);
        fs::remove_dir_all(dir).unwrap();
    }
}
