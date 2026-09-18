//! Sealed sprite-animation binary blob — the SoA/AoS boundary.
//!
//! **Drain provenance:** Ported from `F:\NewRepo\crates\forge-core\src\pixel\sprite_blob.rs`.
//! Customs strip: Removed `bytemuck::Pod` and `bytemuck::Zeroable` derives (v3 is zero-dependency).
//! Binary format, struct layouts, and seal/load remain byte-identical.
//!
//! **P0 of the ANIM-domain fold** (`PLAN-sprite-primitives-vixi-fold` §0/§3). This
//! is the BOUNDARY the whole ANIM group lives across, made concrete:
//!
//! - **CPU = SoA (hot loop):** parallel integer arrays — `frame_index[]` (PRIM-001
//!   cel-link → unique-rect index), `frame_ticks[]` (PRIM-004 per-frame ticks),
//!   `tile_id[]` (PRIM-008 tilemap), `tags[]` (PRIM-002 frame-tag FSM).
//! - **GPU = AoS (structured):** [`SpriteInstance`](crate::sprite_blob::SpriteInstance)
//!   per sprite (atlas UV, palette/
//!   faction bank) — `#[repr(C)]` for direct instanced-VB upload (P3).
//! - **The seal:** a single sovereign binary blob, written/read in **little-nistam**
//!   byte order (nistam = Cree "first": least-significant byte FIRST; NEVER the
//!   host's native byte order) so the round-trip is byte-identical across hosts —
//!   the replay contract. Mirrors the `.forge_reg` discipline: magic + version +
//!   counts + checksum + integer payload.
//!
//! **The two clocks (don't conflate):**
//! [`SpritePlayhead::advance`](crate::sprite_blob::SpritePlayhead::advance) rides the
//! DETERMINISTIC 120 Hz metronome (`ticks`, `u32` whole engine ticks) — same tick
//! stream → same frame, forever. The forge-ump `tick_us` (i64 µs) input stamp is a
//! SEPARATE rail and drives NOTHING here (PRIM-010); conflating them breaks replay.
//!
//! Integer-only. No float anywhere — atlas UV is u16 texel coords (normalized on
//! the GPU at upload, P3); `interp` is reject-linear (Nearest only, PRIM-011).

/// Blob magic — "SPrite-ANim". Identifies a sealed sprite blob.
pub const SPRITE_BLOB_MAGIC: [u8; 4] = *b"SPAN";
/// On-disk format version. Bump on any layout change (pairs with a loader arm).
/// Bumped 1 -> 2 (2026-08-14, F07 revascularize-check): the checksum algorithm
/// swapped from a local wrapping byte-sum (order-blind — `[1,2]` and `[2,1]`
/// checksummed identically; inherited verbatim from the v2 donor) to this
/// crate's own already-landed `checksum::hash_bytes_fnv1a` (FNV-1a, real,
/// tested, already consumed by `diff_pool.rs` in this same crate). No struct
/// layout changed; the header stays 32 bytes, the checksum field stays a
/// `u32` (FNV-1a's low 32 bits) — only what the field CONTAINS is stronger.
/// Bumped anyway per this const's own doctrine: an old blob's stored checksum
/// would otherwise silently fail against the new algorithm as `BadChecksum`
/// (misleading — the blob isn't corrupt, the algorithm changed), not
/// `BadVersion` (honest). No real sealed blob exists on disk yet (P0 of the
/// ANIM-domain fold, pre-consumer) — this is a zero-cost correction now.
pub const SPRITE_BLOB_VERSION: u32 = 2;
/// The faction-palette LUT is exactly 64 entries (the 6-bit R8Uint index space).
pub const MAX_PALETTE: usize = 64;
/// Canonical sprite strip width in texels (PRIM-005).
pub const STRIP_W: u16 = 128;
/// Canonical sprite strip height in texels (PRIM-005).
pub const STRIP_H: u16 = 256;

/// The 13 ANIM-domain `PrimitiveId`s this blob is the runtime substrate for
/// (canon: `crate::primitives` GROUP_ANIM 0x03). Returned so the blob is
/// SIGNAL-CAPABLE — it KNOWS which manifest rows it backs (the P4 `sprite_check`
/// door tool reads this).
pub const fn anim_primitive_ids() -> [u32; 13] {
    [
        0x03_00, 0x03_01, 0x03_02, 0x03_03,
        0x03_04, 0x03_05, 0x03_06, 0x03_07,
        0x03_08, 0x03_09, 0x03_0A, 0x03_0B,
        0x03_0C,
    ]
}

// ── LoopMode (PRIM-014) ───────────────────────────────────────────────────────

/// How a frame-tag's `[start..=end]` range advances at its boundary.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoopMode {
    /// Play once, then hold on the last frame (no wrap).
    Once = 0,
    /// Wrap `end → start` forever.
    Loop = 1,
    /// Bounce `start → end → start …` (reflect at each boundary).
    PingPong = 2,
    /// Never advance — hold on `start` (a single-frame pose).
    Hold = 3,
}

impl LoopMode {
    /// Decode from the wire byte (unknown → `Once`, the safe default).
    pub const fn from_u8(b: u8) -> Self {
        match b {
            1 => LoopMode::Loop,
            2 => LoopMode::PingPong,
            3 => LoopMode::Hold,
            _ => LoopMode::Once,
        }
    }
    /// Encode to the wire byte.
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}

// ── FrameTag (PRIM-002 — the frame-tag FSM) ───────────────────────────────────

/// A named animation state → an inclusive frame range + loop mode. The FSM in
/// [`SpritePlayhead`] walks `[start..=end]`. `#[repr(C)]`, 16 bytes, no padding.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FrameTag {
    /// Hash of the state name (idle/walk/attack…) — integer identity, no string.
    pub name_hash: u64,
    /// First frame index of the range (inclusive).
    pub start: u16,
    /// Last frame index of the range (inclusive).
    pub end: u16,
    /// [`LoopMode`] as a byte.
    pub loop_mode: u8,
    /// Explicit padding to 16 bytes.
    pub _pad: [u8; 3],
}

impl FrameTag {
    /// Construct a tag (pad zeroed).
    pub const fn new(name_hash: u64, start: u16, end: u16, loop_mode: LoopMode) -> Self {
        Self { name_hash, start, end, loop_mode: loop_mode.to_u8(), _pad: [0; 3] }
    }
    /// The loop mode as an enum.
    pub const fn mode(self) -> LoopMode {
        LoopMode::from_u8(self.loop_mode)
    }
}

const _: () = assert!(core::mem::size_of::<FrameTag>() == 16);

// ── SpriteInstance (PRIM-005/006/009 — the GPU AoS row) ───────────────────────

/// One per-sprite GPU instance — atlas rect (integer texels) + palette/faction
/// bank. `#[repr(C)]`, 12 bytes, no padding: fit for direct upload into an
/// instanced vertex buffer at P3. UV is INTEGER texel coords; the shader divides
/// by the atlas dims (no float stored, PRIM-011 keeps the sampler Nearest).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpriteInstance {
    /// Atlas rect origin x (texels).
    pub atlas_u: u16,
    /// Atlas rect origin y (texels).
    pub atlas_v: u16,
    /// Atlas rect width (texels).
    pub atlas_w: u16,
    /// Atlas rect height (texels).
    pub atlas_h: u16,
    /// Which 64-colour palette bank (PRIM-009).
    pub palette_id: u8,
    /// Faction recolour layer (PRIM-006) — a UBO rebind, zero texture edits.
    pub faction_id: u8,
    /// Explicit padding to 12 bytes.
    pub _pad: [u8; 2],
}

impl SpriteInstance {
    /// Construct an instance (pad zeroed).
    pub const fn new(
        atlas_u: u16,
        atlas_v: u16,
        atlas_w: u16,
        atlas_h: u16,
        palette_id: u8,
        faction_id: u8,
    ) -> Self {
        Self { atlas_u, atlas_v, atlas_w, atlas_h, palette_id, faction_id, _pad: [0; 2] }
    }
}

const _: () = assert!(core::mem::size_of::<SpriteInstance>() == 12);

// ── SealError ─────────────────────────────────────────────────────────────────

/// Why a [`SpriteBlob`] failed to seal or load. NEVER silent — every fault is a
/// loud, typed value (root §0: signal-capable, never dies quietly).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SealError {
    /// First 4 bytes were not [`SPRITE_BLOB_MAGIC`].
    BadMagic,
    /// Version field did not match [`SPRITE_BLOB_VERSION`].
    BadVersion(u32),
    /// Byte stream ended before the declared payload.
    Truncated,
    /// Recomputed payload checksum disagreed with the header.
    BadChecksum {
        /// Checksum read from the blob header.
        stored: u32,
        /// Checksum recomputed from the payload.
        computed: u32,
    },
    /// `palette.len()` exceeded [`MAX_PALETTE`] (the 6-bit index space).
    PaletteOverflow(usize),
    /// `frame_index` and `frame_ticks` had different lengths (SoA must be parallel).
    FrameArrayMismatch {
        /// Length of `frame_index` array.
        frame_index: usize,
        /// Length of `frame_ticks` array.
        frame_ticks: usize,
    },
}

impl core::fmt::Display for SealError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a sprite blob: magic is not {:?}", SPRITE_BLOB_MAGIC),
            Self::BadVersion(v) => {
                write!(f, "sprite blob version {v}, this build seals {SPRITE_BLOB_VERSION}")
            }
            Self::Truncated => write!(f, "sprite blob ended before its declared payload"),
            Self::BadChecksum { stored, computed } => {
                write!(f, "sprite blob checksum {stored:#010x} != recomputed {computed:#010x}")
            }
            Self::PaletteOverflow(n) => {
                write!(f, "palette has {n} entries, the 6-bit index space holds {MAX_PALETTE}")
            }
            Self::FrameArrayMismatch { frame_index, frame_ticks } => write!(
                f,
                "SoA arrays are not parallel: frame_index {frame_index}, frame_ticks {frame_ticks}"
            ),
        }
    }
}

impl std::error::Error for SealError {}

// ── SpriteBlob (the CPU SoA + palette) ────────────────────────────────────────

/// The sealed sprite-animation payload — parallel integer SoA arrays (CPU hot
/// loop) plus the GPU AoS instance list and the 64-entry palette LUT.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SpriteBlob {
    /// PRIM-001 — per frame, the unique-rect (cel-link) index.
    pub frame_index: Vec<u16>,
    /// PRIM-004 — per frame, its length in whole 120 Hz metronome ticks (≥1).
    pub frame_ticks: Vec<u16>,
    /// PRIM-008 — flat tilemap, one `u16` tile id per cell (GPU storage buffer).
    pub tile_id: Vec<u16>,
    /// PRIM-002 — the frame-tag FSM states.
    pub tags: Vec<FrameTag>,
    /// PRIM-005/006/009 — the GPU AoS instances.
    pub instances: Vec<SpriteInstance>,
    /// PRIM-009 — the palette LUT (index 0..63 → packed colour). `len() ≤ 64`.
    pub palette: Vec<u32>,
}

impl SpriteBlob {
    /// Validate the SoA invariants WITHOUT sealing (the RED-guard the bake step
    /// and the P4 `sprite_check` door tool both call). `Ok(())` ⟺ sealable.
    pub fn validate(&self) -> Result<(), SealError> {
        if self.palette.len() > MAX_PALETTE {
            return Err(SealError::PaletteOverflow(self.palette.len()));
        }
        if self.frame_index.len() != self.frame_ticks.len() {
            return Err(SealError::FrameArrayMismatch {
                frame_index: self.frame_index.len(),
                frame_ticks: self.frame_ticks.len(),
            });
        }
        Ok(())
    }

    /// Seal to the sovereign binary blob (little-nistam byte order). Validates the
    /// SoA invariants first (`palette ≤ 64`, parallel frame arrays).
    pub fn seal(&self) -> Result<Vec<u8>, SealError> {
        self.validate()?;

        // Payload first, so the header can carry its checksum. Every multi-byte
        // integer is written in little-nistam order (see the nistam helpers below).
        let mut payload = Vec::new();
        for v in &self.frame_index {
            payload.extend_from_slice(&u16_to_nistam(*v));
        }
        for v in &self.frame_ticks {
            payload.extend_from_slice(&u16_to_nistam(*v));
        }
        for v in &self.tile_id {
            payload.extend_from_slice(&u16_to_nistam(*v));
        }
        for t in &self.tags {
            payload.extend_from_slice(&u64_to_nistam(t.name_hash));
            payload.extend_from_slice(&u16_to_nistam(t.start));
            payload.extend_from_slice(&u16_to_nistam(t.end));
            payload.push(t.loop_mode);
            payload.extend_from_slice(&t._pad);
        }
        for s in &self.instances {
            payload.extend_from_slice(&u16_to_nistam(s.atlas_u));
            payload.extend_from_slice(&u16_to_nistam(s.atlas_v));
            payload.extend_from_slice(&u16_to_nistam(s.atlas_w));
            payload.extend_from_slice(&u16_to_nistam(s.atlas_h));
            payload.push(s.palette_id);
            payload.push(s.faction_id);
            payload.extend_from_slice(&s._pad);
        }
        for c in &self.palette {
            payload.extend_from_slice(&u32_to_nistam(*c));
        }

        let checksum = crate::hash_bytes_fnv1a(&payload) as u32;

        let mut out = Vec::with_capacity(32 + payload.len());
        out.extend_from_slice(&SPRITE_BLOB_MAGIC);
        out.extend_from_slice(&u32_to_nistam(SPRITE_BLOB_VERSION));
        out.extend_from_slice(&u32_to_nistam(self.frame_index.len() as u32));
        out.extend_from_slice(&u32_to_nistam(self.tile_id.len() as u32));
        out.extend_from_slice(&u32_to_nistam(self.tags.len() as u32));
        out.extend_from_slice(&u32_to_nistam(self.instances.len() as u32));
        out.extend_from_slice(&u32_to_nistam(self.palette.len() as u32));
        out.extend_from_slice(&u32_to_nistam(checksum));
        out.extend_from_slice(&payload);
        Ok(out)
    }

    /// Load a sealed blob (verifies magic, version, length, and checksum). The
    /// inverse of [`seal`](Self::seal): `load(seal(b)) == b`, byte-for-byte.
    pub fn load(bytes: &[u8]) -> Result<Self, SealError> {
        if bytes.len() < 32 {
            return Err(SealError::Truncated);
        }
        if bytes[0..4] != SPRITE_BLOB_MAGIC {
            return Err(SealError::BadMagic);
        }
        let version = u32_from_nistam(bytes, 4);
        if version != SPRITE_BLOB_VERSION {
            return Err(SealError::BadVersion(version));
        }
        let frame_count = u32_from_nistam(bytes, 8) as usize;
        let tile_count = u32_from_nistam(bytes, 12) as usize;
        let tag_count = u32_from_nistam(bytes, 16) as usize;
        let inst_count = u32_from_nistam(bytes, 20) as usize;
        let pal_count = u32_from_nistam(bytes, 24) as usize;
        let stored_checksum = u32_from_nistam(bytes, 28);

        // Byte budget per section (little-nistam strides).
        let need = frame_count * 2  // frame_index
            + frame_count * 2       // frame_ticks
            + tile_count * 2        // tile_id
            + tag_count * 16        // tags
            + inst_count * 12       // instances
            + pal_count * 4; // palette
        let payload = &bytes[32..];
        if payload.len() < need {
            return Err(SealError::Truncated);
        }
        let payload = &payload[..need];

        let computed = crate::hash_bytes_fnv1a(payload) as u32;
        if computed != stored_checksum {
            return Err(SealError::BadChecksum { stored: stored_checksum, computed });
        }

        let mut off = 0usize;
        let mut frame_index = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            frame_index.push(u16_from_nistam(payload, off));
            off += 2;
        }
        let mut frame_ticks = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            frame_ticks.push(u16_from_nistam(payload, off));
            off += 2;
        }
        let mut tile_id = Vec::with_capacity(tile_count);
        for _ in 0..tile_count {
            tile_id.push(u16_from_nistam(payload, off));
            off += 2;
        }
        let mut tags = Vec::with_capacity(tag_count);
        for _ in 0..tag_count {
            let name_hash = u64_from_nistam(payload, off);
            let start = u16_from_nistam(payload, off + 8);
            let end = u16_from_nistam(payload, off + 10);
            let loop_mode = payload[off + 12];
            let _pad = [payload[off + 13], payload[off + 14], payload[off + 15]];
            tags.push(FrameTag { name_hash, start, end, loop_mode, _pad });
            off += 16;
        }
        let mut instances = Vec::with_capacity(inst_count);
        for _ in 0..inst_count {
            let atlas_u = u16_from_nistam(payload, off);
            let atlas_v = u16_from_nistam(payload, off + 2);
            let atlas_w = u16_from_nistam(payload, off + 4);
            let atlas_h = u16_from_nistam(payload, off + 6);
            let palette_id = payload[off + 8];
            let faction_id = payload[off + 9];
            let _pad = [payload[off + 10], payload[off + 11]];
            instances.push(SpriteInstance {
                atlas_u, atlas_v, atlas_w, atlas_h, palette_id, faction_id, _pad,
            });
            off += 12;
        }
        let mut palette = Vec::with_capacity(pal_count);
        for _ in 0..pal_count {
            palette.push(u32_from_nistam(payload, off));
            off += 4;
        }

        Ok(Self { frame_index, frame_ticks, tile_id, tags, instances, palette })
    }

    /// A loud, structured description of the SoA/AoS layout — the seed of the P4
    /// `sprite_check` "stink" door tool. Never silent: reports counts, strides,
    /// and any invariant violation so the hybrid is smellable from the daemon.
    pub fn report(&self) -> SpriteBlobReport {
        SpriteBlobReport {
            frame_count: self.frame_index.len(),
            tag_count: self.tags.len(),
            tile_count: self.tile_id.len(),
            instance_count: self.instances.len(),
            palette_len: self.palette.len(),
            instance_stride: core::mem::size_of::<SpriteInstance>(),
            tag_stride: core::mem::size_of::<FrameTag>(),
            sealed_len: self.seal().map(|b| b.len()).unwrap_or(0),
            valid: self.validate().is_ok(),
        }
    }
}

/// The layout snapshot [`SpriteBlob::report`] emits (the P4 door payload).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpriteBlobReport {
    /// Total number of frames in the animation.
    pub frame_count: usize,
    /// Total number of animation tags (states).
    pub tag_count: usize,
    /// Total number of tiles in the tilemap.
    pub tile_count: usize,
    /// Total number of GPU instances.
    pub instance_count: usize,
    /// Number of palette entries (≤ 64).
    pub palette_len: usize,
    /// Byte stride of a `SpriteInstance` (fixed at 12).
    pub instance_stride: usize,
    /// Byte stride of a `FrameTag` (fixed at 16).
    pub tag_stride: usize,
    /// Byte length of the sealed blob.
    pub sealed_len: usize,
    /// Whether the blob passes validation.
    pub valid: bool,
}

// ── SpritePlayhead (PRIM-002/004/014 — the deterministic FSM) ─────────────────

/// A frame-tag playhead. [`advance`](Self::advance) walks the active tag's frame
/// range on the DETERMINISTIC metronome (whole engine ticks), so the same tick
/// stream always lands on the same frame — the replay floor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpritePlayhead {
    /// Active tag index (into `blob.tags`).
    pub tag: u16,
    /// Current absolute frame index (within the tag's `[start..=end]`).
    pub frame: u16,
    /// Metronome ticks accumulated inside the current frame.
    pub accum_ticks: u32,
    /// PingPong direction (+1 forward, -1 reflecting); irrelevant for other modes.
    dir: i8,
    /// Set once a `Once` tag has reached its end (held; no further advance).
    done: bool,
}

impl SpritePlayhead {
    /// Start a playhead on `tag`, parked on that tag's first frame.
    pub fn new(blob: &SpriteBlob, tag: u16) -> Self {
        let start = blob.tags.get(tag as usize).map(|t| t.start).unwrap_or(0);
        Self { tag, frame: start, accum_ticks: 0, dir: 1, done: false }
    }

    /// Advance by `ticks` whole 120 Hz metronome ticks. Returns `true` if the
    /// frame changed. Deterministic + drift-free: delivery shape is irrelevant
    /// (`advance(8)` ≡ 8× `advance(1)`); carries surplus across boundaries.
    pub fn advance(&mut self, blob: &SpriteBlob, ticks: u32) -> bool {
        let Some(tag) = blob.tags.get(self.tag as usize) else {
            return false;
        };
        if blob.frame_index.is_empty() {
            return false;
        }
        let start = tag.start;
        let end = tag.end.max(start);
        let mode = tag.mode();

        // Hold never advances — the playhead is a static pose.
        if mode == LoopMode::Hold {
            self.frame = start;
            return false;
        }
        if self.done {
            return false;
        }

        self.accum_ticks += ticks;
        let mut changed = false;
        loop {
            let fi = self.frame as usize;
            let needed = blob.frame_ticks.get(fi).copied().unwrap_or(1).max(1) as u32;
            if self.accum_ticks < needed {
                break;
            }
            self.accum_ticks -= needed;
            match step_frame(start, end, self.frame, self.dir, mode) {
                Step::To(next, dir) => {
                    self.frame = next;
                    self.dir = dir;
                    changed = true;
                }
                Step::HoldEnd => {
                    self.frame = end;
                    self.done = true;
                    changed = true;
                    break;
                }
            }
        }
        changed
    }
}

/// Result of one frame step.
enum Step {
    /// Move to `frame` with the (possibly reflected) ping-pong direction.
    To(u16, i8),
    /// `Once` reached the end — hold there (no wrap).
    HoldEnd,
}

/// Pure frame-advance for one boundary crossing (single-frame ranges stay put).
fn step_frame(start: u16, end: u16, frame: u16, dir: i8, mode: LoopMode) -> Step {
    if end == start {
        // A one-frame range: Loop/Once/PingPong all stay on the single frame.
        return match mode {
            LoopMode::Once => Step::HoldEnd,
            _ => Step::To(start, dir),
        };
    }
    match mode {
        LoopMode::Loop => {
            if frame >= end {
                Step::To(start, 1)
            } else {
                Step::To(frame + 1, 1)
            }
        }
        LoopMode::Once => {
            if frame >= end {
                Step::HoldEnd
            } else {
                Step::To(frame + 1, 1)
            }
        }
        LoopMode::PingPong => {
            // Reflect at the boundaries; dir flips when we would step past.
            if dir >= 0 {
                if frame >= end {
                    Step::To(frame.saturating_sub(1), -1)
                } else {
                    Step::To(frame + 1, 1)
                }
            } else if frame <= start {
                Step::To(frame + 1, 1)
            } else {
                Step::To(frame - 1, -1)
            }
        }
        LoopMode::Hold => Step::To(start, dir),
    }
}

// ── little-nistam byte order (nistam = Cree "first": least-significant byte FIRST) ──
// Cree framing by Sean's law — these helpers pack/unpack by hand (shift + mask),
// so no foreign byte-order term lives in the seal path. The
// blob uses this one fixed order end-to-end → the round-trip is byte-identical
// across hosts (the replay floor). Alignment-safe: reads index single bytes.

/// Pack a `u16` little-nistam (least-significant byte FIRST). Pub since W15:
/// every v3 wire format speaks this ONE order through these helpers — a
/// second byte-order site is the L05 defect.
#[inline]
pub fn u16_to_nistam(v: u16) -> [u8; 2] {
    [v as u8, (v >> 8) as u8]
}
/// Pack a `u32` little-nistam. See [`u16_to_nistam`].
#[inline]
pub fn u32_to_nistam(v: u32) -> [u8; 4] {
    [v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8]
}
/// Pack a `u64` little-nistam. See [`u16_to_nistam`].
#[inline]
pub fn u64_to_nistam(v: u64) -> [u8; 8] {
    [
        v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8,
        (v >> 32) as u8, (v >> 40) as u8, (v >> 48) as u8, (v >> 56) as u8,
    ]
}

/// Read a `u16` written little-nistam at `off`. See [`u16_to_nistam`].
#[inline]
pub fn u16_from_nistam(b: &[u8], off: usize) -> u16 {
    (b[off] as u16) | ((b[off + 1] as u16) << 8)
}
/// Read a `u32` written little-nistam at `off`. See [`u16_to_nistam`].
#[inline]
pub fn u32_from_nistam(b: &[u8], off: usize) -> u32 {
    (b[off] as u32)
        | ((b[off + 1] as u32) << 8)
        | ((b[off + 2] as u32) << 16)
        | ((b[off + 3] as u32) << 24)
}
/// Read a `u64` written little-nistam at `off`. See [`u16_to_nistam`].
#[inline]
pub fn u64_from_nistam(b: &[u8], off: usize) -> u64 {
    (b[off] as u64)
        | ((b[off + 1] as u64) << 8)
        | ((b[off + 2] as u64) << 16)
        | ((b[off + 3] as u64) << 24)
        | ((b[off + 4] as u64) << 32)
        | ((b[off + 5] as u64) << 40)
        | ((b[off + 6] as u64) << 48)
        | ((b[off + 7] as u64) << 56)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small but non-trivial blob exercising every section.
    fn sample() -> SpriteBlob {
        SpriteBlob {
            frame_index: vec![0, 1, 2, 1],
            frame_ticks: vec![8, 8, 4, 8],
            tile_id: vec![10, 11, 12, 13, 14],
            tags: vec![
                FrameTag::new(0xDEAD_BEEF, 0, 3, LoopMode::Loop),
                FrameTag::new(0x1234, 1, 2, LoopMode::PingPong),
            ],
            instances: vec![
                SpriteInstance::new(0, 0, 32, 32, 0, 0),
                SpriteInstance::new(32, 0, 32, 32, 1, 2),
            ],
            palette: vec![0xFF00_00FF, 0x00FF_00FF, 0x0000_FFFF],
        }
    }

    /// Round-trip seal and load; verify byte-identical re-seal.
    #[test]
    fn round_trip_is_byte_identical() {
        let blob = sample();
        let bytes = blob.seal().expect("seal");
        let back = SpriteBlob::load(&bytes).expect("load");
        assert_eq!(blob, back, "struct round-trips exactly");
        // And re-sealing the loaded blob reproduces the same bytes — the replay floor.
        assert_eq!(bytes, back.seal().expect("reseal"), "byte-identical re-seal");
    }

    /// Empty blob must seal and load correctly.
    #[test]
    fn empty_blob_round_trips() {
        let blob = SpriteBlob::default();
        let bytes = blob.seal().expect("seal");
        assert_eq!(SpriteBlob::load(&bytes).expect("load"), blob);
    }

    /// Load rejects blob with wrong magic bytes.
    #[test]
    fn load_rejects_bad_magic() {
        let mut bytes = sample().seal().unwrap();
        bytes[0] = b'X';
        assert_eq!(SpriteBlob::load(&bytes), Err(SealError::BadMagic));
    }

    /// Load rejects blob with unsupported version.
    #[test]
    fn load_rejects_bad_version() {
        let mut bytes = sample().seal().unwrap();
        bytes[4] = 0xFF; // corrupt the version's first nistam byte
        assert!(matches!(SpriteBlob::load(&bytes), Err(SealError::BadVersion(_))));
    }

    /// Load rejects blob shorter than header.
    #[test]
    fn load_rejects_truncation() {
        let bytes = sample().seal().unwrap();
        assert_eq!(SpriteBlob::load(&bytes[..bytes.len() - 3]), Err(SealError::Truncated));
        assert_eq!(SpriteBlob::load(&bytes[..10]), Err(SealError::Truncated));
    }

    /// Load rejects blob with corrupted payload (checksum mismatch).
    #[test]
    fn load_rejects_checksum_corruption() {
        let mut bytes = sample().seal().unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF; // flip a payload byte; header checksum no longer matches
        assert!(matches!(SpriteBlob::load(&bytes), Err(SealError::BadChecksum { .. })));
    }

    /// Seal rejects palette larger than 64 entries.
    #[test]
    fn seal_rejects_palette_overflow() {
        let mut blob = sample();
        blob.palette = vec![0; MAX_PALETTE + 1];
        assert_eq!(blob.seal(), Err(SealError::PaletteOverflow(MAX_PALETTE + 1)));
    }

    /// Seal rejects frame arrays of mismatched length.
    #[test]
    fn seal_rejects_frame_array_mismatch() {
        let mut blob = sample();
        blob.frame_ticks.pop(); // break the parallel-SoA invariant
        assert!(matches!(blob.seal(), Err(SealError::FrameArrayMismatch { .. })));
    }

    /// GPU AoS strides are locked to 12 and 16 bytes respectively.
    #[test]
    fn gpu_aos_strides_are_locked() {
        // The GPU upload (P3) depends on these exact strides — lock them.
        assert_eq!(core::mem::size_of::<SpriteInstance>(), 12);
        assert_eq!(core::mem::size_of::<FrameTag>(), 16);
    }

    /// Playhead advance is deterministic and wraps correctly.
    #[test]
    fn loop_mode_advance_is_deterministic_and_wraps() {
        let blob = sample();
        // Tag 0 = Loop over frames 0..=3, each 8/8/4/8 ticks.
        let mut a = SpritePlayhead::new(&blob, 0);
        let mut b = SpritePlayhead::new(&blob, 0);
        // Same total ticks, different delivery shape → identical frame (drift-free).
        for _ in 0..20 {
            a.advance(&blob, 1);
        }
        b.advance(&blob, 20);
        assert_eq!(a.frame, b.frame, "delivery shape is irrelevant");
        assert_eq!(a.accum_ticks, b.accum_ticks);
    }

    /// Loop mode wraps end frame back to start frame.
    #[test]
    fn loop_wraps_end_to_start() {
        let blob = sample();
        let mut p = SpritePlayhead::new(&blob, 0); // Loop 0..=3
        assert_eq!(p.frame, 0);
        // 8 + 8 + 4 + 8 = 28 ticks completes the loop back to frame 0.
        p.advance(&blob, 28);
        assert_eq!(p.frame, 0, "Loop wrapped end → start");
    }

    /// Once mode holds at the end frame forever.
    #[test]
    fn once_holds_at_end() {
        let mut blob = sample();
        blob.tags[0] = FrameTag::new(0xA, 0, 3, LoopMode::Once);
        let mut p = SpritePlayhead::new(&blob, 0);
        p.advance(&blob, 1000); // far past the end
        assert_eq!(p.frame, 3, "Once held on the last frame");
        assert!(!p.advance(&blob, 1000), "no further change after hold");
    }

    /// PingPong mode bounces between start and end.
    #[test]
    fn pingpong_reflects_at_boundaries() {
        let mut blob = sample();
        // Tag over frames 0..=2 (ticks 8,8,4), ping-pong.
        blob.tags[0] = FrameTag::new(0xB, 0, 2, LoopMode::PingPong);
        let mut p = SpritePlayhead::new(&blob, 0);
        // 0 →(8)→ 1 →(8)→ 2 →(4 at f2)→ reflect to 1 →(8)→ 0
        p.advance(&blob, 8);
        assert_eq!(p.frame, 1);
        p.advance(&blob, 8);
        assert_eq!(p.frame, 2);
        p.advance(&blob, 4);
        assert_eq!(p.frame, 1, "reflected off the end");
        p.advance(&blob, 8);
        assert_eq!(p.frame, 0, "reflected back to the start");
    }

    /// Hold mode never advances the playhead.
    #[test]
    fn hold_mode_never_advances() {
        let mut blob = sample();
        blob.tags[0] = FrameTag::new(0xC, 1, 3, LoopMode::Hold);
        let mut p = SpritePlayhead::new(&blob, 0);
        assert!(!p.advance(&blob, 10_000));
        assert_eq!(p.frame, 1, "Hold parks on start");
    }

    /// Report describes the blob layout accurately.
    #[test]
    fn report_smells_the_layout() {
        let blob = sample();
        let r = blob.report();
        assert_eq!(r.frame_count, 4);
        assert_eq!(r.tag_count, 2);
        assert_eq!(r.instance_count, 2);
        assert_eq!(r.palette_len, 3);
        assert_eq!(r.instance_stride, 12);
        assert_eq!(r.tag_stride, 16);
        assert!(r.valid);
        assert!(r.sealed_len > 32);
    }

    /// Blob exposes all 13 canonical ANIM-domain primitive IDs.
    #[test]
    fn blob_knows_its_canon_anim_rows() {
        // The runtime is bound to the manifest: all 13 ids are group 0x03.
        let ids = anim_primitive_ids();
        assert_eq!(ids.len(), 13);
    }
}
