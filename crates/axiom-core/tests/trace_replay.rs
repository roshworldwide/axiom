//! TLC trace replay.
//!
//! Replays a TLA+-pinned operation trace on the Rust implementation and asserts
//! the final per-replica state matches the state the spec computes.
//!
//! Each fixture under `tests/traces/` is pinned by TLC: the matching
//! `tla/traces/<Crdt>Trace.tla` asserts `Done => state = Expected`, so a clean
//! TLC run (also enforced by the CI `tlc` job) confirms `Expected` IS the spec's
//! result for the scripted ops. Reproducing it here connects the implementation
//! to the spec by a replayed trace — "refinement validated by trace replay".

use std::collections::BTreeMap;

use axiom_core::{GCounter, ReplicaId};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "t")]
enum Op {
    #[serde(rename = "inc")]
    Inc { r: u64 },
    #[serde(rename = "merge")]
    Merge { to: u64, from: u64 },
}

#[derive(Deserialize)]
struct GCounterTrace {
    crdt: String,
    replicas: Vec<u64>,
    ops: Vec<Op>,
    /// replica -> (replica -> component count)
    expected: BTreeMap<String, BTreeMap<String, u64>>,
}

fn load(name: &str) -> String {
    let path = format!("{}/tests/traces/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn gcounter_trace_replay_matches_spec() {
    let trace: GCounterTrace = serde_json::from_str(&load("gcounter.json")).unwrap();
    assert_eq!(trace.crdt, "gcounter");

    // One Rust G-Counter per replica; replay the scripted ops on the system.
    let mut reps: BTreeMap<u64, GCounter> = trace
        .replicas
        .iter()
        .map(|&r| (r, GCounter::new(ReplicaId(r))))
        .collect();

    for op in &trace.ops {
        match op {
            Op::Inc { r } => {
                reps.get_mut(r).expect("unknown replica").increment();
            }
            Op::Merge { to, from } => {
                let src = reps[from].clone();
                reps.get_mut(to).expect("unknown replica").merge(&src);
            }
        }
    }

    // Every replica's refinement state must equal the TLC-pinned expected value.
    // tla_state() omits zero components, so compare per component with default 0.
    for &r in &trace.replicas {
        let got = reps[&r].tla_state().counts;
        let want = &trace.expected[&r.to_string()];
        for &c in &trace.replicas {
            let g = got.get(&ReplicaId(c)).copied().unwrap_or(0);
            let w = want.get(&c.to_string()).copied().unwrap_or(0);
            assert_eq!(g, w, "replica {r} component {c}");
        }
    }
}
