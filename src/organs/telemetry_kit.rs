//! telemetry_kit.rs — strangler for the F12 playtest-overlay panel.
//!
//! Binds live hardware metrics (CPU/GPU, audio) onto the authored
//! telemetry.kit.vixi surface. The overlay is a dev-mode feature, wired in the
//! hotload_dev SKU and absent in retail.
//!
//! STRANGLER PATTERN (2026-08-17): This is a thin organ shell ported from
//! F:\NewRepo\crates\forge-studio\src\telemetry_kit.rs (630 LOC). The census
//! (TELEMETRY_SLOTS), data structs, and pure integer functions stay here; the
//! actual rendering to DrawList delegates to a downstream crate where
//! forge_canvas and forge_vix can be properly scoped. Crate Zero stays
//! zero-dependency (L06).
//!
//! ## Key invariants honoured (from the donor's own footer gates)
//! * float_in_ir = forbidden — the snapshot crosses as INTEGER permyriad-of-percent
//!   (0..10000) + whole MB; the only float (ResourceMetrics) converts host-side
//!   in TelemetryView::from_metrics, never in the IR.
//! * alloc_steady = forbidden — the kit is lowered ONCE and cached by the host
//!   (re-lowered only on resize); render paths emit into a reused DrawList arena
//!   and format into a reused String — zero steady-state alloc.
//! * runtime_parse = forbidden — load_kit runs on the cache miss, never per tick.
//!
//! Stub fields/consts/fns below (colour IDs, pct_q/mb helpers, box_ui) are
//! real donor logic kept intact for the downstream render crate to call —
//! this crate-zero shell doesn't call them itself yet, hence the blanket
//! allow rather than per-item doc churn on donor-verbatim code.
#![allow(missing_docs, dead_code)]

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU8, Ordering};

// TODO: no forge_canvas equivalent in F:\v3 at crate-zero scope, stubbed
// Donor: use forge_canvas::draw::{DrawCmd, DrawList};
// Donor: use forge_canvas::geom::UiRect;
// Donor: use forge_canvas::text::{draw_text, FontAtlas};

/// Stub: draw list for rendering commands (downstream uses forge_canvas::DrawList).
#[doc(hidden)]
pub struct DrawList {
    _phantom: std::marker::PhantomData<()>,
}

impl DrawList {
    pub fn new_boxed() -> Box<Self> {
        Box::new(DrawList { _phantom: std::marker::PhantomData })
    }
}

/// Stub: font atlas (downstream uses forge_canvas::FontAtlas).
#[doc(hidden)]
pub struct FontAtlas;

impl FontAtlas {
    pub fn init(_font_path: &str, _size: f32) -> Self {
        FontAtlas
    }
}

/// Stub: UI rectangle (mirrors forge_canvas::geom::UiRect).
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct UiRect {
    pub x: MilliUnits,
    pub y: MilliUnits,
    pub w: MilliUnits,
    pub h: MilliUnits,
}

/// Stub: MilliUnits wrapper (i64 in 1/1000-pixel units).
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct MilliUnits(pub i64);

impl UiRect {
    pub fn new(x: i64, y: i64, w: i64, h: i64) -> Self {
        UiRect {
            x: MilliUnits(x),
            y: MilliUnits(y),
            w: MilliUnits(w),
            h: MilliUnits(h),
        }
    }
}

// TODO: no forge_vix equivalent in F:\v3 at crate-zero scope, stubbed
// Donor: use forge_vix::ir::LoweredUi;

/// Stub: the lowered UI from the telemetry.kit.vixi panel (downstream from forge_vix).
#[doc(hidden)]
pub struct LoweredUi {
    pub layout: Vec<LayoutBox>,
}

impl Default for LoweredUi {
    fn default() -> Self {
        LoweredUi { layout: vec![] }
    }
}

/// Stub: a layout box with stable key and bounds.
#[doc(hidden)]
pub struct LayoutBox {
    pub stable_key: String,
    pub rect: RectStub,
}

/// Stub: a rectangle with min/max bounds.
#[doc(hidden)]
pub struct RectStub {
    pub min_x: i64,
    pub min_y: i64,
    pub max_x: i64,
    pub max_y: i64,
}

// TODO: no forge_audio equivalent in F:\v3 at crate-zero scope, stubbed
// Donor: use forge_audio::telemetry::DB_SILENCE_FIXED;
// Donor: use forge_audio::telemetry::roadie;
// Donor: use forge_audio::metering::db_to_permyriad;
// Donor: use forge_audio::telemetry::fixed_to_db;

/// Stub: DB_SILENCE_FIXED constant for audio telemetry (−120 dBFS, fixed-point).
/// The real value from forge_audio is -12_000 (dBFS × 100).
const DB_SILENCE_FIXED: i32 = -12_000;

// Synthesia chrome skin (SYN_SKIN index via the dual_loop's color palette).
// Named colours; the donor file maps these via CID (colour index) to RGBA in dual_loop::rgba.
// Stub: we can't access the real palette without the full dual_loop module, so these
// stay as named indices. The downstream renderer will map them to actual pixel colours.
const CID_PANEL: u8 = 1;   // dark slate-navy
const CID_FRAME: u8 = 2;   // dim azure
const CID_TITLE: u8 = 3;   // bright cool
const CID_VALUE: u8 = 4;   // slate-blue
const CID_TRACK: u8 = 0;   // deepest midnight
const CID_OK: u8 = 4;      // < 70% — calm slate
const CID_WARN: u8 = 5;    // 70..90% — warming cyan
const CID_HOT: u8 = 8;     // >= 90% — neon-red alert

/// Inner padding inside every slot box (MilliUnit; 1000 = 1px).
const PAD: i64 = 6_000;

// ── The integer snapshot the overlay renders ────────────────────────────────────

/// A `Copy` integer view of the live hardware state — the value the present thread
/// publishes and the 120 Hz overlay thread reads. Percentages are
/// permyriad-of-percent (`0..10000` = `0.00..100.00%`) so the IR never sees a float
/// (`float_in_ir = forbidden`); byte counts are whole MB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TelemetryView {
    /// Present-loop frame time in microseconds (the 2.0 ms budget reads against this).
    pub frame_us: u32,
    pub vram_pct_q: u16,
    pub ram_pct_q: u16,
    pub ram_used_mb: u32,
    pub ram_total_mb: u32,
    pub vram_used_mb: u32,
    pub vram_total_mb: u32,
    // ── Audio / Roadie ─────────────────────────────────────────────────────
    /// True-peak L channel, fixed-point (dBFS × 100; −12000 = silence, 0 = 0 dBFS).
    pub audio_peak_l: i32,
    pub audio_peak_r: i32,
    /// Master RMS, fixed-point (dBFS × 100).
    pub audio_rms: i32,
    /// Stereo phase correlation × 10000 (10000 = in-phase, −10000 = inverted).
    pub audio_phase_pmy: i32,
    /// Last audio-callback duration in microseconds.
    pub audio_cycle_us: u32,
    /// Roadie severity (0 = OK, 1 = Info, 2 = Low, 3 = Medium, 4 = High).
    pub roadie_severity: u8,
    /// Roadie diagnosis (0 = none, 1 = Clip, 2 = Phase, 3 = Mud, 4 = Harsh, 5 = Thin, 6 = Pump).
    pub roadie_diagnosis: u8,
}

/// Zero is the right rest value for every counter here EXCEPT the dB fields:
/// fixed-point 0 is 0 dBFS, i.e. FULL SCALE, so a derived `Default` published a
/// pegged meter before the audio thread had stored anything once. Silence is
/// `DB_SILENCE_FIXED` (−120 dBFS), the same floor `db_to_fixed` pins to.
impl Default for TelemetryView {
    fn default() -> Self {
        Self {
            frame_us: 0,
            vram_pct_q: 0,
            ram_pct_q: 0,
            ram_used_mb: 0,
            ram_total_mb: 0,
            vram_used_mb: 0,
            vram_total_mb: 0,
            audio_peak_l: DB_SILENCE_FIXED,
            audio_peak_r: DB_SILENCE_FIXED,
            audio_rms: DB_SILENCE_FIXED,
            audio_phase_pmy: 0,
            audio_cycle_us: 0,
            roadie_severity: 0,
            roadie_diagnosis: 0,
        }
    }
}

impl TelemetryView {
    /// Stub: fold live ResourceMetrics into the integer view.
    ///
    /// The real implementation (downstream in forge_studio) calls forge_gpu::devtools::ResourceMetrics
    /// and converts f32 values to permyriad-of-percent. The stub accepts dummy values.
    pub fn from_metrics(_metrics_stub: &[u8], frame_us: u32) -> Self {
        Self {
            frame_us,
            ..Self::default()
        }
    }
}

/// `used/total` as permyriad-of-percent (`0..10000`), saturating — pure integer.
#[inline]
fn pct_q(used: u64, total: u64) -> u16 {
    if total == 0 {
        0
    } else {
        ((used as u128 * 10_000 / total as u128) as u64).min(10_000) as u16
    }
}

/// Bytes → whole MB, saturating into `u32`.
#[inline]
fn mb(bytes: u64) -> u32 {
    (bytes / (1024 * 1024)).min(u32::MAX as u64) as u32
}

// ── Lock-free CPU↔GPU hand-off (Relaxed atomics — telemetry is advisory) ──────────

/// The shared overlay state: the F12 toggle + the latest [`TelemetryView`], packed
/// into atomics so the 120 Hz overlay thread reads it WITHOUT a lock (the Clock
/// Isolation invariant — the metronome never blocks). The present thread `store`s;
/// the overlay thread `load`s.
pub struct TelemetryShared {
    on: AtomicBool,
    frame_us: AtomicU32,
    vram_pct_q: AtomicU32,
    ram_pct_q: AtomicU32,
    ram_used_mb: AtomicU32,
    ram_total_mb: AtomicU32,
    vram_used_mb: AtomicU32,
    vram_total_mb: AtomicU32,
    // audio
    audio_peak_l: AtomicI32,
    audio_peak_r: AtomicI32,
    audio_rms: AtomicI32,
    audio_phase_pmy: AtomicI32,
    audio_cycle_us: AtomicU32,
    roadie_severity: AtomicU8,
    roadie_diagnosis: AtomicU8,
}

/// Seeded from [`TelemetryView::default`] so the shared cell and the view agree
/// about rest: silent, not pegged.
impl Default for TelemetryShared {
    fn default() -> Self {
        let v = TelemetryView::default();
        Self {
            on: AtomicBool::new(false),
            frame_us: AtomicU32::new(v.frame_us),
            vram_pct_q: AtomicU32::new(v.vram_pct_q as u32),
            ram_pct_q: AtomicU32::new(v.ram_pct_q as u32),
            ram_used_mb: AtomicU32::new(v.ram_used_mb),
            ram_total_mb: AtomicU32::new(v.ram_total_mb),
            vram_used_mb: AtomicU32::new(v.vram_used_mb),
            vram_total_mb: AtomicU32::new(v.vram_total_mb),
            audio_peak_l: AtomicI32::new(v.audio_peak_l),
            audio_peak_r: AtomicI32::new(v.audio_peak_r),
            audio_rms: AtomicI32::new(v.audio_rms),
            audio_phase_pmy: AtomicI32::new(v.audio_phase_pmy),
            audio_cycle_us: AtomicU32::new(v.audio_cycle_us),
            roadie_severity: AtomicU8::new(v.roadie_severity),
            roadie_diagnosis: AtomicU8::new(v.roadie_diagnosis),
        }
    }
}

impl TelemetryShared {
    /// Check if the overlay is currently on.
    #[inline]
    pub fn is_on(&self) -> bool {
        self.on.load(Ordering::Relaxed)
    }

    /// Flip the F12 overlay on/off; returns the new state.
    ///
    /// Note: The actual F12 input binding is handled at the shell/host level
    /// (not in this crate-zero module). This toggle is the state machine only.
    #[inline]
    pub fn toggle(&self) -> bool {
        let next = !self.on.load(Ordering::Relaxed);
        self.on.store(next, Ordering::Relaxed);
        next
    }

    /// Publish the latest full view (present thread, on the throttled poll).
    pub fn store(&self, v: &TelemetryView) {
        self.frame_us.store(v.frame_us, Ordering::Relaxed);
        self.vram_pct_q.store(v.vram_pct_q as u32, Ordering::Relaxed);
        self.ram_pct_q.store(v.ram_pct_q as u32, Ordering::Relaxed);
        self.ram_used_mb.store(v.ram_used_mb, Ordering::Relaxed);
        self.ram_total_mb.store(v.ram_total_mb, Ordering::Relaxed);
        self.vram_used_mb.store(v.vram_used_mb, Ordering::Relaxed);
        self.vram_total_mb.store(v.vram_total_mb, Ordering::Relaxed);
    }

    /// Update only the frame time (cheap, every present frame between full polls).
    #[inline]
    pub fn set_frame_us(&self, frame_us: u32) {
        self.frame_us.store(frame_us, Ordering::Relaxed);
    }

    /// Publish live audio / Roadie metrics every present frame (cheap — all Relaxed stores).
    ///
    /// NOTE: stack-only, no allocation (forbidden_ops hot_path_heap_alloc).
    #[inline]
    pub fn set_audio(
        &self,
        peak_l: i32, peak_r: i32, rms: i32,
        phase_pmy: i32, cycle_us: u32,
        severity: u8, diagnosis: u8,
    ) {
        self.audio_peak_l.store(peak_l, Ordering::Relaxed);
        self.audio_peak_r.store(peak_r, Ordering::Relaxed);
        self.audio_rms.store(rms, Ordering::Relaxed);
        self.audio_phase_pmy.store(phase_pmy, Ordering::Relaxed);
        self.audio_cycle_us.store(cycle_us, Ordering::Relaxed);
        self.roadie_severity.store(severity, Ordering::Relaxed);
        self.roadie_diagnosis.store(diagnosis, Ordering::Relaxed);
    }

    /// Read the latest view (overlay thread, once per 120 Hz tick — lock-free).
    pub fn load(&self) -> TelemetryView {
        TelemetryView {
            frame_us: self.frame_us.load(Ordering::Relaxed),
            vram_pct_q: self.vram_pct_q.load(Ordering::Relaxed) as u16,
            ram_pct_q: self.ram_pct_q.load(Ordering::Relaxed) as u16,
            ram_used_mb: self.ram_used_mb.load(Ordering::Relaxed),
            ram_total_mb: self.ram_total_mb.load(Ordering::Relaxed),
            vram_used_mb: self.vram_used_mb.load(Ordering::Relaxed),
            vram_total_mb: self.vram_total_mb.load(Ordering::Relaxed),
            audio_peak_l: self.audio_peak_l.load(Ordering::Relaxed),
            audio_peak_r: self.audio_peak_r.load(Ordering::Relaxed),
            audio_rms: self.audio_rms.load(Ordering::Relaxed),
            audio_phase_pmy: self.audio_phase_pmy.load(Ordering::Relaxed),
            audio_cycle_us: self.audio_cycle_us.load(Ordering::Relaxed),
            roadie_severity: self.roadie_severity.load(Ordering::Relaxed),
            roadie_diagnosis: self.roadie_diagnosis.load(Ordering::Relaxed),
        }
    }
}

// ── Pure layout helpers (integer-only, headlessly testable) ───────────────────────

/// Find a lowered slot box by its authored `stable_key` (the kit's slot path) and
/// convert its `IrRect` (i64 MilliUnit min/max) to a `UiRect` (x/y/w/h).
///
/// TODO: no forge_vix equivalent in Crate Zero, stubbed lookup.
/// Real implementation (downstream) calls forge_vix::ir::LoweredUi::layout.
#[inline]
fn box_ui(ui: &LoweredUi, key: &str) -> Option<UiRect> {
    ui.layout.iter().find(|b| b.stable_key.as_str() == key).map(|b| {
        UiRect::new(b.rect.min_x, b.rect.min_y, b.rect.max_x - b.rect.min_x, b.rect.max_y - b.rect.min_y)
    })
}

/// The filled portion of a bar slot for a given pressure: width scales with `pct_q`,
/// clamped to the slot (over-100% never overflows). Pure + integer so the
/// value→geometry mapping is a headless discriminator.
pub fn bar_fill_rect(slot: UiRect, pct_q: u16) -> UiRect {
    let pq = (pct_q as i64).min(10_000);
    let fill_w = slot.w.0 * pq / 10_000;
    UiRect::new(slot.x.0, slot.y.0, fill_w, slot.h.0)
}

/// Map a pressure (permyriad-of-percent) to a colour index band — calm → warm → hot.
/// The three distinct bands make the bar a discriminator (a constant would hide a broken fill).
#[inline]
pub fn pressure_color(pct_q: u16) -> u8 {
    if pct_q >= 9_000 {
        CID_HOT
    } else if pct_q >= 7_000 {
        CID_WARN
    } else {
        CID_OK
    }
}

// ── The strangler: bind the live view onto the authored slots ─────────────────────

/// Render the authored telemetry kit with live values into `draw` (a reused arena).
///
/// TODO: no forge_canvas or forge_vix equivalent in Crate Zero, stubbed.
/// Real implementation (downstream) emits DrawCmd::Rect, DrawCmd::RectOutline, and
/// DrawCmd::Text for each slot. Donor: F:\NewRepo\crates\forge-studio\src\telemetry_kit.rs:316–361.
///
/// Returns the count of bound slots (0 in stub; real: should be 11 for full kit).
pub fn render_telemetry_kit(
    ui: &LoweredUi,
    _view: &TelemetryView,
    _draw: &mut DrawList,
    _atlas: &mut FontAtlas,
    _scratch: &mut String,
) -> usize {
    // Stub: check if root exists; real impl binds ~11 slots (headers, values, bars).
    if ui.layout.iter().any(|b| b.stable_key == "root") {
        1
    } else {
        0
    }
}

/// The panel's own metering floor: the master bus reads down to −60 dBFS, the
/// same span `forge_audio::metering` maps from. Below it the meter sits empty
/// rather than shimmering on the noise floor.
pub const AUDIO_METER_FLOOR_DB: i64 = -60;

/// `TelemetryView::audio_rms` (fixed-point dBFS × 100) as a permyriad bar fill.
///
/// TODO: no forge_audio equivalent in Crate Zero, stubbed conversion.
/// Real implementation (downstream) calls forge_audio::metering::db_to_permyriad
/// and forge_audio::telemetry::fixed_to_db. This stub maps -6000 (−60 dBFS, floor) → 0
/// and 0 dBFS → 10000. Donor: F:\NewRepo\crates\forge-studio\src\telemetry_kit.rs:436–441.
///
/// Stack-only, no allocation (forbidden_ops).
pub fn audio_rms_permyriad(rms_fixed: i32) -> u16 {
    // Stub: linear map from the meter floor to 0 dBFS.
    // Real: db_to_permyriad(fixed_to_db(rms_fixed), AUDIO_METER_FLOOR_DB)
    let min_db = AUDIO_METER_FLOOR_DB as i32 * 100; // −6000 fixed
    let max_db = 0i32; // 0 dBFS
    if rms_fixed <= min_db {
        0
    } else if rms_fixed >= max_db {
        10_000
    } else {
        let span = (rms_fixed - min_db) as u32;
        let total = (max_db - min_db) as u32;
        ((span as u64 * 10_000 / total as u64).min(10_000)) as u16
    }
}

/// Render the live audio / Roadie diagnostics panel.
///
/// TODO: no forge_canvas or forge_vix equivalent in Crate Zero, stubbed.
/// Real implementation (downstream in forge_studio) emits DrawCmd and draw_text
/// for the audio section, positioned at (x_mu, y_mu) with width w_mu.
/// Donor: F:\NewRepo\crates\forge-studio\src\telemetry_kit.rs:445–520.
pub fn render_audio_section(
    _view: &TelemetryView,
    _draw: &mut DrawList,
    _atlas: &mut FontAtlas,
    _scratch: &mut String,
    _x_mu: i64,
    _y_mu: i64,
    _w_mu: i64,
) {
    // Stub: no-op. Real impl draws audio panel with RMS bar, peak L/R, phase, cycle, roadie status.
}

// TODO: F12 overlay input wiring is handled at the shell level (not in Crate Zero).
// Donor: the toggle state lives in TelemetryShared::on (atomics, Relaxed).
// The shell level (forge_gui or equivalent) intercepts F12 key and calls TelemetryShared::toggle().
// Stub: no input binding here; only the state machine.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_default_is_silent_not_pegged() {
        let v = TelemetryView::default();
        assert_eq!(v.audio_peak_l, DB_SILENCE_FIXED);
        assert_eq!(v.audio_rms, DB_SILENCE_FIXED);
        assert_eq!(v.frame_us, 0);
    }

    #[test]
    fn pct_q_clamps_and_handles_zero_total() {
        assert_eq!(pct_q(5, 10), 5_000, "50%");
        assert_eq!(pct_q(2_500_000_000, 10_000_000_000), 2_500, "25%");
        assert_eq!(pct_q(10_000, 10_000), 10_000, "100%");
        assert_eq!(pct_q(0, 0), 0, "zero total");
        assert_eq!(pct_q(1, 0), 0, "nonzero numerator, zero denom");
    }

    #[test]
    fn mb_saturates_and_converts() {
        assert_eq!(mb(1_048_576), 1, "1 MiB");
        assert_eq!(mb(1_073_741_824), 1_024, "1 GiB");
        assert_eq!(mb(0), 0);
    }

    #[test]
    fn bar_fill_rect_scales_and_clamps() {
        let slot = UiRect::new(0, 0, 200_000, 14_000);
        let lo = bar_fill_rect(slot, 2_500).w.0;
        let hi = bar_fill_rect(slot, 7_500).w.0;
        assert!(hi > lo, "fill width must grow with pressure ({hi} !> {lo})");
        assert_eq!(bar_fill_rect(slot, 10_000).w.0, 200_000, "100% fills the slot exactly");
        assert_eq!(bar_fill_rect(slot, 25_000).w.0, 200_000, "over-100% clamps");
        assert_eq!(bar_fill_rect(slot, 0).w.0, 0, "0% is empty");
    }

    #[test]
    fn pressure_color_is_monotone_discriminator() {
        // Three distinct bands
        assert_ne!(pressure_color(1_000), pressure_color(9_500));
        assert_ne!(pressure_color(7_500), pressure_color(9_500));
        assert_ne!(pressure_color(1_000), pressure_color(7_500));
        assert_eq!(pressure_color(9_500), CID_HOT);
    }

    #[test]
    fn audio_rms_permyriad_maps_floor_to_max() {
        assert_eq!(audio_rms_permyriad(-6_000), 0, "floor (-60 dBFS)");
        assert_eq!(audio_rms_permyriad(0), 10_000, "0 dBFS full");
        assert_eq!(audio_rms_permyriad(-3_000), 5_000, "-30 dBFS half");
        assert_eq!(audio_rms_permyriad(-12_000), 0, "below floor");
    }

    #[test]
    fn telemetry_shared_toggles_and_stores() {
        let s = TelemetryShared::default();
        assert!(!s.is_on(), "starts off");
        assert!(s.toggle());
        assert!(s.is_on());
        let v = TelemetryView { frame_us: 1_234, vram_pct_q: 9_999, ram_pct_q: 5_555, ram_used_mb: 42, ram_total_mb: 99, vram_used_mb: 7, vram_total_mb: 8, ..TelemetryView::default() };
        s.store(&v);
        assert_eq!(s.load(), v, "round-trip through atomics");
        s.set_frame_us(2_000);
        assert_eq!(s.load().frame_us, 2_000, "frame-only update");
        assert_eq!(s.load().vram_pct_q, 9_999, "other fields unchanged");
    }

    #[test]
    fn telemetry_shared_set_audio_stores_all_fields() {
        let s = TelemetryShared::default();
        s.set_audio(-18, -20, -3_000, 8_500, 2_100, 1, 2);
        let v = s.load();
        assert_eq!(v.audio_peak_l, -18);
        assert_eq!(v.audio_peak_r, -20);
        assert_eq!(v.audio_rms, -3_000);
        assert_eq!(v.audio_phase_pmy, 8_500);
        assert_eq!(v.audio_cycle_us, 2_100);
        assert_eq!(v.roadie_severity, 1);
        assert_eq!(v.roadie_diagnosis, 2);
    }

    #[test]
    fn render_telemetry_kit_returns_zero_on_empty_ui() {
        let ui = LoweredUi::default();
        let mut draw = DrawList::new_boxed();
        let mut atlas = FontAtlas::init("", 16.0);
        let mut scratch = String::new();
        assert_eq!(
            render_telemetry_kit(&ui, &TelemetryView::default(), &mut draw, &mut atlas, &mut scratch),
            0,
            "empty UI binds nothing"
        );
    }

    #[test]
    fn render_audio_section_stub_does_not_panic() {
        let mut draw = DrawList::new_boxed();
        let mut atlas = FontAtlas::init("", 16.0);
        let mut scratch = String::new();
        render_audio_section(&TelemetryView::default(), &mut draw, &mut atlas, &mut scratch, 0, 0, 400_000);
        // Stub: no-op; real impl will emit draw commands.
    }

    #[test]
    fn telemetry_view_from_metrics_stub() {
        let v = TelemetryView::from_metrics(&[], 8_333);
        assert_eq!(v.frame_us, 8_333);
        assert_eq!(v.vram_pct_q, 0);
        assert_eq!(v.ram_pct_q, 0);
    }
}
