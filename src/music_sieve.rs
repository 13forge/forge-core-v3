//! MusicSieve — Ledger-driven O(1) acoustic profile updates.
//!
//! Intercepts `VixelDiff` entries from the DiffPool and maintains running
//! macro-acoustic totals (mass, ring_freq, attack) as i32 integers.
//! No iteration over chunk voxels — one diff = one O(1) lookup pair.
//!
//! Published acoustic profile uses a lock-free seqlock pattern with 3 pre-allocated
//! slots and atomic slot-index + generation counter (zero mutex, ~1ns handoff).
//! The audio thread reads the latest profile without locking.
//!
//! All arithmetic is integer-only. Fits in a single cache line (64 bytes).

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, AtomicI32, Ordering};

use crate::diff_pool::VixelDiff;

/// Macro-acoustic profile for a chunk.
///
/// Three running totals derived from the `.forge_reg` DODRegistries columns.
/// Updated incrementally via O(1) integer deltas per diff.
/// Sized to fit in a single cache line (12 bytes data + padding).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcousticProfile {
    /// Running total of mass across all voxels in the chunk.
    pub chunk_mass: i32,
    /// Running total of ring frequency across all voxels.
    pub chunk_ring: i32,
    /// Running total of attack across all voxels.
    pub chunk_attack: i32,
}

/// Minimal DOD registry view for acoustic lookups.
///
/// The MusicSieve only needs mass, ring_freq, and attack columns.
/// This trait abstracts the lookup so we don't depend on the full
/// DODRegistries struct (which requires mmap and file I/O).
pub trait AcousticRegistry {
    /// Look up mass value for a material ID.
    fn mass(&self, mat_id: u16) -> u16;
    /// Look up ring frequency value for a material ID.
    fn ring_freq(&self, mat_id: u16) -> u16;
    /// Look up attack value for a material ID.
    fn attack(&self, mat_id: u16) -> u16;
}

/// A single acoustic profile slot using atomic i32 fields.
///
/// Allows lock-free reads and writes of profile values without UnsafeCell.
/// Each field is independently atomic, enabling safe interior mutation.
struct AtomicSlot {
    /// Atomically stored mass value.
    chunk_mass: AtomicI32,
    /// Atomically stored ring frequency value.
    chunk_ring: AtomicI32,
    /// Atomically stored attack value.
    chunk_attack: AtomicI32,
}

impl AtomicSlot {
    /// Create a new slot initialized to zero values.
    fn new() -> Self {
        Self {
            chunk_mass: AtomicI32::new(0),
            chunk_ring: AtomicI32::new(0),
            chunk_attack: AtomicI32::new(0),
        }
    }

    /// Atomically store a profile to this slot.
    fn store(&self, profile: AcousticProfile, ordering: Ordering) {
        self.chunk_mass.store(profile.chunk_mass, ordering);
        self.chunk_ring.store(profile.chunk_ring, ordering);
        self.chunk_attack.store(profile.chunk_attack, ordering);
    }

    /// Atomically load a profile from this slot.
    fn load(&self, ordering: Ordering) -> AcousticProfile {
        AcousticProfile {
            chunk_mass: self.chunk_mass.load(ordering),
            chunk_ring: self.chunk_ring.load(ordering),
            chunk_attack: self.chunk_attack.load(ordering),
        }
    }
}

/// Shared state for lock-free acoustic profile publishing.
///
/// Holds exactly 3 pre-allocated profile slots (never reallocated, never freed),
/// an atomic slot index, and a generation counter for seqlock-pattern consistency.
struct PublishedState {
    /// Three pre-allocated AtomicSlot cells for lock-free reads/writes
    slots: [AtomicSlot; 3],
    /// Index of currently published slot (0, 1, or 2, rotates mod 3)
    published_slot: AtomicU8,
    /// Generation counter: incremented before and after publish (odd/even = in-progress/committed)
    generation: AtomicU64,
}

impl PublishedState {
    fn new() -> Self {
        Self {
            slots: [AtomicSlot::new(), AtomicSlot::new(), AtomicSlot::new()],
            published_slot: AtomicU8::new(0),
            generation: AtomicU64::new(0),
        }
    }
}

/// Audio thread's subscription handle to read published acoustic profiles.
///
/// Holds an Arc to the shared published state. Readers call `read()` to fetch
/// the latest profile using a seqlock pattern (generation check + slot select).
pub struct AcousticSubscriber {
    state: Arc<PublishedState>,
}

impl AcousticSubscriber {
    /// Read the latest published acoustic profile.
    ///
    /// Seqlock pattern: detects mid-publish writes by checking generation counter.
    /// Zero mutex, lock-free, ~1ns atomic loads. Returns a copy of the profile.
    pub fn read(&self) -> AcousticProfile {
        loop {
            let gen_before = self.state.generation.load(Ordering::Acquire);
            let slot_idx = self.state.published_slot.load(Ordering::Acquire);

            // Load profile from the currently published slot atomically.
            // slot_idx is always 0, 1, or 2 (modulo 3 in publish()).
            let profile = self.state.slots[slot_idx as usize].load(Ordering::Acquire);

            let gen_after = self.state.generation.load(Ordering::Acquire);

            // If generation unchanged, we read atomically before any write started.
            if gen_before == gen_after {
                return profile;
            }
            // Generation changed; writer was mid-publish; retry to get consistent snapshot.
        }
    }
}

impl Clone for AcousticSubscriber {
    fn clone(&self) -> Self {
        AcousticSubscriber {
            state: Arc::clone(&self.state),
        }
    }
}

/// MusicSieve — intercepts diffs, maintains O(1) acoustic running totals.
///
/// One instance per active chunk. The TickEngine calls `on_diff()` for each
/// `VixelDiff` that touches this chunk. The audio thread reads the latest
/// `AcousticProfile` via the subscriber obtained from `subscriber()`.
pub struct MusicSieve {
    /// Shared state: 3-slot array + atomic slot index + generation counter
    state: Arc<PublishedState>,
    /// Current acoustic profile (physics thread owned, not yet published)
    profile: AcousticProfile,
}

impl MusicSieve {
    /// Create a new MusicSieve with zeroed acoustic totals.
    ///
    /// Allocates PublishedState exactly once (3 fixed slots, never reallocated).
    pub fn new() -> Self {
        Self {
            state: Arc::new(PublishedState::new()),
            profile: AcousticProfile::default(),
        }
    }

    /// O(1) acoustic update on a single VixelDiff.
    ///
    /// Exactly 3 integer operations: mass delta, ring_freq delta, attack delta.
    /// No iteration over chunk voxels. No floating-point math.
    pub fn on_diff(&mut self, diff: &VixelDiff, reg: &dyn AcousticRegistry) {
        let old = diff.old_mat;
        let new = diff.new_mat;

        self.profile.chunk_mass += reg.mass(new) as i32 - reg.mass(old) as i32;
        self.profile.chunk_ring += reg.ring_freq(new) as i32 - reg.ring_freq(old) as i32;
        self.profile.chunk_attack += reg.attack(new) as i32 - reg.attack(old) as i32;
    }

    /// Publish the current acoustic profile to the audio thread.
    ///
    /// Atomic operations: rotate slot, bump generation counter.
    /// ~1ns, zero mutex, lock-free. Audio thread detects completion via generation match.
    pub fn publish(&self) {
        // Pick next slot (one that is not currently published)
        let current_slot = self.state.published_slot.load(Ordering::Acquire);
        let next_slot = (current_slot + 1) % 3;

        // Atomically write profile to next_slot (no unsafe, all atomic operations)
        self.state.slots[next_slot as usize].store(self.profile, Ordering::Release);

        // Publish sequence: bump generation (signals mid-publish), swap slot, bump generation (signals done)
        // Readers detect this via generation mismatch (seqlock pattern).
        self.state.generation.fetch_add(1, Ordering::Release);
        self.state.published_slot.store(next_slot, Ordering::Release);
        self.state.generation.fetch_add(1, Ordering::Release);
    }

    /// Get a subscription handle for the audio thread to read published profiles.
    ///
    /// Returns an AcousticSubscriber that can safely call `read()` from any thread.
    pub fn subscriber(&self) -> AcousticSubscriber {
        AcousticSubscriber {
            state: Arc::clone(&self.state),
        }
    }

    /// Current profile (physics thread side, not yet published).
    pub fn profile(&self) -> &AcousticProfile {
        &self.profile
    }
}

impl Default for MusicSieve {
    fn default() -> Self {
        Self::new()
    }
}

// ── Manual tests (replacing proptest) ──────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_pool::VixelDiff;

    /// Test registry with fixed values for manual testing.
    struct TestRegistry {
        mass: [u16; 256],
        ring_freq: [u16; 256],
        attack: [u16; 256],
    }

    impl TestRegistry {
        fn new(mass: [u16; 256], ring_freq: [u16; 256], attack: [u16; 256]) -> Self {
            Self {
                mass,
                ring_freq,
                attack,
            }
        }
    }

    impl AcousticRegistry for TestRegistry {
        fn mass(&self, mat_id: u16) -> u16 {
            self.mass[mat_id as usize % 256]
        }
        fn ring_freq(&self, mat_id: u16) -> u16 {
            self.ring_freq[mat_id as usize % 256]
        }
        fn attack(&self, mat_id: u16) -> u16 {
            self.attack[mat_id as usize % 256]
        }
    }

    /// Test O(1) invariant: single diff produces exact integer deltas.
    ///
    /// Replaces: prop_cp7_acoustic_o1_invariant
    /// Verifies: Mass, ring, attack deltas match expected registry lookups.
    #[test]
    fn test_cp7_single_diff_o1_delta() {
        let mut mass = [0u16; 256];
        let mut ring = [0u16; 256];
        let mut attack = [0u16; 256];
        mass[0] = 1000;
        mass[1] = 5000;
        ring[0] = 2000;
        ring[1] = 3000;
        attack[0] = 500;
        attack[1] = 7000;
        let reg = TestRegistry::new(mass, ring, attack);

        let mut sieve = MusicSieve::new();
        let before = *sieve.profile();

        let diff = VixelDiff {
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
            index: 0,
            old_mat: 0,
            new_mat: 1,
        };
        sieve.on_diff(&diff, &reg);

        let after = *sieve.profile();

        // Verify: exactly the O(1) integer delta
        let expected_mass_delta = 5000i32 - 1000i32; // 4000
        let expected_ring_delta = 3000i32 - 2000i32; // 1000
        let expected_attack_delta = 7000i32 - 500i32; // 6500

        assert_eq!(
            after.chunk_mass - before.chunk_mass,
            expected_mass_delta,
            "Mass delta mismatch"
        );
        assert_eq!(
            after.chunk_ring - before.chunk_ring,
            expected_ring_delta,
            "Ring delta mismatch"
        );
        assert_eq!(
            after.chunk_attack - before.chunk_attack,
            expected_attack_delta,
            "Attack delta mismatch"
        );
    }

    /// Test accumulation: multiple sequential diffs accumulate correctly.
    ///
    /// Replaces: prop_cp7_multi_diff_accumulation
    /// Verifies: Running totals match sum of individual deltas.
    #[test]
    fn test_cp7_multi_diff_accumulation() {
        let mut mass = [0u16; 256];
        let mut ring = [0u16; 256];
        let mut attack = [0u16; 256];
        mass[1] = 100;
        mass[2] = 200;
        mass[3] = 300;
        ring[1] = 50;
        ring[2] = 75;
        ring[3] = 100;
        attack[1] = 10;
        attack[2] = 20;
        attack[3] = 30;
        let reg = TestRegistry::new(mass, ring, attack);

        let mut sieve = MusicSieve::new();

        // Apply sequence of diffs: (old_mat, new_mat) transitions
        let transitions = vec![(0, 1), (1, 2), (2, 3), (3, 1), (1, 0)];
        let mut expected_mass = 0i32;
        let mut expected_ring = 0i32;
        let mut expected_attack = 0i32;

        for (old_mat, new_mat) in transitions {
            let diff = VixelDiff {
                chunk_x: 0,
                chunk_y: 0,
                chunk_z: 0,
                index: 0,
                old_mat,
                new_mat,
            };
            sieve.on_diff(&diff, &reg);

            expected_mass += reg.mass(new_mat) as i32 - reg.mass(old_mat) as i32;
            expected_ring += reg.ring_freq(new_mat) as i32 - reg.ring_freq(old_mat) as i32;
            expected_attack += reg.attack(new_mat) as i32 - reg.attack(old_mat) as i32;
        }

        let profile = sieve.profile();
        assert_eq!(profile.chunk_mass, expected_mass, "Accumulated mass mismatch");
        assert_eq!(profile.chunk_ring, expected_ring, "Accumulated ring mismatch");
        assert_eq!(
            profile.chunk_attack, expected_attack,
            "Accumulated attack mismatch"
        );
    }

    /// Test publish/subscribe lock-free handoff with seqlock pattern.
    ///
    /// Replaces: test_publish_subscribe
    /// Verifies: Published profiles are readable by subscriber without mutex.
    #[test]
    fn test_publish_subscribe_handoff() {
        let mut sieve = MusicSieve::new();
        let sub = sieve.subscriber();

        // Initial state: all slots start at default profile
        let profile = sub.read();
        assert_eq!(profile, AcousticProfile::default());

        // Create registry
        let mut mass = [0u16; 256];
        let mut ring = [0u16; 256];
        let mut attack = [0u16; 256];
        mass[1] = 5000;
        ring[1] = 3000;
        attack[1] = 7000;
        let reg = TestRegistry::new(mass, ring, attack);

        // Apply diff: air (0) → material 1
        let diff = VixelDiff {
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
            index: 0,
            old_mat: 0,
            new_mat: 1,
        };
        sieve.on_diff(&diff, &reg);
        sieve.publish();

        // Subscriber reads latest published profile
        let profile = sub.read();
        assert_eq!(profile.chunk_mass, 5000);
        assert_eq!(profile.chunk_ring, 3000);
        assert_eq!(profile.chunk_attack, 7000);
    }

    /// Test slot rotation and generation counter consistency.
    ///
    /// Verifies: Multiple publishes rotate through 3 slots correctly,
    /// and generation counter prevents stale reads.
    #[test]
    fn test_slot_rotation_and_generation() {
        let mut sieve = MusicSieve::new();
        let sub = sieve.subscriber();

        let mut mass = [0u16; 256];
        mass[10] = 2000;
        mass[20] = 4000;
        let reg = TestRegistry::new(mass, [0u16; 256], [0u16; 256]);

        // First publish: transition to material 10 (mass = 2000)
        let diff1 = VixelDiff {
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
            index: 0,
            old_mat: 0,
            new_mat: 10,
        };
        sieve.on_diff(&diff1, &reg);
        sieve.publish();

        let p1 = sub.read();
        assert_eq!(p1.chunk_mass, 2000, "First publish read mismatch");

        // Second publish: add material 20 (mass = 4000, total now 6000)
        let diff2 = VixelDiff {
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
            index: 1,
            old_mat: 0,
            new_mat: 20,
        };
        sieve.on_diff(&diff2, &reg);
        sieve.publish();

        let p2 = sub.read();
        assert_eq!(p2.chunk_mass, 6000, "Second publish read mismatch");

        // Third publish: remove material 10 (mass = 4000)
        let diff3 = VixelDiff {
            chunk_x: 0,
            chunk_y: 0,
            chunk_z: 0,
            index: 0,
            old_mat: 10,
            new_mat: 0,
        };
        sieve.on_diff(&diff3, &reg);
        sieve.publish();

        let p3 = sub.read();
        assert_eq!(p3.chunk_mass, 4000, "Third publish read mismatch");
    }
}
