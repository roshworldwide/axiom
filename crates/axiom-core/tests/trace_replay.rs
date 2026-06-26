//! TLC trace replay.
//!
//! Replays a TLA+-pinned operation trace on the Rust implementation and asserts
//! the final state matches the state the spec computes. Each fixture under
//! `tests/traces/` is pinned by TLC: the matching `tla/traces/<Crdt>Trace.tla`
//! asserts `Done => <observable> = Expected`, so a clean TLC run (also enforced
//! by the CI `tlc` job) confirms `Expected` IS the spec's result for the
//! scripted ops. Reproducing it here connects the implementation to the spec by
//! a replayed trace — "refinement validated by trace replay".
//!
//! Ids are reconciled at the right abstraction so incidental encoding cannot
//! cause false mismatches, while the meaningful outcomes match exactly: G-Counter
//! compares per-replica component counts; OR-Set compares per-replica membership
//! (live elements), so tag encoding (`<<replica,counter>>` in TLA+ vs Uuid in
//! Rust) is irrelevant; RGA compares the visible id sequence and the tombstone
//! set, feeding the trace's `<<counter,replica>>` ids into the impl so the id
//! tie-break matches the spec's.

use std::collections::{BTreeMap, BTreeSet};

use axiom_core::{ElementId, GCounter, Hlc, ORSet, ReplicaId, Rga};
use serde::Deserialize;

fn load(name: &str) -> String {
    let path = format!("{}/tests/traces/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

// ---- G-Counter -------------------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "t")]
enum GcOp {
    #[serde(rename = "inc")]
    Inc { r: u64 },
    #[serde(rename = "merge")]
    Merge { to: u64, from: u64 },
}

#[derive(Deserialize)]
struct GCounterTrace {
    crdt: String,
    replicas: Vec<u64>,
    ops: Vec<GcOp>,
    expected: BTreeMap<String, BTreeMap<String, u64>>,
}

/// Replay a G-Counter trace; returns per-replica component counts
/// (replica -> {component -> count}, zero-filled over all replicas) so both the
/// positive test and the negativity check can reuse it.
fn replay_gcounter(trace: &GCounterTrace) -> BTreeMap<u64, BTreeMap<u64, u64>> {
    let mut reps: BTreeMap<u64, GCounter> = trace
        .replicas
        .iter()
        .map(|&r| (r, GCounter::new(ReplicaId(r))))
        .collect();
    for op in &trace.ops {
        match op {
            GcOp::Inc { r } => {
                reps.get_mut(r).expect("unknown replica").increment();
            }
            GcOp::Merge { to, from } => {
                let src = reps[from].clone();
                reps.get_mut(to).expect("unknown replica").merge(&src);
            }
        }
    }
    trace
        .replicas
        .iter()
        .map(|&r| {
            let counts = reps[&r].tla_state().counts;
            let row = trace
                .replicas
                .iter()
                .map(|&c| (c, counts.get(&ReplicaId(c)).copied().unwrap_or(0)))
                .collect();
            (r, row)
        })
        .collect()
}

/// The pinned per-replica component counts from the fixture's `expected` block.
fn gcounter_expected(trace: &GCounterTrace) -> BTreeMap<u64, BTreeMap<u64, u64>> {
    trace
        .expected
        .iter()
        .map(|(r, row)| {
            let r = r.parse::<u64>().expect("replica key");
            let row = row
                .iter()
                .map(|(c, &v)| (c.parse::<u64>().expect("component key"), v))
                .collect();
            (r, row)
        })
        .collect()
}

#[test]
fn gcounter_trace_replay_matches_spec() {
    let trace: GCounterTrace = serde_json::from_str(&load("gcounter.json")).unwrap();
    assert_eq!(trace.crdt, "gcounter");
    let got = replay_gcounter(&trace);
    let want = gcounter_expected(&trace);
    for &r in &trace.replicas {
        assert_eq!(got[&r], want[&r], "replica {r} component counts");
    }
}

// ---- OR-Set (membership) ---------------------------------------------------

#[derive(Deserialize)]
#[serde(tag = "op")]
enum OrSetOp {
    #[serde(rename = "add")]
    Add { r: u64, x: u8 },
    #[serde(rename = "remove")]
    Remove { r: u64, x: u8 },
    #[serde(rename = "merge")]
    Merge { r: u64, s: u64 },
}

#[derive(Deserialize)]
struct OrSetTrace {
    crdt: String,
    replicas: Vec<u64>,
    elements: Vec<u8>,
    ops: Vec<OrSetOp>,
    expected_membership: BTreeMap<String, Vec<u8>>,
}

/// Replay an OR-Set trace; returns the per-replica live-element set the
/// implementation reaches (so the negative check can reuse it).
fn replay_orset(trace: &OrSetTrace) -> BTreeMap<u64, BTreeSet<u8>> {
    let mut reps: BTreeMap<u64, ORSet<u8>> =
        trace.replicas.iter().map(|&r| (r, ORSet::new())).collect();
    for op in &trace.ops {
        match op {
            OrSetOp::Add { r, x } => {
                reps.get_mut(r).expect("unknown replica").add(*x);
            }
            OrSetOp::Remove { r, x } => {
                reps.get_mut(r).expect("unknown replica").remove(x);
            }
            OrSetOp::Merge { r, s } => {
                let src = reps[s].clone();
                reps.get_mut(r).expect("unknown replica").merge(&src);
            }
        }
    }
    trace
        .replicas
        .iter()
        .map(|&r| {
            let live = trace
                .elements
                .iter()
                .copied()
                .filter(|x| reps[&r].contains(x))
                .collect();
            (r, live)
        })
        .collect()
}

#[test]
fn orset_trace_replay_matches_spec() {
    let trace: OrSetTrace = serde_json::from_str(&load("orset.json")).unwrap();
    assert_eq!(trace.crdt, "orset");
    let got = replay_orset(&trace);
    for &r in &trace.replicas {
        let want: BTreeSet<u8> = trace.expected_membership[&r.to_string()]
            .iter()
            .copied()
            .collect();
        assert_eq!(got[&r], want, "replica {r} membership");
    }
}

// ---- RGA (visible sequence + tombstones) -----------------------------------

#[derive(Deserialize)]
#[serde(tag = "op")]
enum RgaOp {
    #[serde(rename = "insert")]
    Insert {
        r: u64,
        id: [u64; 2],
        after: Option<[u64; 2]>,
    },
    #[serde(rename = "delete")]
    Delete { r: u64, target: [u64; 2] },
    #[serde(rename = "merge")]
    Merge { r: u64, s: u64 },
}

#[derive(Deserialize)]
struct RgaTrace {
    crdt: String,
    replicas: Vec<u64>,
    ops: Vec<RgaOp>,
    expected_visible: Vec<[u64; 2]>,
    expected_tombstones: Vec<[u64; 2]>,
}

/// `[counter, replica]` (the spec's `<<counter, replica>>` id) -> Rust ElementId.
/// `wall = 0` makes the Rust `(wall, counter, replica)` order reduce to the
/// spec's `(counter, replica)` tie-break.
fn eid(p: [u64; 2]) -> ElementId {
    ElementId {
        hlc: Hlc {
            wall: 0,
            counter: p[0] as u32,
        },
        replica: ReplicaId(p[1]),
    }
}

fn unid(e: ElementId) -> [u64; 2] {
    [u64::from(e.hlc.counter), e.replica.0]
}

/// Per-replica (visible id sequence, tombstone set).
type RgaView = BTreeMap<u64, (Vec<[u64; 2]>, BTreeSet<[u64; 2]>)>;

/// Replay an RGA trace; returns per-replica (visible id sequence, tombstone set).
fn replay_rga(trace: &RgaTrace) -> RgaView {
    let mut reps: BTreeMap<u64, Rga<u8>> = trace
        .replicas
        .iter()
        .map(|&r| (r, Rga::new(ReplicaId(r))))
        .collect();
    for op in &trace.ops {
        match op {
            RgaOp::Insert { r, id, after } => {
                let aft = after.map(eid);
                reps.get_mut(r)
                    .expect("unknown replica")
                    .insert_after_with_id(eid(*id), aft, 0u8);
            }
            RgaOp::Delete { r, target } => {
                reps.get_mut(r)
                    .expect("unknown replica")
                    .delete(eid(*target));
            }
            RgaOp::Merge { r, s } => {
                let src = reps[s].clone();
                reps.get_mut(r).expect("unknown replica").merge(&src);
            }
        }
    }
    trace
        .replicas
        .iter()
        .map(|&r| {
            let visible: Vec<[u64; 2]> = reps[&r].ids().into_iter().map(unid).collect();
            let tomb: BTreeSet<[u64; 2]> = reps[&r]
                .tla_state()
                .tombstones
                .into_iter()
                .map(unid)
                .collect();
            (r, (visible, tomb))
        })
        .collect()
}

#[test]
fn rga_trace_replay_matches_spec() {
    let trace: RgaTrace = serde_json::from_str(&load("rga.json")).unwrap();
    assert_eq!(trace.crdt, "rga");
    let got = replay_rga(&trace);
    let want_tomb: BTreeSet<[u64; 2]> = trace.expected_tombstones.iter().copied().collect();
    for &r in &trace.replicas {
        let (visible, tomb) = &got[&r];
        assert_eq!(
            visible, &trace.expected_visible,
            "replica {r} visible sequence"
        );
        assert_eq!(tomb, &want_tomb, "replica {r} tombstones");
    }
}

// ---- Negativity checks: give every positive test above teeth ---------------
// Each perturbs the pinned trace and confirms the match FAILS, so all three
// positive tests (G-Counter, OR-Set, RGA) are not vacuous.

#[test]
fn gcounter_negative_dropping_merge_changes_component_counts() {
    let mut trace: GCounterTrace = serde_json::from_str(&load("gcounter.json")).unwrap();
    // Op #6 (0-based) is `merge to=3 from=1` — how replica 3 learns r1's and r2's
    // increments. Drop it and replica 3 keeps only its own increment, so its
    // component counts must diverge from the pinned final state.
    trace.ops.remove(6);
    let got = replay_gcounter(&trace);
    let want = gcounter_expected(&trace);
    eprintln!(
        "[gcounter negative] dropped the merge to r3 -> r3 counts {:?} (pinned was {:?})",
        got[&3], want[&3]
    );
    assert_ne!(
        got[&3], want[&3],
        "perturbing the trace must change replica 3's component counts"
    );
    assert_eq!(
        got[&3][&1], 0,
        "replica 3 must no longer know r1's increments (the merge was dropped)"
    );
}

#[test]
fn orset_negative_dropping_concurrent_add_breaks_membership() {
    let mut trace: OrSetTrace = serde_json::from_str(&load("orset.json")).unwrap();
    // Op #2 (0-based) is the concurrent re-add of element 1 (a fresh tag) that
    // makes add-wins hold. Drop it: the only tag of 1 is then the one the remove
    // tombstoned, so element 1 must vanish.
    trace.ops.remove(2);
    let got = replay_orset(&trace);
    eprintln!(
        "[orset negative] dropped the concurrent re-add -> membership r1={:?} r2={:?} (pinned was {{1}})",
        got[&1], got[&2]
    );
    let pinned: BTreeSet<u8> = [1u8].into_iter().collect();
    assert_ne!(
        got[&1], pinned,
        "perturbing the trace must change membership"
    );
    assert!(
        !got[&1].contains(&1),
        "element 1 must now be absent (add-wins broken)"
    );
}

#[test]
fn rga_negative_dropping_delete_changes_visible_sequence() {
    let mut trace: RgaTrace = serde_json::from_str(&load("rga.json")).unwrap();
    // Op #6 (0-based) is the delete of <<1,1>>. Drop it: <<1,1>> stays visible,
    // so the sequence is [<<0,1>>, <<1,1>>, <<0,2>>], not the pinned 2-element one.
    trace.ops.remove(6);
    let got = replay_rga(&trace);
    eprintln!(
        "[rga negative] dropped the delete -> visible r1={:?} (pinned was {:?})",
        got[&1].0, trace.expected_visible
    );
    assert_ne!(
        got[&1].0, trace.expected_visible,
        "perturbing the trace must change the visible sequence"
    );
    assert!(
        got[&1].0.contains(&[1, 1]),
        "<<1,1>> must now be visible (its tombstone was dropped)"
    );
}
