use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::collections::HashMap;

use oxide_exec::executor::Executor;
use oxide_compat::CompatMode;
use oxide_exec::jobs::JobManager;

fn bench_pipelines(c: &mut Criterion) {
    c.bench_function("pipeline_parsing_and_routing", |b| {
        // We set up the state ONCE outside the loop so we don't benchmark the setup
        let mut mode = CompatMode::Oxide;
        let mut aliases = HashMap::new();
        let mut job_manager = JobManager::new();
        
        b.iter(|| {
            let mut executor = Executor::new();
            let mut last_exit_code = 0;

            // We use black_box to prevent the Rust compiler from "optimizing away" the string
            // We use built-in 'echo' commands so we benchmark the shell, not the OS!
            executor.execute_line(
                black_box("echo 'bench' | echo 'mark'"),
                &mut mode,
                &mut aliases,
                &mut last_exit_code,
                &mut job_manager,
            );
        })
    });
}

// Wire it into Criterion
criterion_group!(benches, bench_pipelines);
criterion_main!(benches);