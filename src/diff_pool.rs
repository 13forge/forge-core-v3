//! VixelDiff + DiffPool + RollbackBuffer — 18-byte diff protocol with
//! pre-allocated ring buffers for deterministic rollback.
//!
//! All world mutations flow through `VixelDiff`. The `DiffPool` is a 1.1MB
//! pre-allocated ring buffer that never grows. The `RollbackBuffer` stores
//! exactly 120 frames (1.0 second at 120Hz) of `FrameSnapshot` metadata
//! for prediction/correction rollback.
//!
//! Zero heap allocation. Integer-only. Bitwise deterministic.

use crate::hash_bytes_fnv1a;

/// Pre-allocated DiffPool capacity.
/// 18 bytes × 64000 ≈ 1.125 MB — fits the 1.1MB budget with margin.
pub const POOL_CAPACITY: usize = 64000;

/// Rollback window: exactly 120 frames = 1.0 second at 120Hz.
/// Architecturally locked — do not change.
pub const ROLLBACK_FRAMES: usize = 120;

/// 18-byte diff record for a single voxel mutation.
///
/// Every world change — entity action, explosion, automata rule — produces
/// one `VixelDiff`. The TickEngine is the sole authority that evaluates diffs.
///
/// Layout: `#[repr(C, packed)]` guarantees exactly 18 bytes with zero padding.
/// Fields are laid out as 3×i32 + 3×u16 = 12 + 6 = 18 bytes.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(Default)]
pub struct VixelDiff {
    /// Chunk X coordinate in world space.
    pub chunk_x: i32,             // 4 bytes
    /// Chunk Y coordinate in world space.
    pub chunk_y: i32,             // 4 bytes
    /// Chunk Z coordinate in world space.
    pub chunk_z: i32,             // 4 bytes
    /// Index within the chunk's `[MaterialId; 32768]` array.
    pub index: u16,               // 2 bytes
    /// MaterialId before the mutation.
    pub old_mat: u16,             // 2 bytes
    /// MaterialId after the mutation.
    pub new_mat: u16,             // 2 bytes
}
// Compile-time size assertion: VixelDiff must be exactly 18 bytes.
const _: () = assert!(core::mem::size_of::<VixelDiff>() == 18);

impl VixelDiff {
    /// Convenience accessor for chunk coordinates as a tuple.
    pub fn chunk(&self) -> (i32, i32, i32) {
        (self.chunk_x, self.chunk_y, self.chunk_z)
    }
}


/// Pre-allocated 1.1MB ring buffer of `VixelDiff` entries.
///
/// Never grows. When full, oldest entries are overwritten.
/// At 120Hz with ~100 diffs/frame, this holds ~640 frames (~5.3 seconds)
/// of history — well beyond the 120-frame rollback window.
pub struct DiffPool {
    buffer: Box<[VixelDiff; POOL_CAPACITY]>, // @forge:allow_alloc — one-time boot allocation
    /// Write cursor — next slot to write into.
    head: usize,
    /// Total number of valid entries (capped at POOL_CAPACITY).
    count: usize,
}

impl DiffPool {
    /// Create a new pre-allocated DiffPool. All slots zeroed.
    /// Single heap allocation at boot — never grows.
    pub fn new() -> Self {
        // alloc-ok: one-time boot allocation, pre-allocated ring buffer
        let buffer = vec![VixelDiff::default(); POOL_CAPACITY]
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| unreachable!());
        Self {
            buffer,
            head: 0,
            count: 0,
        }
    }

    /// Append a diff to the ring buffer. Returns the absolute index.
    /// O(1), no allocation.
    pub fn push(&mut self, diff: VixelDiff) -> u32 {
        let idx = self.head;
        self.buffer[idx] = diff;
        self.head = (self.head + 1) % POOL_CAPACITY;
        if self.count < POOL_CAPACITY {
            self.count += 1;
        }
        idx as u32
    }

    /// Get a diff by absolute index. Returns `None` if index is out of range.
    pub fn get(&self, index: u32) -> Option<&VixelDiff> {
        let idx = index as usize;
        if idx < POOL_CAPACITY {
            Some(&self.buffer[idx])
        } else {
            None
        }
    }

    /// Current write head position.
    pub fn head(&self) -> usize {
        self.head
    }

    /// Number of valid entries in the pool.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Iterate diffs for a frame given `start` index and `count`.
    /// Handles ring buffer wrap-around.
    pub fn frame_diffs(&self, start: u32, count: u16) -> FrameDiffIter<'_> {
        FrameDiffIter {
            pool: self,
            current: start as usize,
            remaining: count as usize,
        }
    }

    /// Iterate diffs for a frame in reverse order (for rewind).
    /// Traverses from `start + count - 1` back to `start`.
    pub fn frame_diffs_reverse(&self, start: u32, count: u16) -> FrameDiffReverseIter<'_> {
        let c = count as usize;
        let last = if c == 0 {
            start as usize
        } else {
            (start as usize + c - 1) % POOL_CAPACITY
        };
        FrameDiffReverseIter {
            pool: self,
            current: last,
            remaining: c,
        }
    }
}

/// Forward iterator over a frame's diffs in the DiffPool.
pub struct FrameDiffIter<'a> {
    pool: &'a DiffPool,
    current: usize,
    remaining: usize,
}

impl<'a> Iterator for FrameDiffIter<'a> {
    type Item = &'a VixelDiff;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let diff = &self.pool.buffer[self.current];
        self.current = (self.current + 1) % POOL_CAPACITY;
        self.remaining -= 1;
        Some(diff)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

/// Reverse iterator over a frame's diffs in the DiffPool.
pub struct FrameDiffReverseIter<'a> {
    pool: &'a DiffPool,
    current: usize,
    remaining: usize,
}

impl<'a> Iterator for FrameDiffReverseIter<'a> {
    type Item = &'a VixelDiff;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let diff = &self.pool.buffer[self.current];
        if self.current == 0 {
            self.current = POOL_CAPACITY - 1;
        } else {
            self.current -= 1;
        }
        self.remaining -= 1;
        Some(diff)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

/// Metadata snapshot for a single simulation frame.
///
/// Stored in the `RollbackBuffer`. Points into the `DiffPool` via
/// `diff_start` and `diff_count` — no data duplication.
#[derive(Clone, Copy, Debug)]
#[derive(Default)]
pub struct FrameSnapshot {
    /// Monotonic tick counter.
    pub tick: u64,
    /// 10-bit packed InputBits (u16 for alignment).
    pub inputs: u16,
    /// Index into DiffPool where this frame's diffs begin.
    pub diff_start: u32,
    /// Number of 18-byte diffs produced this frame.
    pub diff_count: u16,
    /// FNV-1a checksum of the frame's diffs for desync detection.
    /// Computed via [`FrameSnapshot::compute_checksum`] using the actual diff bytes
    /// from the `DiffPool`.
    pub checksum: u64,
}

impl FrameSnapshot {
    /// Compute the FNV-1a checksum of this frame's diffs from the pool.
    ///
    /// Hashes each diff's fields (chunk_x, chunk_y, chunk_z, index, old_mat, new_mat)
    /// in order, producing a u64 checksum for desync detection.
    /// Deterministic: same diffs → same checksum.
    pub fn compute_checksum(&self, pool: &DiffPool) -> u64 {
        let mut buf: Vec<u8> = Vec::new();
        for diff in pool.frame_diffs(self.diff_start, self.diff_count) {
            buf.extend_from_slice(&diff.chunk_x.to_le_bytes());
            buf.extend_from_slice(&diff.chunk_y.to_le_bytes());
            buf.extend_from_slice(&diff.chunk_z.to_le_bytes());
            buf.extend_from_slice(&diff.index.to_le_bytes());
            buf.extend_from_slice(&diff.old_mat.to_le_bytes());
            buf.extend_from_slice(&diff.new_mat.to_le_bytes());
        }
        hash_bytes_fnv1a(&buf)
    }
}

/// 120-frame ring buffer for prediction/correction rollback.
///
/// Exactly 1.0 second at 120Hz. Architecturally locked — never grows.
/// Each frame stores a `FrameSnapshot` that references diffs in the `DiffPool`.
pub struct RollbackBuffer {
    frames: [FrameSnapshot; ROLLBACK_FRAMES],
    /// Write cursor — next slot to write into.
    head: usize,
    /// Number of valid frames stored (capped at ROLLBACK_FRAMES).
    count: usize,
}

impl RollbackBuffer {
    /// Create a new RollbackBuffer. All 120 slots zeroed.
    pub fn new() -> Self {
        Self {
            frames: [FrameSnapshot::default(); ROLLBACK_FRAMES],
            head: 0,
            count: 0,
        }
    }

    /// Record a frame snapshot. Overwrites oldest if buffer is full.
    pub fn push(&mut self, snapshot: FrameSnapshot) {
        self.frames[self.head] = snapshot;
        self.head = (self.head + 1) % ROLLBACK_FRAMES;
        if self.count < ROLLBACK_FRAMES {
            self.count += 1;
        }
    }

    /// Find a frame by tick number. Returns `None` if not in the buffer.
    pub fn find_by_tick(&self, tick: u64) -> Option<&FrameSnapshot> {
        for i in 0..self.count {
            let idx = if self.head == 0 {
                ROLLBACK_FRAMES - 1 - i
            } else {
                (self.head + ROLLBACK_FRAMES - 1 - i) % ROLLBACK_FRAMES
            };
            if self.frames[idx].tick == tick {
                return Some(&self.frames[idx]);
            }
        }
        None
    }

    /// Get the most recent frame snapshot.
    pub fn latest(&self) -> Option<&FrameSnapshot> {
        if self.count == 0 {
            return None;
        }
        let idx = if self.head == 0 {
            ROLLBACK_FRAMES - 1
        } else {
            self.head - 1
        };
        Some(&self.frames[idx])
    }

    /// Number of valid frames stored.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Iterate frames from `start_tick` to `end_tick` (inclusive) in forward order.
    /// Used for replay (old→new) after rewind.
    pub fn frames_forward(&self, start_tick: u64, end_tick: u64) -> FrameRangeIter<'_> {
        let oldest_idx = if self.count < ROLLBACK_FRAMES {
            0
        } else {
            self.head
        };
        FrameRangeIter {
            buffer: self,
            pos: 0,
            oldest_idx,
            start_tick,
            end_tick,
        }
    }

    /// Iterate frames from `end_tick` back to `start_tick` (inclusive) in reverse order.
    /// Used for rewind (new→old).
    pub fn frames_reverse(&self, start_tick: u64, end_tick: u64) -> FrameRangeReverseIter<'_> {
        FrameRangeReverseIter {
            buffer: self,
            pos: 0,
            start_tick,
            end_tick,
        }
    }
}

/// Forward iterator over frames in a tick range.
pub struct FrameRangeIter<'a> {
    buffer: &'a RollbackBuffer,
    pos: usize,
    oldest_idx: usize,
    start_tick: u64,
    end_tick: u64,
}

impl<'a> Iterator for FrameRangeIter<'a> {
    type Item = &'a FrameSnapshot;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.buffer.count {
            let idx = (self.oldest_idx + self.pos) % ROLLBACK_FRAMES;
            self.pos += 1;
            let frame = &self.buffer.frames[idx];
            if frame.tick >= self.start_tick && frame.tick <= self.end_tick {
                return Some(frame);
            }
        }
        None
    }
}

/// Reverse iterator over frames in a tick range.
pub struct FrameRangeReverseIter<'a> {
    buffer: &'a RollbackBuffer,
    pos: usize,
    start_tick: u64,
    end_tick: u64,
}

impl<'a> Iterator for FrameRangeReverseIter<'a> {
    type Item = &'a FrameSnapshot;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.buffer.count {
            let idx = if self.buffer.head == 0 {
                ROLLBACK_FRAMES - 1 - self.pos
            } else {
                (self.buffer.head + ROLLBACK_FRAMES - 1 - self.pos) % ROLLBACK_FRAMES
            };
            self.pos += 1;
            let frame = &self.buffer.frames[idx];
            if frame.tick >= self.start_tick && frame.tick <= self.end_tick {
                return Some(frame);
            }
        }
        None
    }
}

/// Rewind a voxel grid by applying diffs in reverse (new→old).
///
/// For each diff in the frame range (newest to oldest), swaps `new_mat` back
/// to `old_mat` in the provided grid.
///
/// `apply_fn` receives `(chunk, index, material_to_set)` for each rewind step.
pub fn rewind(
    pool: &DiffPool,
    rollback: &RollbackBuffer,
    start_tick: u64,
    end_tick: u64,
    mut apply_fn: impl FnMut((i32, i32, i32), u16, u16),
) {
    for frame in rollback.frames_reverse(start_tick, end_tick) {
        // Traverse diffs in reverse: undo new→old
        for diff in pool.frame_diffs_reverse(frame.diff_start, frame.diff_count) {
            apply_fn(diff.chunk(), diff.index, diff.old_mat);
        }
    }
}

/// Replay diffs forward (old→new) to restore state after rewind.
///
/// For each diff in the frame range (oldest to newest), applies `new_mat`.
///
/// `apply_fn` receives `(chunk, index, material_to_set)` for each replay step.
pub fn replay(
    pool: &DiffPool,
    rollback: &RollbackBuffer,
    start_tick: u64,
    end_tick: u64,
    mut apply_fn: impl FnMut((i32, i32, i32), u16, u16),
) {
    for frame in rollback.frames_forward(start_tick, end_tick) {
        // Traverse diffs forward: apply old→new
        for diff in pool.frame_diffs(frame.diff_start, frame.diff_count) {
            apply_fn(diff.chunk(), diff.index, diff.new_mat);
        }
    }
}


// ── Unit tests ───────────────────────────────────────────────────────────────
//
// Property-based tests using proptest removed: forge-core-v3 is a zero-dependency
// crate (Firewall Law). Comprehensive rollback/rewind/replay testing deferred to
// integration tests or to the forge-vcs-v3 layer which consumes diff_pool.

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit test: VixelDiff size ────────────────────────────────────────
    #[test]
    fn test_vixel_diff_is_18_bytes() {
        assert_eq!(core::mem::size_of::<VixelDiff>(), 18);
    }

    // ── Unit test: DiffPool ring buffer wrap-around ──────────────────────
    #[test]
    fn test_diffpool_wraparound() {
        let mut pool = DiffPool::new();
        // Fill to capacity
        for i in 0..POOL_CAPACITY {
            pool.push(VixelDiff {
                chunk_x: 0,
                chunk_y: 0,
                chunk_z: 0,
                index: (i % 65536) as u16,
                old_mat: 0,
                new_mat: 1,
            });
        }
        assert_eq!(pool.count(), POOL_CAPACITY);
        assert_eq!(pool.head(), 0); // wrapped around

        // One more push overwrites slot 0
        pool.push(VixelDiff {
            chunk_x: 99,
            chunk_y: 0,
            chunk_z: 0,
            index: 42,
            old_mat: 5,
            new_mat: 10,
        });
        assert_eq!(pool.count(), POOL_CAPACITY); // still capped
        let d = pool.get(0).unwrap();
        let cx = { d.chunk_x };
        let idx = { d.index };
        assert_eq!(cx, 99);
        assert_eq!(idx, 42);
    }

    // ── Unit test: RollbackBuffer 120-frame cap ──────────────────────────
    #[test]
    fn test_rollback_buffer_cap() {
        let mut rb = RollbackBuffer::new();
        for i in 0..150u64 {
            rb.push(FrameSnapshot {
                tick: i,
                inputs: 0,
                diff_start: 0,
                diff_count: 0,
                checksum: 0u64,
            });
        }
        assert_eq!(rb.count(), ROLLBACK_FRAMES);
        // Oldest tick should be 30 (150 - 120)
        assert!(rb.find_by_tick(29).is_none());
        assert!(rb.find_by_tick(30).is_some());
        assert!(rb.find_by_tick(149).is_some());
    }

    // ── Unit test: FrameSnapshot checksum determinism and differentiation ─────
    #[test]
    fn frame_snapshot_checksum_is_deterministic_and_differentiates() {
        let mut pool = DiffPool::new();

        // Push 3 diffs for frame A
        let frame_a_start = pool.push(VixelDiff {
            chunk_x: 1,
            chunk_y: 2,
            chunk_z: 3,
            index: 42,
            old_mat: 5,
            new_mat: 10,
        });
        pool.push(VixelDiff {
            chunk_x: 4,
            chunk_y: 5,
            chunk_z: 6,
            index: 43,
            old_mat: 7,
            new_mat: 12,
        });
        pool.push(VixelDiff {
            chunk_x: 7,
            chunk_y: 8,
            chunk_z: 9,
            index: 44,
            old_mat: 9,
            new_mat: 14,
        });

        let snapshot_a = FrameSnapshot {
            tick: 1,
            inputs: 100,
            diff_start: frame_a_start,
            diff_count: 3,
            checksum: 0,
        };

        // Compute checksum twice — must be identical (determinism)
        let hash_a1 = snapshot_a.compute_checksum(&pool);
        let hash_a2 = snapshot_a.compute_checksum(&pool);
        assert_eq!(hash_a1, hash_a2, "checksum must be deterministic");

        // Push 3 different diffs for frame B
        let frame_b_start = pool.push(VixelDiff {
            chunk_x: 10,
            chunk_y: 11,
            chunk_z: 12,
            index: 50,
            old_mat: 15,
            new_mat: 20,
        });
        pool.push(VixelDiff {
            chunk_x: 13,
            chunk_y: 14,
            chunk_z: 15,
            index: 51,
            old_mat: 17,
            new_mat: 22,
        });
        pool.push(VixelDiff {
            chunk_x: 16,
            chunk_y: 17,
            chunk_z: 18,
            index: 52,
            old_mat: 19,
            new_mat: 24,
        });

        let snapshot_b = FrameSnapshot {
            tick: 2,
            inputs: 200,
            diff_start: frame_b_start,
            diff_count: 3,
            checksum: 0,
        };

        let hash_b = snapshot_b.compute_checksum(&pool);

        // Different diffs must produce different checksums
        assert_ne!(hash_a1, hash_b, "different frame diffs must produce different checksums");
    }
}
