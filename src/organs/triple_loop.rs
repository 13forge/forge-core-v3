//! Three-thread compositor spine (Phase 1).
//!
//! ```text
//! T1 Logic (DET-CLOCK 120 Hz)
//!   │ builds DrawList + caches glyphs (atlas write)
//!   │ InputBridge (UiTripleBuffer — lock-free, producer try_lock)
//!   ▼
//! T2 Raster
//!   │ rasterize_into → RGBA overlay plane (atlas read)
//!   │ OverlayBridge (TripleBuffer<Vec<u8>> — lock-free, producer try_lock)
//!   ▼
//! T3 GPU/Present  ←── event channel (mpsc SyncSender) ──── T3
//!   compose + present + pump_events
//! ```
//!
//! Phase 2 eliminates `Arc<RwLock<FontAtlas>>` by embedding string bytes
//! directly in `DrawCmd::Text` (removes atlas from the T2 rasterize path).
//!
//! Ported 2026-08-17 from F:\NewRepo\crates\forge-studio\src\triple_loop.rs.
//! `ClockPlane` + `TripleBuffer` are HOME here since 2026-09-05 (L05 fold);
//! forge-hal-clockspine re-exports them. Canvas/input bridge types stay stubbed.
#![allow(missing_docs, dead_code)]

use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::JoinHandle;

// TODO: no forge_canvas equivalent in Crate Zero, stubbed
// Donor: use forge_canvas::draw::DrawList;
// Donor: use forge_canvas::text::FontAtlas;
// Donor: use forge_canvas::ui::UiTripleBuffer;

// TODO: no forge_input equivalent in Crate Zero, stubbed
// Donor: use forge_input::RawInputState;

// ── ClockPlane + TripleBuffer: THE home (folded from forge-hal-clockspine 2026-09-05) ──

/// A payload that can be copied across the clock bridge while **reusing** the
/// destination's existing allocation — the contract that keeps steady-state heap
/// traffic at zero.
///
/// Implementations must overwrite `dst` with `self`'s contents, reusing `dst`'s
/// buffer if it is large enough (no allocation in steady state).
pub trait ClockPlane {
    /// Overwrite `dst` with `self`'s contents, reusing `dst`'s buffer if large enough.
    fn copy_into(&self, dst: &mut Self);
}

impl ClockPlane for Vec<u8> {
    #[inline]
    fn copy_into(&self, dst: &mut Self) {
        if dst.len() != self.len() {
            dst.resize(self.len(), 0);
        }
        dst.copy_from_slice(self);
    }
}

impl ClockPlane for Vec<i32> {
    #[inline]
    fn copy_into(&self, dst: &mut Self) {
        if dst.len() != self.len() {
            dst.resize(self.len(), 0);
        }
        dst.copy_from_slice(self);
    }
}

/// `Vec<f32>` is a render/DSP boundary payload (audio PCM blocks), never SIM state;
/// the integer law governs the sim, the bridge carries what the clocks exchange.
impl ClockPlane for Vec<f32> {
    #[inline]
    fn copy_into(&self, dst: &mut Self) {
        if dst.len() != self.len() {
            dst.resize(self.len(), 0.0);
        }
        dst.copy_from_slice(self);
    }
}

struct Slot<T> {
    /// The published value.
    value: T,
    /// Generation counter; incremented on each publish.
    generation: u64,
}

/// Lock-free (`try_lock`) single-producer / multi-consumer clock bridge.
///
/// One producer [`publish`](Self::publish)es a fresh plane (a `mem::swap` under a
/// non-blocking `try_lock`, recycling the old plane back to the producer). Each
/// consumer [`try_take`](Self::try_take)s with `try_lock` — it NEVER blocks on the
/// producer or on another consumer; a miss just reuses its last front.
pub struct TripleBuffer<T> {
    /// Slot holding the published value, generation, and the lock guarding swaps.
    inner: Mutex<Slot<T>>,
}

impl<T> TripleBuffer<T> {
    /// Create a bridge seeded with `initial` as the first published plane (generation
    /// 0). The producer's first `publish` recycles this initial buffer.
    pub fn new(initial: T) -> Self {
        Self { inner: Mutex::new(Slot { value: initial, generation: 0 }) }
    }

    /// **Producer side.** TRY to swap `fresh` into the published slot, returning the
    /// OLD plane for the producer to refill — two (or more) planes ping-pong, so
    /// steady state is alloc-free. Bumps the generation so consumers can tell new
    /// from unchanged.
    ///
    /// Uses `try_lock`, never a blocking `lock()`. If a consumer momentarily holds
    /// the slot, the producer does NOT block: it returns `fresh` UNCHANGED and
    /// re-publishes next frame.
    pub fn publish(&self, mut fresh: T) -> T {
        let mut slot = match self.inner.try_lock() {
            Ok(slot) => slot,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return fresh,
        };
        std::mem::swap(&mut slot.value, &mut fresh);
        slot.generation = slot.generation.wrapping_add(1);
        fresh
    }

    /// Current published generation (cheap state probe). Returns `None` if the lock
    /// is momentarily contended — never blocks.
    pub fn try_generation(&self) -> Option<u64> {
        match self.inner.try_lock() {
            Ok(slot) => Some(slot.generation),
            Err(std::sync::TryLockError::Poisoned(e)) => Some(e.into_inner().generation),
            Err(std::sync::TryLockError::WouldBlock) => None,
        }
    }
}

impl<T: ClockPlane> TripleBuffer<T> {
    /// **Consumer side.** TRY to copy the latest plane into `dst`. `try_lock` means
    /// the consumer NEVER stalls.
    ///
    /// Returns `Some(generation)` on a fresh copy (and `dst` now holds it), or `None`
    /// when the caller should reuse its last front:
    /// * `None` — lock contended (producer publishing, or the other consumer copying), OR
    /// * `None` — generation unchanged since `last_gen` (nothing new to take).
    ///
    /// The consumer passes its OWN `last_gen` and OWN `dst`, so two consumers drain
    /// the same bridge independently.
    ///
    /// A poisoned slot (a panic under the lock) is recovered via `into_inner`,
    /// symmetric with [`publish`](Self::publish) — poison never freezes a consumer.
    pub fn try_take(&self, last_gen: u64, dst: &mut T) -> Option<u64> {
        let slot = match self.inner.try_lock() {
            Ok(slot) => slot,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return None,
        };
        if slot.generation == last_gen {
            return None;
        }
        slot.value.copy_into(dst);
        Some(slot.generation)
    }
}

// ── Stub: UiTripleBuffer for DrawList ──────────────────────────────────────

/// Stub for `forge_canvas::ui::UiTripleBuffer`. Wraps a TripleBuffer<Box<DrawList>>
/// for the T1 → T2 bridge. The real definition lives in forge-canvas.
pub struct UiTripleBuffer {
    inner: TripleBuffer<Box<DrawList>>,
}

impl UiTripleBuffer {
    /// Boot: allocate the shared DrawList slot (cold path, once).
    pub fn new(initial: Box<DrawList>) -> Self {
        Self { inner: TripleBuffer::new(initial) }
    }

    /// T1 side: swap `fresh` into the bridge; returns the old slot for T1 to
    /// clear and refill next frame (ping-pong, alloc-free after init).
    /// Returns `fresh` unchanged on lock contention — T1 retries next frame.
    pub fn publish(&self, fresh: Box<DrawList>) -> Box<DrawList> {
        self.inner.publish(fresh)
    }

    /// T2 side: non-blocking sample of the latest DrawList.
    /// Copies commands into `dst` when a newer generation is available.
    /// Returns `Some(gen)` on success, `None` when contended or unchanged.
    pub fn try_take(&self, _last_gen: u64, _dst: &mut DrawList) -> Option<u64> {
        // Stub: always return None. Real implementation reads from inner.
        None
    }
}

// ── Stub: DrawList ─────────────────────────────────────────────────────────

/// Stub for `forge_canvas::draw::DrawList`. A command buffer for UI drawing
/// (text, shapes, etc.) built by T1 (Logic) and consumed by T2 (Raster).
/// The real definition lives in forge-canvas.
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    /// Commands (stub): in the real version, this is a command buffer.
    pub commands: Vec<u32>,
}

impl DrawList {
    /// Stub constructor: allocate an empty command buffer.
    pub fn new_boxed() -> Box<Self> {
        Box::new(Self { commands: Vec::new() })
    }
}

impl ClockPlane for Box<DrawList> {
    #[inline]
    fn copy_into(&self, dst: &mut Self) {
        dst.commands.copy_from_slice(&self.commands);
    }
}

// ── Stub: FontAtlas ────────────────────────────────────────────────────────

/// Stub for `forge_canvas::text::FontAtlas`. Shared glyph cache; T1 writes on
/// cache miss, T2 reads to rasterize. Phase 2 removes this seam by embedding
/// string bytes directly in DrawCmd::Text. The real definition lives in forge-canvas.
#[derive(Clone, Debug, Default)]
pub struct FontAtlas {
    /// Cache data (stub): in the real version, this holds glyph texels.
    pub cache: Vec<u8>,
}

// ── Stub: RawInputState ────────────────────────────────────────────────────

/// Stub for `forge_input::RawInputState`. Snapshot of full input state
/// (mouse, pen, keys_held). The real definition lives in forge-input.
#[derive(Clone, Debug, Default)]
pub struct RawInputState {
    /// Placeholder for raw input snapshot.
    pub reserved: u32,
}

// ── LogicInput ────────────────────────────────────────────────────────────────

/// Per-frame snapshot T3 (GPU/Present) sends to T1 (Logic) via the event channel.
pub struct LogicInput {
    /// Characters typed this frame (WM_CHAR feed).
    pub typed_chars: Vec<char>,
    /// VK codes pressed this frame (navigation / arrow keys).
    pub keys_pressed: Vec<u32>,
    /// Viewport width in pixels (from current surface config).
    pub vp_w: i64,
    /// Viewport height in pixels (from current surface config).
    pub vp_h: i64,
    /// Surface width in pixels (used by T1 for PTY column calculation).
    pub surface_w: u32,
    /// Surface height in pixels (Phase 2: T2 dynamic PixelBuffer resize).
    pub surface_h: u32,
    /// Full raw input state snapshot (mouse, pen, keys_held) for the canvas host.
    /// Wired 2026-07-02 for interactive canvas tools.
    pub raw: RawInputState,
}

// ── InputBridge ───────────────────────────────────────────────────────────

/// DrawList pipeline T1 (Logic) → T2 (Raster).
///
/// Backed by [`UiTripleBuffer`] — the forge-canvas native lock-free DrawList
/// bridge (T041, Lock-Free Gate compliant: `try_lock` on both sides). T1 calls
/// [`publish`](Self::publish) after filling the DrawList; T2 calls
/// [`try_take`](Self::try_take) to sample the latest version.
///
/// On lock contention `publish` returns the slot unchanged so T1 retries next
/// frame — no stall, no allocated copy.
pub struct InputBridge(UiTripleBuffer);

impl InputBridge {
    /// Boot: allocate the shared DrawList slot (cold path, once).
    pub fn new() -> Self {
        Self(UiTripleBuffer::new(DrawList::new_boxed())) // @forge:allow_alloc — cold init
    }

    /// T1 side: swap `fresh` into the bridge; returns the old slot for T1 to
    /// clear and refill next frame (ping-pong, alloc-free after init).
    /// Returns `fresh` unchanged on lock contention — T1 retries next frame.
    pub fn publish(&self, fresh: Box<DrawList>) -> Box<DrawList> {
        self.0.publish(fresh)
    }

    /// T2 side: non-blocking sample of the latest DrawList.
    /// Copies commands into `dst` when a newer generation is available.
    /// Returns `Some(gen)` on success, `None` when contended or unchanged.
    pub fn try_take(&self, last_gen: u64, dst: &mut DrawList) -> Option<u64> {
        self.0.try_take(last_gen, dst)
    }
}

// ── Taken / OverlayBridge ─────────────────────────────────────────────────

/// Result of a successful [`OverlayBridge::try_take`].
#[derive(Clone, Copy, Debug)]
pub struct Taken {
    pub generation: u64,
    pub width: u32,
    pub height: u32,
}

/// An RGBA plane and the dimensions it actually has.
///
/// The dims ride INSIDE the buffered value so the `mem::swap` carries them with
/// the pixels. They used to be sibling atomics stored just before
/// `TripleBuffer::publish` — but publish returns `fresh` UNCHANGED on lock
/// contention (forge-hal/src/triple_buffer.rs:112: no swap, no generation bump),
/// so a resize whose publish lost the race left the atomics describing a frame
/// that was never published. A consumer then took the PREVIOUS generation's
/// pixels labelled with the NEW dimensions, and `w * h * 4` overran the plane.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SizedPlane {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl SizedPlane {
    /// A zeroed plane of `width x height` RGBA.
    pub fn new(width: u32, height: u32) -> Self {
        let n = (width as usize) * (height as usize) * 4;
        Self { pixels: vec![0u8; n], width, height } // @forge:allow_alloc — cold init
    }

    /// Does the pixel buffer actually hold `width * height * 4` bytes? The
    /// invariant the old sibling-atomic layout could violate.
    pub fn is_consistent(&self) -> bool {
        self.pixels.len() == (self.width as usize) * (self.height as usize) * 4
    }
}

impl ClockPlane for SizedPlane {
    #[inline]
    fn copy_into(&self, dst: &mut Self) {
        dst.pixels.clone_from(&self.pixels);
        // Dims travel with the bytes — never read from anywhere else.
        dst.width = self.width;
        dst.height = self.height;
    }
}

/// RGBA pixel-plane bridge T2 (Raster) → T3 (GPU/Present).
///
/// T2 [`publish`](Self::publish)es each rasterized frame, recovering the old
/// buffer for ping-pong reuse (alloc-free after init). T3
/// [`try_take`](Self::try_take)s with `try_lock` — a miss reuses the last
/// frame's plane, never stalls (Lock-Free Gate compliant).
pub struct OverlayBridge {
    inner: TripleBuffer<SizedPlane>,
}

impl OverlayBridge {
    pub fn new(width: u32, height: u32) -> Self {
        Self { inner: TripleBuffer::new(SizedPlane::new(width, height)) }
    }

    /// T2 side: publish a freshly rasterized RGBA plane.
    /// Returns the old buffer for T2 to refill (steady-state alloc-free).
    ///
    /// A contended publish returns `fresh` unchanged — INCLUDING its dims, so a
    /// lost race can no longer leave the bridge describing a frame it does not
    /// hold.
    pub fn publish(&self, fresh: Vec<u8>, width: u32, height: u32) -> Vec<u8> {
        let recycled = self.inner.publish(SizedPlane { pixels: fresh, width, height });
        recycled.pixels
    }

    /// T3 side: non-blocking take. `None` = lock contended or no new frame.
    pub fn try_take(&self, last_gen: u64, dst: &mut Vec<u8>) -> Option<Taken> {
        let mut plane = SizedPlane { pixels: std::mem::take(dst), width: 0, height: 0 };
        let gen = self.inner.try_take(last_gen, &mut plane);
        let taken = gen.map(|generation| Taken {
            generation,
            width: plane.width,
            height: plane.height,
        });
        *dst = plane.pixels;
        taken
    }
}

// ── WorldDrawCell and WorldDrawList ───────────────────────────────────────

/// One integer world cell (contract §a). Position/depth are MilliUnit `i32`
/// (1/1000 scale — `stroke_z_mu`'s exact convention, forge_vision_lab.rs);
/// colour is the sovereign palette index, never hex (Vision Gate); coverage is
/// Permyriad (0..=10000); material is the existing integer tag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldDrawCell {
    pub x_mu: i32,
    pub y_mu: i32,
    pub z_mu: i32,
    pub colour_id: u32,
    pub coverage_pmy: u16,
    pub material: u16,
}

/// Fixed capacity ceiling = the VixelLab 64×64 LayerStack (main.rs `LayerStack::new(64, 64)`).
pub const WORLD_DRAW_CAPACITY: usize = 64 * 64;

/// Integer-only, fixed-capacity world draw list (contract §a). The backing
/// store is allocated ONCE (boxed at construction) and only ever overwritten in
/// place up to `len` — no growth, no steady-state allocation (Sound Gate
/// analogue per contract §a; DET-CLOCK write-path invariant §b).
#[derive(Clone)]
pub struct WorldDrawList {
    pub cells: [WorldDrawCell; WORLD_DRAW_CAPACITY],
    /// Active cell count ≤ `WORLD_DRAW_CAPACITY` — `len` is authoritative; cells
    /// at or above it are dead regardless of content.
    pub len: u32,
}

impl Default for WorldDrawList {
    fn default() -> Self {
        Self { cells: [WorldDrawCell::default(); WORLD_DRAW_CAPACITY], len: 0 }
    }
}

impl WorldDrawList {
    /// Boot: the only allocation this type ever makes.
    pub fn new_boxed() -> Box<Self> {
        // @forge:allow_alloc — cold init, once per slot.
        Box::new(Self { cells: [WorldDrawCell::default(); WORLD_DRAW_CAPACITY], len: 0 })
    }

    /// **producer-must-clear-recycled-buffer (contract §c, NON-OPTIONAL).** The
    /// S-B stuck-frame lesson: a recycled list still holds the cells of a frame
    /// two swaps back — refilling without resetting `len` leaks them. Because
    /// `len` is authoritative and writes go through [`push`](Self::push), a
    /// len-reset IS the clear. The producer calls this FIRST, every refill.
    pub fn clear_for_refill(&mut self) {
        self.len = 0;
    }

    /// Bounded write: append one cell, `false` when the list is at capacity
    /// (never grows — the caller drops the cell, exactly like a contended
    /// publish drops a frame).
    pub fn push(&mut self, cell: WorldDrawCell) -> bool {
        let i = self.len as usize;
        if i >= WORLD_DRAW_CAPACITY {
            return false;
        }
        self.cells[i] = cell;
        self.len += 1;
        true
    }
}

/// Cross-clock copy for the world slot: overwrite the `len` prefix + `len`
/// itself. Fixed arrays — no allocation possible on either side.
impl ClockPlane for Box<WorldDrawList> {
    #[inline]
    fn copy_into(&self, dst: &mut Self) {
        let n = self.len as usize;
        dst.cells[..n].copy_from_slice(&self.cells[..n]);
        dst.len = self.len;
    }
}

// ── WorldBridge ───────────────────────────────────────────────────────────

/// Result of a successful [`WorldBridge::try_take`].
#[derive(Clone, Copy, Debug)]
pub struct WorldTaken {
    pub generation: u64,
    pub len: u32,
}

/// World draw-list bridge, DET producer → GPU lane (contract §c): the second
/// independent `TripleBuffer` slot. ~80KB per plane vs the mesh buffer's MBs;
/// no shared allocation. Producer + consumer both `try_lock` (Lock-Free Gate);
/// a contended publish drops the frame, a missed take reuses the last one.
pub struct WorldBridge {
    inner: TripleBuffer<Box<WorldDrawList>>,
}

impl WorldBridge {
    pub fn new() -> Self {
        Self { inner: TripleBuffer::new(WorldDrawList::new_boxed()) } // @forge:allow_alloc — cold init
    }

    /// DET side: swap `fresh` in; the recycled list comes back for next tick.
    /// The producer MUST [`clear_for_refill`](WorldDrawList::clear_for_refill)
    /// the returned list before refilling it — contract §c precondition,
    /// asserted at the producer call site, never assumed inside `publish`.
    pub fn publish(&self, fresh: Box<WorldDrawList>) -> Box<WorldDrawList> {
        self.inner.publish(fresh)
    }

    /// GPU side: non-blocking take of the newest generation. `None` = contended
    /// or nothing newer than `last_gen` → the consumer reuses its last list.
    pub fn try_take(&self, last_gen: u64, dst: &mut Box<WorldDrawList>) -> Option<WorldTaken> {
        let gen = self.inner.try_take(last_gen, dst)?;
        Some(WorldTaken { generation: gen, len: dst.len })
    }
}

impl Default for WorldBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── TripleLoop ────────────────────────────────────────────────────────────

/// Three-thread compositor spine: bridges + shared atlas.
///
/// Caller decomposes the fields and moves the correct pieces into each thread
/// closure before calling [`spawn_threads`](Self::spawn_threads):
///
/// | Thread | Owned pieces |
/// |--------|-------------|
/// | T1 Logic | `atlas` (write clone), `input_bridge` (publish clone), `event_rx` |
/// | T2 Raster | `atlas` (read clone), `input_bridge` (try_take clone), `overlay` (publish clone) |
/// | T3 GPU/Present | `overlay` (try_take clone), `event_tx` |
///
/// T3 must remain on the window thread (Win32 message-pump constraint).
pub struct TripleLoop {
    /// Shared atlas: T1 acquires write for glyph-cache fills; T2 acquires read
    /// for `rasterize_into`. Phase 2 removes this seam (string bytes in DrawCmd::Text).
    pub atlas: Arc<RwLock<FontAtlas>>,
    /// DrawList pipeline T1 → T2 (UiTripleBuffer; Lock-Free Gate compliant).
    pub input_bridge: Arc<InputBridge>,
    /// RGBA overlay plane T2 → T3 (TripleBuffer<Vec<u8>>; producer + consumer try_lock, Lock-Free Gate compliant).
    pub overlay: Arc<OverlayBridge>,
    /// T3 sends per-frame input snapshots to T1 (mpsc SyncSender, capacity 4).
    pub event_tx: SyncSender<LogicInput>,
    /// T1 receives per-frame input snapshots from T3.
    pub event_rx: Receiver<LogicInput>,
    /// Latest surface dims — T3 stores on resize (Relaxed), T2 reads each frame
    /// for dynamic PixelBuffer allocation. [0]=width [1]=height.
    pub surface_dims: Arc<[AtomicU32; 2]>,
}

impl TripleLoop {
    /// Construct the spine with a pre-warmed atlas and initial surface dimensions.
    pub fn new(atlas: FontAtlas, width: u32, height: u32) -> Self {
        let (event_tx, event_rx) = mpsc::sync_channel(4);
        Self {
            atlas: Arc::new(RwLock::new(atlas)),
            input_bridge: Arc::new(InputBridge::new()),
            overlay: Arc::new(OverlayBridge::new(width, height)),
            event_tx,
            event_rx,
            surface_dims: Arc::new([AtomicU32::new(width), AtomicU32::new(height)]),
        }
    }

    /// Spawn T1 (Logic) and T2 (Raster) on named threads.
    /// T3 (GPU/Present) remains on the caller — it owns the Win32 window.
    ///
    /// The caller must have already moved the correct bridge pieces into each
    /// closure. Returns `(t1_handle, t2_handle)`.
    pub fn spawn_threads(
        t1_fn: impl FnOnce() + Send + 'static,
        t2_fn: impl FnOnce() + Send + 'static,
    ) -> (JoinHandle<()>, JoinHandle<()>) {
        let t1 = std::thread::Builder::new()
            .name("forge-t1-logic".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(t1_fn)
            .expect("[triple_loop] T1 Logic spawn failed — SIGNAL LAW BREACH");
        let t2 = std::thread::Builder::new()
            .name("forge-t2-raster".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(t2_fn)
            .expect("[triple_loop] T2 Raster spawn failed — SIGNAL LAW BREACH");
        (t1, t2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(x: i32, cid: u32) -> WorldDrawCell {
        WorldDrawCell { x_mu: x, y_mu: 0, z_mu: 4200, colour_id: cid, coverage_pmy: 10000, material: 1 }
    }

    fn plane_u8(fill: u8) -> Vec<u8> {
        vec![fill; 4]
    }

    fn plane_i32(fill: i32) -> Vec<i32> {
        vec![fill; 4]
    }

    fn plane_f32(fill: f32) -> Vec<f32> {
        vec![fill; 4]
    }

    #[test]
    fn publish_recycles_old_buffer_zero_alloc() {
        let bridge = TripleBuffer::new(plane_u8(0));
        let spare = plane_u8(0);
        let cap_before = spare.capacity();
        let recycled = bridge.publish(spare);
        assert_eq!(recycled.len(), 4);
        assert_eq!(recycled.capacity(), cap_before);
    }

    #[test]
    fn consumer_takes_fresh_then_reuses_last_on_unchanged() {
        let bridge = TripleBuffer::new(plane_u8(0));
        let _ = bridge.publish(plane_u8(0xAA));
        let mut dst = plane_u8(0);
        let mut last = 0u64;

        let g = bridge.try_take(last, &mut dst).expect("fresh plane");
        assert_eq!(g, 1);
        assert_eq!(dst, plane_u8(0xAA));
        last = g;

        assert!(bridge.try_take(last, &mut dst).is_none(), "must NOT falsely report fresh");
        assert_eq!(dst, plane_u8(0xAA));
    }

    #[test]
    fn two_consumers_drain_independently() {
        let bridge = TripleBuffer::new(plane_u8(0));
        let _ = bridge.publish(plane_u8(0x11));

        let (mut a_dst, mut a_gen) = (plane_u8(0), 0u64);
        let (mut b_dst, mut b_gen) = (plane_u8(0), 0u64);

        a_gen = bridge.try_take(a_gen, &mut a_dst).expect("A fresh");
        b_gen = bridge.try_take(b_gen, &mut b_dst).expect("B still fresh after A");
        assert_eq!(a_gen, 1);
        assert_eq!(b_gen, 1);
        assert_eq!(a_dst, plane_u8(0x11));
        assert_eq!(b_dst, plane_u8(0x11));

        let _ = bridge.publish(plane_u8(0x22));
        a_gen = bridge.try_take(a_gen, &mut a_dst).expect("A sees gen 2");
        assert_eq!(a_gen, 2);
        assert_eq!(a_dst, plane_u8(0x22));
        b_gen = bridge.try_take(b_gen, &mut b_dst).expect("B catches latest");
        assert_eq!(b_gen, 2);
        assert_eq!(b_dst, plane_u8(0x22));
    }

    #[test]
    fn resize_grows_dst_then_reuses() {
        let bridge = TripleBuffer::new(vec![0u8; 0]);
        let _ = bridge.publish(vec![0x7F; 8]);
        let mut dst: Vec<u8> = Vec::new();
        let g = bridge.try_take(0, &mut dst).expect("fresh");
        assert_eq!(g, 1);
        assert_eq!(dst, vec![0x7F; 8]);
    }

    #[test]
    fn i32_vec_clockplane_works() {
        let bridge = TripleBuffer::new(plane_i32(0));
        let _ = bridge.publish(plane_i32(42));
        let mut dst = plane_i32(0);
        let g = bridge.try_take(0, &mut dst).expect("fresh");
        assert_eq!(g, 1);
        assert_eq!(dst, plane_i32(42));
    }

    #[test]
    fn f32_vec_clockplane_works() {
        let bridge = TripleBuffer::new(plane_f32(0.0));
        let _ = bridge.publish(plane_f32(3.5));
        let mut dst = plane_f32(0.0);
        let g = bridge.try_take(0, &mut dst).expect("fresh");
        assert_eq!(g, 1);
        assert_eq!(dst, plane_f32(3.5));
    }

    #[test]
    fn try_take_under_live_contention_never_blocks() {
        use std::thread;

        let bridge = Arc::new(TripleBuffer::new(plane_u8(0)));
        let producer = {
            let b = Arc::clone(&bridge);
            thread::spawn(move || {
                let mut spare = plane_u8(0);
                for i in 0..10_000u32 {
                    spare.iter_mut().for_each(|b| *b = (i & 0xFF) as u8);
                    spare = b.publish(spare);
                }
            })
        };

        let mut dst = plane_u8(0);
        let mut last = 0u64;
        let mut fresh_hits = 0u32;
        for _ in 0..50_000 {
            if let Some(g) = bridge.try_take(last, &mut dst) {
                assert!(g >= last || last == u64::MAX);
                last = g;
                fresh_hits += 1;
            }
        }
        producer.join().unwrap();
        assert!(fresh_hits > 0, "consumer should have caught fresh planes");
    }

    #[test]
    fn a_poisoned_slot_never_freezes_the_consumers() {
        use std::thread;

        let bridge = Arc::new(TripleBuffer::new(plane_u8(0)));
        let _ = bridge.publish(plane_u8(0x33));

        let saboteur = {
            let b = Arc::clone(&bridge);
            thread::spawn(move || {
                let _guard = b.inner.lock().unwrap();
                panic!("deliberate poison under the slot lock");
            })
        };
        assert!(saboteur.join().is_err(), "saboteur must panic while holding the lock");

        let mut dst = plane_u8(0);
        let g = bridge.try_take(0, &mut dst).expect("poisoned slot still serves the latest plane");
        assert_eq!(g, 1);
        assert_eq!(dst, plane_u8(0x33));
        assert_eq!(bridge.try_generation(), Some(1), "generation probe survives poison");

        let _ = bridge.publish(plane_u8(0x44));
        let g2 = bridge.try_take(g, &mut dst).expect("post-poison publishes reach consumers");
        assert_eq!(g2, 2);
        assert_eq!(dst, plane_u8(0x44));
    }

    /// Contract §e step-1 oracle: capacity matches the VixelLab 64×64 ceiling and
    /// the whole list is KBs, not MBs — a compile-time-shaped size proof, no heap
    /// in the type (fixed array + u32).
    #[test]
    fn world_draw_list_is_fixed_capacity_and_kb_sized() {
        assert_eq!(WORLD_DRAW_CAPACITY, 4096, "64×64 LayerStack ceiling");
        let cell_sz = std::mem::size_of::<WorldDrawCell>();
        assert_eq!(cell_sz, 20, "integer-only cell: 3×i32 + u32 + 2×u16, no float, no padding");
        let list_sz = std::mem::size_of::<WorldDrawList>();
        assert_eq!(list_sz, cell_sz * WORLD_DRAW_CAPACITY + 4, "array + len, nothing hidden");
        assert!(list_sz < 96 * 1024, "world slot is KBs ({list_sz}B), not the mesh buffer's MBs");
    }

    /// Contract §e step-2 oracle: the world slot is a second INDEPENDENT bridge.
    #[test]
    fn a_taken_plane_always_matches_the_dims_reported_with_it() {
        let bridge = OverlayBridge::new(4, 4);
        let mut gen = 0u64;
        let mut dst = Vec::new();

        // Resize on every publish — the exact motion that could desync dims.
        for (w, h) in [(4u32, 4u32), (8, 2), (16, 16), (1, 1), (7, 3)] {
            let plane = vec![0xABu8; (w as usize) * (h as usize) * 4];
            bridge.publish(plane, w, h);
            let taken = bridge.try_take(gen, &mut dst).expect("a fresh generation");
            gen = taken.generation;
            assert_eq!(
                dst.len(),
                (taken.width as usize) * (taken.height as usize) * 4,
                "reported {}x{} does not describe a {}-byte plane",
                taken.width,
                taken.height,
                dst.len()
            );
        }
    }

    #[test]
    fn a_sized_plane_knows_when_it_is_inconsistent() {
        let p = SizedPlane::new(5, 3);
        assert!(p.is_consistent());
        let bad = SizedPlane { pixels: vec![0u8; 4], width: 5, height: 3 };
        assert!(!bad.is_consistent(), "4 bytes cannot be a 5x3 RGBA plane");
    }

    #[test]
    fn world_bridge_is_an_independent_second_slot() {
        let world = WorldBridge::new();
        let overlay = OverlayBridge::new(8, 8);
        // Publish only on the world slot: its generation advances to 1 while the
        // overlay stays at its seed generation 0 — a consumer already at 0 finds
        // nothing newer there. Distinct counters = distinct backing stores.
        let mut fresh = WorldDrawList::new_boxed();
        fresh.push(cell(1000, 7));
        world.publish(fresh);
        let mut w_dst = WorldDrawList::new_boxed();
        let taken = world.try_take(0, &mut w_dst).expect("world slot has a new generation");
        assert_eq!(taken.len, 1);
        let mut o_dst = vec![0u8; 8 * 8 * 4];
        assert!(
            overlay.try_take(0, &mut o_dst).is_none(),
            "overlay still at seed generation 0 — world publishes never move it (independent slots)"
        );
    }

    /// Round-trip + reuse semantics: a take copies the len prefix; the same
    /// generation twice yields None (consumer reuses its last).
    #[test]
    fn world_bridge_round_trips_cells_and_generations() {
        let bridge = WorldBridge::new();
        let mut fresh = WorldDrawList::new_boxed();
        assert!(fresh.push(cell(1000, 7)));
        assert!(fresh.push(cell(2000, 9)));
        bridge.publish(fresh);
        let mut dst = WorldDrawList::new_boxed();
        let t = bridge.try_take(0, &mut dst).expect("new generation available");
        assert_eq!((t.len, dst.len), (2, 2));
        assert_eq!(dst.cells[0], cell(1000, 7));
        assert_eq!(dst.cells[1], cell(2000, 9));
        assert!(
            bridge.try_take(t.generation, &mut dst).is_none(),
            "no newer generation → None → consumer reuses last list (never blocks)"
        );
    }

    /// THE S-B stuck-frame discriminator: a recycled list refilled SHORTER
    /// after clear_for_refill must expose only the new cells — the stale third
    /// cell from two swaps back is dead above `len`.
    #[test]
    fn recycled_buffer_clear_kills_stale_cells_above_len() {
        let bridge = WorldBridge::new();
        let mut a = WorldDrawList::new_boxed();
        a.push(cell(1, 1));
        a.push(cell(2, 2));
        a.push(cell(3, 3));
        let mut recycled = bridge.publish(a); // seed list comes back
        // Negative control first: skipping the clear would leave len stale.
        recycled.push(cell(9, 9));
        assert_eq!(recycled.len, 1, "seed list was empty; this push is its first cell");
        // The precondition, as the producer must run it: clear THEN refill.
        recycled.clear_for_refill();
        assert_eq!(recycled.len, 0, "clear_for_refill resets len");
        recycled.push(cell(10, 10));
        recycled.push(cell(20, 20));
        let mut recycled2 = bridge.publish(recycled); // 3-cell list comes back recycled
        assert_eq!(recycled2.len, 3, "recycled list still carries the old frame — WHY the clear is law");
        recycled2.clear_for_refill();
        recycled2.push(cell(100, 100));
        bridge.publish(recycled2);
        let mut dst = WorldDrawList::new_boxed();
        let t = bridge.try_take(1, &mut dst).expect("latest generation");
        assert_eq!(t.len, 1, "only the freshly written cell is live");
        assert_eq!(dst.cells[0], cell(100, 100));
        assert_eq!(dst.len, 1, "stale cells two swaps back are unreachable via len");
    }

    /// Bounded write law: at capacity, push refuses (drops the cell) — the list
    /// NEVER grows.
    #[test]
    fn world_draw_list_push_is_bounded_at_capacity() {
        let mut list = WorldDrawList::new_boxed();
        for i in 0..WORLD_DRAW_CAPACITY {
            assert!(list.push(cell(i as i32, 1)), "pushes below capacity land");
        }
        assert_eq!(list.len as usize, WORLD_DRAW_CAPACITY);
        assert!(!list.push(cell(-1, 2)), "push at capacity refuses — no growth");
        assert_eq!(list.len as usize, WORLD_DRAW_CAPACITY, "len untouched by the refused push");
    }
}
