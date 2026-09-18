//! The spine — provenance, commit class, and the lock verdict.
//!
//! Every artifact that enters the deterministic fabric is wrapped in an
//! [`AuthorityTicket`] declaring (a) what it is ([`CarrierKind`]), (b) where it
//! came from ([`SourceKind`]), (c) what receipt is being issued
//! ([`ReceiptKind`]), and (d) the lineage chain via [`CarrierHeader`].
//!
//! Discriminants are stable and are **const-asserted below**. Reordering a
//! variant breaks replay, so it breaks `cargo check` first; only append new
//! variants at the end.
//!
//! ## What is deliberately absent, and why
//!
//! This crate has zero dependencies (`Cargo.toml:9-11`) and blake3 in particular
//! is firewalled out of it — BLAKE3 lives downstream in `forge-vcs-v3`. Two
//! names therefore arrive here with a hole in them, and the hole is the point:
//!
//! - [`BrutalHash`] is the 64-bit **carrier** of a truncated blake3 digest. It
//!   has `ZERO` and `as_u64` but **no `of()`** and no `combine()`; both hash.
//!   The type crosses the firewall, the hashing does not.
//! - [`AuthorityTicket`] has **no `deterministic_hash`**. Its final act was
//!   `BrutalHash::of(&buf)`, so it is a blake3 call wearing a serialization
//!   coat. It belongs to `forge-vcs-v3` alongside `mint()`.
//!
//! Two further omissions from the ported sources, recorded so their absence
//! reads as a decision rather than an oversight: `Trit::rgb` and its `AnsiEmit`
//! impl needed a `provenance` module that does not exist here, and the ledger
//! helper `append_global` does not port at all — the VCS is the ledger.

/// One axis of a lock: fault, in-flight, or sealed.
///
/// A binary mutex says HELD or FREE and nothing else, so a lock that was
/// *broken* reads identical to one that was never taken. Three of these carry
/// claim, alignment and proof as separate axes; any `-1` poisons the vector.
///
/// The balanced values are the same `{-1, 0, +1}` the lattice uses, and
/// [`Trit::shifted`] is the same `+1` shift as
/// [`crate::atom::TritCell5D::from_trits`] — asserted below, not assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i8)]
pub enum Trit {
    /// `-1` — poisoned: broken, stale-purged, drifted, or rejected.
    Fault = -1,
    /// `0` — claimed and running.
    Intent = 0,
    /// `+1` — verified, bound, sealed.
    Sealed = 1,
}

impl Trit {
    /// Every axis state in balanced order.
    pub const ALL: [Trit; 3] = [Trit::Fault, Trit::Intent, Trit::Sealed];

    /// `<`, `0`, `>` — the glyph face, one preattentive char per axis.
    #[inline(always)]
    pub const fn glyph(self) -> char {
        match self {
            Trit::Fault => '<',
            Trit::Intent => '0',
            Trit::Sealed => '>',
        }
    }

    /// Balanced value shifted into `0..=2` for positional radix-3 packing.
    #[inline(always)]
    pub const fn shifted(self) -> u8 {
        (self as i8 + 1) as u8
    }

    /// Inverse of [`Trit::shifted`]. Out-of-range input reads `Intent` — an
    /// unknown verdict is one nobody has yet, never a silent `Sealed`.
    #[inline(always)]
    pub const fn from_shifted(v: u8) -> Trit {
        match v {
            0 => Trit::Fault,
            2 => Trit::Sealed,
            _ => Trit::Intent,
        }
    }

    /// The PROCESS edge. POSIX nails 0 to success, so the balanced value can
    /// never ride the wire: `Sealed 0 · Fault 1 · Intent 2`. This is the ONLY
    /// place a verdict becomes an exit code; a verb that spells its own ladder
    /// is a second, disagreeing gate.
    #[inline(always)]
    pub const fn exit_code(self) -> i32 {
        match self {
            Trit::Sealed => 0,
            Trit::Fault => 1,
            Trit::Intent => 2,
        }
    }

    /// Inverse of [`Trit::exit_code`]. `None` for any other code: a process
    /// that produced no verdict at all is off the lattice, not a third reading.
    #[inline(always)]
    pub const fn from_exit(code: i32) -> Option<Trit> {
        match code {
            0 => Some(Trit::Sealed),
            1 => Some(Trit::Fault),
            2 => Some(Trit::Intent),
            _ => None,
        }
    }
}

/// Commit class for any artifact entering the deterministic fabric.
///
/// This is the time-pressure / authority class an artifact commits under. It is
/// not a worker-routing lane; those are a different axis and a different type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Lane {
    /// This-tick commit. Blocks frame if delayed. Spinal reflex path.
    Critical = 0,
    /// Next 1-2 ticks. Pre-warmed by the scheduler.
    NearFuture = 1,
    /// Binds from a prior tick or event. Truth-source derived. Cortical reflex.
    PriorAuthority = 2,
    /// Perception only. Derenderable on epoch mismatch.
    Speculative = 3,
    /// Decoration. First to evict under pressure.
    Discardable = 4,
}

impl Lane {
    /// Every lane in discriminant order.
    pub const ALL: [Lane; 5] = [
        Lane::Critical,
        Lane::NearFuture,
        Lane::PriorAuthority,
        Lane::Speculative,
        Lane::Discardable,
    ];

    /// Discriminant value for wire encoding.
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Decode a stored lane. `None` outside `0..=4` — an unknown lane is
    /// corruption, never a default.
    #[inline(always)]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Critical),
            1 => Some(Self::NearFuture),
            2 => Some(Self::PriorAuthority),
            3 => Some(Self::Speculative),
            4 => Some(Self::Discardable),
            _ => None,
        }
    }

    /// Eviction order rank under memory pressure. Lower = evicted first.
    #[inline(always)]
    pub const fn eviction_rank(self) -> u8 {
        match self {
            Self::Discardable => 0,
            Self::Speculative => 1,
            Self::PriorAuthority => 2,
            Self::NearFuture => 3,
            Self::Critical => 4,
        }
    }
}

/// What kind of process produced the carrier payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum SourceKind {
    /// Created directly by a human.
    HumanAuthored = 0,
    /// Generated by ahead-of-time compilation.
    AOTCompiled = 1,
    /// Candidate output from an LLM.
    LLMCandidate = 2,
    /// External world event.
    WorldEvent = 3,
    /// Event from the ledger system.
    LedgerEvent = 4,
    /// Photometric measurement (color, brightness).
    PhotometricSample = 5,
    /// Voice characteristics or fingerprint.
    VocalFingerprint = 6,
    /// Chroma (color) information sample.
    ChromaSample = 7,
    /// General audio-telemetry sample (meter readings, limiter breach,
    /// clipping, underrun) — the cross-sensor correlation the vision judge
    /// needs; without it the judge stays deaf.
    AudioSample = 8,
}

impl SourceKind {
    /// Every source kind in discriminant order. The variant list has one home,
    /// so a downstream crate digesting the wire contract cannot hold a stale copy.
    pub const ALL: [SourceKind; 9] = [
        SourceKind::HumanAuthored,
        SourceKind::AOTCompiled,
        SourceKind::LLMCandidate,
        SourceKind::WorldEvent,
        SourceKind::LedgerEvent,
        SourceKind::PhotometricSample,
        SourceKind::VocalFingerprint,
        SourceKind::ChromaSample,
        SourceKind::AudioSample,
    ];

    /// Discriminant value for wire encoding.
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// What kind of pack the carrier is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum CarrierKind {
    /// Dialogue or conversation data.
    DialoguePack = 0,
    /// Voice data.
    VoicePack = 1,
    /// General audio data.
    AudioPack = 2,
    /// Chroma (color) data.
    ChromaPack = 3,
    /// User interface data.
    UiPack = 4,
    /// Evidence or evaluation data.
    EvidencePack = 5,
    /// World state or environment data.
    WorldstatePack = 6,
    /// Agent data.
    AgentPack = 7,
    /// Method or procedure data.
    MethodPack = 8,
    /// Cue or trigger data.
    CuePack = 9,
    /// MIDI 2.0 UMP authority-ticket pack. Append-only; do not reorder.
    UmpTicketPack = 10,
    /// Semantic-bus carrier — provenance tickets stamped for UI/Studio events
    /// routed through the dispatcher + authority-ledger spine.
    CarrierPack = 11,
    /// A source-tree file snapshot: the carrier the VCS stamps per commit.
    /// Append-only; do not reorder.
    SourceFilePack = 12,
}

impl CarrierKind {
    /// Every pack kind in discriminant order. One home for the variant list.
    pub const ALL: [CarrierKind; 13] = [
        CarrierKind::DialoguePack,
        CarrierKind::VoicePack,
        CarrierKind::AudioPack,
        CarrierKind::ChromaPack,
        CarrierKind::UiPack,
        CarrierKind::EvidencePack,
        CarrierKind::WorldstatePack,
        CarrierKind::AgentPack,
        CarrierKind::MethodPack,
        CarrierKind::CuePack,
        CarrierKind::UmpTicketPack,
        CarrierKind::CarrierPack,
        CarrierKind::SourceFilePack,
    ];

    /// Discriminant value for wire encoding.
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Decode a stored kind. `None` past the last variant — a 14th pack is
    /// corruption, not an extension.
    #[inline(always)]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::DialoguePack),
            1 => Some(Self::VoicePack),
            2 => Some(Self::AudioPack),
            3 => Some(Self::ChromaPack),
            4 => Some(Self::UiPack),
            5 => Some(Self::EvidencePack),
            6 => Some(Self::WorldstatePack),
            7 => Some(Self::AgentPack),
            8 => Some(Self::MethodPack),
            9 => Some(Self::CuePack),
            10 => Some(Self::UmpTicketPack),
            11 => Some(Self::CarrierPack),
            12 => Some(Self::SourceFilePack),
            _ => None,
        }
    }
}

/// What kind of receipt the [`AuthorityTicket`] is issuing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ReceiptKind {
    /// From source origin.
    Source = 0,
    /// From compilation.
    Compile = 1,
    /// From merge operation.
    Merge = 2,
    /// From promotion operation.
    Promote = 3,
    /// From evidence or evaluation.
    Evidence = 4,
    /// Emitted when ledger eviction crypto-shreds a stale ticket — it makes
    /// erasure visible, so a forgotten artifact is never a silent vanish.
    /// Append-only; do not reorder.
    Forgotten = 5,
}

impl ReceiptKind {
    /// Every receipt kind in discriminant order. One home for the variant list.
    pub const ALL: [ReceiptKind; 6] = [
        ReceiptKind::Source,
        ReceiptKind::Compile,
        ReceiptKind::Merge,
        ReceiptKind::Promote,
        ReceiptKind::Evidence,
        ReceiptKind::Forgotten,
    ];

    /// Discriminant value for wire encoding.
    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Truncated blake3 content hash — first 8 bytes of the digest, little-endian.
///
/// Stable wire format: byte order must not change across schema versions,
/// because replay tools depend on bit-for-bit equality.
///
/// **This crate cannot construct one from bytes.** Truncating blake3 to 64 bits
/// is fine for in-process provenance, equality and ordering, but it is not a
/// cryptographic primitive — collision resistance at `u64` is only ~2^32
/// birthday-bounded — and blake3 is firewalled out of Crate Zero regardless.
/// `of()` and `combine()` live in `forge-vcs-v3`. Here it is a carrier and a
/// comparison, nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BrutalHash(/// The truncated blake3 digest as a little-endian 64-bit value.
pub u64);

impl BrutalHash {
    /// Sentinel zero hash. The "no parent" / "unset" marker in lineage chains.
    pub const ZERO: Self = Self(0);

    /// Extracts the contained 64-bit value.
    #[inline(always)]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// True for the unset marker. A zero hash is never a real digest here: it
    /// is the terminator, exactly as [`crate::soul::SoulId::ROOT`] is.
    #[inline(always)]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Validation failure for a [`CarrierHeader`]. Typed, never a string — callers
/// match the variant instead of parsing a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CarrierHeaderError {
    /// Compiler or tool version string is empty.
    MissingCompilerVersion,
    /// Schema version is zero (must be non-zero).
    SchemaVersionZero,
    /// No parent_hash or source_hashes provided.
    MissingLineage,
}

/// Lineage and identity metadata for a carrier.
///
/// At minimum a header must have a non-zero `schema_version`, a non-empty
/// `compiler_version`, and either a `parent_hash` OR at least one
/// `source_hash` — every commit must trace back to something.
///
/// `#[repr(C)]` is not inherited from the port; it is added here so the field
/// order is a locked contract rather than a compiler choice. The offsets below
/// are what make a reordering fail `cargo check` instead of review.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C)]
pub struct CarrierHeader {
    /// Type of carrier this header describes.
    pub carrier_kind: CarrierKind,
    /// Wire protocol schema version (must be non-zero).
    pub schema_version: u32,
    /// Name or version of the compiler or tool that created this carrier.
    pub compiler_version: String,
    /// Hash of the parent commit, if any.
    pub parent_hash: Option<BrutalHash>,
    /// Hashes of source files or dependencies.
    pub source_hashes: Vec<BrutalHash>,
}

impl CarrierHeader {
    /// Reject malformed headers at the airlock. Errors are deterministic.
    pub fn validate(&self) -> Result<(), CarrierHeaderError> {
        if self.schema_version == 0 {
            return Err(CarrierHeaderError::SchemaVersionZero);
        }
        if self.compiler_version.is_empty() {
            return Err(CarrierHeaderError::MissingCompilerVersion);
        }
        if self.parent_hash.is_none() && self.source_hashes.is_empty() {
            return Err(CarrierHeaderError::MissingLineage);
        }
        Ok(())
    }
}

/// Per-carrier authority + lineage receipt. The atomic unit of provenance in
/// the spine.
///
/// Field order is load-bearing: `forge-vcs-v3`'s `deterministic_hash`
/// concatenates these in declaration order, so reordering them changes every
/// receipt id ever minted. `#[repr(C)]` plus the offset locks below make that
/// reordering a compile error here, one crate *below* the code that would
/// silently produce different hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C)]
pub struct AuthorityTicket {
    /// Truncated blake3 hash of the carrier payload.
    pub carrier_hash: BrutalHash,
    /// Lineage and identity metadata.
    pub header: CarrierHeader,
    /// Commit class and urgency tier.
    pub lane: Lane,
    /// Origin of the payload.
    pub source_kind: SourceKind,
    /// Type of receipt being issued.
    pub receipt_kind: ReceiptKind,
}

impl AuthorityTicket {
    /// Delegates to [`CarrierHeader::validate`].
    pub fn validate(&self) -> Result<(), CarrierHeaderError> {
        self.header.validate()
    }
}

// ---------------------------------------------------------------------------
// LAYOUT LOCKS
// ---------------------------------------------------------------------------
// Every number below was emitted by `rustc` before it was written down.
const _: () = assert!(core::mem::size_of::<Trit>() == 1);
const _: () = assert!(core::mem::size_of::<Lane>() == 1);
const _: () = assert!(core::mem::size_of::<SourceKind>() == 1);
const _: () = assert!(core::mem::size_of::<CarrierKind>() == 1);
const _: () = assert!(core::mem::size_of::<ReceiptKind>() == 1);
const _: () = assert!(core::mem::size_of::<CarrierHeaderError>() == 1);
const _: () = assert!(core::mem::size_of::<BrutalHash>() == 8);
const _: () = assert!(core::mem::align_of::<BrutalHash>() == 8);
// `String`/`Vec` are 24 B on 64-bit, 12 B on 32-bit (wasm32); the pins that see
// them carry both numbers, each emitted by rustc for its target.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::size_of::<CarrierHeader>() == 72);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::size_of::<CarrierHeader>() == 56);
const _: () = assert!(core::mem::align_of::<CarrierHeader>() == 8);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::size_of::<AuthorityTicket>() == 88);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::size_of::<AuthorityTicket>() == 72);
const _: () = assert!(core::mem::align_of::<AuthorityTicket>() == 8);

// `BrutalHash` has no niche, so `Option` pays a full word for the tag rather
// than folding `None` into a spare bit pattern. Locked because it is the reason
// `parent_hash` is an `Option` and not a `BrutalHash::ZERO` sentinel: the
// Option costs 8 bytes and buys a state the type system checks.
const _: () = assert!(core::mem::size_of::<Option<BrutalHash>>() == 16);
const _: () = assert!(core::mem::size_of::<Option<BrutalHash>>() == 2 * core::mem::size_of::<BrutalHash>());

// ---------------------------------------------------------------------------
// OFFSET LOCKS
// ---------------------------------------------------------------------------
// `size_of` alone is weak wherever a struct has padding to absorb a widened
// field, and both of these have padding. The offsets are what pin the order.
const _: () = assert!(core::mem::offset_of!(CarrierHeader, carrier_kind) == 0);
const _: () = assert!(core::mem::offset_of!(CarrierHeader, schema_version) == 4);
const _: () = assert!(core::mem::offset_of!(CarrierHeader, compiler_version) == 8);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::offset_of!(CarrierHeader, parent_hash) == 32);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::offset_of!(CarrierHeader, source_hashes) == 48);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::offset_of!(CarrierHeader, parent_hash) == 24);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::offset_of!(CarrierHeader, source_hashes) == 40);

const _: () = assert!(core::mem::offset_of!(AuthorityTicket, carrier_hash) == 0);
const _: () = assert!(core::mem::offset_of!(AuthorityTicket, header) == 8);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::offset_of!(AuthorityTicket, lane) == 80);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::offset_of!(AuthorityTicket, source_kind) == 81);
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::offset_of!(AuthorityTicket, receipt_kind) == 82);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::offset_of!(AuthorityTicket, lane) == 64);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::offset_of!(AuthorityTicket, source_kind) == 65);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(core::mem::offset_of!(AuthorityTicket, receipt_kind) == 66);

// The three enum bytes are ADJACENT and last. This is the shape the handoff
// settled on when `append_global` was dropped: the tape line widens by exactly
// these three columns. A padding byte appearing between them would mean the
// widening is not a contiguous read.
const _: () = assert!(
    core::mem::offset_of!(AuthorityTicket, source_kind)
        == core::mem::offset_of!(AuthorityTicket, lane) + 1
);
const _: () = assert!(
    core::mem::offset_of!(AuthorityTicket, receipt_kind)
        == core::mem::offset_of!(AuthorityTicket, source_kind) + 1
);

// Padding is accounted for rather than tolerated: whatever the fields do not
// occupy is the hole a future field could hide in, so it is named.
// 64-bit: 3 (after carrier_kind). 32-bit: 3 + 4 (before the 8-aligned Option) + 4 (tail).
#[cfg(target_pointer_width = "64")]
const CARRIER_HEADER_PADDING: usize = 3;
#[cfg(target_pointer_width = "32")]
const CARRIER_HEADER_PADDING: usize = 11;
const _: () = assert!(
    core::mem::size_of::<CarrierKind>()
        + core::mem::size_of::<u32>()
        + core::mem::size_of::<String>()
        + core::mem::size_of::<Option<BrutalHash>>()
        + core::mem::size_of::<Vec<BrutalHash>>()
        + CARRIER_HEADER_PADDING
        == core::mem::size_of::<CarrierHeader>()
);
const _: () = assert!(
    core::mem::offset_of!(AuthorityTicket, receipt_kind)
        + core::mem::size_of::<ReceiptKind>()
        + 5
        == core::mem::size_of::<AuthorityTicket>()
);

// ---------------------------------------------------------------------------
// DISCRIMINANT LOCKS
// ---------------------------------------------------------------------------
// Discriminants are the wire format. Reordering a variant silently rewrites
// every replay log ever produced, so it fails the build instead.
const _: () = assert!(Lane::Critical.as_u8() == 0);
const _: () = assert!(Lane::NearFuture.as_u8() == 1);
const _: () = assert!(Lane::PriorAuthority.as_u8() == 2);
const _: () = assert!(Lane::Speculative.as_u8() == 3);
const _: () = assert!(Lane::Discardable.as_u8() == 4);
const _: () = assert!(Lane::ALL.len() == 5);

const _: () = assert!(SourceKind::HumanAuthored.as_u8() == 0);
const _: () = assert!(SourceKind::AOTCompiled.as_u8() == 1);
const _: () = assert!(SourceKind::LLMCandidate.as_u8() == 2);
const _: () = assert!(SourceKind::WorldEvent.as_u8() == 3);
const _: () = assert!(SourceKind::LedgerEvent.as_u8() == 4);
const _: () = assert!(SourceKind::PhotometricSample.as_u8() == 5);
const _: () = assert!(SourceKind::VocalFingerprint.as_u8() == 6);
const _: () = assert!(SourceKind::ChromaSample.as_u8() == 7);
const _: () = assert!(SourceKind::AudioSample.as_u8() == 8);

const _: () = assert!(CarrierKind::DialoguePack.as_u8() == 0);
const _: () = assert!(CarrierKind::CuePack.as_u8() == 9);
const _: () = assert!(CarrierKind::UmpTicketPack.as_u8() == 10);
const _: () = assert!(CarrierKind::CarrierPack.as_u8() == 11);
const _: () = assert!(CarrierKind::SourceFilePack.as_u8() == 12);

const _: () = assert!(ReceiptKind::Source.as_u8() == 0);
const _: () = assert!(ReceiptKind::Compile.as_u8() == 1);
const _: () = assert!(ReceiptKind::Merge.as_u8() == 2);
const _: () = assert!(ReceiptKind::Promote.as_u8() == 3);
const _: () = assert!(ReceiptKind::Evidence.as_u8() == 4);
const _: () = assert!(ReceiptKind::Forgotten.as_u8() == 5);

// Each `ALL` is complete and in discriminant order — `ALL[i].as_u8() == i`. A
// variant appended to the enum but not to `ALL` leaves a hole the downstream
// layout digest would silently hash around, so the hole fails the build here.
const _: () = assert!(SourceKind::ALL.len() == 9);
const _: () = assert!(CarrierKind::ALL.len() == 13);
const _: () = assert!(ReceiptKind::ALL.len() == 6);
const _: () = {
    let mut i = 0;
    while i < Lane::ALL.len() {
        assert!(Lane::ALL[i].as_u8() as usize == i);
        i += 1;
    }
    let mut i = 0;
    while i < SourceKind::ALL.len() {
        assert!(SourceKind::ALL[i].as_u8() as usize == i);
        i += 1;
    }
    let mut i = 0;
    while i < CarrierKind::ALL.len() {
        assert!(CarrierKind::ALL[i].as_u8() as usize == i);
        i += 1;
    }
    let mut i = 0;
    while i < ReceiptKind::ALL.len() {
        assert!(ReceiptKind::ALL[i].as_u8() as usize == i);
        i += 1;
    }
};

// ---------------------------------------------------------------------------
// THE TRIT IS THE SAME TRIT
// ---------------------------------------------------------------------------
// `Trit` and the lattice both claim balanced ternary. If the two shifts ever
// disagree, a verdict packed by one and unpacked by the other flips meaning.
// Proven digit by digit against `TritCell5D::from_trits`, whose other four
// digits are pinned to `-1` so they contribute nothing.
const _: () = assert!(Trit::ALL.len() == crate::atom::RADIX as usize);
const _: () = assert!(
    crate::atom::TritCell5D::from_trits([Trit::Fault as i8, -1, -1, -1, -1]).0
        == Trit::Fault.shifted()
);
const _: () = assert!(
    crate::atom::TritCell5D::from_trits([Trit::Intent as i8, -1, -1, -1, -1]).0
        == Trit::Intent.shifted()
);
const _: () = assert!(
    crate::atom::TritCell5D::from_trits([Trit::Sealed as i8, -1, -1, -1, -1]).0
        == Trit::Sealed.shifted()
);

// POSIX owns zero. The balanced value can never be the exit code, and this is
// the assert that stops someone "simplifying" `exit_code` into `self as i32`.
const _: () = assert!(Trit::Sealed.exit_code() == 0);
const _: () = assert!(Trit::Sealed as i8 != 0);

/// The raw UMP wire packet — the 128-bit MIDI 2.0 Universal MIDI Packet, POD-safe
/// for sieve/GPU lanes. Relocated down from `forge-ump-v3` 2026-08-14 (Sean
/// "we brought bytemuck already" — L19 dep-grab already cleared bytemuck's bar for
/// 8 other v3 crates), completing the same "go to primitives / forge-core floor"
/// relocation v2 did on 2026-07-12. `forge-ump-v3::packet` re-exports this module
/// so every `forge_ump_v3::{Ump, Channel, Group, Stamped}` / `forge_ump_v3::packet::…`
/// path stays unchanged for its consumers.
pub mod packet {
    /// A Universal MIDI Packet normalized to four 32-bit little-nistam-decoded words
    /// (nistam = Cree "first": least-significant byte FIRST — see
    /// [`crate::sprite_blob::u32_to_nistam`] for the shared v3 wire-order law; no
    /// foreign byte-order term belongs on this path).
    ///
    /// Shorter packets keep unused trailing words at zero. The struct is exactly
    /// 16 bytes so it can be shared with sieve/GPU-style lanes without heap data.
    #[repr(C)]
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct Ump {
        /// The four 32-bit words of the packet.
        pub words: [u32; 4],
    }

    impl Ump {
        /// Build a packet from its four 32-bit words.
        #[inline]
        pub const fn new(words: [u32; 4]) -> Self {
            Self { words }
        }

        /// Message type — top nibble of word 0.
        #[inline]
        pub const fn mt(self) -> u8 {
            ((self.words[0] >> 28) & 0x0f) as u8
        }

        /// Group nibble of word 0.
        #[inline]
        pub const fn group(self) -> Group {
            Group(((self.words[0] >> 24) & 0x0f) as u8)
        }

        /// Status nibble of word 0.
        #[inline]
        pub const fn status(self) -> u8 {
            ((self.words[0] >> 20) & 0x0f) as u8
        }

        /// Build a MIDI 2.0 Channel Voice 2 Note On packet (message type
        /// `0x4`, status `0x9` — the spec-defined encoding, not a repo
        /// convention). Only words `[0..2)` carry data; `[2..4)` stay zero to
        /// fill this type's fixed 4-word width. `note` is masked to 7 bits
        /// (MIDI note range), `group`/`channel` to their 4-bit nibbles.
        ///
        /// Floor-tier home (aspire.rs `mutate5d-ump-cue-port`): pure integer
        /// bit-packing per the MIDI 2.0 spec belongs once, here, next to the
        /// `Ump` wire type itself — not re-derived per consumer crate.
        #[inline]
        pub const fn note_on(group: Group, channel: Channel, note: u8, velocity: u16) -> Self {
            const MT_CHANNEL_VOICE_2: u32 = 0x4;
            const ST_NOTE_ON: u32 = 0x9;
            let w0 = (MT_CHANNEL_VOICE_2 << 28)
                | ((group.0 as u32 & 0xF) << 24)
                | (ST_NOTE_ON << 20)
                | ((channel.0 as u32 & 0xF) << 16)
                | ((note as u32 & 0x7F) << 8);
            let w1 = (velocity as u32) << 16;
            Self { words: [w0, w1, 0, 0] }
        }
    }

    /// UMP group (0..=15).
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Group(pub u8);

    /// UMP channel (0..=15).
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Channel(pub u8);

    /// A payload stamped with its universal (wall) tick, in microseconds.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Stamped<T> {
        /// The universal wall-clock tick, in microseconds.
        pub universal_tick_us: i64,
        /// The stamped payload.
        pub payload: T,
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ump_is_pod_safe() {
            fn assert_pod<T: bytemuck::Pod>() {}
            fn assert_zeroable<T: bytemuck::Zeroable>() {}
            assert_pod::<Ump>();
            assert_zeroable::<Ump>();
        }

        #[test]
        fn ump_total_size_is_16_bytes() {
            assert_eq!(core::mem::size_of::<Ump>(), 16);
            assert_eq!(core::mem::align_of::<Ump>(), 4);
        }

        #[test]
        fn note_on_encodes_channel_voice_2_note_on() {
            let ump = Ump::note_on(Group(1), Channel(3), 60, 0x8000);
            assert_eq!(ump.mt(), 0x4, "message type must be Channel Voice 2");
            assert_eq!(ump.group(), Group(1));
            assert_eq!(ump.status(), 0x9, "status must be Note On");
            assert_eq!(ump.words[2], 0, "unused word stays zero");
            assert_eq!(ump.words[3], 0, "unused word stays zero");
        }

        #[test]
        fn note_on_note_and_velocity_round_trip_through_the_words() {
            let a = Ump::note_on(Group(0), Channel(0), 60, 0x1234);
            let b = Ump::note_on(Group(0), Channel(0), 61, 0x1234);
            assert_ne!(a.words[0], b.words[0], "differing note must change word 0");
            let c = Ump::note_on(Group(0), Channel(0), 60, 0x5678);
            assert_ne!(a.words[1], c.words[1], "differing velocity must change word 1");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Trit --------------------------------------------------------------

    #[test]
    fn the_shift_round_trips_and_agrees_with_the_lattice() {
        for t in Trit::ALL {
            assert_eq!(Trit::from_shifted(t.shifted()), t, "{t:?} round-trips");
            assert!(t.shifted() < crate::atom::RADIX, "a shifted trit is one radix-3 digit");
            // Same packing the lattice uses for its low digit.
            let cell = crate::atom::TritCell5D::from_trits([t as i8, -1, -1, -1, -1]);
            assert_eq!(cell.0, t.shifted());
        }
        // Out of range reads Intent — an unknown verdict, never a silent Sealed.
        for v in 3u8..=255 {
            assert_eq!(Trit::from_shifted(v), Trit::Intent, "{v} must not decode as a verdict");
        }
    }

    #[test]
    fn the_exit_ladder_round_trips_and_usage_is_not_a_verdict() {
        for t in Trit::ALL {
            assert_eq!(Trit::from_exit(t.exit_code()), Some(t), "{t:?} round-trips");
        }
        assert_eq!(Trit::Sealed.exit_code(), 0, "POSIX nails success to 0");
        // Anything off the ladder is the absence of a reading, not a third one.
        for code in [3, 64, -1, i32::MAX] {
            assert_eq!(Trit::from_exit(code), None, "{code} is off-lattice");
        }
    }

    #[test]
    fn every_axis_has_its_own_glyph() {
        let glyphs: Vec<char> = Trit::ALL.iter().map(|t| t.glyph()).collect();
        assert_eq!(glyphs, vec!['<', '0', '>']);
        assert_eq!(
            std::collections::BTreeSet::from(['<', '0', '>']).len(),
            3,
            "pass, fail and in-flight must not share a face"
        );
    }

    // ---- Lane --------------------------------------------------------------

    #[test]
    fn every_lane_round_trips_and_nothing_else_decodes() {
        for (i, lane) in Lane::ALL.iter().enumerate() {
            assert_eq!(lane.as_u8() as usize, i);
            assert_eq!(Lane::from_u8(i as u8), Some(*lane));
        }
        for v in Lane::ALL.len() as u8..=255 {
            assert!(Lane::from_u8(v).is_none(), "{v} is not a lane");
        }
    }

    #[test]
    fn eviction_ranks_are_a_total_order_and_the_inverse_of_urgency() {
        let ranks: Vec<u8> = Lane::ALL.iter().map(|l| l.eviction_rank()).collect();
        assert_eq!(ranks, vec![4, 3, 2, 1, 0], "urgency ascends, eviction rank descends");
        assert_eq!(
            std::collections::BTreeSet::from_iter(ranks.iter()).len(),
            Lane::ALL.len(),
            "two lanes sharing a rank makes eviction non-deterministic"
        );
    }

    // ---- CarrierKind -------------------------------------------------------

    #[test]
    fn every_carrier_kind_round_trips_and_a_fourteenth_is_corruption() {
        for v in 0..=12u8 {
            let k = CarrierKind::from_u8(v).expect("variant exists");
            assert_eq!(k.as_u8(), v);
        }
        for v in 13u8..=255 {
            assert!(CarrierKind::from_u8(v).is_none(), "{v} is not a pack");
        }
    }

    // ---- CarrierHeader / AuthorityTicket -----------------------------------

    fn sample_header() -> CarrierHeader {
        CarrierHeader {
            carrier_kind: CarrierKind::CuePack,
            schema_version: 1,
            compiler_version: "spine-v3-test".to_string(),
            parent_hash: None,
            source_hashes: vec![BrutalHash(0x5115_A0A0_1234_5678), BrutalHash(9)],
        }
    }

    fn sample_ticket() -> AuthorityTicket {
        AuthorityTicket {
            carrier_hash: BrutalHash(0xDEAD_BEEF_CAFE_F00D),
            header: sample_header(),
            lane: Lane::NearFuture,
            source_kind: SourceKind::AOTCompiled,
            receipt_kind: ReceiptKind::Compile,
        }
    }

    #[test]
    fn a_header_without_a_schema_is_refused() {
        let mut h = sample_header();
        h.schema_version = 0;
        assert_eq!(h.validate(), Err(CarrierHeaderError::SchemaVersionZero));
    }

    #[test]
    fn a_header_without_a_compiler_is_refused() {
        let mut h = sample_header();
        h.compiler_version = String::new();
        assert_eq!(h.validate(), Err(CarrierHeaderError::MissingCompilerVersion));
    }

    #[test]
    fn every_commit_must_trace_back_to_something() {
        // Neither a parent nor a source → nothing to trace to.
        let mut orphan = sample_header();
        orphan.source_hashes.clear();
        orphan.parent_hash = None;
        assert_eq!(orphan.validate(), Err(CarrierHeaderError::MissingLineage));

        // A parent alone is lineage.
        let mut parented = sample_header();
        parented.source_hashes.clear();
        parented.parent_hash = Some(BrutalHash(7));
        assert!(parented.validate().is_ok());

        // Sources alone are lineage.
        assert!(sample_header().validate().is_ok());
    }

    /// `BrutalHash::ZERO` is the unset marker, and `Some(ZERO)` is NOT unset.
    /// The distinction is the whole reason `parent_hash` pays 8 bytes for an
    /// `Option` tag instead of overloading the zero value.
    #[test]
    fn a_zero_parent_hash_is_still_a_parent() {
        assert!(BrutalHash::ZERO.is_zero());
        assert_eq!(BrutalHash::ZERO.as_u64(), 0);
        assert!(!BrutalHash(1).is_zero());

        let mut h = sample_header();
        h.source_hashes.clear();
        h.parent_hash = Some(BrutalHash::ZERO);
        assert!(h.validate().is_ok(), "Some(ZERO) is a stated parent, not an absent one");

        h.parent_hash = None;
        assert_eq!(h.validate(), Err(CarrierHeaderError::MissingLineage));
    }

    #[test]
    fn the_ticket_delegates_validation_to_its_header() {
        let t = sample_ticket();
        assert!(t.validate().is_ok());

        let mut broken = sample_ticket();
        broken.header.schema_version = 0;
        assert_eq!(broken.validate(), Err(CarrierHeaderError::SchemaVersionZero));
        assert_eq!(broken.validate(), broken.header.validate(), "one home for the verdict");
    }

    /// The ticket is the hash preimage `forge-vcs-v3` will serialize, in this
    /// order. A reordering changes every receipt id ever minted, so the order
    /// is pinned by offset one crate *below* the code that would hash it.
    #[test]
    fn the_preimage_order_is_carrier_header_lane_source_receipt() {
        let o = [
            core::mem::offset_of!(AuthorityTicket, carrier_hash),
            core::mem::offset_of!(AuthorityTicket, header),
            core::mem::offset_of!(AuthorityTicket, lane),
            core::mem::offset_of!(AuthorityTicket, source_kind),
            core::mem::offset_of!(AuthorityTicket, receipt_kind),
        ];
        let mut ascending = o;
        ascending.sort_unstable();
        assert_eq!(o, ascending, "declaration order must be address order");

        // The three enum bytes the tape line widens by are one contiguous read.
        assert_eq!(o[4] - o[2], 2);
        let t = sample_ticket();
        assert_eq!(
            [t.lane.as_u8(), t.source_kind.as_u8(), t.receipt_kind.as_u8()],
            [1, 1, 1],
            "NearFuture / AOTCompiled / Compile"
        );
    }

    /// This crate carries the hash and cannot compute one. Stated as a test so
    /// the firewall is visible from the file it constrains: `BrutalHash` is
    /// constructible only from a `u64` a caller already has.
    #[test]
    fn crate_zero_can_carry_a_hash_but_not_make_one() {
        let carried = BrutalHash(0x0123_4567_89AB_CDEF);
        assert_eq!(carried.as_u64(), 0x0123_4567_89AB_CDEF);
        assert_eq!(carried, BrutalHash(carried.as_u64()), "carrier round-trip, no digest");
        assert_ne!(carried, BrutalHash::ZERO);
    }
}
