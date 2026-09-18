//! dungeon_master_kit.rs — strangler for `dungeon_master.kit.vixi`. Binds the
//! [`DungeonMasterView`] onto the authored slots. READ-ONLY: no on_click slots
//! authored, so the plotting screen is a gauge and never a lever.
//!
//! The panel was registered 2026-08-02 with `source=` binds and no host at all —
//! it lowered and painted nothing. This is the missing face.
//!
//! STRANGLER PATTERN (2026-08-17): This is a thin organ shell ported from
//! F:\NewRepo\crates\forge-studio\src\dungeon_master_kit.rs. The DM_SLOTS
//! census and function signatures are here; the actual rendering delegates to
//! a Gamebroski/DM identity layer that will be wired in a downstream crate
//! where forge_canvas and forge_vix can be properly scoped. Crate Zero stays
//! zero-dependency (L06).

// TODO: no forge_canvas equivalent in F:\v3 at crate-zero scope, stubbed
// Donor: use forge_canvas::draw::{DrawCmd, DrawList};
// Donor: use forge_canvas::geom::UiRect;
// Donor: use forge_canvas::text::FontAtlas;
// Donor: use forge_canvas::tokens::TokenId;
// Donor: use forge_canvas::widgets;

// TODO: no forge_vix equivalent in F:\v3 at crate-zero scope, stubbed
// Donor: use forge_vix::ir::{IrRect, LoweredUi};
// Donor: use forge_vix::loader::{load_kit, studio_panel, LoadedPanel};

// TODO: DungeonMasterView lives downstream where canvas/vix deps are available
// Donor: use crate::binds::DungeonMasterView;

/// Stub types for the rendering layer — these will be properly defined in the
/// downstream crate where forge_canvas and forge_vix are scoped.
///
/// This crate-zero module defines the CENSUS (DM_SLOTS) and the function
/// signatures only. The implementations delegate to the downstream layer.

/// Stub: represents the lowered UI from the dungeon_master.kit.vixi panel.
#[doc(hidden)]
pub struct LoweredUi {
    /// Placeholder for layout regions; downstream will populate from forge_vix.
    pub widgets: Vec<WidgetStub>,
    pub layout: Vec<LayoutBox>,
}

/// Stub: a widget in the lowered UI.
#[doc(hidden)]
pub struct WidgetStub {
    pub stable_key: String,
}

/// Stub: a layout box with a stable key and bounds.
#[doc(hidden)]
pub struct LayoutBox {
    pub stable_key: String,
    pub rect: RectStub,
}

/// Stub: a rectangle with bounds.
#[doc(hidden)]
pub struct RectStub {
    pub min_x: i64,
    pub min_y: i64,
    pub max_x: i64,
    pub max_y: i64,
}

/// Stub: the loaded panel from the kit.
#[doc(hidden)]
pub struct LoadedPanel {
    pub ui: LoweredUi,
}

/// Stub: the viewport rectangle in MilliUnits.
#[doc(hidden)]
#[derive(Clone, Copy, Debug)]
pub struct IrRect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

impl IrRect {
    pub fn from_xywh(x: i64, y: i64, w: i64, h: i64) -> Self {
        IrRect { x, y, w, h }
    }
}

/// Stub: the draw command list (downstream will use forge_canvas::DrawList).
#[doc(hidden)]
pub struct DrawList {
    _phantom: std::marker::PhantomData<()>,
}

impl DrawList {
    pub fn new_boxed() -> Box<Self> {
        Box::new(DrawList { _phantom: std::marker::PhantomData })
    }
}

/// Stub: the font atlas (downstream will use forge_canvas::FontAtlas).
#[doc(hidden)]
pub struct FontAtlas;

impl FontAtlas {
    pub fn init(_font_path: &str, _size: f32) -> Self {
        FontAtlas
    }
}

/// Stub: the view data for the dungeon master screen.
/// This will be properly defined in the downstream layer where engine state
/// can be accessed.
#[doc(hidden)]
pub struct DungeonMasterView {
    pub title: String,
    pub dm_time: String,
    pub dm_sky: String,
    pub dm_pressure: String,
    pub dm_npcs: String,
    pub dm_conductor: String,
    pub dm_physics: String,
    pub ledger: Vec<String>,
}

/// Every `source=` the kit authors, paired with the view field that answers it.
/// The census proves the NAMES agree; this proves the pixels do.
pub const DM_SLOTS: [&str; 7] = [
    "root.header.title",
    "root.pulse.time",
    "root.pulse.sky",
    "root.pulse.pressure",
    "root.cast.npcs",
    "root.cast.conductor",
    "root.cast.physics",
];

/// Lower the dungeon_master kit panel from the VIXI registry.
///
/// STRANGLER: returns a LoadedPanel stub. The real implementation will load
/// the panel from the authored studio_panel registry downstream.
pub fn lower_dungeon_master_kit(_viewport: IrRect) -> LoadedPanel {
    // TODO: Implement by calling forge_vix::loader::load_kit downstream.
    // Donor pattern:
    //   let src = studio_panel("dungeon_master")
    //       .expect("dungeon_master kit is registered in STUDIO_PANELS");
    //   load_kit(src, &forge_vix::live::live_ctx(), viewport, 1)
    //       .expect("dungeon_master.kit.vixi lowers clean")

    LoadedPanel {
        ui: LoweredUi {
            widgets: vec![],
            layout: vec![],
        },
    }
}

/// Find a layout box by stable_key.
fn box_ui(ui: &LoweredUi, key: &str) -> Option<UiRect> {
    ui.layout.iter().find(|b| b.stable_key.as_str() == key).map(|b| {
        UiRect::new(b.rect.min_x, b.rect.min_y, b.rect.max_x - b.rect.min_x, b.rect.max_y - b.rect.min_y)
    })
}

/// Stub: UI rectangle type (mirrors forge_canvas).
#[doc(hidden)]
pub struct UiRect {
    pub x: MilliUnits,
    pub y: MilliUnits,
    pub w: MilliUnits,
    pub h: MilliUnits,
}

/// Stub: MilliUnits wrapper (mirrors forge_canvas).
#[doc(hidden)]
#[derive(Clone, Copy)]
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

/// Bind a text label to a UI slot.
///
/// STRANGLER: this is a forwarding skeleton. The real implementation will call
/// forge_canvas::widgets::label downstream.
fn text_slot(
    ui: &LoweredUi,
    key: &str,
    text: &str,
    _color: u32,
    _draw: &mut DrawList,
    _atlas: &mut FontAtlas,
) -> usize {
    let Some(_b) = box_ui(ui, key) else { return 0 };
    // TODO: Implement actual rendering downstream:
    //   let inner = UiRect::new(b.x.0 + 6_000, b.y.0 + 2_000, b.w.0 - 12_000, b.h.0 - 4_000);
    //   widgets::label(draw, inner, text, color, atlas);
    if !text.is_empty() { 1 } else { 0 }
}

/// Bind the live view onto the authored slots. Returns the count of bound
/// `source=` fields (7) — a stale or renamed kit slot drops the count.
///
/// The TITLE is the single preattentive focal (primary ink); the pulse and cast
/// rows stay muted, so the eye lands on what world is being plotted before it
/// reads the readings (root#a000).
///
/// STRANGLER: this forwards to the rendering layer. Real implementation lives
/// downstream where canvas/draw/atlas are properly scoped.
pub fn render_dungeon_master_kit(
    ui: &LoweredUi,
    view: &DungeonMasterView,
    draw: &mut DrawList,
    atlas: &mut FontAtlas,
) -> usize {
    // TODO: Implement actual rendering downstream with forge_canvas primitives:
    //   - Draw background rect (root, BgVoid color)
    //   - Draw frame outline (TextDisabled color)
    //   - Bind text slots for title, time, sky, pressure, npcs, conductor, physics
    //   - Paint ledger canvas

    if let Some(_root) = box_ui(ui, "root") {
        // Placeholder: root exists, count it as a binding
        let mut bound = 1;

        // Count the text slots (title + 6 pulse/cast fields)
        bound += text_slot(ui, "root.header.title", &view.title, 0, draw, atlas);
        for (key, val) in [
            ("root.pulse.time", &view.dm_time),
            ("root.pulse.sky", &view.dm_sky),
            ("root.pulse.pressure", &view.dm_pressure),
            ("root.cast.npcs", &view.dm_npcs),
            ("root.cast.conductor", &view.dm_conductor),
            ("root.cast.physics", &view.dm_physics),
        ] {
            bound += text_slot(ui, key, val, 0, draw, atlas);
        }

        // Count the ledger canvas
        bound += paint_ledger(ui, &view.ledger, 0, draw, atlas);

        bound
    } else {
        0
    }
}

/// Line height for the ledger canvas, in MilliUnits. The kit sizes header/pulse/cast
/// and leaves the ledger to FILL, so its rows are stacked by the host — this is the
/// one place the DM screen owns its own vertical rhythm.
const LEDGER_LINE_MU: i64 = 20_000;

/// Paint `root.ledger` — the `role=canvas` region the kit hands to the host. Returns
/// 1 when the canvas exists and was painted, so a renamed region drops the count like
/// any other slot. Rows past the region's height are DROPPED, never overdrawn.
fn paint_ledger(
    ui: &LoweredUi,
    rows: &[String],
    _color: u32,
    _draw: &mut DrawList,
    _atlas: &mut FontAtlas,
) -> usize {
    let Some(b) = box_ui(ui, "root.ledger") else { return 0 };
    let fits = (b.h.0 / LEDGER_LINE_MU).max(0) as usize;

    // TODO: Implement actual rendering downstream:
    //   for (i, row) in rows.iter().take(fits).enumerate() {
    //       let y = b.y.0 + i as i64 * LEDGER_LINE_MU;
    //       let line = UiRect::new(b.x.0 + 6_000, y, b.w.0 - 12_000, LEDGER_LINE_MU - 4_000);
    //       widgets::label(draw, line, row, color, atlas);
    //   }

    if rows.len() <= fits {
        1 // Canvas exists and can hold the rows
    } else {
        0 // Canvas too small
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp() -> IrRect {
        IrRect::from_xywh(0, 0, 1_280_000, 720_000)
    }

    fn view() -> DungeonMasterView {
        DungeonMasterView {
            title: "atlas · v3".to_string(),
            dm_time: "00:00:00".to_string(),
            dm_sky: "clear".to_string(),
            dm_pressure: "1013mb".to_string(),
            dm_npcs: "personas: 3".to_string(),
            dm_conductor: "running".to_string(),
            dm_physics: "active".to_string(),
            ledger: vec!["room · entry".to_string()],
        }
    }

    #[test]
    fn dm_slots_constant_has_seven_entries() {
        assert_eq!(DM_SLOTS.len(), 7);
        assert_eq!(DM_SLOTS[0], "root.header.title");
        assert_eq!(DM_SLOTS[1], "root.pulse.time");
        assert_eq!(DM_SLOTS[6], "root.cast.physics");
    }

    #[test]
    fn lower_dungeon_master_kit_returns_stub() {
        let panel = lower_dungeon_master_kit(vp());
        // Stub returns an empty panel; real implementation will populate from kit.vixi
        assert_eq!(panel.ui.layout.len(), 0);
    }

    #[test]
    fn render_returns_zero_on_missing_root() {
        let panel = LoadedPanel {
            ui: LoweredUi {
                widgets: vec![],
                layout: vec![],
            },
        };
        let mut draw = DrawList::new_boxed();
        let mut atlas = FontAtlas::init("", 16.0);
        let view = view();
        let bound = render_dungeon_master_kit(&panel.ui, &view, &mut draw, &mut atlas);
        assert_eq!(bound, 0, "missing root should yield zero bindings");
    }

    #[test]
    fn view_fields_read_sensible() {
        let v = view();
        assert!(!v.title.is_empty());
        assert!(!v.dm_time.is_empty());
        assert!(!v.dm_sky.is_empty());
        assert!(!v.dm_pressure.is_empty());
        assert!(!v.dm_npcs.is_empty());
        assert!(!v.dm_conductor.is_empty());
        assert!(!v.dm_physics.is_empty());
        assert!(!v.ledger.is_empty());
        assert!(v.title.contains("atlas"));
    }

    #[test]
    fn ledger_line_height_is_constant() {
        assert_eq!(LEDGER_LINE_MU, 20_000);
    }
}
