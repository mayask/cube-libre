//! Reconcile immediate move predictions with authoritative snapshots.
//! A missing move makes the display explicitly stale until a snapshot repairs it.
use crate::{CubeState, Move};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedMove {
    pub counter: u8,
    pub cube_time_ms: u32,
    pub movement: Move,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MoveOutcome {
    pub accepted: bool,
    pub request_state: bool,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnapshotOutcome {
    pub accepted: bool,
    pub request_state: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Tracker {
    pub state: Option<CubeState>,
    pub counter: Option<u8>,
    pub synced: bool,
    pub observed_turns: u64,
    pub missed_turns: u64,
    pub corrections: u64,
    pending: VecDeque<ObservedMove>,
    recent: VecDeque<(u8, u32)>,
}
impl Tracker {
    pub fn invalidate(&mut self) {
        self.synced = false;
    }
    pub fn on_move(&mut self, event: ObservedMove) -> MoveOutcome {
        let key = (event.counter, event.cube_time_ms);
        if self.recent.contains(&key) {
            return MoveOutcome::default();
        }
        if self.synced
            && let Some(counter) = self.counter
        {
            let delta = event.counter.wrapping_sub(counter);
            // GAN's shared history window is modulo 256 across these drivers.
            // Gen3/4 wire counters are normalized to their low byte. Older events must
            // not undo a newer authoritative snapshot.
            if delta == 0 || delta >= 128 {
                return MoveOutcome::default();
            }
        }
        self.recent.push_back(key);
        if self.recent.len() > 128 {
            self.recent.pop_front();
        }
        self.observed_turns += 1;
        if self.synced
            && let (Some(counter), Some(state)) = (self.counter, &mut self.state)
        {
            let delta = event.counter.wrapping_sub(counter);
            if delta == 1 {
                state.apply(event.movement);
                self.counter = Some(event.counter);
                return MoveOutcome {
                    accepted: true,
                    request_state: false,
                };
            }
            self.missed_turns += u64::from(delta - 1);
        }
        self.synced = false;
        if self.pending.len() == 128 {
            self.pending.clear();
        }
        self.pending.push_back(event);
        MoveOutcome {
            accepted: true,
            request_state: true,
        }
    }
    pub fn on_snapshot(&mut self, counter: u8, state: CubeState) -> SnapshotOutcome {
        if self.synced
            && let Some(previous) = self.counter
        {
            let delta = counter.wrapping_sub(previous);
            if delta >= 128 {
                return SnapshotOutcome::default();
            }
            if delta > 0 {
                self.missed_turns += u64::from(delta);
            }
        }
        if self.state.as_ref().is_some_and(|old| old != &state) {
            self.corrections += 1;
        }
        self.state = Some(state);
        self.counter = Some(counter);
        self.synced = true;
        let mut pending: Vec<_> = self
            .pending
            .drain(..)
            .filter(|m| {
                let d = m.counter.wrapping_sub(counter);
                d > 0 && d < 128
            })
            .collect();
        pending.sort_by_key(|m| m.counter.wrapping_sub(counter));
        for event in pending {
            let delta = event
                .counter
                .wrapping_sub(self.counter.expect("snapshot counter"));
            if delta == 0 {
                continue;
            }
            if self.synced && delta == 1 {
                self.state
                    .as_mut()
                    .expect("snapshot state")
                    .apply(event.movement);
                self.counter = Some(event.counter);
            } else {
                self.synced = false;
                self.pending.push_back(event);
            }
        }
        SnapshotOutcome {
            accepted: true,
            request_state: !self.synced,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(counter: u8, text: &str) -> ObservedMove {
        ObservedMove {
            counter,
            cube_time_ms: counter as u32 * 100,
            movement: text.parse().unwrap(),
        }
    }
    #[test]
    fn never_claims_solved_before_receiving_state() {
        let mut t = Tracker::default();
        assert!(t.state.is_none());
        assert!(!t.synced);
        assert!(t.on_move(event(4, "U")).request_state);
        assert!(t.state.is_none());
    }
    #[test]
    fn contiguous_moves_predict_and_wrap() {
        let mut t = Tracker::default();
        t.on_snapshot(254, CubeState::solved());
        for e in [event(255, "R"), event(0, "U"), event(1, "R'")] {
            assert!(!t.on_move(e).request_state);
        }
        let mut expected = CubeState::solved();
        for text in ["R", "U", "R'"] {
            expected.apply(text.parse().unwrap());
        }
        assert_eq!(t.state, Some(expected));
        assert!(t.synced);
        assert_eq!(t.counter, Some(1));
    }
    #[test]
    fn missing_moves_freeze_prediction_and_snapshot_repairs() {
        let mut t = Tracker::default();
        t.on_snapshot(10, CubeState::solved());
        assert!(t.on_move(event(13, "R")).request_state);
        assert!(!t.synced);
        assert!(t.state.as_ref().unwrap().is_solved());
        assert_eq!(t.missed_turns, 2);
        let mut actual = CubeState::solved();
        for text in ["F", "U", "R"] {
            actual.apply(text.parse().unwrap());
        }
        assert!(!t.on_snapshot(13, actual.clone()).request_state);
        assert!(t.synced);
        assert_eq!(t.state, Some(actual));
    }
    #[test]
    fn snapshot_replays_only_newer_buffered_moves() {
        let mut t = Tracker::default();
        t.on_move(event(8, "R"));
        t.on_move(event(9, "U"));
        t.on_snapshot(8, CubeState::solved());
        let mut expected = CubeState::solved();
        expected.apply("U".parse().unwrap());
        assert_eq!(t.state, Some(expected));
        assert!(t.synced);
        assert_eq!(t.counter, Some(9));
    }
    #[test]
    fn duplicate_and_older_packets_do_not_rewind_cube() {
        let mut t = Tracker::default();
        t.on_snapshot(10, CubeState::solved());
        let e = event(11, "R");
        t.on_move(e);
        assert!(!t.on_move(e).accepted);
        assert!(!t.on_move(event(9, "F")).accepted);
        let before = t.state.clone();
        assert!(!t.on_snapshot(10, CubeState::solved()).accepted);
        assert_eq!(t.state, before);
        assert_eq!(t.observed_turns, 1);
    }
    #[test]
    fn snapshot_corrects_prediction_even_at_same_counter() {
        let mut t = Tracker::default();
        t.on_snapshot(10, CubeState::solved());
        t.on_move(event(11, "R"));
        t.on_snapshot(11, CubeState::solved());
        assert!(t.state.unwrap().is_solved());
        assert_eq!(t.corrections, 1);
    }
    #[test]
    fn unresolved_buffer_gap_requires_another_snapshot() {
        let mut t = Tracker::default();
        t.on_move(event(15, "R"));
        assert!(t.on_snapshot(10, CubeState::solved()).request_state);
        assert!(!t.synced);
        assert!(!t.on_snapshot(15, CubeState::solved()).request_state);
        assert!(t.synced);
    }
}
