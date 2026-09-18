#![forbid(unsafe_code)]

pub mod axiom_verify;
pub mod causal_broadcast;
pub mod gcounter;
pub mod hlc;
pub mod orset;
pub mod pncounter;
pub mod rga;
pub mod vector_clock;

pub use axiom_verify::join_in_order;
pub use causal_broadcast::{CausalProcess, Message, Network};
pub use gcounter::{GCounter, GCounterOp, TlaGCounterState};
pub use hlc::{Hlc, HlcClock};
pub use orset::{ORSet, ORSetOp, TlaORSetState};
pub use pncounter::{PNCounter, PNCounterOp, TlaPNCounterState};
pub use rga::{ElementId, Rga, TlaRgaState};
pub use vector_clock::{ReplicaId, VectorClock};

#[cfg(test)]
pub(crate) fn proptest_cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(256)
}
