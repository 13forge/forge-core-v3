//! `Pose5D` / `UmpEventBundle` — the 5D-pose event bundle from
//! `C:\Users\seanm\Desktop\MMX3PARITY.txt`, landed on Sean's direct order
//! (2026-08-15, "port it, turn it to 5D"). Verified this session (F01): NO
//! type carrying this shape existed anywhere in F:\v3 before this file —
//! `spine::packet::Ump` is the opaque `[u32;4]` MIDI wire packet (no named
//! slots), and `ump_word::UmpWord` is a DIFFERENT 16-byte MoM routing word
//! (both already correctly distinguished from each other in `ump_word.rs`'s
//! own doc comment). This is a THIRD, additive face — not a second home for
//! either (L05) — for the specific thing MMX3PARITY.txt:30-31 describes: "a
//! sound, an animation frame, a hitbox window, and a particle are the SAME
//! record."
//!
//! \[APERTURE\] MMX3PARITY.txt's own prose says "slots 1-4" (i.e. 4x32-bit
//! MIDI-style words = 16 bytes) hold "the 5D pose + Morton key." That byte
//! budget is NOT honored here: packing 3 `MilliUnit` (i64) coordinates + a
//! `SimTick` (u64) + a 64-bit Morton key into 16 bytes total requires a lossy
//! quantization scheme this file does not invent — picking bit-widths per
//! axis without a real precision/world-bounds decision would be exactly the
//! unearned-precision guess T1 `zero_hallucination` forbids. `Pose5D` here is
//! the FULL-PRECISION representation (matches `Ghostmoon`'s own `MilliUnit`/
//! `SimTick` convention, `ghostmoon.rs`). A quantized-to-4-words variant, and
//! the Morton key itself, are named follow-up work, ARCH000-gated on the same
//! class of decision as the clock-ratio question (M05 in the skill file) —
//! not invented here.

use crate::fixed_point::{MilliUnit, SimTick};
use crate::spine::packet::Ump;

/// A point in the 5D pose space MMX3PARITY.txt describes: world position
/// (`x`,`y`,`z`), the rollback/replay time axis (`t`), and animation phase
/// (`phi`, a BAM-style fraction of `[0,1)` — `0`=windup, `~32768`=strike,
/// `65535`=~recovery, matching MMX3PARITY.txt:30 exactly). `repr(C)` pins
/// the lane order — same discipline as `Ghostmoon`'s offset locks.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pose5D {
    /// World X, milli-units.
    pub x: MilliUnit,
    /// World Y, milli-units.
    pub y: MilliUnit,
    /// World Z / layer depth, milli-units.
    pub z: MilliUnit,
    /// The frame-window / rollback axis.
    pub t: SimTick,
    /// Animation phase, `[0,1)` as a BAM-style fraction: `0`=windup,
    /// `32768`=strike, wrapping toward `65535`=recovery.
    pub phi: u16,
}

impl Pose5D {
    /// The origin pose: `(0,0,0)` at tick zero, phase `0` (windup).
    pub const ORIGIN: Self = Self { x: MilliUnit(0), y: MilliUnit(0), z: MilliUnit(0), t: SimTick::ZERO, phi: 0 };

    /// Build a pose from its lanes.
    pub const fn new(x: MilliUnit, y: MilliUnit, z: MilliUnit, t: SimTick, phi: u16) -> Self {
        Self { x, y, z, t, phi }
    }

    /// `true` once `phi` has crossed the strike point (`>= 32768`, the
    /// midpoint MMX3PARITY.txt:30 names as "0.5=strike") — the phase-as-
    /// combat-currency read MMX3PARITY.txt:46 proposes ("parries, armor
    /// cancels, weakness windows that are literally phase intervals"),
    /// landed as the one honest primitive this pass: a boundary test, not
    /// the full parry-window design (that's a combat-feel decision, not a
    /// data-shape one — out of scope here, C16 diff-floor).
    #[inline(always)]
    pub const fn past_strike(self) -> bool {
        self.phi >= 32768
    }
}

/// Which of the two named engine events (MMX3PARITY.txt:5,31: "vfx-spawn,
/// soundfont-layer, HITBOX") a slot in [`UmpEventBundle::engine_events`]
/// carries. Bitflags, not an enum: MMX3PARITY.txt allows more than one to
/// fire on the same tick (e.g. a hitbox opening WITH a vfx spawn).
pub const ENGINE_EVENT_VFX_SPAWN: u32 = 1 << 0;
/// See [`ENGINE_EVENT_VFX_SPAWN`].
pub const ENGINE_EVENT_SOUNDFONT_LAYER: u32 = 1 << 1;
/// See [`ENGINE_EVENT_VFX_SPAWN`]. The one MMX3PARITY.txt calls out as the
/// action-game-critical case: a hitbox window opening on this tick.
pub const ENGINE_EVENT_HITBOX: u32 = 1 << 2;

/// The full 13-slot event bundle MMX3PARITY.txt:30-31 describes: one
/// [`Ump`] wire packet (slot 0: playhead/note-on-off/velocity/stamp-id,
/// ALREADY LIVE, reused verbatim — not redefined here, L05) plus the pose,
/// joint rotations, FX, and engine-event slots layered around it.
/// `repr(C)`, layout locked below (measured, not hand-computed).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UmpEventBundle {
    /// Slot 0: the existing MIDI-2.0-style wire packet (playhead, note-on/
    /// off, 32-bit velocity, stamp-id) — reused from `spine::packet::Ump`.
    pub header: Ump,
    /// Slots 1-4 (aperture-widened, see module doc): the 5D pose.
    pub pose: Pose5D,
    /// Slots 5-8: four 32-bit joint rotations (or per-note pitch bends).
    pub joint_rotations: [u32; 4],
    /// Slots 9-10: FX — `[cutoff_reso, reverb_pressure]`, each a packed pair
    /// of `u16` permyriad values (0..=10000). Packing scheme only, no
    /// semantic mapping decided here (which u16 is cutoff vs reso is an
    /// audio-domain decision, named, not guessed).
    pub fx: [u32; 2],
    /// Slots 11-12: engine events — bitflags from `ENGINE_EVENT_*`, two
    /// independent slots so two unrelated event groups can co-occur without
    /// collision (e.g. a hitbox on slot 0 and a soundfont layer on slot 1).
    pub engine_events: [u32; 2],
}

impl UmpEventBundle {
    /// Build a bundle. No validation beyond what the types already enforce
    /// (`Ump`/`Pose5D` are both total constructors) — an event bundle with
    /// all-zero engine_events is a valid, silent tick, not an error.
    pub const fn new(header: Ump, pose: Pose5D, joint_rotations: [u32; 4], fx: [u32; 2], engine_events: [u32; 2]) -> Self {
        Self { header, pose, joint_rotations, fx, engine_events }
    }

    /// `true` if either engine-event slot carries [`ENGINE_EVENT_HITBOX`] —
    /// the one query a combat system actually needs each tick.
    #[inline(always)]
    pub const fn has_hitbox(&self) -> bool {
        (self.engine_events[0] & ENGINE_EVENT_HITBOX != 0) || (self.engine_events[1] & ENGINE_EVENT_HITBOX != 0)
    }
}

// ---------------------------------------------------------------------------
// LAYOUT LOCKS. Measured with rustc, never hand-computed.
// ---------------------------------------------------------------------------
const _: () = assert!(core::mem::size_of::<Pose5D>() == 40);
const _: () = assert!(core::mem::offset_of!(Pose5D, x) == 0);
const _: () = assert!(core::mem::offset_of!(Pose5D, y) == 8);
const _: () = assert!(core::mem::offset_of!(Pose5D, z) == 16);
const _: () = assert!(core::mem::offset_of!(Pose5D, t) == 24);
const _: () = assert!(core::mem::offset_of!(Pose5D, phi) == 32);

const _: () = assert!(core::mem::size_of::<UmpEventBundle>() == 88);
const _: () = assert!(core::mem::offset_of!(UmpEventBundle, header) == 0);
const _: () = assert!(core::mem::offset_of!(UmpEventBundle, pose) == 16);
const _: () = assert!(core::mem::offset_of!(UmpEventBundle, joint_rotations) == 56);
const _: () = assert!(core::mem::offset_of!(UmpEventBundle, fx) == 72);
const _: () = assert!(core::mem::offset_of!(UmpEventBundle, engine_events) == 80);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_pose_is_all_zero_and_not_past_strike() {
        assert_eq!(Pose5D::ORIGIN.x, MilliUnit(0));
        assert_eq!(Pose5D::ORIGIN.t, SimTick::ZERO);
        assert!(!Pose5D::ORIGIN.past_strike(), "phase 0 (windup) must not read as past-strike");
    }

    #[test]
    fn past_strike_boundary_is_inclusive_at_the_named_midpoint() {
        let windup = Pose5D::new(MilliUnit(0), MilliUnit(0), MilliUnit(0), SimTick::ZERO, 32767);
        let strike = Pose5D::new(MilliUnit(0), MilliUnit(0), MilliUnit(0), SimTick::ZERO, 32768);
        assert!(!windup.past_strike(), "one BAM unit before the named strike point must still be windup/pre-strike");
        assert!(strike.past_strike(), "the named strike point (32768, MMX3PARITY.txt's 0.5) must read as past-strike");
    }

    #[test]
    fn hitbox_flag_is_detected_regardless_of_which_slot_carries_it() {
        let bundle_slot0 = UmpEventBundle::new(Ump::new([0; 4]), Pose5D::ORIGIN, [0; 4], [0; 2], [ENGINE_EVENT_HITBOX, 0]);
        let bundle_slot1 = UmpEventBundle::new(Ump::new([0; 4]), Pose5D::ORIGIN, [0; 4], [0; 2], [0, ENGINE_EVENT_HITBOX]);
        let bundle_neither = UmpEventBundle::new(Ump::new([0; 4]), Pose5D::ORIGIN, [0; 4], [0; 2], [ENGINE_EVENT_VFX_SPAWN, ENGINE_EVENT_SOUNDFONT_LAYER]);
        assert!(bundle_slot0.has_hitbox());
        assert!(bundle_slot1.has_hitbox());
        assert!(!bundle_neither.has_hitbox(), "vfx_spawn/soundfont_layer flags must not read as a hitbox");
    }

    #[test]
    fn engine_event_flags_are_independent_bits_not_a_shared_enum() {
        let both = ENGINE_EVENT_VFX_SPAWN | ENGINE_EVENT_HITBOX;
        assert_ne!(both & ENGINE_EVENT_VFX_SPAWN, 0, "vfx_spawn bit must survive co-occurring with hitbox");
        assert_ne!(both & ENGINE_EVENT_HITBOX, 0, "hitbox bit must survive co-occurring with vfx_spawn");
        assert_eq!(both & ENGINE_EVENT_SOUNDFONT_LAYER, 0, "unset bits must read as unset");
    }

    // ── L18-style sabotage: prove has_hitbox is not vacuously true ────────────
    #[test]
    fn sabotaged_has_hitbox_would_be_caught() {
        // Sabotage: check the wrong flag entirely.
        let bundle = UmpEventBundle::new(Ump::new([0; 4]), Pose5D::ORIGIN, [0; 4], [0; 2], [ENGINE_EVENT_VFX_SPAWN, 0]);
        let sabotaged_result = (bundle.engine_events[0] & ENGINE_EVENT_VFX_SPAWN != 0) || (bundle.engine_events[1] & ENGINE_EVENT_VFX_SPAWN != 0);
        // The sabotaged check (looking for VFX_SPAWN under the name "hitbox") would wrongly
        // report true here since this bundle DOES carry vfx_spawn — proving the test fixture
        // can distinguish flags, i.e. a mixed-up implementation would be caught by the real
        // has_hitbox() disagreeing with this deliberately-wrong check.
        assert!(sabotaged_result, "fixture sanity: vfx_spawn bit is actually set");
        assert!(!bundle.has_hitbox(), "real has_hitbox() must NOT be fooled by a set vfx_spawn bit");
    }
}
