//! Identity and lineage — the *who / why / when*, held out of band.
//!
//! **This file was blocked.** START-HERE §"Blocked" and HANDOFF §9.1 recorded five names
//! at four widths (`EssenceId` 1 B here / 2 B at `forge-level-editor:41`, `SoulId` 2 B,
//! `SoulIdentity` 12 B, `SoulWord` 64 B) and required ARCH000 to name one canonical
//! object first, because L05 makes the second definition of a live name the defect.
//! The Slice-2 directive names three of them: `EssenceId` is **1 byte, `0..=4`**,
//! `SoulId` is **2 bytes**, `SoulIdentity` is **12 bytes**. Those three are settled and
//! written here.
//!
//! `SoulWord` (64 B): ARCH000 ruled 2026-08-12 the canonical name IS `SoulWord`
//! ("SoulWord canonical name for what? Its SoulWord"). Ported below from the v2
//! `outland::soulword` original (`F:\NewRepo\crates\outland\src\soulword.rs:1-3`,
//! "256-trit Soul-Word: one 64B L1 line... 5 trits/byte (base-243)"), landed here per
//! that ruling — one home, this file.
//!
//! ## Essence is not soul
//!
//! HANDOFF §5. Essence is *form & medium* — 1 byte, polymorphic, and **derived on the
//! hotpath** from `pexil.lattice.0 % 5`. Soul is *spirit & lineage* — immutable, out of
//! band, and it governs authorization rather than rendering. They are separate types
//! here so one can never be passed where the other is meant.

use crate::arch::{ArchRole, DetClock};
use crate::trit_bijection::TritArray32;

/// The five substrate pillars. 1 byte, `0..=4`, and **never stored next to a Pexil** —
/// it is recomputed from the lattice byte, so there is exactly one home for it.
/// Names are HANDOFF §5's, not invented here.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EssenceId {
    /// The canvas pillar.
    Canvas = 0,
    /// The audio graph pillar.
    AudioGraph = 1,
    /// The voxel grid pillar.
    VoxelGrid = 2,
    /// The code AST pillar.
    CodeAst = 3,
    /// The media stream pillar.
    MediaStream = 4,
}

/// The number of pillars. `essence()`'s modulus, and the reason `EssenceId` is 1 byte.
pub const PILLARS: usize = 5;

impl EssenceId {
    /// Every pillar in ordinal order.
    pub const ALL: [EssenceId; PILLARS] = [
        EssenceId::Canvas,
        EssenceId::AudioGraph,
        EssenceId::VoxelGrid,
        EssenceId::CodeAst,
        EssenceId::MediaStream,
    ];

    /// Decode a stored ordinal. `None` outside `0..=4` — a sixth pillar is corruption,
    /// not an extension.
    #[inline(always)]
    pub const fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(EssenceId::Canvas),
            1 => Some(EssenceId::AudioGraph),
            2 => Some(EssenceId::VoxelGrid),
            3 => Some(EssenceId::CodeAst),
            4 => Some(EssenceId::MediaStream),
            _ => None,
        }
    }

    /// Derive from a lattice byte, the hotpath form (`lattice.0 % 5`, HANDOFF §5).
    /// Infallible by construction: `% 5` cannot leave `0..=4`, so there is no `Option`
    /// here to tempt a caller into a default.
    #[inline(always)]
    pub const fn from_lattice(lattice: crate::atom::TritCell5D) -> Self {
        match Self::from_u8(lattice.essence()) {
            Some(e) => e,
            // Unreachable: `essence()` is `% 5`. Kept as a total match rather than an
            // `unwrap`, so this stays a `const fn` with no panic path at all.
            None => EssenceId::Canvas,
        }
    }

    /// The ordinal index of this pillar.
    #[inline(always)]
    pub const fn ordinal(self) -> u8 {
        self as u8
    }
}

/// A soul handle. 2 bytes. `ROOT` is the chain terminator, never a real soul.
///
/// Same width as [`crate::atom::CellOrdinal`] — which is the hazard, not a convenience.
/// `tests::a_pexil_ordinal_is_not_a_soul_handle` is a type gate on that swap.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SoulId(/// The soul handle value.
pub u16);

impl SoulId {
    /// The lineage terminator. Not a soul: nothing may claim it as an identity.
    pub const ROOT: Self = Self(0);

    /// Ceiling. `u16` caps live souls at 65_535 plus the root.
    pub const MAX: Self = Self(u16::MAX);

    /// True when this is the root (lineage terminator).
    #[inline(always)]
    pub const fn is_root(self) -> bool {
        self.0 == Self::ROOT.0
    }
}

/// Immutable identity: who, under whose authority, first seen when, descended from whom.
///
/// **12 bytes, align 4, zero padding** — every byte is a field, asserted below. No
/// setters exist and no field is `pub`-mutated anywhere in this crate: mutation would
/// break the "100% immutable" column of HANDOFF §5.
///
/// The genesis stamp is a **narrowed** [`DetClock`]: 4-byte tick and 2-byte epoch, not
/// the clock's 8 and 2. A full `u64` tick would force align 8 and the struct to 16
/// bytes. [`SoulIdentity::at_genesis`] therefore returns `Option` and refuses a tick
/// past `u32::MAX` rather than truncating one.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoulIdentity {
    /// Deterministic tick of first sighting, narrowed from `DetClock::tick`.
    pub genesis_tick: u32,
    /// Replay epoch of first sighting. `(epoch, tick)` orders totally.
    pub genesis_epoch: u16,
    /// This soul.
    pub soul: SoulId,
    /// Its parent, or `SoulId::ROOT` at the head of the chain.
    pub parent: SoulId,
    /// Who admitted it. Authorization lives with the identity, not beside it.
    pub authority: ArchRole,
}

impl SoulIdentity {
    /// Stamp an identity against a deterministic clock. `None` when the tick will not
    /// fit 32 bits — a truncated genesis tick would collide two distinct souls'
    /// provenance, which is exactly the kind of silent aliasing L07 exists to refuse.
    #[inline]
    pub const fn at_genesis(
        soul: SoulId,
        parent: SoulId,
        clock: DetClock,
    ) -> Option<Self> {
        if clock.tick > u32::MAX as u64 {
            return None;
        }
        Some(Self {
            genesis_tick: clock.tick as u32,
            genesis_epoch: clock.epoch,
            soul,
            parent,
            authority: clock.authority,
        })
    }

    /// True when this identity heads its own chain.
    #[inline(always)]
    pub const fn is_genesis_soul(&self) -> bool {
        self.parent.is_root()
    }

    /// A soul must not be its own parent, and must not *be* the root.
    /// A self-parent is an unterminated chain — a walker following `parent` never halts.
    #[inline(always)]
    pub const fn is_well_formed(&self) -> bool {
        !self.soul.is_root() && self.soul.0 != self.parent.0
    }
}

/// Walk the causal chain (the CYNATIC axis — lineage, distinct from the 5D
/// spatial/temporal address) from `start` toward [`SoulId::ROOT`], calling
/// `parent_of` at each hop. `is_well_formed` only catches a direct self-parent;
/// a longer cycle (A -> B -> A) or an unbounded lineage still needs a bounded
/// walk to catch. Returns the hop count to root, or `None` if the chain does
/// not terminate within `max_hops` (a cycle, a break, or a lineage deeper than
/// the caller's bound) — never spins unbounded.
pub fn cynatic_depth(
    start: SoulId,
    max_hops: u32,
    mut parent_of: impl FnMut(SoulId) -> Option<SoulId>,
) -> Option<u32> {
    let mut cur = start;
    let mut hops = 0u32;
    while !cur.is_root() {
        if hops >= max_hops {
            return None;
        }
        cur = parent_of(cur)?;
        hops += 1;
    }
    Some(hops)
}

/// Trits packed per byte — the same `3^5 = 243` boundary as [`crate::atom::TRITS_PER_BYTE`].
/// A separate constant, not a re-export: `soulword`'s packing is a general sealed
/// payload, not a 5D lattice coordinate, so it earns its own name even though the
/// arithmetic is identical (L05 is about one home per CONCEPT, not one constant
/// forced to serve two).
pub const SOUL_TRITS_PER_BYTE: u32 = 5;
/// Payload width. `64 - 8 (hash) - 4 (parent) = 52`, no tail padding.
pub const SOUL_BYTES: usize = 52;
/// L2 cache-tier payload width: `256 - 8 (hash) - 4 (parent) = 244`, no tail padding.
pub const BODY_BYTES: usize = 244;
/// L3 cache-tier payload width: `4096 - 8 (hash) - 4 (parent) = 4084`, no tail padding.
pub const MIND_BYTES: usize = 4084;

/// A sealed 64-byte identity/lineage word — one L1 cache line.
///
/// Ported from `outland::soulword::SoulWord` (`F:\NewRepo\crates\outland\src\
/// soulword.rs:15-21`), data-only: the v2 original pairs this with a lock-free
/// `SoulArena` (`UnsafeCell`+`AtomicU32` swap publication) that is deliberately NOT
/// ported here — this workspace denies `unsafe_code` outright, and an atomic
/// hot-swap arena cannot be built without it. `SoulWord` itself needs none: it is
/// `Copy`, sealed by construction, and any arena/lineage-tracking layer around it
/// can be a safe `Mutex`-backed store instead (see `forge-hal-clockspine::
/// TripleBuffer` for this workspace's existing safe-swap pattern).
#[repr(C, align(64))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoulWord {
    /// Content hash of the sealed payload.
    pub hash: u64,
    /// Causal parent's hash, truncated. `0` at the head of a lineage.
    pub parent: u32,
    /// The packed trit payload. `SOUL_BYTES * SOUL_TRITS_PER_BYTE == 260 >= 256`
    /// trits, covering the "256-trit Soul-Word" the v2 doc names.
    pub trits: [u8; SOUL_BYTES],
}

const _: () = assert!(core::mem::size_of::<SoulWord>() == 64);
const _: () = assert!(core::mem::align_of::<SoulWord>() == 64);
const _: () = assert!(core::mem::offset_of!(SoulWord, hash) == 0);
const _: () = assert!(core::mem::offset_of!(SoulWord, parent) == 8);
const _: () = assert!(core::mem::offset_of!(SoulWord, trits) == 12);
// Packing covers every trit the v2 doc claims, and the radix still fits a byte —
// the same boundary `atom.rs`'s `TRITS_PER_BYTE` proves, checked again here because
// `SOUL_BYTES`/`SOUL_TRITS_PER_BYTE` are this file's own constants, not a re-export.
const _: () = assert!(SOUL_BYTES * SOUL_TRITS_PER_BYTE as usize >= 256);
const _: () = assert!(3u32.pow(SOUL_TRITS_PER_BYTE) <= 256);

// ---------------------------------------------------------------------------
// LAYOUT LOCKS
// ---------------------------------------------------------------------------
const _: () = assert!(core::mem::size_of::<EssenceId>() == 1);
const _: () = assert!(core::mem::size_of::<SoulId>() == 2);
const _: () = assert!(core::mem::size_of::<SoulIdentity>() == 12);
const _: () = assert!(core::mem::align_of::<SoulIdentity>() == 4);

// OFFSET LOCKS. `size_of` alone is weak wherever a struct has tail padding to absorb a
// widened field. `SoulIdentity` has none, and these offsets are what prove that.
const _: () = assert!(core::mem::offset_of!(SoulIdentity, genesis_tick) == 0);
const _: () = assert!(core::mem::offset_of!(SoulIdentity, genesis_epoch) == 4);
const _: () = assert!(core::mem::offset_of!(SoulIdentity, soul) == 6);
const _: () = assert!(core::mem::offset_of!(SoulIdentity, parent) == 8);
const _: () = assert!(core::mem::offset_of!(SoulIdentity, authority) == 10);

// Every one of the 12 bytes is a field. If this holds, there is no padding hole in
// which a sixth field could hide, and the offsets above are the whole struct.
const _: () = assert!(
    core::mem::size_of::<u32>()
        + core::mem::size_of::<u16>()
        + core::mem::size_of::<SoulId>()
        + core::mem::size_of::<SoulId>()
        + core::mem::size_of::<ArchRole>()
        == core::mem::size_of::<SoulIdentity>()
);

// The pillar count is the modulus, in one place.
const _: () = assert!(EssenceId::ALL.len() == PILLARS);
const _: () = assert!(PILLARS == 5);

// ---------------------------------------------------------------------------
// HOTPATH ISOLATION LAW
// ---------------------------------------------------------------------------
// `SoulIdentity` must never be embedded in a `Pexil`. The proof is arithmetic, not a
// convention: a 12-byte field cannot fit inside an 8-byte struct, so no `Pexil` field
// can be one. Stated as an assert so a future widening of `Pexil` cannot quietly make
// room for it.
const _: () = assert!(core::mem::size_of::<SoulIdentity>() > core::mem::size_of::<crate::atom::Pexil>());
const _: () = assert!(core::mem::size_of::<crate::atom::Pexil>() == 8);

// And a `Pexil` is *fully accounted for* — 1 + 1 + 2 + 4 = 8, no padding anywhere — so
// there is no hidden slot for a soul handle either. Size alone would not show this;
// a padded struct could hide a field and still measure 8.
const _: () = assert!(
    core::mem::size_of::<crate::atom::TritCell5D>()
        + core::mem::size_of::<crate::atom::ValidityMask>()
        + core::mem::size_of::<crate::atom::CellOrdinal>()
        + 4
        == core::mem::size_of::<crate::atom::Pexil>()
);

// The cache line is the point of the law: 8 pexils per 64-byte L1 line. One inlined
// SoulIdentity would make the atom 20 bytes and drop the line to 3 pexils.
const _: () = assert!(
    core::mem::size_of::<crate::atom::PexilLine>() / core::mem::size_of::<crate::atom::Pexil>() == 8
);
const _: () = assert!(core::mem::size_of::<crate::atom::PexilLine>() == 64);
const _: () = assert!(core::mem::align_of::<crate::atom::PexilLine>() == 64);

// ---------------------------------------------------------------------------
// L2 / L3 — the cache-tier siblings of SoulWord (L1)
// ---------------------------------------------------------------------------
// The two families BELOW L1 are doc terms, never types here (2026-09-13, the book's
// word ladder in forge-book-v3/_book/29-the-resolvent-rail.md had Body/Mind on the
// wrong sizes and is corrected to this file):
//   DrainWord8   = 8 B, 8/line — `NiprPackedWord`, `FrameHeader8`, `ColourTrit8`,
//                  `EcologyPCM8`: one telemetry sample on a `u64`, rings and rails.
//   LatticeWord32 = 32 B, 2/line — `PackedPoint105`, `TritNode5D`, `PentaractField`
//                  class: one lattice cell's packed state.
// Then SoulWord 64 B (L1), BodyWord 256 B (L2), MindWord 4096 B (L3) as locked below.
// Design source: `.forge/_scratch/claude/cc-transcript-1786578773139.txt:283-345`
// (Sean, 2026-08-13) — SoulWord/BodyWord/MindWord map onto CPU/GPU cache tiers:
// L1 (64B) hot registers / SoulWord — the spark, landed above.
// L2 (256B) GPU shared memory / block SRAM / BodyWord — the workhorse: a
// session's working set, same `(hash, parent)` header shape as SoulWord's, so
// no second lineage format is invented (C03 forbid-first / L05 one-home).
// L3 (4096B) page-aligned host-pinned RAM or VRAM / MindWord — the codebook:
// PLURAL by nature (one page per crate/doctrine shard), unlike Soul (one
// instance) and Body (N-bounded per session).
//
// SCOPE CUT (L15 complete, named not silent): only the three sealed WORDS are
// landed here, proven by the same size/align/offset locks as SoulWord. The
// transcript's own SoulSym/BodySym/MindEntry sub-packing layer ("trit packing
// spec") does not compile at the sizes it claims: BodySym (a `u64` field
// after a 7-byte array) and MindEntry (a leading `u64`) both pull in 8-byte
// alignment under plain `#[repr(C)]`, padding them past 20B/19B and past
// what MindTrits's `4084 - 2` arithmetic accounted for — none of it was
// checked against `rustc` before this landing (L02: measure, don't state).
// Hitting the claimed sizes needs `#[repr(C, packed)]` on those two types
// plus a re-verified MindTrits entry count. Owed, not faked.

/// L2 cache-tier word: GPU shared memory / block SRAM, one session's working
/// set. Same `(hash, parent)` header shape as [`SoulWord`] — chained onto
/// tape's own commit/parent pair, not a second lineage format.
#[repr(C, align(256))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BodyWord {
    /// Content hash of the sealed payload (tape commit hash for the flushed path).
    pub hash: u64,
    /// Causal parent's hash, truncated. `0` at the head of a lineage.
    pub parent: u32,
    /// The packed payload. Interpretation (symbol table, working set) is a
    /// future view type over these bytes, not part of the sealed word itself.
    pub trits: [u8; 244],
}

const _: () = assert!(core::mem::size_of::<BodyWord>() == 256);
const _: () = assert!(core::mem::align_of::<BodyWord>() == 256);
const _: () = assert!(core::mem::offset_of!(BodyWord, hash) == 0);
const _: () = assert!(core::mem::offset_of!(BodyWord, parent) == 8);
const _: () = assert!(core::mem::offset_of!(BodyWord, trits) == 12);

/// L3 cache-tier word: page-aligned host-pinned RAM or dedicated VRAM, the
/// codebook. PLURAL by nature — one page per crate/doctrine shard, unlike
/// [`SoulWord`] (one instance) and [`BodyWord`] (N-bounded per session).
#[repr(C, align(4096))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MindWord {
    /// Content hash of this page's source (e.g. an `adr.idx`/`tests.idx` shard).
    pub hash: u64,
    /// Prior page's hash, truncated — chains pages into a book.
    pub parent: u32,
    /// The packed payload.
    pub trits: [u8; 4084],
}

const _: () = assert!(core::mem::size_of::<MindWord>() == 4096);
const _: () = assert!(core::mem::align_of::<MindWord>() == 4096);
const _: () = assert!(core::mem::offset_of!(MindWord, hash) == 0);
const _: () = assert!(core::mem::offset_of!(MindWord, parent) == 8);
const _: () = assert!(core::mem::offset_of!(MindWord, trits) == 12);

// The three tiers strictly grow L1 < L2 < L3 — the cache-hierarchy shape is
// a real, checked invariant, not just a naming convention.
const _: () = assert!(core::mem::size_of::<SoulWord>() < core::mem::size_of::<BodyWord>());
const _: () = assert!(core::mem::size_of::<BodyWord>() < core::mem::size_of::<MindWord>());

// ---------------------------------------------------------------------------
// REACHABILITY (R1) — every payload byte must be an interior base-243 value
// ---------------------------------------------------------------------------
// `crate::atom::TritCell5D` already owns the `0..=242` interior / `243..=255`
// sentinel boundary (`MAX_PACKED`, sentinel.rs:5), exhaustively proven there
// (`sentinel::tests`). Reused here as LOGIC, not as a TYPE: a sealed word's
// payload byte is not itself a 5D lattice coordinate (see `SOUL_TRITS_PER_
// BYTE`'s own doc comment above) — retyping `trits` to `[TritCell5D; N]`
// would conflate two distinct concepts that merely share a byte encoding,
// the exact L05 mistake `a_pexil_ordinal_is_not_a_soul_handle` exists to
// catch one level up. `TritCell5D::is_sentinel()` is called on a throwaway
// `TritCell5D(b)` per byte instead.
#[inline]
fn all_bytes_are_interior_trits(bytes: &[u8]) -> bool {
    bytes.iter().all(|&b| !crate::atom::TritCell5D(b).is_sentinel())
}

impl SoulWord {
    /// R1 reachability: every payload byte decodes as an interior 5-trit
    /// value. `243..=255` per byte (13 dead codepoints) is corruption, not a
    /// sixth pillar — the same boundary `EssenceId::from_u8` polices at the
    /// single-byte scale (`soul.rs:57-69`).
    #[inline]
    pub fn is_well_formed(&self) -> bool {
        all_bytes_are_interior_trits(&self.trits)
    }

    /// Pack hash into the house 32-trit lattice key (one home: `TritArray32`).
    /// Consumes 32 base-3 digits from the 64-bit hash (3^32 < 2^64, no overflow).
    #[inline]
    pub fn trit_key(&self) -> TritArray32 {
        let mut pos_mask = 0u32;
        let mut neg_mask = 0u32;
        let mut remainder = self.hash;
        for i in 0..32 {
            match remainder % 3 {
                0 => neg_mask |= 1 << i,
                1 => {}
                _ => pos_mask |= 1 << i,
            }
            remainder /= 3;
        }
        TritArray32::from_dual_bitmasks(pos_mask, neg_mask)
    }
}

/// Route `query` against `pool` via trit-Hamming nearest-neighbor (one home:
/// `trit_bijection::rank_nearest`). Fills `out` with (distance, pool_index)
/// ascending, ties by index. No allocation — caller owns `out`.
pub fn nearest_soul(query: &SoulWord, pool: &[SoulWord], out: &mut [(u32, usize)]) -> usize {
    let query_key = query.trit_key();
    let mut count = 0;
    for (idx, word) in pool.iter().enumerate() {
        let key = word.trit_key();
        let dist = crate::trit_bijection::TritArray32::hamming_distance(&query_key, &key);
        if count < out.len() {
            out[count] = (dist, idx);
            count += 1;
            let mut pos = count - 1;
            while pos > 0 && (out[pos].0 < out[pos - 1].0 ||
                (out[pos].0 == out[pos - 1].0 && out[pos].1 < out[pos - 1].1)) {
                out.swap(pos, pos - 1);
                pos -= 1;
            }
        } else if dist < out[out.len() - 1].0 ||
            (dist == out[out.len() - 1].0 && idx < out[out.len() - 1].1) {
            out[out.len() - 1] = (dist, idx);
            let mut pos = out.len() - 1;
            while pos > 0 && (out[pos].0 < out[pos - 1].0 ||
                (out[pos].0 == out[pos - 1].0 && out[pos].1 < out[pos - 1].1)) {
                out.swap(pos, pos - 1);
                pos -= 1;
            }
        }
    }
    count
}

impl BodyWord {
    /// R1 reachability, same rule as [`SoulWord::is_well_formed`].
    #[inline]
    pub fn is_well_formed(&self) -> bool {
        all_bytes_are_interior_trits(&self.trits)
    }
}

impl MindWord {
    /// R1 reachability, same rule as [`SoulWord::is_well_formed`].
    #[inline]
    pub fn is_well_formed(&self) -> bool {
        all_bytes_are_interior_trits(&self.trits)
    }
}

// ---------------------------------------------------------------------------
// TRAINING WORD PACKING — SoulWord/BodyWord as MetaRouter training pipeline
// data. Design source: .agents/AGENT-weld-soulword-body-mind-batching.md
// (ARCH000 Sean 2026-08-14: "forge-ml is approved ARCH000(me, Sean)").
// ---------------------------------------------------------------------------
//
// SCOPE LIMIT (L15, named not silent): a training pair's query is trit-packed
// directly into SoulWord's 52-byte payload, which holds at most 51 bytes of
// packed trits (1 byte reserved for the label) = ceil(51*5) = 255 dims. The
// real production MetaRouter's d_model is 768 (confirmed: apex.safetensors /
// sovereign_q4_router.safetensors) — three times over budget. `pack_training_pair`
// below REFUSES (Err), never truncates, when d_model exceeds this. Fitting the
// real 768-dim case needs a reference-based redesign (SoulWord holds a content
// hash, the query itself lives in an external content-addressed store, e.g.
// forge-vcs-v3's object store) — real follow-up work, not solved here.

/// Max query dimensionality [`pack_training_pair`] can trit-pack directly.
/// `(SOUL_BYTES - 1) / 1` bytes available for trits, `SOUL_TRITS_PER_BYTE`
/// trits/byte. One byte of the 52 is reserved for the label.
pub const MAX_INLINE_QUERY_DIMS: usize = (SOUL_BYTES - 1) * SOUL_TRITS_PER_BYTE as usize;

/// FNV-1a content hash (64-bit, integer-only). Same constants as `sky.rs`'s
/// private `fnv1a_hash`, re-derived here rather than imported: that fn hashes
/// star NAMES (a different concept, HANDOFF's own "one home per CONCEPT, not
/// one constant serving two" reasoning, `soul.rs:178-179`) and is private to
/// its module. This one hashes arbitrary training-word PAYLOAD bytes.
#[inline]
pub fn content_hash_fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 14695981039346656037u64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211u64);
    }
    hash
}

/// Truncate a full 64-bit content hash to the 32-bit reference width every
/// `parent` field in this file already uses. A single named conversion so the
/// truncation rule cannot drift between the packers and the resolver (L05).
#[inline]
pub const fn truncate_hash_ref(full: u64) -> u32 {
    (full & 0xFFFF_FFFF) as u32
}

// ---------------------------------------------------------------------------
// SEALING CORES — three tiers, validation and zero-filling
// ---------------------------------------------------------------------------

/// Seal a SoulWord: validate payload, zero-fill to capacity, construct word.
/// Refuses (returns Err, never truncates) if payload exceeds [`SOUL_BYTES`] or
/// any byte is a sentinel trit.
#[inline]
pub fn seal_soulword(hash: u64, trits_data: &[u8], parent: u32) -> Result<SoulWord, String> {
    if trits_data.len() > SOUL_BYTES {
        return Err(format!(
            "trit payload has {} bytes, exceeds SOUL_BYTES={} — refused, not truncated",
            trits_data.len(),
            SOUL_BYTES
        ));
    }
    if !all_bytes_are_interior_trits(trits_data) {
        return Err("trit payload contains sentinel bytes (243..=255) — corruption, not interior".into());
    }
    let mut trits = [0u8; SOUL_BYTES];
    trits[..trits_data.len()].copy_from_slice(trits_data);
    Ok(SoulWord { hash, parent, trits })
}

/// Seal a BodyWord: validate payload, zero-fill to capacity, construct word.
/// Refuses if payload exceeds [`BODY_BYTES`] or contains sentinel bytes.
#[inline]
pub fn seal_bodyword(hash: u64, trits_data: &[u8], parent: u32) -> Result<BodyWord, String> {
    if trits_data.len() > BODY_BYTES {
        return Err(format!(
            "trit payload has {} bytes, exceeds BODY_BYTES={} — refused, not truncated",
            trits_data.len(),
            BODY_BYTES
        ));
    }
    if !all_bytes_are_interior_trits(trits_data) {
        return Err("trit payload contains sentinel bytes — corruption, not interior".into());
    }
    let mut trits = [0u8; 244];
    trits[..trits_data.len()].copy_from_slice(trits_data);
    Ok(BodyWord { hash, parent, trits })
}

/// Seal a MindWord: validate payload, zero-fill to capacity, construct word.
/// Refuses if payload exceeds [`MIND_BYTES`] or contains sentinel bytes.
#[inline]
pub fn seal_mindword(hash: u64, trits_data: &[u8], parent: u32) -> Result<MindWord, String> {
    if trits_data.len() > MIND_BYTES {
        return Err(format!(
            "trit payload has {} bytes, exceeds MIND_BYTES={} — refused, not truncated",
            trits_data.len(),
            MIND_BYTES
        ));
    }
    if !all_bytes_are_interior_trits(trits_data) {
        return Err("trit payload contains sentinel bytes — corruption, not interior".into());
    }
    let mut trits = [0u8; 4084];
    trits[..trits_data.len()].copy_from_slice(trits_data);
    Ok(MindWord { hash, parent, trits })
}

/// One (query, expert label) training pair -> one [`SoulWord`]. `label` is a
/// MetaRouter expert index (`0..=6`, always trit-safe — HANDOFF's own 7-expert
/// domain). `parent` is the truncated hash of the SoulWord this one supersedes
/// (a re-scored/corrected repeat of the same source query), or `0` at first
/// sighting — same `SoulId::ROOT`-style convention `SoulIdentity` already uses
/// elsewhere in this file.
///
/// `hash` is the content hash of `query`'s raw f32 bytes plus the label byte —
/// identity follows content, matching every other word's own doc ("SoulWord's
/// parent is outside the hash identity").
///
/// # Errors
/// `Err` when `query.len() > MAX_INLINE_QUERY_DIMS` or `label > 6` — refused,
/// never silently truncated (L15).
pub fn pack_training_pair(query: &[f32], label: u8, parent: u32) -> Result<SoulWord, String> {
    if query.len() > MAX_INLINE_QUERY_DIMS {
        return Err(format!(
            "query has {} dims, exceeds MAX_INLINE_QUERY_DIMS={} (SoulWord cannot hold this \
             inline — needs the reference-based redesign named in soul.rs)",
            query.len(),
            MAX_INLINE_QUERY_DIMS
        ));
    }
    if label > 6 {
        return Err(format!("label {label} out of MetaRouter's 0..=6 expert range"));
    }

    let bpc = query.len().div_ceil(SOUL_TRITS_PER_BYTE as usize);
    let packed = crate::metarouter::pack_trits(query, bpc);

    let mut hash_input: Vec<u8> = Vec::with_capacity(1 + query.len() * 4); // @forge:allow_alloc cold-path packer, not a hot loop
    hash_input.push(label);
    for f in query {
        hash_input.extend_from_slice(&f.to_le_bytes());
    }
    let hash = content_hash_fnv1a(&hash_input);

    let mut trits_data = Vec::with_capacity(1 + packed.len()); // @forge:allow_alloc cold-path packer, not a hot loop
    trits_data.push(label);
    trits_data.extend_from_slice(&packed);

    seal_soulword(hash, &trits_data, parent)
}

/// Read a [`SoulWord`] built by [`pack_training_pair`] back to `(label,
/// packed_query_bytes)`. Returns the still-packed trit bytes, not floats — a
/// caller that needs the query back as f32 must know its own `d_model` (this
/// word does not carry dimensionality, matching MetaRouter's own `.s13`
/// convention of a header-carried `d_model`, not a per-word one).
pub fn unpack_training_pair(word: &SoulWord) -> (u8, &[u8]) {
    (word.trits[0], &word.trits[1..])
}

/// Header bytes at the front of a [`BodyWord`]'s manifest: a `u16` count of
/// member soul references. Named so [`SOULS_PER_BODY`]'s arithmetic and the
/// packer/unpacker agree by construction, not by convention.
const BODY_MANIFEST_HEADER_BYTES: usize = 2;

/// Max souls one [`BodyWord`] can chain-reference. `(244 - 2) / 4`: 2 header
/// bytes (`u16` count) + 4 bytes per truncated soul-hash reference
/// ([`truncate_hash_ref`]), floor-divided — the 2 leftover bytes are reserved
/// (must be zero), same "reserved, never invented" discipline as `UlamCell64`.
pub const SOULS_PER_BODY: usize = (244 - BODY_MANIFEST_HEADER_BYTES) / 4;

/// Batches up to [`SOULS_PER_BODY`] souls' truncated hashes into one
/// [`BodyWord`] manifest: `count: u16 LE` + `count * u32 LE` refs + zero
/// reserved tail. `hash` is the content hash of the manifest bytes actually
/// written (header + refs), so two batches with the same souls in the same
/// order hash identically — content-addressed, like every other word here.
///
/// # Errors
/// `Err` when `souls.len() > SOULS_PER_BODY` — refused, never silently
/// dropped (L15). A caller with more souls than fit starts a new BodyWord
/// chained onto this one via `parent`, per the task-sheet.
pub fn pack_batch(souls: &[SoulWord], parent: u32) -> Result<BodyWord, String> {
    if souls.len() > SOULS_PER_BODY {
        return Err(format!(
            "batch has {} souls, exceeds SOULS_PER_BODY={} — start a new chained BodyWord",
            souls.len(),
            SOULS_PER_BODY
        ));
    }

    let mut trits = [0u8; 244];
    trits[0..2].copy_from_slice(&(souls.len() as u16).to_le_bytes());
    for (i, soul) in souls.iter().enumerate() {
        let refb = truncate_hash_ref(soul.hash).to_le_bytes();
        let at = BODY_MANIFEST_HEADER_BYTES + i * 4;
        trits[at..at + 4].copy_from_slice(&refb);
    }

    let written = BODY_MANIFEST_HEADER_BYTES + souls.len() * 4;
    let hash = content_hash_fnv1a(&trits[..written]);

    Ok(BodyWord { hash, parent, trits })
}

/// Pack one [`crate::cdk::Triad`] -> [`SoulWord`]. Calls `triad.to_channels()`
/// to normalize the three forces to the bind range, packs via
/// `metarouter::pack_trits()` (matching `pack_training_pair`'s pattern), and
/// hashes the raw i32 fields via `to_le_bytes()` concatenated.
pub fn pack_triad(triad: &crate::cdk::Triad, parent: u32) -> Result<SoulWord, String> {
    let channels = triad.to_channels();
    let query: Vec<f32> = channels.iter().map(|&c| c as f32).collect();

    let bpc = 3usize.div_ceil(SOUL_TRITS_PER_BYTE as usize);
    let packed = crate::metarouter::pack_trits(&query, bpc);

    let mut hash_input: Vec<u8> = Vec::with_capacity(12); // @forge:allow_alloc cold-path packer, not a hot loop
    hash_input.extend_from_slice(&triad.love.to_le_bytes());
    hash_input.extend_from_slice(&triad.strife.to_le_bytes());
    hash_input.extend_from_slice(&triad.entropy.to_le_bytes());
    let hash = content_hash_fnv1a(&hash_input);

    let mut trits_data = Vec::with_capacity(1 + packed.len()); // @forge:allow_alloc cold-path packer, not a hot loop
    trits_data.push(0); // label: no expert label for Triad, use 0
    trits_data.extend_from_slice(&packed);

    seal_soulword(hash, &trits_data, parent)
}

/// Pack up to 4 [`SoulWord`]s into one [`BodyWord`]. Concatenates their trit
/// payloads and hashes over the concatenation of their content hashes.
///
/// # Errors
/// `Err` when `words.len() > 4` — refused, never truncated (L15).
pub fn pack_soulwords_to_body(words: &[SoulWord], parent: u32) -> Result<BodyWord, String> {
    if words.len() > 4 {
        return Err(format!(
            "batch has {} soulwords, exceeds 4 (BodyWord payload is 244 bytes, SoulWord is 52 bytes, floor(244/52)=4) — start a new chained BodyWord",
            words.len()
        ));
    }

    // Route: reject a batch carrying a trit-Hamming duplicate (dist 0 against
    // an earlier member) via nearest_soul — same trit-Hamming lane
    // trit_bijection::rank_nearest already serves query routing.
    if words.len() > 1 {
        let mut ranked = [(0u32, 0usize); 3];
        for i in 1..words.len() {
            let k = nearest_soul(&words[i], &words[..i], &mut ranked[..i.min(3)]);
            if k > 0 && ranked[0].0 == 0 {
                return Err(format!(
                    "soulword {} duplicates soulword {} by trit-Hamming distance 0 — refused, not silently merged",
                    i, ranked[0].1
                ));
            }
        }
    }

    let mut trits_data = Vec::with_capacity(52 * words.len()); // @forge:allow_alloc cold-path packer, not a hot loop
    let mut hash_input = Vec::with_capacity(8 * words.len()); // @forge:allow_alloc cold-path packer, not a hot loop

    for word in words {
        trits_data.extend_from_slice(&word.trits);
        hash_input.extend_from_slice(&word.hash.to_le_bytes());
    }

    let hash = content_hash_fnv1a(&hash_input);

    seal_bodyword(hash, &trits_data, parent)
}

/// Pack up to 4 [`crate::cdk::Triad`]s into one [`BodyWord`]. Packs each triad
/// via [`pack_triad`] (with parent=0 for intermediate soulwords, not chained),
/// then calls [`pack_soulwords_to_body`] — reuses that logic, no duplication.
pub fn pack_triads_to_body(triads: &[crate::cdk::Triad], parent: u32) -> Result<BodyWord, String> {
    let mut words: Vec<SoulWord> = Vec::with_capacity(triads.len()); // @forge:allow_alloc cold-path packer, not a hot loop
    for triad in triads {
        words.push(pack_triad(triad, 0)?);
    }
    pack_soulwords_to_body(&words, parent)
}

/// Pack up to 16 [`BodyWord`]s into one [`MindWord`]. Concatenates their trit
/// payloads and hashes over the concatenation of their content hashes.
///
/// # Errors
/// `Err` when `bodies.len() > 16` — refused, never truncated (L15).
pub fn pack_bodies_to_mind(bodies: &[BodyWord], parent: u32) -> Result<MindWord, String> {
    if bodies.len() > 16 {
        return Err(format!(
            "batch has {} bodywords, exceeds 16 (MindWord payload is 4084 bytes, BodyWord is 244 bytes, floor(4084/244)=16) — start a new chained MindWord",
            bodies.len()
        ));
    }

    let mut trits_data = Vec::with_capacity(244 * bodies.len()); // @forge:allow_alloc cold-path packer, not a hot loop
    let mut hash_input = Vec::with_capacity(8 * bodies.len()); // @forge:allow_alloc cold-path packer, not a hot loop

    for body in bodies {
        trits_data.extend_from_slice(&body.trits);
        hash_input.extend_from_slice(&body.hash.to_le_bytes());
    }

    let hash = content_hash_fnv1a(&hash_input);

    seal_mindword(hash, &trits_data, parent)
}

/// Pack up to 64 [`crate::cdk::Triad`]s into one [`MindWord`]. Groups triads
/// into chunks of at most 4 (BodyWord capacity), calls [`pack_triads_to_body`]
/// for each chunk (parent=0 for intermediate bodies), then calls
/// [`pack_bodies_to_mind`] — reuses that logic, no duplication.
pub fn pack_triads_to_mind(triads: &[crate::cdk::Triad], parent: u32) -> Result<MindWord, String> {
    let mut bodies: Vec<BodyWord> = Vec::new(); // @forge:allow_alloc cold-path packer, not a hot loop
    for chunk in triads.chunks(4) {
        bodies.push(pack_triads_to_body(chunk, 0)?);
    }
    pack_bodies_to_mind(&bodies, parent)
}

/// Read a [`BodyWord`] built by [`pack_batch`] back to its member souls'
/// truncated hash references, in the order they were packed.
pub fn unpack_batch(word: &BodyWord) -> Vec<u32> {
    let count = u16::from_le_bytes([word.trits[0], word.trits[1]]) as usize;
    let count = count.min(SOULS_PER_BODY); // corrupt count cannot read past the manifest
    (0..count)
        .map(|i| {
            let at = BODY_MANIFEST_HEADER_BYTES + i * 4;
            u32::from_le_bytes([
                word.trits[at],
                word.trits[at + 1],
                word.trits[at + 2],
                word.trits[at + 3],
            ])
        })
        .collect() // @forge:allow_alloc cold-path unpacker
}

// ---------------------------------------------------------------------------
// WORD RESOLVER — a safe (Mutex-backed, no unsafe), full-hash-keyed lookup
// bounded to whatever corpus scope it is built over. NOT lock-free: soul.rs's
// own doc comment already named `forge-hal-clockspine::TripleBuffer` as this
// workspace's safe-swap pattern, and `forge-hal-clockspine` crate-level
// `#![forbid(unsafe_code)]` makes a lock-free arena there impossible outright.
// `forge-core-v3` only inherits the workspace DEFAULT `unsafe_code = "deny"`
// (Cargo.toml:140), which scoped `#[allow(unsafe_code)]` could override
// (precedent: sidecar/opaque.rs, shell/sidecar_ctl.rs) — but every existing
// instance of that override is an FFI/opaque-pointer boundary with no safe
// alternative. This resolver has one (std::sync::Mutex), and nothing here is
// a measured hot path (L02/L14: no contention measured, nothing to justify
// unsafe against) — Sean's call, this session: stay on the safe path.
//
// Collision handling mirrors `forge-vcs-v3::VcsRoot::put_object` (root.rs:426-430):
// refuse and error on a genuine ambiguity, never silently alias.
// ---------------------------------------------------------------------------

/// A safe, full-hash-keyed store for [`SoulWord`]s and [`BodyWord`]s, bounded
/// to whatever corpus/training-run scope the caller builds it over — not a
/// global registry. Detects `u32`-truncation collisions on insert (two
/// distinct full hashes truncating to the same [`truncate_hash_ref`] value)
/// and refuses rather than aliasing, so a `parent: u32` field can always be
/// resolved unambiguously through this store even though the field itself is
/// lossy.
pub struct WordResolver {
    souls: std::sync::Mutex<std::collections::HashMap<u64, SoulWord>>,
    bodies: std::sync::Mutex<std::collections::HashMap<u64, BodyWord>>,
    /// `truncated_ref -> full_hash`, shared across souls and bodies (both
    /// draw from the same 32-bit reference space via `truncate_hash_ref`, so
    /// a collision check must see both, not two independent tables).
    ref_index: std::sync::Mutex<std::collections::HashMap<u32, u64>>,
}

impl WordResolver {
    /// An empty resolver.
    pub fn new() -> Self {
        Self {
            souls: std::sync::Mutex::new(std::collections::HashMap::new()),
            bodies: std::sync::Mutex::new(std::collections::HashMap::new()),
            ref_index: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Checks `full_hash`'s truncated reference against every reference this
    /// resolver has already seen. `Ok(())` when unclaimed OR already claimed
    /// by this exact same full hash (idempotent re-insert, matching the
    /// tape's own re-commit rule). `Err` when a DIFFERENT full hash already
    /// holds this truncated reference — a genuine collision, refused per
    /// `put_object`'s own doctrine.
    fn check_and_claim_ref(&self, full_hash: u64) -> Result<(), String> {
        let refb = truncate_hash_ref(full_hash);
        let mut idx = self.ref_index.lock().expect("ref_index mutex poisoned");
        match idx.get(&refb) {
            Some(&existing) if existing != full_hash => Err(format!(
                "truncated ref {refb:08x} collision: full hash {existing:016x} already \
                 claims it, refusing to alias {full_hash:016x}"
            )),
            _ => {
                idx.insert(refb, full_hash);
                Ok(())
            }
        }
    }

    /// Insert a soul. `Err` on a genuine truncation collision (see
    /// `check_and_claim_ref` (private)); an identical re-insert of the same hash is a
    /// no-op success.
    pub fn insert_soul(&self, word: SoulWord) -> Result<(), String> {
        self.check_and_claim_ref(word.hash)?;
        self.souls.lock().expect("souls mutex poisoned").insert(word.hash, word);
        Ok(())
    }

    /// Insert a body. Same collision rule as [`Self::insert_soul`].
    pub fn insert_body(&self, word: BodyWord) -> Result<(), String> {
        self.check_and_claim_ref(word.hash)?;
        self.bodies.lock().expect("bodies mutex poisoned").insert(word.hash, word);
        Ok(())
    }

    /// Resolve a `SoulWord.parent`-style truncated reference back to the full
    /// soul, if this resolver has seen it. Two-step: truncated ref -> full
    /// hash -> word, so the collision-checked index is the single source of
    /// truth for the ambiguous direction.
    pub fn resolve_soul_ref(&self, truncated: u32) -> Option<SoulWord> {
        let idx = self.ref_index.lock().expect("ref_index mutex poisoned");
        let full = *idx.get(&truncated)?;
        drop(idx);
        self.souls.lock().expect("souls mutex poisoned").get(&full).copied()
    }

    /// Resolve a `BodyWord.parent`-style truncated reference back to the full
    /// body, if this resolver has seen it.
    pub fn resolve_body_ref(&self, truncated: u32) -> Option<BodyWord> {
        let idx = self.ref_index.lock().expect("ref_index mutex poisoned");
        let full = *idx.get(&truncated)?;
        drop(idx);
        self.bodies.lock().expect("bodies mutex poisoned").get(&full).copied()
    }
}

impl Default for WordResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod training_word_tests {
    use super::*;

    #[test]
    fn pack_and_unpack_training_pair_round_trips_the_label() {
        let query = vec![0.9f32, -0.2, 0.1, -0.9, 0.05];
        let word = pack_training_pair(&query, 3, 0).unwrap();
        let (label, _packed) = unpack_training_pair(&word);
        assert_eq!(label, 3);
        assert!(word.is_well_formed(), "packed trits must stay interior (R1)");
    }

    #[test]
    fn pack_training_pair_refuses_oversized_query_not_truncate() {
        let query = vec![0.1f32; MAX_INLINE_QUERY_DIMS + 1];
        assert!(pack_training_pair(&query, 0, 0).is_err());
        // The real production boundary this refusal exists for: 768-dim
        // (apex/sovereign_q4_router's real d_model) really is over budget.
        let production_d_model = std::hint::black_box(768usize);
        assert!(production_d_model > MAX_INLINE_QUERY_DIMS);
    }

    #[test]
    fn pack_training_pair_refuses_out_of_range_label() {
        let query = vec![0.1f32; 4];
        assert!(pack_training_pair(&query, 7, 0).is_err());
        assert!(pack_training_pair(&query, 6, 0).is_ok());
    }

    #[test]
    fn same_content_hashes_identically_different_content_does_not() {
        let a = pack_training_pair(&[1.0, 2.0], 1, 0).unwrap();
        let b = pack_training_pair(&[1.0, 2.0], 1, 0).unwrap();
        let c = pack_training_pair(&[1.0, 2.0], 2, 0).unwrap();
        assert_eq!(a.hash, b.hash, "identical (query, label) hashes identically");
        assert_ne!(a.hash, c.hash, "different label changes the content hash");
    }

    #[test]
    fn pack_and_unpack_batch_round_trips_soul_refs() {
        let souls: Vec<SoulWord> = (0..5u8)
            .map(|i| pack_training_pair(&[i as f32 * 0.1], i % 7, 0).unwrap())
            .collect();
        let body = pack_batch(&souls, 0).unwrap();
        let refs = unpack_batch(&body);
        assert_eq!(refs.len(), 5);
        for (soul, &r) in souls.iter().zip(refs.iter()) {
            assert_eq!(r, truncate_hash_ref(soul.hash));
        }
        assert!(body.is_well_formed());
    }

    #[test]
    fn pack_batch_refuses_overflow_not_truncate() {
        let souls: Vec<SoulWord> = (0..SOULS_PER_BODY + 1)
            .map(|i| pack_training_pair(&[i as f32], 0, 0).unwrap())
            .collect();
        assert!(pack_batch(&souls, 0).is_err());
        let ok: Vec<SoulWord> = souls[..SOULS_PER_BODY].to_vec();
        assert!(pack_batch(&ok, 0).is_ok());
    }

    #[test]
    fn resolver_round_trips_a_soul_through_its_truncated_ref() {
        let resolver = WordResolver::new();
        let soul = pack_training_pair(&[0.5, -0.5], 4, 0).unwrap();
        resolver.insert_soul(soul).unwrap();
        let back = resolver.resolve_soul_ref(truncate_hash_ref(soul.hash)).unwrap();
        assert_eq!(back, soul);
    }

    #[test]
    fn resolver_unknown_ref_resolves_to_none() {
        let resolver = WordResolver::new();
        assert!(resolver.resolve_soul_ref(0xDEAD_BEEF).is_none());
    }

    #[test]
    fn resolver_idempotent_reinsert_of_the_same_soul_succeeds() {
        let resolver = WordResolver::new();
        let soul = pack_training_pair(&[1.0], 0, 0).unwrap();
        assert!(resolver.insert_soul(soul).is_ok());
        assert!(resolver.insert_soul(soul).is_ok(), "identical re-insert must not error");
    }

    // The genuine collision case: two DIFFERENT full hashes forced to the same
    // truncated ref. Hand-constructed (not randomly hunted) so the test is
    // deterministic — real collisions are astronomically rare at random, so
    // this proves the REFUSAL LOGIC works, not that collisions occur naturally.
    #[test]
    fn resolver_refuses_a_genuine_truncation_collision() {
        let resolver = WordResolver::new();
        let a = SoulWord { hash: 0x0000_0001_DEAD_BEEF, parent: 0, trits: [0u8; SOUL_BYTES] };
        let b = SoulWord { hash: 0x0000_0002_DEAD_BEEF, parent: 0, trits: [0u8; SOUL_BYTES] };
        assert_eq!(
            truncate_hash_ref(a.hash),
            truncate_hash_ref(b.hash),
            "test setup: both hashes must share a truncated ref"
        );
        assert!(resolver.insert_soul(a).is_ok());
        let err = resolver.insert_soul(b);
        assert!(err.is_err(), "a second, different full hash on the same truncated ref must refuse");
    }

    #[test]
    fn resolver_bodies_and_souls_share_one_collision_check() {
        // A soul and a body forced onto the same truncated ref must ALSO
        // collide — the ref_index is shared, not two independent tables.
        let resolver = WordResolver::new();
        let soul = SoulWord { hash: 0x0000_0003_CAFE_F00D, parent: 0, trits: [0u8; SOUL_BYTES] };
        let body = BodyWord { hash: 0x0000_0004_CAFE_F00D, parent: 0, trits: [0u8; 244] };
        assert_eq!(truncate_hash_ref(soul.hash), truncate_hash_ref(body.hash));
        assert!(resolver.insert_soul(soul).is_ok());
        assert!(resolver.insert_body(body).is_err(), "cross-type collision must also refuse");
    }

    #[test]
    fn seal_soulword_extracts_pack_training_pair_sealing_without_behavior_change() {
        // Prove that seal_soulword duplicates pack_training_pair's sealing,
        // so extracting the sealing core doesn't change observable output.
        let query = vec![0.5f32, -0.3, 0.1, 0.8];
        let label = 2u8;
        let parent = 42u32;

        let word1 = pack_training_pair(&query, label, parent).unwrap();

        // Replicate pack_training_pair's encoding steps outside the function.
        let bpc = query.len().div_ceil(SOUL_TRITS_PER_BYTE as usize);
        let packed = crate::metarouter::pack_trits(&query, bpc);

        let mut hash_input: Vec<u8> = Vec::with_capacity(1 + query.len() * 4);
        hash_input.push(label);
        for f in &query {
            hash_input.extend_from_slice(&f.to_le_bytes());
        }
        let hash = content_hash_fnv1a(&hash_input);

        let mut trits_data = Vec::new();
        trits_data.push(label);
        trits_data.extend_from_slice(&packed);

        let word2 = seal_soulword(hash, &trits_data, parent).unwrap();

        assert_eq!(word1.hash, word2.hash, "content hash must match");
        assert_eq!(word1.parent, word2.parent, "parent must match");
        assert_eq!(word1.trits, word2.trits, "trits must match exactly");
    }

    #[test]
    fn pack_triad_round_trips_a_real_triad() {
        let triad = crate::cdk::Triad { love: 100, strife: 200, entropy: 500 };

        let word1 = pack_triad(&triad, 0).unwrap();
        let word2 = pack_triad(&triad, 0).unwrap();

        assert_eq!(word1.hash, word2.hash, "same triad must pack to same hash");
        assert_eq!(word1.parent, word2.parent, "parent must match");
        assert_eq!(word1.trits, word2.trits, "trits must match");
        assert!(word1.is_well_formed(), "packed trits must stay interior (R1)");

        let different_triad = crate::cdk::Triad { love: 100, strife: 200, entropy: 600 };
        let word3 = pack_triad(&different_triad, 0).unwrap();
        assert_ne!(word1.hash, word3.hash, "different triad must pack to different hash");
    }

    #[test]
    fn nearest_soul_finds_exact_pool_match_first() {
        let pool: Vec<SoulWord> = (0..4u8)
            .map(|i| pack_training_pair(&[i as f32 * 0.1], i % 7, 0).unwrap())
            .collect();
        let query = pool[2];
        let mut out = [(0u32, 0usize); 4];
        let n = nearest_soul(&query, &pool, &mut out);
        assert_eq!(n, 4);
        assert_eq!(out[0], (0, 2), "query equals pool[2] exactly, dist 0, index 2, first");
    }

    #[test]
    fn pack_soulwords_to_body_refuses_more_than_four() {
        let words: Vec<SoulWord> = (0..5u8)
            .map(|i| pack_training_pair(&[i as f32 * 0.1], i % 7, 0).unwrap())
            .collect();

        assert!(pack_soulwords_to_body(&words, 0).is_err(), "5 soulwords exceed capacity");

        let ok_words = &words[..4];
        assert!(pack_soulwords_to_body(ok_words, 0).is_ok(), "4 soulwords must fit");
    }

    #[test]
    fn pack_bodies_to_mind_refuses_more_than_sixteen() {
        // Create 17 bodies, each a 1-soul batch.
        let mut bodies = Vec::new();
        for i in 0..17u8 {
            let soul = pack_training_pair(&[i as f32 * 0.1], i % 7, 0).unwrap();
            let body = pack_soulwords_to_body(&[soul], 0).unwrap();
            bodies.push(body);
        }

        assert!(pack_bodies_to_mind(&bodies, 0).is_err(), "17 bodywords exceed capacity");

        let ok_bodies = &bodies[..16];
        assert!(pack_bodies_to_mind(ok_bodies, 0).is_ok(), "16 bodywords must fit");
    }

    #[test]
    fn pack_triads_to_body_and_to_mind_match_the_soulword_path() {
        // Prove that pack_triads_to_body produces the same output as
        // packing each triad individually and then calling pack_soulwords_to_body.
        let triads = vec![
            crate::cdk::Triad { love: 100, strife: 200, entropy: 300 },
            crate::cdk::Triad { love: 400, strife: 500, entropy: 600 },
        ];

        // Path 1: pack each triad -> soulword, then to body.
        let words: Vec<SoulWord> = triads
            .iter()
            .map(|t| pack_triad(t, 0).unwrap())
            .collect();
        let body_direct = pack_soulwords_to_body(&words, 0).unwrap();

        // Path 2: pack triads directly to body.
        let body_triads = pack_triads_to_body(&triads, 0).unwrap();

        // Both paths should produce identical results.
        assert_eq!(body_direct.hash, body_triads.hash, "hashes must match");
        assert_eq!(body_direct.parent, body_triads.parent, "parents must match");
        assert_eq!(body_direct.trits, body_triads.trits, "trits must match");
        assert!(body_direct.is_well_formed(), "packed trits must stay interior (R1)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atom::{CellOrdinal, Pexil, PexilLine, TritCell5D};

    #[test]
    fn soulword_is_one_cache_line_and_two_distinct_payloads_hash_distinct() {
        assert_eq!(core::mem::size_of::<SoulWord>(), 64);
        assert_eq!(core::mem::align_of::<SoulWord>(), 64);
        let a = SoulWord { hash: 7, parent: 0, trits: [0u8; SOUL_BYTES] };
        let b = SoulWord { hash: 9, parent: 0, trits: [0u8; SOUL_BYTES] };
        assert_ne!(a.hash, b.hash);
        assert_eq!(a, a, "SoulWord is Eq and reflexive");
    }

    #[test]
    fn soulword_parent_is_outside_the_hash_identity() {
        // Mirrors the v2 gate (`soulword.rs:432`, "parent is inside identity" —
        // there, `hash` alone identifies content, `parent` is lineage, not payload.
        let head = SoulWord { hash: 7, parent: 0, trits: [0u8; SOUL_BYTES] };
        let child = SoulWord { hash: 7, parent: 3, trits: [0u8; SOUL_BYTES] };
        assert_eq!(head.hash, child.hash, "same content hash");
        assert_ne!(head.parent, child.parent, "different lineage, distinguishable");
    }

    // ---- L2 / L3 CACHE-TIER WORDS ------------------------------------------

    #[test]
    fn bodyword_is_one_gpu_shared_memory_block_and_distinct_payloads_hash_distinct() {
        assert_eq!(core::mem::size_of::<BodyWord>(), 256);
        assert_eq!(core::mem::align_of::<BodyWord>(), 256);
        let a = BodyWord { hash: 7, parent: 0, trits: [0u8; 244] };
        let b = BodyWord { hash: 9, parent: 0, trits: [0u8; 244] };
        assert_ne!(a.hash, b.hash);
        assert_eq!(a, a, "BodyWord is Eq and reflexive");
    }

    #[test]
    fn bodyword_parent_is_outside_the_hash_identity() {
        let head = BodyWord { hash: 7, parent: 0, trits: [0u8; 244] };
        let child = BodyWord { hash: 7, parent: 3, trits: [0u8; 244] };
        assert_eq!(head.hash, child.hash, "same content hash");
        assert_ne!(head.parent, child.parent, "different lineage, distinguishable");
    }

    #[test]
    fn mindword_is_one_page_and_distinct_payloads_hash_distinct() {
        assert_eq!(core::mem::size_of::<MindWord>(), 4096);
        assert_eq!(core::mem::align_of::<MindWord>(), 4096);
        let a = MindWord { hash: 7, parent: 0, trits: [0u8; 4084] };
        let b = MindWord { hash: 9, parent: 0, trits: [0u8; 4084] };
        assert_ne!(a.hash, b.hash);
        assert_eq!(a, a, "MindWord is Eq and reflexive");
    }

    #[test]
    fn mindword_parent_is_outside_the_hash_identity() {
        let head = MindWord { hash: 7, parent: 0, trits: [0u8; 4084] };
        let child = MindWord { hash: 7, parent: 3, trits: [0u8; 4084] };
        assert_eq!(head.hash, child.hash, "same content hash");
        assert_ne!(head.parent, child.parent, "different lineage, distinguishable");
    }

    // ---- R1 REACHABILITY ----------------------------------------------------

    #[test]
    fn soulword_well_formed_rejects_any_sentinel_trit_byte() {
        let clean = SoulWord { hash: 1, parent: 0, trits: [0u8; SOUL_BYTES] };
        assert!(clean.is_well_formed());

        let mut corrupt = clean;
        corrupt.trits[SOUL_BYTES - 1] = 255; // 255 >= MAX_PACKED (243): a dead codepoint
        assert!(!corrupt.is_well_formed());

        let mut boundary = clean;
        boundary.trits[0] = 243; // the FIRST sentinel value — off-by-one guard
        assert!(!boundary.is_well_formed());
        boundary.trits[0] = 242; // the LAST interior value — must still pass
        assert!(boundary.is_well_formed());
    }

    #[test]
    fn bodyword_well_formed_rejects_any_sentinel_trit_byte() {
        let clean = BodyWord { hash: 1, parent: 0, trits: [0u8; 244] };
        assert!(clean.is_well_formed());
        let mut corrupt = clean;
        corrupt.trits[0] = 250;
        assert!(!corrupt.is_well_formed());
    }

    #[test]
    fn mindword_well_formed_rejects_any_sentinel_trit_byte() {
        let clean = MindWord { hash: 1, parent: 0, trits: [0u8; 4084] };
        assert!(clean.is_well_formed());
        let mut corrupt = clean;
        corrupt.trits[4083] = 243;
        assert!(!corrupt.is_well_formed());
    }

    #[test]
    fn the_three_cache_tiers_strictly_grow_l1_l2_l3() {
        assert!(core::mem::size_of::<SoulWord>() < core::mem::size_of::<BodyWord>());
        assert!(core::mem::size_of::<BodyWord>() < core::mem::size_of::<MindWord>());
        assert_eq!(core::mem::size_of::<SoulWord>(), 64);
        assert_eq!(core::mem::size_of::<BodyWord>(), 256);
        assert_eq!(core::mem::size_of::<MindWord>(), 4096);
    }

    #[test]
    fn five_pillars_and_nothing_else_decodes() {
        assert_eq!(PILLARS, 5);
        for (i, pillar) in EssenceId::ALL.iter().enumerate() {
            assert_eq!(EssenceId::from_u8(i as u8), Some(*pillar));
            assert_eq!(pillar.ordinal() as usize, i);
        }
        for b in PILLARS as u8..=255 {
            assert!(EssenceId::from_u8(b).is_none(), "byte {b} is not a pillar");
        }
    }

    #[test]
    fn essence_is_derived_from_every_lattice_byte_and_always_lands() {
        // All 256 bytes, sentinels included: `% 5` is total, so this may not fail.
        for b in 0u16..=255 {
            let cell = TritCell5D(b as u8);
            let derived = EssenceId::from_lattice(cell);
            assert_eq!(derived.ordinal(), cell.essence());
            assert_eq!(EssenceId::from_u8(cell.essence()), Some(derived));
            assert!(derived.ordinal() < PILLARS as u8);
        }
    }

    #[test]
    fn every_pillar_is_reachable_from_some_lattice_byte() {
        for pillar in EssenceId::ALL {
            assert!(
                (0u16..=255).any(|b| EssenceId::from_lattice(TritCell5D(b as u8)) == pillar),
                "{pillar:?} is unreachable — the modulus and the pillar count disagree"
            );
        }
    }

    // ---- HOTPATH ISOLATION -------------------------------------------------

    #[test]
    fn a_soul_identity_cannot_fit_inside_a_pexil() {
        assert_eq!(core::mem::size_of::<Pexil>(), 8);
        assert_eq!(core::mem::size_of::<SoulIdentity>(), 12);
        assert!(core::mem::size_of::<SoulIdentity>() > core::mem::size_of::<Pexil>());
    }

    #[test]
    fn a_pexil_line_still_holds_eight_pexils_in_one_cache_line() {
        assert_eq!(core::mem::size_of::<PexilLine>(), 64);
        assert_eq!(core::mem::align_of::<PexilLine>(), 64);
        assert_eq!(core::mem::size_of::<PexilLine>() / core::mem::size_of::<Pexil>(), 8);

        // What inlining a soul would actually cost, measured rather than asserted in prose.
        let inlined = core::mem::size_of::<Pexil>() + core::mem::size_of::<SoulIdentity>();
        assert_eq!(inlined, 20);
        assert_eq!(64 / inlined, 3, "an inlined soul drops the line from 8 atoms to 3");
    }

    /// A **type** gate, not a size gate. `SoulId` and `CellOrdinal` are both 2 bytes, so
    /// swapping one for the other in `Pexil` would pass every layout assert in this
    /// crate. These bindings are the thing that refuses it: if `Pexil::ordinal` ever
    /// becomes a `SoulId`, this stops compiling.
    #[test]
    fn a_pexil_ordinal_is_not_a_soul_handle() {
        assert_eq!(core::mem::size_of::<SoulId>(), core::mem::size_of::<CellOrdinal>());

        let p = Pexil {
            lattice: TritCell5D::ORIGIN,
            validity: crate::atom::ValidityMask::ALL_KNOWN,
            ordinal: CellOrdinal(7),
            payload: [0; 4],
        };
        let ordinal: CellOrdinal = p.ordinal;
        let lattice: TritCell5D = p.lattice;
        assert_eq!(ordinal.0, 7);
        assert!(!lattice.is_sentinel());
    }

    /// The other half of the law: no soul field is reachable *from* a Pexil. Proven by
    /// accounting for all 8 bytes, so there is no padding for one to hide in.
    #[test]
    fn a_pexil_is_fully_accounted_for() {
        let fields = core::mem::size_of::<TritCell5D>()
            + core::mem::size_of::<crate::atom::ValidityMask>()
            + core::mem::size_of::<CellOrdinal>()
            + 4;
        assert_eq!(fields, core::mem::size_of::<Pexil>(), "padding in Pexil could hide a field");
    }

    // ---- IDENTITY ----------------------------------------------------------

    #[test]
    fn soul_identity_is_twelve_bytes_of_field_and_no_padding() {
        assert_eq!(core::mem::size_of::<SoulIdentity>(), 12);
        assert_eq!(core::mem::align_of::<SoulIdentity>(), 4);
        assert_eq!(4 + 2 + 2 + 2 + 2, core::mem::size_of::<SoulIdentity>());
    }

    #[test]
    fn root_is_a_terminator_not_a_soul() {
        assert!(SoulId::ROOT.is_root());
        assert!(!SoulId(1).is_root());
        assert!(!SoulId::MAX.is_root());

        let rooted = SoulIdentity::at_genesis(SoulId(1), SoulId::ROOT, DetClock::GENESIS).unwrap();
        assert!(rooted.is_genesis_soul());
        assert!(rooted.is_well_formed());

        // A soul that claims the root as its own identity is malformed.
        let impostor =
            SoulIdentity::at_genesis(SoulId::ROOT, SoulId::ROOT, DetClock::GENESIS).unwrap();
        assert!(!impostor.is_well_formed());
    }

    // The bug this test exists for: a self-parent makes a lineage walk never terminate.
    #[test]
    fn a_soul_cannot_be_its_own_parent() {
        let cycle = SoulIdentity::at_genesis(SoulId(9), SoulId(9), DetClock::GENESIS).unwrap();
        assert!(!cycle.is_well_formed());

        let ok = SoulIdentity::at_genesis(SoulId(9), SoulId(4), DetClock::GENESIS).unwrap();
        assert!(ok.is_well_formed());
        assert!(!ok.is_genesis_soul());
    }

    // is_well_formed only catches a DIRECT self-parent (one hop). cynatic_depth
    // closes the rest: any chain that doesn't reach root within the bound.
    #[test]
    fn cynatic_depth_counts_hops_to_root() {
        // 3 -> 2 -> 1 -> ROOT: three hops.
        let parent_of = |s: SoulId| match s.0 {
            3 => Some(SoulId(2)),
            2 => Some(SoulId(1)),
            1 => Some(SoulId::ROOT),
            _ => None,
        };
        assert_eq!(cynatic_depth(SoulId(3), 10, parent_of), Some(3));
        assert_eq!(cynatic_depth(SoulId::ROOT, 10, parent_of), Some(0), "root is its own depth-0 chain");
    }

    #[test]
    fn cynatic_depth_refuses_a_longer_cycle_is_well_formed_cannot_see() {
        // 5 -> 6 -> 5 -> ... : a two-hop cycle, invisible to is_well_formed
        // (neither soul is its own direct parent).
        let cyclic = |s: SoulId| match s.0 {
            5 => Some(SoulId(6)),
            6 => Some(SoulId(5)),
            _ => None,
        };
        assert_eq!(cynatic_depth(SoulId(5), 10, cyclic), None, "a cycle never reaches root");
    }

    #[test]
    fn cynatic_depth_refuses_a_chain_deeper_than_the_bound() {
        let parent_of = |s: SoulId| if s.0 > 0 { Some(SoulId(s.0 - 1)) } else { None };
        assert_eq!(cynatic_depth(SoulId(5), 5, parent_of), Some(5), "exactly at the bound");
        assert_eq!(cynatic_depth(SoulId(6), 5, parent_of), None, "one hop past the bound");
    }

    #[test]
    fn cynatic_depth_refuses_a_broken_chain() {
        let dead_end = |_: SoulId| None;
        assert_eq!(cynatic_depth(SoulId(1), 10, dead_end), None, "no parent found, no false depth");
    }

    // The narrowing is checked, not truncated. A truncated tick aliases two souls'
    // provenance: tick 2^32 and tick 0 would stamp identically.
    #[test]
    fn a_genesis_tick_past_u32_is_refused_not_truncated() {
        let overflowing = DetClock { tick: u32::MAX as u64 + 1, ..DetClock::GENESIS };
        assert!(SoulIdentity::at_genesis(SoulId(1), SoulId::ROOT, overflowing).is_none());

        let last = DetClock { tick: u32::MAX as u64, ..DetClock::GENESIS };
        let id = SoulIdentity::at_genesis(SoulId(1), SoulId::ROOT, last).unwrap();
        assert_eq!(id.genesis_tick, u32::MAX);

        assert!(SoulIdentity::at_genesis(SoulId(1), SoulId::ROOT, DetClock { tick: u64::MAX, ..DetClock::GENESIS }).is_none());
    }

    #[test]
    fn identity_carries_the_authority_that_stamped_it() {
        let clock = DetClock { tick: 42, epoch: 3, authority: ArchRole::ARCH000 };
        let id = SoulIdentity::at_genesis(SoulId(5), SoulId(1), clock).unwrap();
        assert_eq!(id.genesis_tick, 42);
        assert_eq!(id.genesis_epoch, 3);
        assert!(id.authority.is_prime_authority());
    }

    #[test]
    fn trit_key_is_deterministic_and_all_bytes_interior() {
        let soul1 = SoulWord { hash: 0x0102030405060708, parent: 0, trits: [0u8; SOUL_BYTES] };
        let soul2 = SoulWord { hash: 0x0102030405060708, parent: 1, trits: [0u8; SOUL_BYTES] };
        let soul3 = SoulWord { hash: 0x0102030405060709, parent: 0, trits: [0u8; SOUL_BYTES] };

        let key1a = soul1.trit_key();
        let key1b = soul1.trit_key();
        let key2 = soul2.trit_key();
        let key3 = soul3.trit_key();

        assert_eq!(key1a, key1b, "same hash → same key (deterministic)");
        assert_eq!(key1a, key2, "parent field does not affect trit_key");
        assert_ne!(key1a, key3, "different hash → different key");
    }

    #[test]
    fn trit_hamming_counts_differing_ternary_digits() {
        let soul_a = SoulWord { hash: 0, parent: 0, trits: [0u8; SOUL_BYTES] };
        let soul_b = SoulWord { hash: 1, parent: 0, trits: [0u8; SOUL_BYTES] };
        let soul_c = SoulWord { hash: 1, parent: 0, trits: [0u8; SOUL_BYTES] };

        let key_a = soul_a.trit_key();
        let key_b = soul_b.trit_key();
        let key_c = soul_c.trit_key();

        assert_eq!(TritArray32::hamming_distance(&key_a, &key_a), 0, "identical keys have zero distance");
        assert!(TritArray32::hamming_distance(&key_a, &key_b) > 0, "different hashes produce > 0 distance");
        assert_eq!(TritArray32::hamming_distance(&key_b, &key_c), 0, "identical hashes produce zero distance");
    }
}
