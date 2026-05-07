use std::env;
use std::path::{Path, PathBuf};
use crate::Builtin;

pub struct CdCommand;

impl CdCommand {
    fn get_target(&self, args: &[String], runtime: &mut crate::Runtime) -> PathBuf {
        let dir = match args.get(0) {
            Some(d) if d == "-" => {
                return PathBuf::from(runtime.scope.get("OLDPWD").unwrap_or_else(|| ".".to_string()));
            }
            Some(d) => d.clone(),
            None => runtime.scope.get("HOME").unwrap_or_else(|| ".".to_string()),
        };

        // Handle CDPATH search
        if !dir.starts_with('/') && !dir.starts_with('.') {
            if let Some(cdpath) = runtime.scope.get("CDPATH") {
                for path in cdpath.split(':') {
                    let full_path = Path::new(path).join(&dir);
                    if full_path.is_dir() { return full_path; }
                }
            }
        }
        PathBuf::from(dir)
    }
}

impl Builtin for CdCommand {
    fn name(&self) -> &str { "cd" }

    fn execute(&self, args: &[String], runtime: &mut crate::Runtime) -> i32 {
        let current_pwd = env::current_dir().unwrap_or_default();
        let target = self.get_target(args, runtime);

        // Process options -L (default) and -P
        let final_path = if args.contains(&"-P".to_string()) {
            target.canonicalize().unwrap_or(target)
        } else {
            target
        };

        match env::set_current_dir(&final_path) {
            Ok(_) => {
                runtime.scope.set("OLDPWD", &current_pwd.to_string_lossy());
                runtime.scope.set("PWD", &env::current_dir().unwrap().to_string_lossy());
                0
            }
            Err(e) => {
                eprintln!("oxide: cd: {}: {}", final_path.display(), e);
                1
            }
        }
    }
}