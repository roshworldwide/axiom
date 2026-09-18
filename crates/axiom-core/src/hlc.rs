use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Hlc {
    pub wall: u64,
    pub counter: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HlcClock {
    last: Hlc,
}

impl HlcClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn last(&self) -> Hlc {
        self.last
    }

    pub fn tick_at(&mut self, now_millis: u64) -> Hlc {
        let wall = self.last.wall.max(now_millis);
        self.last = if wall == self.last.wall {
            incremented(wall, self.last.counter)
        } else {
            Hlc { wall, counter: 0 }
        };
        self.last
    }

    pub fn tick(&mut self) -> Hlc {
        self.tick_at(now_millis())
    }

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

    pub fn observe(&mut self, remote: Hlc) -> Hlc {
        self.observe_at(now_millis(), remote)
    }
}

fn incremented(wall: u64, counter: u32) -> Hlc {
    match counter.checked_add(1) {
        Some(c) => Hlc { wall, counter: c },
        None => Hlc {
            wall: wall.saturating_add(1),
            counter: 0,
        },
    }
}

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
        let b = c.tick_at(50);
        assert!(b > a);
        assert_eq!(b.wall, 100);
    }

    #[test]
    fn counter_overflow_rolls_wall_and_stays_monotonic() {
        let mut c = HlcClock::new();
        let t1 = c.observe_at(
            0,
            Hlc {
                wall: 5,
                counter: u32::MAX,
            },
        );
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
        #![proptest_config(ProptestConfig::with_cases(crate::proptest_cases()))]

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
