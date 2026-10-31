use std::collections::{HashMap, VecDeque};

use crate::args::parse_flags;
use crate::{Ctx, Io};

pub fn run(args: &[String], io: &mut Io<'_>, _ctx: &mut Ctx<'_>) -> i32 {
    let (_, files) = match parse_flags(args, "") {
        Ok(parsed) => parsed,
        Err(msg) => {
            ewln!(io, "tsort: {}", msg);
            ewln!(io, "Usage: tsort [file]");
            return 1;
        }
    };
    if let Some(extra) = files.get(1) {
        ewln!(io, "tsort: extra operand '{}'", extra);
        return 1;
    }
    let name = files.first().copied().unwrap_or("-");

    let mut text = String::new();
    let status = io.each_input("tsort", &files, |_, input, _| {
        input.read_to_string(&mut text).map(drop)
    });
    if status != 0 {
        return status;
    }
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if !tokens.len().is_multiple_of(2) {
        ewln!(
            io,
            "tsort: {}: input contains an odd number of tokens",
            name
        );
        return 1;
    }

    let mut graph = Graph::default();
    for pair in tokens.chunks(2) {
        graph.edge(pair[0], pair[1]);
    }
    let (order, loops) = graph.sort();
    for cycle in &loops {
        ewln!(io, "tsort: {}: input contains a loop:", name);
        for &node in cycle {
            ewln!(io, "tsort: {}", graph.names[node]);
        }
    }
    for node in order {
        wln!(io, "{}", graph.names[node]);
    }
    if loops.is_empty() {
        0
    } else {
        1
    }
}

/// Items numbered in the order they first appear, with the edges between
/// them kept both ways: successors to sort, predecessors to trace loops.
#[derive(Default)]
struct Graph<'a> {
    names: Vec<&'a str>,
    index: HashMap<&'a str, usize>,
    succ: Vec<Vec<usize>>,
    pred: Vec<Vec<usize>>,
}

impl<'a> Graph<'a> {
    fn node(&mut self, name: &'a str) -> usize {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.names.len();
        self.names.push(name);
        self.index.insert(name, i);
        self.succ.push(Vec::new());
        self.pred.push(Vec::new());
        i
    }

    /// Records that `before` comes before `after`. A pair naming one item
    /// twice just makes sure it's in the output.
    fn edge(&mut self, before: &'a str, after: &'a str) {
        let (a, b) = (self.node(before), self.node(after));
        if a != b {
            self.succ[a].push(b);
            self.pred[b].push(a);
        }
    }

    /// Orders every item after all those that must come before it, taking
    /// ready items in the order they first appeared.
    ///
    /// When everything left is waiting on something else there's a loop.
    /// It's recorded and broken at its earliest item, and sorting carries
    /// on, so the output still holds every item once.
    fn sort(&self) -> (Vec<usize>, Vec<Vec<usize>>) {
        let count = self.names.len();
        let mut waiting: Vec<usize> = self.pred.iter().map(Vec::len).collect();
        let mut done = vec![false; count];
        let mut order = Vec::with_capacity(count);
        let mut loops = Vec::new();
        let mut ready: VecDeque<usize> = (0..count).filter(|&i| waiting[i] == 0).collect();

        while order.len() < count {
            let Some(next) = ready.pop_front() else {
                let cycle = self.find_cycle(&done);
                waiting[cycle[0]] = 0;
                ready.push_back(cycle[0]);
                loops.push(cycle);
                continue;
            };
            // A loop's breaking point can be queued again by the edge that
            // was ignored to break it.
            if done[next] {
                continue;
            }
            done[next] = true;
            order.push(next);
            for &after in &self.succ[next] {
                if !done[after] {
                    waiting[after] = waiting[after].saturating_sub(1);
                    if waiting[after] == 0 {
                        ready.push_back(after);
                    }
                }
            }
        }
        (order, loops)
    }

    /// A loop among the items not yet output, starting from its earliest.
    ///
    /// Every such item still waits on another one not yet output, so
    /// walking back through predecessors never stops; it must come round
    /// to an item it has already passed.
    fn find_cycle(&self, done: &[bool]) -> Vec<usize> {
        let start = done.iter().position(|&d| !d).expect("items remain");
        let mut seen_at = vec![None; done.len()];
        let mut path = vec![start];
        seen_at[start] = Some(0);
        let mut current = start;
        loop {
            let before = *self.pred[current]
                .iter()
                .find(|&&p| !done[p])
                .expect("every item left waits on another");
            if let Some(i) = seen_at[before] {
                let mut cycle = path.split_off(i);
                // The walk went backwards; turn it to run in edge order,
                // then rotate it to begin at its earliest item.
                cycle.reverse();
                let first = (0..cycle.len()).min_by_key(|&i| cycle[i]).unwrap_or(0);
                cycle.rotate_left(first);
                return cycle;
            }
            seen_at[before] = Some(path.len());
            path.push(before);
            current = before;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::run;

    #[test]
    fn orders_items_after_what_they_depend_on() {
        assert_eq!(run("tsort", &[], "b c\na b\n").stdout, "a\nb\nc\n");
        assert_eq!(run("tsort", &[], "a b a c b d c d").stdout, "a\nb\nc\nd\n");
    }

    #[test]
    fn independent_items_keep_their_first_appearance_order() {
        assert_eq!(run("tsort", &[], "x y\na b\n").stdout, "x\na\ny\nb\n");
    }

    #[test]
    fn a_pair_of_the_same_item_just_adds_it() {
        assert_eq!(run("tsort", &[], "a a\nb a\n").stdout, "b\na\n");
    }

    #[test]
    fn a_loop_is_reported_and_broken() {
        let out = run("tsort", &[], "c a\na b\nb c\nc d\n");
        assert_eq!(out.status, 1);
        assert_eq!(
            out.stderr,
            "tsort: -: input contains a loop:\ntsort: c\ntsort: a\ntsort: b\n"
        );
        // c's successors are queued in order, so d is ready before b.
        assert_eq!(out.stdout, "c\na\nd\nb\n");
    }

    #[test]
    fn rejects_an_odd_number_of_tokens() {
        let out = run("tsort", &[], "a b c");
        assert_eq!(out.status, 1);
        assert!(out.stderr.contains("odd number of tokens"));
    }
}
