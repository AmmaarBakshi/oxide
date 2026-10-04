use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, ctx: &mut Ctx<'_>) -> i32 {
    if args.is_empty() {
        for (key, val) in ctx.aliases.iter() {
            wln!(io, "alias {}='{}'", key, val);
        }
        return 0;
    }

    for arg in args {
        if let Some((key, value)) = arg.split_once('=') {
            let clean_value = value.trim_matches(|c| c == '"' || c == '\'');
            ctx.aliases.insert(key.to_string(), clean_value.to_string());
        } else {
            ewln!(io, "oxide: alias: invalid format. Use name=value");
            return 1;
        }
    }
    0
}
