//! Per-tick peer input barrier + FNV1a state-hash chain + desync verdict + authoritative
//! rollback. Integer-only, fixed arrays, no allocation in the steady path.
//!
//! LINEAGE (four generations, `repo-map.tsv` 2026-08-15):
//! 1. `astrakey_skill/network/ak_lockstep_coordinator.gd` — GDScript, unbounded peers,
//!    Dictionary buffers, string hashes (`E:\.airgap`, corpse prior).
//! 2. `F:\NewRepo\crates\forge-core\src\lockstep.rs` — Sean 2026-07-17, the bounding pass.
//! 3. `F:\NewRepo\crates\forge-game-systems\src\network\lockstep.rs` — session layer above it.
//! 4. here.
//!
//! The v2 pass tightened the Godot original deliberately: bounded peers, a fixed window, and
//! fixed arrays instead of dictionaries. That is not a simplification — unbounded history and
//! map iteration are precisely what make a netcode chain unreplayable, because iteration order
//! leaks into the hash. This port keeps the tightening and adds nothing.
//!
//! The one thing this module buys: [`LockstepBarrier::try_advance`] folds peer inputs into the
//! chain in **peer index order**, so two machines that saw the same inputs derive the same
//! `chain_hash` with no coordinator and no agreement protocol.

use crate::checksum::{fnv1a64_fold, FNV_OFFSET_BASIS};

/// Maximum simultaneous peers a barrier will track.
///
/// Fixed so the slot table is a stack array and `submit` cannot allocate. Raising it widens
/// every `TickBundle`; it is a layout decision, not a tuning knob.
pub const MAX_PEERS: usize = 4;

/// Ticks the barrier will hold inputs for before the caller must advance.
///
/// The slot table is a ring of this depth, so a peer may run at most this far ahead of the
/// slowest one. Beyond it, `submit` fails loud rather than dropping an input.
pub const WINDOW: usize = 16;

/// Why a `submit` was refused. Every variant is a loud failure, never a silent drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockstepErr {
    /// Peer index was at or beyond the barrier's active peer count.
    PeerOutOfRange,
    /// The tick has already been released; its inputs can no longer change history.
    TickInPast,
    /// The tick is further ahead than [`WINDOW`] — the sender is outrunning the barrier.
    TickBeyondWindow,
}

/// One released tick: every active peer's input, plus the chain hash after folding them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickBundle {
    /// The tick that was released.
    pub tick: u64,
    /// Each peer's input word, indexed by peer. Inactive peers read 0.
    pub inputs: [u32; MAX_PEERS],
    /// The chain hash after this tick folded in — the value peers compare.
    pub chain_hash: u64,
}

const _: () = assert!(core::mem::size_of::<TickBundle>() == 32);
const _: () = assert!(core::mem::align_of::<TickBundle>() == 8);

/// The result of comparing a remote chain hash against the local one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Both sides derived the same history.
    Match,
    /// Histories diverged. Carries both hashes so the caller can log what disagreed.
    Desync {
        /// The tick at which the comparison was made.
        tick: u64,
        /// This machine's chain hash.
        local: u64,
        /// The peer's chain hash.
        remote: u64,
    },
}

/// The fundamental lockstep constraint: the sim cannot advance past a tick until EVERY active
/// peer's input for that tick is present.
///
/// Inputs fold into the chain hash in peer order — canonical order, replay-stable, no map
/// iteration. Two machines given the same inputs produce the same hash without talking.
pub struct LockstepBarrier {
    peers: u8,
    tick: u64,
    chain_hash: u64,
    slots: [[Option<u32>; MAX_PEERS]; WINDOW],
}

impl LockstepBarrier {
    /// A barrier awaiting `peers` participants, clamped to `1..=MAX_PEERS`.
    ///
    /// The chain starts at [`FNV_OFFSET_BASIS`], so an untouched barrier already has a
    /// well-defined hash to compare.
    pub fn new(peers: u8) -> Self {
        let peers = peers.clamp(1, MAX_PEERS as u8);
        Self { peers, tick: 0, chain_hash: FNV_OFFSET_BASIS, slots: [[None; MAX_PEERS]; WINDOW] }
    }

    /// The tick the barrier is currently waiting to release.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// The rolling hash of every tick released so far.
    pub fn chain_hash(&self) -> u64 {
        self.chain_hash
    }

    /// How many peers this barrier waits on.
    pub fn peers(&self) -> u8 {
        self.peers
    }

    /// Queue a peer's input word for a tick inside the window.
    ///
    /// Fails loud on a past tick or one beyond [`WINDOW`] rather than dropping the input —
    /// a silently discarded input is a desync nobody can trace.
    pub fn submit(&mut self, tick: u64, peer: u8, input: u32) -> Result<(), LockstepErr> {
        if peer >= self.peers {
            return Err(LockstepErr::PeerOutOfRange);
        }
        if tick < self.tick {
            return Err(LockstepErr::TickInPast);
        }
        if tick >= self.tick + WINDOW as u64 {
            return Err(LockstepErr::TickBeyondWindow);
        }
        self.slots[(tick % WINDOW as u64) as usize][peer as usize] = Some(input);
        Ok(())
    }

    /// True when the current tick has an input from every active peer.
    pub fn ready(&self) -> bool {
        let row = &self.slots[(self.tick % WINDOW as u64) as usize];
        (0..self.peers as usize).all(|p| row[p].is_some())
    }

    /// Release the current tick if complete: fold tick + inputs (peer order) into the chain,
    /// clear the slot, advance. `None` while any peer is still missing.
    pub fn try_advance(&mut self) -> Option<TickBundle> {
        if !self.ready() {
            return None;
        }
        let idx = (self.tick % WINDOW as u64) as usize;
        let mut inputs = [0u32; MAX_PEERS];
        for p in 0..self.peers as usize {
            inputs[p] = self.slots[idx][p].take().unwrap_or(0);
        }
        for p in self.peers as usize..MAX_PEERS {
            self.slots[idx][p] = None;
        }
        let mut h = fnv1a64_fold(self.chain_hash, self.tick);
        for p in 0..self.peers as usize {
            h = fnv1a64_fold(h, inputs[p] as u64);
        }
        self.chain_hash = h;
        let bundle = TickBundle { tick: self.tick, inputs, chain_hash: h };
        self.tick += 1;
        Some(bundle)
    }

    /// Compare a remote chain hash against the local one, at equal tick counts.
    pub fn verify_remote(&self, remote: u64) -> Verdict {
        if remote == self.chain_hash {
            Verdict::Match
        } else {
            Verdict::Desync { tick: self.tick, local: self.chain_hash, remote }
        }
    }

    /// Authoritative rollback: adopt a trusted `(tick, chain_hash)` point and drop every
    /// queued input, because inputs banked against a rejected history are themselves rejected.
    pub fn rollback_to(&mut self, tick: u64, chain_hash: u64) {
        self.tick = tick;
        self.chain_hash = chain_hash;
        self.slots = [[None; MAX_PEERS]; WINDOW];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrier_holds_until_every_peer_submits() {
        let mut b = LockstepBarrier::new(2);
        assert!(!b.ready());
        b.submit(0, 0, 0xAA).unwrap();
        assert!(!b.ready(), "one peer is not enough");
        assert!(b.try_advance().is_none());
        b.submit(0, 1, 0xBB).unwrap();
        assert!(b.ready());
        let bundle = b.try_advance().expect("both peers in, must release");
        assert_eq!(bundle.tick, 0);
        assert_eq!(bundle.inputs[0], 0xAA);
        assert_eq!(bundle.inputs[1], 0xBB);
        assert_eq!(b.tick(), 1);
    }

    #[test]
    fn two_machines_agree_without_talking() {
        let (mut a, mut z) = (LockstepBarrier::new(2), LockstepBarrier::new(2));
        for tick in 0..8u64 {
            for peer in 0..2u8 {
                let word = tick as u32 * 31 + peer as u32;
                a.submit(tick, peer, word).unwrap();
                z.submit(tick, peer, word).unwrap();
            }
            a.try_advance().unwrap();
            z.try_advance().unwrap();
        }
        assert_eq!(a.verify_remote(z.chain_hash()), Verdict::Match);
    }

    #[test]
    fn one_different_input_desyncs() {
        let (mut a, mut z) = (LockstepBarrier::new(2), LockstepBarrier::new(2));
        a.submit(0, 0, 1).unwrap();
        a.submit(0, 1, 2).unwrap();
        z.submit(0, 0, 1).unwrap();
        z.submit(0, 1, 3).unwrap(); // the lie
        a.try_advance().unwrap();
        z.try_advance().unwrap();
        match a.verify_remote(z.chain_hash()) {
            Verdict::Desync { tick, local, remote } => {
                assert_eq!(tick, 1);
                assert_ne!(local, remote);
            }
            Verdict::Match => panic!("differing inputs must not agree"),
        }
    }

    #[test]
    fn peer_order_is_canonical_not_arrival_order() {
        // Same inputs, submitted in opposite order. The hash must not care.
        let (mut a, mut z) = (LockstepBarrier::new(2), LockstepBarrier::new(2));
        a.submit(0, 0, 7).unwrap();
        a.submit(0, 1, 9).unwrap();
        z.submit(0, 1, 9).unwrap();
        z.submit(0, 0, 7).unwrap();
        a.try_advance().unwrap();
        z.try_advance().unwrap();
        assert_eq!(a.chain_hash(), z.chain_hash(), "arrival order must not reach the hash");
    }

    #[test]
    fn input_order_between_peers_changes_the_hash() {
        // Swapping WHICH peer sent WHICH word is a different history, and must hash differently.
        let (mut a, mut z) = (LockstepBarrier::new(2), LockstepBarrier::new(2));
        a.submit(0, 0, 7).unwrap();
        a.submit(0, 1, 9).unwrap();
        z.submit(0, 0, 9).unwrap();
        z.submit(0, 1, 7).unwrap();
        a.try_advance().unwrap();
        z.try_advance().unwrap();
        assert_ne!(a.chain_hash(), z.chain_hash());
    }

    #[test]
    fn rollback_reconverges_a_desynced_peer() {
        let (mut a, mut z) = (LockstepBarrier::new(2), LockstepBarrier::new(2));
        a.submit(0, 0, 1).unwrap();
        a.submit(0, 1, 2).unwrap();
        z.submit(0, 0, 1).unwrap();
        z.submit(0, 1, 99).unwrap();
        a.try_advance().unwrap();
        z.try_advance().unwrap();
        assert_ne!(a.chain_hash(), z.chain_hash());

        z.rollback_to(a.tick(), a.chain_hash());
        assert_eq!(a.verify_remote(z.chain_hash()), Verdict::Match);
        assert_eq!(z.tick(), a.tick());
    }

    #[test]
    fn submits_fail_loud_out_of_range() {
        let mut b = LockstepBarrier::new(2);
        assert_eq!(b.submit(0, 2, 0), Err(LockstepErr::PeerOutOfRange));
        assert_eq!(b.submit(WINDOW as u64, 0, 0), Err(LockstepErr::TickBeyondWindow));
        b.submit(0, 0, 1).unwrap();
        b.submit(0, 1, 1).unwrap();
        b.try_advance().unwrap();
        assert_eq!(b.submit(0, 0, 5), Err(LockstepErr::TickInPast));
    }

    #[test]
    fn peer_count_clamps_into_range() {
        assert_eq!(LockstepBarrier::new(0).peers(), 1);
        assert_eq!(LockstepBarrier::new(99).peers(), MAX_PEERS as u8);
    }

    #[test]
    fn fresh_barrier_starts_at_the_basis() {
        assert_eq!(LockstepBarrier::new(2).chain_hash(), FNV_OFFSET_BASIS);
        assert_eq!(LockstepBarrier::new(2).tick(), 0);
    }
}
