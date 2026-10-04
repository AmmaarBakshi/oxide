use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::HashMap;

use oxide_compat::CompatMode;
use oxide_exec::executor::Executor;
use oxide_exec::jobs::JobManager;
use oxide_perf::cache::CommandCache;

fn bench_pipelines(c: &mut Criterion) {
    c.bench_function("pipeline_parsing_and_routing", |b| {
        // We set up the state ONCE outside the loop so we don't benchmark the setup
        let mut mode = CompatMode::Oxide;
        let mut aliases = HashMap::new();
        let mut job_manager = JobManager::new();
        let history: Vec<String> = Vec::new();
        let mut command_cache = CommandCache::new();

        b.iter(|| {
            let mut executor = Executor::new();
            let mut last_exit_code = 0;

            // black_box stops the compiler from optimizing the input away.
            //
            // Both stages are builtins, so this measures the lexer, parser,
            // and dispatch rather than a process spawn. The `grep` pattern is
            // chosen not to match, keeping terminal I/O out of the timing.
            executor.execute_line(
                black_box("echo 'bench' | grep 'nomatch'"),
                &mut mode,
                &mut aliases,
                &mut last_exit_code,
                &mut job_manager,
                &history,
                &mut command_cache,
            );
        })
    });
}

// Wire it into Criterion
criterion_group!(benches, bench_pipelines);
criterion_main!(benches);
