//! Hybrid Logical Clock (HLC).
//!
//! An [`Hlc`] timestamp combines a physical-time component (`wall`, in millis)
//! with a logical `counter` that breaks ties when several timestamps land in
//! the same millisecond. Successive timestamps from one [`HlcClock`] are
//! strictly increasing (hence unique), and [`HlcClock::observe`] keeps a clock
//! ahead of timestamps it learns from peers — so an HLC tracks causality while
//! staying close to wall-clock time.
//!
//! Combined with a `ReplicaId`, an `Hlc` yields the globally-unique, totally-
//! ordered element ids the RGA needs.

use serde::{Deserialize, Serialize};

/// A hybrid logical timestamp. Ordered by `wall`, then `counter`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Hlc {
    /// Physical time component, in milliseconds since the Unix epoch.
    pub wall: u64,
    /// Logical tiebreaker among timestamps sharing the same `wall`.
    pub counter: u32,
}

/// Generates monotonically increasing [`Hlc`] timestamps for one replica.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HlcClock {
    last: Hlc,
}

impl HlcClock {
    /// A fresh clock starting at the zero timestamp.
    pub fn new() -> Self {
        Self::default()
    }

    /// The most recently issued timestamp.
    pub fn last(&self) -> Hlc {
        self.last
    }

    /// Issue the next timestamp given the current physical time `now_millis`.
    ///
    /// The result is always strictly greater than every timestamp this clock
    /// has previously issued, so timestamps are monotonic and unique.
    pub fn tick_at(&mut self, now_millis: u64) -> Hlc {
        let wall = self.last.wall.max(now_millis);
        self.last = if wall == self.last.wall {
            incremented(wall, self.last.counter)
        } else {
            Hlc { wall, counter: 0 }
        };
        self.last
    }

    /// Issue the next timestamp using the system clock.
    pub fn tick(&mut self) -> Hlc {
        self.tick_at(now_millis())
    }

    /// Advance past a `remote` timestamp observed from a peer, given the current
    /// physical time. The result is strictly greater than both this clock's last
    /// timestamp and `remote`.
    pub fn observe_at(&mut self, now_millis: u64, remote: Hlc) -> Hlc {
        let wall = self.last.wall.max(remote.wall).max(now_millis);
        self.last = if wall == self.last.wall && wall == remote.wall {
            incremented(wall, self.last.counter.max(remote.counter))
        } else if wall == self.last.wall {
            incremented(wall, self.last.counter)
        } else if wall == remote.wall {
            incremented(wall, remote.counter)
        } else {
            Hlc { wall, counter: 0 }
        };
        self.last
    }

    /// Advance past a `remote` timestamp using the system clock.
    pub fn observe(&mut self, remote: Hlc) -> Hlc {
        self.observe_at(now_millis(), remote)
    }
}

/// The timestamp strictly after `(wall, counter)`: increment the counter, or —
/// if it is exhausted — roll into the next logical millisecond. Either way the
/// result is strictly greater than `(wall, counter)` and never reuses a value,
/// so monotonicity and uniqueness survive even a maxed-out (e.g. hostile peer)
/// counter rather than panicking or wrapping.
fn incremented(wall: u64, counter: u32) -> Hlc {
    match counter.checked_add(1) {
        Some(c) => Hlc { wall, counter: c },
        None => Hlc {
            wall: wall.saturating_add(1),
            counter: 0,
        },
    }
}

/// Milliseconds since the Unix epoch (saturating to `0` before 1970).
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn ticks_within_one_millisecond_are_distinct() {
        let mut c = HlcClock::new();
        let a = c.tick_at(100);
        let b = c.tick_at(100);
        let d = c.tick_at(100);
        assert_eq!((a.wall, a.counter), (100, 0));
        assert_eq!((b.wall, b.counter), (100, 1));
        assert_eq!((d.wall, d.counter), (100, 2));
    }

    #[test]
    fn wall_going_backwards_keeps_monotonicity() {
        let mut c = HlcClock::new();
        let a = c.tick_at(100);
        let b = c.tick_at(50); // clock skew backwards
        assert!(b > a); // still strictly increasing
        assert_eq!(b.wall, 100);
    }

    #[test]
    fn counter_overflow_rolls_wall_and_stays_monotonic() {
        let mut c = HlcClock::new();
        // Drive the clock to (wall=5, counter=u32::MAX) by observing a maxed
        // remote (reachable from a hostile/corrupt peer via Rga::merge).
        let t1 = c.observe_at(
            0,
            Hlc {
                wall: 5,
                counter: u32::MAX,
            },
        );
        // Rolls into the next logical millisecond instead of panicking/wrapping.
        assert_eq!(
            t1,
            Hlc {
                wall: 6,
                counter: 0
            }
        );
        assert!(
            t1 > Hlc {
                wall: 5,
                counter: u32::MAX
            }
        );
        // A subsequent tick keeps strictly increasing.
        let t2 = c.tick_at(6);
        assert!(t2 > t1);
    }

    #[test]
    fn observe_advances_past_remote() {
        let mut c = HlcClock::new();
        c.tick_at(10);
        let remote = Hlc {
            wall: 1000,
            counter: 5,
        };
        let t = c.observe_at(20, remote);
        assert!(t > remote);
    }

    proptest! {
        /// A sequence of ticks is strictly increasing (monotonic + unique),
        /// whatever the physical-time readings (including going backwards).
        #[test]
        fn ticks_are_strictly_increasing(times in prop::collection::vec(0u64..1000, 1..50)) {
            let mut c = HlcClock::new();
            let mut prev: Option<Hlc> = None;
            for now in times {
                let t = c.tick_at(now);
                if let Some(p) = prev {
                    prop_assert!(t > p);
                }
                prev = Some(t);
            }
        }
    }
}
