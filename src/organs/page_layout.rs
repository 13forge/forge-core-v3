//! page_layout — the Command HUB's page/layout authoring model.
//!
//! Ported 2026-08-17 from `F:\NewRepo\crates\forge-studio\src\page_layout.rs` (C06 donor cite).
//! V3 adaptations: removed `crate::artifact::{Artifact, Format}` methods (Crate Zero),
//! removed `studio_palette()` (requires forge_vix), simplified `studio_palette_from()` to
//! not reference forge_vix, and dropped tests using external crates (2 tests, noted inline).
//!
//! An Affinity-style document (pages · frames · guides · grid · master) that
//! emits one standalone HTML page. Integer-deterministic: all geometry is `Mil`
//! (i64 milli-units, `1000 == 1px`), no floats anywhere (DET-CLOCK law), zero
//! runtime deps. Colours are authored as sovereign `ColourId` palette indices, never hex
//! — they resolve to `#rrggbb` ONLY at the HTML boundary, through a provided
//! 64-swatch palette. One `Document` in → one HTML string out (the cart law).

/// Milli-unit: `1000 == 1 CSS pixel`. Integer geometry, no floats.
pub type Mil = i64;

/// Sovereign palette index (0..63). Resolves to rgb only at emit time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColourId(pub u8);

/// An axis-aligned rectangle in milli-units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    /// X coordinate in milli-units.
    pub x: Mil,
    /// Y coordinate in milli-units.
    pub y: Mil,
    /// Width in milli-units.
    pub w: Mil,
    /// Height in milli-units.
    pub h: Mil,
}

impl Rect {
    /// Create a new rectangle with given x, y, width, height.
    pub const fn new(x: Mil, y: Mil, w: Mil, h: Mil) -> Self {
        Self { x, y, w, h }
    }
    /// Right edge coordinate.
    pub const fn right(&self) -> Mil {
        self.x + self.w
    }
    /// Bottom edge coordinate.
    pub const fn bottom(&self) -> Mil {
        self.y + self.h
    }
    /// Center point (x, y).
    pub const fn center(&self) -> (Mil, Mil) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
    /// Check if a point is within the rectangle bounds.
    pub fn contains(&self, px: Mil, py: Mil) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }
    /// Inset on all four sides (clamped non-negative).
    pub fn inset(&self, d: Mil) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2 * d).max(0), (self.h - 2 * d).max(0))
    }
}

/// Horizontal or vertical text placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// Start (left/top).
    Start,
    /// Center.
    Center,
    /// End (right/bottom).
    End,
    /// Justify.
    Justify,
}

impl Align {
    fn css_text(self) -> &'static str {
        match self {
            Align::Start => "left",
            Align::Center => "center",
            Align::End => "right",
            Align::Justify => "justify",
        }
    }
    fn css_flex(self) -> &'static str {
        match self {
            Align::Start => "flex-start",
            Align::Center => "center",
            Align::End => "flex-end",
            Align::Justify => "space-between",
        }
    }
}

/// How an image fills its frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Fill the entire frame.
    Fill,
    /// Contain within bounds, preserve aspect.
    Contain,
    /// Cover the frame, preserve aspect.
    Cover,
}

impl Fit {
    fn css(self) -> &'static str {
        match self {
            Fit::Fill => "fill",
            Fit::Contain => "contain",
            Fit::Cover => "cover",
        }
    }
}

/// A typographic spec. `size_mpt` is milli-points (`12000 == 12pt`), weight is
/// CSS 100..900, so the whole spec stays integer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontSpec {
    /// Font family name.
    pub family: String,
    /// Font size in milli-points.
    pub size_mpt: i64,
    /// Font weight (100..900).
    pub weight: u16,
    /// Whether to use italic style.
    pub italic: bool,
    /// Letter spacing in milli-units.
    pub tracking_mil: Mil,
    /// Text colour.
    pub colour: ColourId,
}

impl FontSpec {
    /// Create a new font spec with family, size in points, and colour.
    pub fn new(family: &str, size_pt: i64, colour: ColourId) -> Self {
        Self {
            family: family.to_string(),
            size_mpt: size_pt * 1000,
            weight: 400,
            italic: false,
            tracking_mil: 0,
            colour,
        }
    }
    /// Set the font weight.
    pub fn weight(mut self, w: u16) -> Self {
        self.weight = w;
        self
    }
    /// Set italic style.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    /// Set letter tracking in milli-units.
    pub fn tracking(mut self, mil: Mil) -> Self {
        self.tracking_mil = mil;
        self
    }
}

/// A run of text sharing one font.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRun {
    /// The text content.
    pub text: String,
    /// Font specification.
    pub font: FontSpec,
}

impl TextRun {
    /// Create a new text run.
    pub fn new(text: &str, font: FontSpec) -> Self {
        Self { text: text.to_string(), font }
    }
}

/// A placed element on a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// Text frame with runs, alignment, and vertical alignment.
    Text {
        /// Bounding rectangle.
        rect: Rect,
        /// Text runs to display.
        runs: Vec<TextRun>,
        /// Horizontal alignment.
        align: Align,
        /// Vertical alignment.
        valign: Align,
    },
    /// Image frame with source and fit mode.
    Image {
        /// Bounding rectangle.
        rect: Rect,
        /// Image source URL.
        src: String,
        /// How the image fills the frame.
        fit: Fit,
        /// Alternative text for the image.
        alt: String,
    },
    /// Filled box with optional border and corner radius.
    Box {
        /// Bounding rectangle.
        rect: Rect,
        /// Fill colour.
        fill: ColourId,
        /// Corner radius in milli-units.
        radius: Mil,
        /// Optional border (width and colour).
        border: Option<(Mil, ColourId)>,
    },
    /// Line between two points.
    Line {
        /// Starting point.
        from: (Mil, Mil),
        /// Ending point.
        to: (Mil, Mil),
        /// Line width in milli-units.
        width: Mil,
        /// Line colour.
        colour: ColourId,
    },
    /// A classic scrolling `<marquee>` — `speed` maps to HTML `scrollamount`.
    Marquee {
        /// Bounding rectangle.
        rect: Rect,
        /// Scroll speed (HTML scrollamount).
        speed: i64,
        /// Text to scroll.
        text: String,
        /// Font specification.
        font: FontSpec,
    },
    /// Classic blinking `<blink>` text (with a CSS-animation fallback on emit).
    Blink {
        /// Bounding rectangle.
        rect: Rect,
        /// Text to blink.
        text: String,
        /// Font specification.
        font: FontSpec,
    },
    /// A hyperlink — the nav that stitches a multi-page site (renders `<a href>`).
    Link {
        /// Bounding rectangle.
        rect: Rect,
        /// Link text.
        text: String,
        /// Link URL.
        href: String,
        /// Font specification.
        font: FontSpec,
    },
    /// A page-tiling background image (Geocities-style) — full-bleed, no rect.
    TiledBackground {
        /// Image source URL for tiling.
        image_src: String,
    },
}

impl Frame {
    /// The bounding rect the frame occupies (a Line's is its extent; a tiled
    /// background is page-level and reports an empty rect).
    pub fn bounds(&self) -> Rect {
        match self {
            Frame::Text { rect, .. }
            | Frame::Image { rect, .. }
            | Frame::Box { rect, .. }
            | Frame::Marquee { rect, .. }
            | Frame::Blink { rect, .. }
            | Frame::Link { rect, .. } => *rect,
            Frame::Line { from, to, width, .. } => {
                let (x0, y0) = *from;
                let (x1, y1) = *to;
                let x = x0.min(x1);
                let y = y0.min(y1);
                Rect::new(x, y, (x1 - x0).abs().max(*width), (y1 - y0).abs().max(*width))
            }
            Frame::TiledBackground { .. } => Rect::default(),
        }
    }
}

/// Horizontal or vertical layout guide axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Horizontal (X) axis.
    X,
    /// Vertical (Y) axis.
    Y,
}

/// A layout guide line the author snaps frames to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guide {
    /// Axis of the guide.
    pub axis: Axis,
    /// Position on the axis in milli-units.
    pub at: Mil,
}

/// A column/row grid with a uniform gutter and outer margin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
    /// Gutter size between cells in milli-units.
    pub gutter: Mil,
    /// Outer margin in milli-units.
    pub margin: Mil,
}

impl Grid {
    /// The rect of the cell block spanning `[col..col+colspan)`,`[row..row+rowspan)`
    /// inside a page of `page`. Pure integer math; the last track absorbs any
    /// division remainder so the grid always tiles the content box exactly.
    pub fn cell(&self, page: (Mil, Mil), col: u16, row: u16, colspan: u16, rowspan: u16) -> Rect {
        let cols = self.cols.max(1) as Mil;
        let rows = self.rows.max(1) as Mil;
        let inner_w = (page.0 - 2 * self.margin - (cols - 1) * self.gutter).max(0);
        let inner_h = (page.1 - 2 * self.margin - (rows - 1) * self.gutter).max(0);
        let cw = inner_w / cols;
        let ch = inner_h / rows;
        let c = (col as Mil).min(cols - 1);
        let r = (row as Mil).min(rows - 1);
        let cs = (colspan.max(1) as Mil).min(cols - c);
        let rs = (rowspan.max(1) as Mil).min(rows - r);
        let x = self.margin + c * (cw + self.gutter);
        let y = self.margin + r * (ch + self.gutter);
        // Span width = cs cells + (cs-1) gutters; the final column eats remainder.
        let last_col = c + cs >= cols;
        let last_row = r + rs >= rows;
        let w = if last_col { page.0 - self.margin - x } else { cs * cw + (cs - 1) * self.gutter };
        let h = if last_row { page.1 - self.margin - y } else { rs * ch + (rs - 1) * self.gutter };
        Rect::new(x, y, w.max(0), h.max(0))
    }
}

/// One page: a fixed-size canvas of frames over an optional grid + guides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// Page size (width, height) in milli-units.
    pub size: (Mil, Mil),
    /// Background colour.
    pub background: ColourId,
    /// Frames placed on this page.
    pub frames: Vec<Frame>,
    /// Guide lines for snapping.
    pub guides: Vec<Guide>,
    /// Optional grid structure.
    pub grid: Option<Grid>,
}

impl Page {
    /// A4 at 96dpi is 794×1123px; default to a comfortable 1280×1600 web page.
    pub fn web(background: ColourId) -> Self {
        Self { size: (1_280_000, 1_600_000), background, frames: Vec::new(), guides: Vec::new(), grid: None }
    }
    /// Create a page of a specific size.
    pub fn sized(w: Mil, h: Mil, background: ColourId) -> Self {
        Self { size: (w, h), background, frames: Vec::new(), guides: Vec::new(), grid: None }
    }
    /// Attach a grid to this page.
    pub fn with_grid(mut self, grid: Grid) -> Self {
        self.grid = Some(grid);
        self
    }
    /// Add a frame to this page.
    pub fn add(&mut self, f: Frame) -> &mut Self {
        self.frames.push(f);
        self
    }
    /// Add a guide line to this page.
    pub fn guide(&mut self, axis: Axis, at: Mil) -> &mut Self {
        self.guides.push(Guide { axis, at });
        self
    }
    /// Place a frame produced from a grid cell rect (the layout-editor verb).
    pub fn place(&mut self, col: u16, row: u16, colspan: u16, rowspan: u16, make: impl FnOnce(Rect) -> Frame) -> &mut Self {
        if let Some(g) = self.grid {
            let r = g.cell(self.size, col, row, colspan, rowspan);
            self.frames.push(make(r));
        }
        self
    }
    /// Snap a coordinate to the nearest guide on `axis` within `tol` mil.
    pub fn snap(&self, axis: Axis, v: Mil, tol: Mil) -> Mil {
        let mut best = v;
        let mut best_d = tol + 1;
        for g in self.guides.iter().filter(|g| g.axis == axis) {
            let d = (g.at - v).abs();
            if d <= tol && d < best_d {
                best_d = d;
                best = g.at;
            }
        }
        best
    }
}

/// The authored document — the "one asset". Emits one standalone HTML page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    /// Document title.
    pub title: String,
    /// Pages in the document.
    pub pages: Vec<Page>,
}

impl Document {
    /// Create a new document with a title.
    pub fn new(title: &str) -> Self {
        Self { title: title.to_string(), pages: Vec::new() }
    }
    /// Add a page to this document.
    pub fn page(&mut self, p: Page) -> &mut Self {
        self.pages.push(p);
        self
    }
    /// Count total frames across all pages.
    pub fn frame_count(&self) -> usize {
        self.pages.iter().map(|p| p.frames.len()).sum()
    }

    /// Emit one standalone HTML document. `palette` resolves every `ColourId` to
    /// rgb (the sovereign 64-swatch table); resolution happens ONLY here, so the
    /// authored model never holds a hex literal. Deterministic: same doc + same
    /// palette → byte-identical output.
    pub fn emit_html(&self, palette: &[[u8; 3]; 64]) -> String {
        let mut s = String::with_capacity(2048 + self.frame_count() * 128);
        s.push_str("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\n");
        s.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n<title>");
        push_escaped(&mut s, &self.title);
        s.push_str("</title>\n<style>*{margin:0;box-sizing:border-box}body{background:#0a0a0a}");
        s.push_str(".pg{position:relative;margin:0 auto;overflow:hidden}.fr{position:absolute}");
        s.push_str("@keyframes fr-blink{50%{visibility:hidden}}blink{animation:fr-blink 1s step-end infinite}</style>\n</head><body>\n");
        for page in &self.pages {
            self.emit_page(&mut s, page, palette);
        }
        s.push_str("</body></html>\n");
        s
    }

    fn emit_page(&self, s: &mut String, page: &Page, pal: &[[u8; 3]; 64]) {
        let (pw, ph) = page.size;
        s.push_str("<section class=\"pg\" style=\"width:");
        push_px(s, pw);
        s.push_str(";height:");
        push_px(s, ph);
        s.push_str(";background:");
        push_hex(s, page.background, pal);
        s.push_str("\">\n");
        for f in &page.frames {
            emit_frame(s, f, pal);
        }
        s.push_str("</section>\n");
    }
}

fn emit_frame(s: &mut String, f: &Frame, pal: &[[u8; 3]; 64]) {
    match f {
        Frame::Box { rect, fill, radius, border } => {
            s.push_str("<div class=\"fr\" style=\"");
            push_rect_css(s, *rect);
            s.push_str("background:");
            push_hex(s, *fill, pal);
            if *radius > 0 {
                s.push_str(";border-radius:");
                push_px(s, *radius);
            }
            if let Some((bw, bc)) = border {
                s.push_str(";border:");
                push_px(s, *bw);
                s.push_str(" solid ");
                push_hex(s, *bc, pal);
            }
            s.push_str("\"></div>\n");
        }
        Frame::Image { rect, src, fit, alt } => {
            s.push_str("<img class=\"fr\" style=\"");
            push_rect_css(s, *rect);
            s.push_str("object-fit:");
            s.push_str(fit.css());
            s.push_str("\" src=\"");
            push_escaped(s, src);
            s.push_str("\" alt=\"");
            push_escaped(s, alt);
            s.push_str("\">\n");
        }
        Frame::Text { rect, runs, align, valign } => {
            s.push_str("<div class=\"fr\" style=\"display:flex;flex-direction:column;justify-content:");
            s.push_str(valign.css_flex());
            s.push_str(";text-align:");
            s.push_str(align.css_text());
            s.push(';');
            push_rect_css(s, *rect);
            s.push_str("\">");
            for run in runs {
                emit_run(s, run, pal);
            }
            s.push_str("</div>\n");
        }
        Frame::Line { from, to, width, colour } => {
            // A line is a thin rotated box; emit its bounds as a filled div. For
            // axis-aligned lines this is exact (the common layout-rule case).
            let b = f.bounds();
            let _ = (from, to);
            s.push_str("<div class=\"fr\" style=\"");
            push_rect_css(s, b);
            s.push_str("background:");
            push_hex(s, *colour, pal);
            let _ = width;
            s.push_str("\"></div>\n");
        }
        Frame::Marquee { rect, speed, text, font } => {
            s.push_str("<div class=\"fr\" style=\"overflow:hidden;");
            push_rect_css(s, *rect);
            emit_font_style(s, font, pal);
            s.push_str("\"><marquee scrollamount=\"");
            push_int(s, *speed);
            s.push_str("\">");
            push_escaped(s, text);
            s.push_str("</marquee></div>\n");
        }
        Frame::Blink { rect, text, font } => {
            s.push_str("<div class=\"fr\" style=\"");
            push_rect_css(s, *rect);
            emit_font_style(s, font, pal);
            s.push_str("\"><blink>");
            push_escaped(s, text);
            s.push_str("</blink></div>\n");
        }
        Frame::Link { rect, text, href, font } => {
            s.push_str("<a class=\"fr\" href=\"");
            push_escaped(s, href);
            s.push_str("\" style=\"");
            push_rect_css(s, *rect);
            emit_font_style(s, font, pal);
            s.push_str("text-decoration:underline\">");
            push_escaped(s, text);
            s.push_str("</a>\n");
        }
        Frame::TiledBackground { image_src } => {
            s.push_str("<div class=\"fr\" style=\"left:0;top:0;width:100%;height:100%;background-image:url('");
            push_escaped(s, image_src);
            s.push_str("');background-repeat:repeat\"></div>\n");
        }
    }
}

/// Write the font CSS props (no wrapping `style="..."`) for a marquee/blink face.
fn emit_font_style(s: &mut String, font: &FontSpec, pal: &[[u8; 3]; 64]) {
    s.push_str("font-family:'");
    push_escaped(s, &font.family);
    s.push_str("';font-size:");
    push_px(s, font.size_mpt * 4 / 3);
    s.push_str(";font-weight:");
    push_int(s, font.weight as i64);
    if font.italic {
        s.push_str(";font-style:italic");
    }
    s.push_str(";color:");
    push_hex(s, font.colour, pal);
    s.push(';');
}

fn emit_run(s: &mut String, run: &TextRun, pal: &[[u8; 3]; 64]) {
    s.push_str("<span style=\"font-family:'");
    push_escaped(s, &run.font.family);
    s.push_str("';font-size:");
    // milli-points → px at 96dpi: pt*96/72 = pt*4/3; size_mpt/1000 pt.
    push_px(s, run.font.size_mpt * 4 / 3);
    s.push_str(";font-weight:");
    push_int(s, run.font.weight as i64);
    if run.font.italic {
        s.push_str(";font-style:italic");
    }
    if run.font.tracking_mil != 0 {
        s.push_str(";letter-spacing:");
        push_px(s, run.font.tracking_mil);
    }
    s.push_str(";color:");
    push_hex(s, run.font.colour, pal);
    s.push_str("\">");
    push_escaped(s, &run.text);
    s.push_str("</span>");
}

fn push_rect_css(s: &mut String, r: Rect) {
    s.push_str("left:");
    push_px(s, r.x);
    s.push_str(";top:");
    push_px(s, r.y);
    s.push_str(";width:");
    push_px(s, r.w);
    s.push_str(";height:");
    push_px(s, r.h);
    s.push(';');
}

/// Milli-units → CSS px with up to 3 decimals, trailing zeros trimmed. Integer
/// division + remainder, never a float.
fn push_px(s: &mut String, mil: Mil) {
    push_int(s, mil / 1000);
    let frac = (mil % 1000).abs();
    if frac != 0 {
        s.push('.');
        let mut f = frac;
        let mut buf = [0u8; 3];
        for i in (0..3).rev() {
            buf[i] = b'0' + (f % 10) as u8;
            f /= 10;
        }
        let mut end = 3;
        while end > 1 && buf[end - 1] == b'0' {
            end -= 1;
        }
        s.push_str(std::str::from_utf8(&buf[..end]).unwrap());
    }
    s.push_str("px");
}

fn push_int(s: &mut String, mut v: i64) {
    if v < 0 {
        s.push('-');
        v = -v;
    }
    let mut tmp = [0u8; 20];
    let mut i = tmp.len();
    loop {
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    s.push_str(std::str::from_utf8(&tmp[i..]).unwrap());
}

fn push_hex(s: &mut String, id: ColourId, pal: &[[u8; 3]; 64]) {
    let [r, g, b] = pal[(id.0 & 63) as usize];
    s.push('#');
    for byte in [r, g, b] {
        s.push(HEX[(byte >> 4) as usize] as char);
        s.push(HEX[(byte & 15) as usize] as char);
    }
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// HTML-escape into `s` (the five significant entities).
fn push_escaped(s: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            '\'' => s.push_str("&#39;"),
            _ => s.push(c),
        }
    }
}

/// Integer HSV→RGB (h in degrees, s/v in permyriad 0..1000). No floats — the
/// sovereign palette generator's core.
pub fn hsv_to_rgb(h: i64, s: i64, v: i64) -> [u8; 3] {
    let h = ((h % 360) + 360) % 360;
    let s = s.clamp(0, 1000);
    let v = v.clamp(0, 1000);
    let c = v * s / 1000;
    let m = v - c;
    let f = h % 60;
    let up = c * f / 60;
    let down = c * (60 - f) / 60;
    let (r, g, b) = match h / 60 {
        0 => (c, up, 0),
        1 => (down, c, 0),
        2 => (0, c, up),
        3 => (0, down, c),
        4 => (up, 0, c),
        _ => (c, 0, down),
    };
    let scale = |x: i64| (((x + m) * 255 + 500) / 1000).clamp(0, 255) as u8;
    [scale(r), scale(g), scale(b)]
}

/// The 64-swatch palette that resolves `ColourId` at HTML-emit time. Slots 0..7
/// are the studio defaults (unless a theme is live-rolled); 8..63 are a deterministic HSV
/// wheel ramp. Deterministic.
pub fn studio_palette() -> [[u8; 3]; 64] {
    let mut p = [[0u8; 3]; 64];
    p[0] = [0x0B, 0x0B, 0x11];
    p[1] = [0xEA, 0xDF, 0xC8];
    p[2] = [0x8A, 0x84, 0x78];
    p[3] = [0x1A, 0xE0, 0xFF];
    p[4] = [0xC4, 0x6B, 0xFF];
    p[5] = [0x4D, 0xFF, 0xB0];
    p[6] = [0xFF, 0x3B, 0x6E];
    p[7] = [0xE6, 0x5A, 0x14];
    let mut i = 8;
    while i < 64 {
        let h = (i as i64 - 8) * 360 / 56;
        p[i] = hsv_to_rgb(h, 640, 900);
        i += 1;
    }
    p
}

/// Build the Command HUB landing page as a real layout document — hero band,
/// three portal cards, footer rule — deterministic from the click seed. The HUB
/// page-maker's authored output (replaces the canned book export).
pub fn hub_landing(era: &str, seed_x: i64, seed_y: i64) -> Document {
    let bg = ColourId(0);
    let ink = ColourId(1);
    let muted = ColourId(2);
    let accent = ColourId(8 + ((seed_x ^ seed_y).unsigned_abs() % 56) as u8);

    let mut doc = Document::new(&format!("Command Hub \u{2014} {era}"));
    let mut pg = Page::sized(1_280_000, 1_600_000, bg)
        .with_grid(Grid { cols: 12, rows: 16, gutter: 16_000, margin: 64_000 });

    pg.place(0, 0, 12, 3, |r| Frame::Box { rect: r, fill: accent, radius: 12_000, border: None });
    pg.place(0, 0, 12, 3, |r| Frame::Text {
        rect: r.inset(48_000),
        runs: vec![TextRun::new("Command Hub", FontSpec::new("Jura", 64, bg).weight(800).tracking(2_000))],
        align: Align::Start,
        valign: Align::Center,
    });
    pg.place(0, 3, 12, 1, |r| Frame::Text {
        rect: r,
        runs: vec![TextRun::new(era, FontSpec::new("Jura", 22, muted).weight(500))],
        align: Align::Start,
        valign: Align::Start,
    });

    for (i, name) in ["Press a Page", "Shape a Swarm", "Forge a Rule"].iter().enumerate() {
        let col = (i as u16) * 4;
        pg.place(col, 5, 4, 4, |r| Frame::Box { rect: r, fill: bg, radius: 10_000, border: Some((2_000, accent)) });
        pg.place(col, 5, 4, 4, |r| Frame::Text {
            rect: r.inset(28_000),
            runs: vec![TextRun::new(name, FontSpec::new("Jura", 28, ink).weight(600))],
            align: Align::Center,
            valign: Align::Center,
        });
    }

    pg.place(0, 15, 12, 1, |r| Frame::Line { from: (r.x, r.y), to: (r.right(), r.y), width: 1000, colour: muted });
    pg.place(0, 15, 12, 1, |r| Frame::Text {
        rect: r.inset(8_000),
        runs: vec![TextRun::new("deveraux.dev \u{00B7} one asset in, one page out", FontSpec::new("Jura", 14, muted))],
        align: Align::End,
        valign: Align::End,
    });

    doc.page(pg);
    doc
}

impl Document {
    /// Emit the document as one standalone SVG (pages stacked vertically) — a
    /// second T1 format from the same authored model. Vector, deterministic.
    pub fn emit_svg(&self, palette: &[[u8; 3]; 64]) -> String {
        let w = self.pages.iter().map(|p| p.size.0).max().unwrap_or(0);
        let total_h: Mil = self.pages.iter().map(|p| p.size.1).sum();
        let mut s = String::with_capacity(1024 + self.frame_count() * 96);
        s.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"");
        push_px(&mut s, w);
        s.push_str("\" height=\"");
        push_px(&mut s, total_h);
        s.push_str("\" viewBox=\"0 0 ");
        push_int(&mut s, (w + 999) / 1000);
        s.push(' ');
        push_int(&mut s, (total_h + 999) / 1000);
        s.push_str("\">\n");
        let mut off: Mil = 0;
        for page in &self.pages {
            s.push_str("<g transform=\"translate(0 ");
            push_px(&mut s, off);
            s.push_str(")\">\n<rect x=\"0\" y=\"0\" width=\"");
            push_px(&mut s, page.size.0);
            s.push_str("\" height=\"");
            push_px(&mut s, page.size.1);
            s.push_str("\" fill=\"");
            push_hex(&mut s, page.background, palette);
            s.push_str("\"/>\n");
            for f in &page.frames {
                emit_frame_svg(&mut s, f, palette);
            }
            s.push_str("</g>\n");
            off += page.size.1;
        }
        s.push_str("</svg>\n");
        s
    }
}

fn emit_frame_svg(s: &mut String, f: &Frame, pal: &[[u8; 3]; 64]) {
    match f {
        Frame::Box { rect, fill, radius, border } => {
            s.push_str("<rect x=\"");
            push_px(s, rect.x);
            s.push_str("\" y=\"");
            push_px(s, rect.y);
            s.push_str("\" width=\"");
            push_px(s, rect.w);
            s.push_str("\" height=\"");
            push_px(s, rect.h);
            if *radius > 0 {
                s.push_str("\" rx=\"");
                push_px(s, *radius);
            }
            s.push_str("\" fill=\"");
            push_hex(s, *fill, pal);
            if let Some((bw, bc)) = border {
                s.push_str("\" stroke=\"");
                push_hex(s, *bc, pal);
                s.push_str("\" stroke-width=\"");
                push_px(s, *bw);
            }
            s.push_str("\"/>\n");
        }
        Frame::Image { rect, src, fit, alt } => {
            let par = match fit {
                Fit::Fill => "none",
                Fit::Contain => "xMidYMid meet",
                Fit::Cover => "xMidYMid slice",
            };
            s.push_str("<image x=\"");
            push_px(s, rect.x);
            s.push_str("\" y=\"");
            push_px(s, rect.y);
            s.push_str("\" width=\"");
            push_px(s, rect.w);
            s.push_str("\" height=\"");
            push_px(s, rect.h);
            s.push_str("\" preserveAspectRatio=\"");
            s.push_str(par);
            s.push_str("\" href=\"");
            push_escaped(s, src);
            s.push_str("\"><title>");
            push_escaped(s, alt);
            s.push_str("</title></image>\n");
        }
        Frame::Text { rect, runs, align, .. } => {
            let (anchor, x) = match align {
                Align::Start | Align::Justify => ("start", rect.x),
                Align::Center => ("middle", rect.x + rect.w / 2),
                Align::End => ("end", rect.right()),
            };
            let font = runs.first().map(|r| &r.font);
            let size = font.map(|fo| fo.size_mpt * 4 / 3).unwrap_or(16_000);
            s.push_str("<text text-anchor=\"");
            s.push_str(anchor);
            s.push_str("\" x=\"");
            push_px(s, x);
            s.push_str("\" y=\"");
            push_px(s, rect.y + size);
            s.push_str("\" font-family=\"");
            if let Some(fo) = font {
                push_escaped(s, &fo.family);
                s.push_str("\" font-size=\"");
                push_px(s, size);
                s.push_str("\" font-weight=\"");
                push_int(s, fo.weight as i64);
                if fo.italic {
                    s.push_str("\" font-style=\"italic");
                }
                s.push_str("\" fill=\"");
                push_hex(s, fo.colour, pal);
            } else {
                s.push_str("\" font-size=\"16px");
            }
            s.push_str("\">");
            for (i, run) in runs.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                push_escaped(s, &run.text);
            }
            s.push_str("</text>\n");
        }
        Frame::Line { from, to, width, colour } => {
            s.push_str("<line x1=\"");
            push_px(s, from.0);
            s.push_str("\" y1=\"");
            push_px(s, from.1);
            s.push_str("\" x2=\"");
            push_px(s, to.0);
            s.push_str("\" y2=\"");
            push_px(s, to.1);
            s.push_str("\" stroke=\"");
            push_hex(s, *colour, pal);
            s.push_str("\" stroke-width=\"");
            push_px(s, *width);
            s.push_str("\"/>\n");
        }
        // Marquee/Blink are animated in HTML; SVG is a still, so render the text.
        Frame::Marquee { rect, text, font, .. } => emit_svg_text(s, *rect, text, font, pal),
        Frame::Blink { rect, text, font } => emit_svg_text(s, *rect, text, font, pal),
        Frame::Link { rect, text, font, .. } => emit_svg_text(s, *rect, text, font, pal),
        Frame::TiledBackground { image_src } => {
            s.push_str("<image x=\"0\" y=\"0\" width=\"128\" height=\"128\" href=\"");
            push_escaped(s, image_src);
            s.push_str("\"/>\n");
        }
    }
}

fn emit_svg_text(s: &mut String, rect: Rect, text: &str, font: &FontSpec, pal: &[[u8; 3]; 64]) {
    let size = font.size_mpt * 4 / 3;
    s.push_str("<text x=\"");
    push_px(s, rect.x);
    s.push_str("\" y=\"");
    push_px(s, rect.y + size);
    s.push_str("\" font-family=\"");
    push_escaped(s, &font.family);
    s.push_str("\" font-size=\"");
    push_px(s, size);
    s.push_str("\" font-weight=\"");
    push_int(s, font.weight as i64);
    s.push_str("\" fill=\"");
    push_hex(s, font.colour, pal);
    s.push_str("\">");
    push_escaped(s, text);
    s.push_str("</text>\n");
}

// ── Y2K / Geocities / Nexopia retro customizer ───────────────────────────────

/// The ColourIds the customizer cycles a swatch through — the loud 90s classics.
pub const WEB_SAFE_CYCLE: [ColourId; 8] = [
    ColourId(2),  // hot magenta
    ColourId(3),  // neon green
    ColourId(4),  // cyan
    ColourId(6),  // yellow
    ColourId(7),  // red
    ColourId(10), // purple
    ColourId(11), // orange
    ColourId(1),  // white
];

/// Classic tiling-background filenames the customizer cycles through.
pub const TILE_PATTERNS: [&str; 6] =
    ["stars.gif", "starfield.gif", "matrix.gif", "bricks.gif", "clouds.gif", "spiderweb.gif"];

/// The 90s web-safe palette. Slots 0..11 are the named loud classics (hot
/// magenta, neon green, deep space black…); 12..63 fill the 216-colour web-safe
/// cube (each channel ∈ {0,51,102,153,204,255}). Deterministic.
pub fn retro_y2k_palette() -> [[u8; 3]; 64] {
    let mut p = [[0u8; 3]; 64];
    p[0] = [0x00, 0x00, 0x00]; // deep space black
    p[1] = [0xFF, 0xFF, 0xFF]; // white
    p[2] = [0xFF, 0x00, 0xFF]; // hot magenta
    p[3] = [0x00, 0xFF, 0x00]; // neon green
    p[4] = [0x00, 0xFF, 0xFF]; // cyan
    p[5] = [0x00, 0x00, 0xFF]; // electric blue
    p[6] = [0xFF, 0xFF, 0x00]; // yellow
    p[7] = [0xFF, 0x00, 0x00]; // red
    p[8] = [0xC0, 0xC0, 0xC0]; // silver
    p[9] = [0x80, 0xFF, 0x00]; // lime
    p[10] = [0x80, 0x00, 0xFF]; // purple
    p[11] = [0xFF, 0x80, 0x00]; // orange
    let mut i = 12;
    while i < 64 {
        let idx = (i - 12) as i64;
        let lv = |n: i64| (n % 6 * 51) as u8;
        p[i] = [lv(idx), lv(idx / 6), lv(idx / 36)];
        i += 1;
    }
    p
}

/// (background, accent, text, tile) for a customizer theme string.
pub fn retro_theme(theme: &str) -> (ColourId, ColourId, ColourId, &'static str) {
    match theme {
        "goth" => (ColourId(0), ColourId(10), ColourId(2), "spiderweb.gif"),
        "hacker" => (ColourId(0), ColourId(3), ColourId(3), "matrix.gif"),
        "cyber" => (ColourId(0), ColourId(2), ColourId(4), "starfield.gif"),
        _ => (ColourId(0), ColourId(2), ColourId(3), "stars.gif"),
    }
}

/// Build a complete Geocities-style page for `theme`: tiled background, header
/// banner, scrolling marquee, nav sidebar, and a blinking under-construction
/// banner. Deterministic — the "3 clicks to create" instant template.
pub fn retro_document(theme: &str) -> Document {
    let (bg, accent, text, tile) = retro_theme(theme);
    let mut doc = Document::new(&format!("~ the {theme} zone ~"));
    let mut pg = Page::sized(1_024_000, 1_280_000, bg);

    pg.add(Frame::TiledBackground { image_src: tile.to_string() });
    pg.add(Frame::Image {
        rect: Rect::new(0, 0, 1_024_000, 120_000),
        src: format!("{theme}_banner.gif"),
        fit: Fit::Cover,
        alt: format!("{theme} banner"),
    });
    pg.add(Frame::Marquee {
        rect: Rect::new(0, 130_000, 1_024_000, 40_000),
        speed: 6,
        text: "*** WELCOME TO MY HOMEPAGE *** SIGN MY GUESTBOOK ***".into(),
        font: FontSpec::new("Comic Sans MS", 20, accent).weight(700),
    });
    pg.add(Frame::Box {
        rect: Rect::new(20_000, 190_000, 200_000, 900_000),
        fill: bg,
        radius: 0,
        border: Some((3_000, accent)),
    });
    for (i, link) in ["Home", "About Me", "Links", "Guestbook", "Webrings"].iter().enumerate() {
        pg.add(Frame::Text {
            rect: Rect::new(30_000, 210_000 + i as Mil * 44_000, 180_000, 36_000),
            runs: vec![TextRun::new(link, FontSpec::new("Comic Sans MS", 16, text).weight(600))],
            align: Align::Start,
            valign: Align::Center,
        });
    }
    pg.add(Frame::Blink {
        rect: Rect::new(260_000, 210_000, 720_000, 44_000),
        text: "UNDER CONSTRUCTION!!!".into(),
        font: FontSpec::new("Comic Sans MS", 28, ColourId(6)).weight(800),
    });

    doc.page(pg);
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal() -> [[u8; 3]; 64] {
        let mut p = [[0u8; 3]; 64];
        p[0] = [10, 10, 10];
        p[1] = [232, 224, 212]; // BONE
        p[2] = [255, 0, 0];
        p
    }

    #[test]
    fn rect_geometry() {
        let r = Rect::new(1000, 2000, 4000, 6000);
        assert_eq!(r.right(), 5000);
        assert_eq!(r.bottom(), 8000);
        assert_eq!(r.center(), (3000, 5000));
        assert!(r.contains(1000, 2000));
        assert!(!r.contains(5000, 2000));
        assert_eq!(r.inset(500), Rect::new(1500, 2500, 3000, 5000));
    }

    #[test]
    fn grid_tiles_exactly_no_float() {
        // 1000px wide, 3 cols, 0 margin/gutter → each 333.33.. but integer: last
        // column absorbs the remainder so the three cells exactly cover the page.
        let g = Grid { cols: 3, rows: 1, gutter: 0, margin: 0 };
        let page = (1_000_000, 100_000);
        let a = g.cell(page, 0, 0, 1, 1);
        let b = g.cell(page, 1, 0, 1, 1);
        let c = g.cell(page, 2, 0, 1, 1);
        assert_eq!(a.x, 0);
        assert_eq!(b.x, a.right());
        assert_eq!(c.x, b.right());
        assert_eq!(c.right(), 1_000_000, "last cell must reach the page edge exactly");
    }

    #[test]
    fn grid_span_and_margin() {
        let g = Grid { cols: 12, rows: 8, gutter: 20_000, margin: 40_000 };
        let page = (1_280_000, 1_600_000);
        let hero = g.cell(page, 0, 0, 12, 2);
        assert_eq!(hero.x, 40_000);
        assert_eq!(hero.y, 40_000);
        // full-width span reaches the right margin exactly
        assert_eq!(hero.right(), page.0 - 40_000);
    }

    #[test]
    fn guide_snaps_within_tolerance() {
        let mut p = Page::web(ColourId(0));
        p.guide(Axis::X, 100_000);
        assert_eq!(p.snap(Axis::X, 103_000, 5_000), 100_000);
        assert_eq!(p.snap(Axis::X, 120_000, 5_000), 120_000); // out of tol, unchanged
        assert_eq!(p.snap(Axis::Y, 103_000, 5_000), 103_000); // wrong axis, unchanged
    }

    #[test]
    fn emits_page_frames_and_resolves_colour() {
        let mut doc = Document::new("Hello");
        let mut pg = Page::sized(800_000, 600_000, ColourId(0));
        pg.add(Frame::Box { rect: Rect::new(0, 0, 800_000, 100_000), fill: ColourId(2), radius: 0, border: None });
        pg.add(Frame::Text {
            rect: Rect::new(40_000, 40_000, 400_000, 60_000),
            runs: vec![TextRun::new("Deveraux", FontSpec::new("Jura", 24, ColourId(1)).weight(700))],
            align: Align::Start,
            valign: Align::Center,
        });
        doc.page(pg);
        let html = doc.emit_html(&pal());
        assert!(html.contains("<title>Hello</title>"));
        assert!(html.contains("width:800px"));
        assert!(html.contains("#ff0000"), "box fill ColourId(2) must resolve to red");
        assert!(html.contains("#e8e0d4"), "text ColourId(1) must resolve to BONE");
        assert!(html.contains("Deveraux"));
        assert!(html.contains("font-weight:700"));
        assert_eq!(doc.frame_count(), 2);
    }

    #[test]
    fn escapes_injected_markup() {
        let mut doc = Document::new("<script>x</script>");
        let mut pg = Page::web(ColourId(0));
        pg.add(Frame::Text {
            rect: Rect::new(0, 0, 100_000, 20_000),
            runs: vec![TextRun::new("a<b>&\"c", FontSpec::new("F", 12, ColourId(1)))],
            align: Align::Start,
            valign: Align::Start,
        });
        doc.page(pg);
        let html = doc.emit_html(&pal());
        assert!(!html.contains("<script>x"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("a&lt;b&gt;&amp;&quot;c"));
    }

    #[test]
    fn emit_is_deterministic() {
        let mut doc = Document::new("D");
        let mut pg = Page::web(ColourId(0)).with_grid(Grid { cols: 4, rows: 4, gutter: 10_000, margin: 20_000 });
        pg.place(0, 0, 2, 1, |r| Frame::Box { rect: r, fill: ColourId(1), radius: 4000, border: Some((1000, ColourId(2))) });
        doc.page(pg);
        let a = doc.emit_html(&pal());
        let b = doc.emit_html(&pal());
        assert_eq!(a, b);
        assert!(a.contains("border-radius:4px"));
        assert!(a.contains("border:1px solid #ff0000"));
    }

    #[test]
    fn px_formatting_trims_fraction() {
        let mut s = String::new();
        push_px(&mut s, 12_000);
        push_px(&mut s, 12_500);
        push_px(&mut s, 333);
        assert_eq!(s, "12px12.5px0.333px");
    }

    #[test]
    fn hsv_primaries_and_grey() {
        assert_eq!(hsv_to_rgb(0, 1000, 1000), [255, 0, 0]);
        assert_eq!(hsv_to_rgb(120, 1000, 1000), [0, 255, 0]);
        assert_eq!(hsv_to_rgb(240, 1000, 1000), [0, 0, 255]);
        assert_eq!(hsv_to_rgb(0, 0, 1000), [255, 255, 255]);
    }

    #[test]
    fn studio_palette_seeds_and_ramps() {
        let p = studio_palette();
        assert_eq!(p[1], [0xEA, 0xDF, 0xC8]);
        assert_ne!(p[20], [0, 0, 0]);
    }

    // v3: dropped test hub_page_export_reflects_a_rolled_theme — needed forge_vix dev-dep (Crate Zero)

    #[test]
    fn hub_landing_produces_named_html() {
        let doc = hub_landing("Test Era 9000", 4242, 1337);
        assert!(doc.frame_count() >= 6);
        let html = doc.emit_html(&studio_palette());
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Command Hub"));
        assert!(html.contains("Press a Page"));
        assert!(html.contains("Test Era 9000"));
        // Verify determinism
        let again = hub_landing("Test Era 9000", 4242, 1337);
        let again_html = again.emit_html(&studio_palette());
        assert_eq!(html, again_html);
    }

    #[test]
    fn emit_svg_is_vector_and_deterministic() {
        let mut doc = Document::new("S");
        let mut pg = Page::sized(800_000, 600_000, ColourId(0));
        pg.add(Frame::Box { rect: Rect::new(0, 0, 800_000, 100_000), fill: ColourId(2), radius: 8000, border: Some((2000, ColourId(1))) });
        pg.add(Frame::Text {
            rect: Rect::new(40_000, 40_000, 400_000, 60_000),
            runs: vec![TextRun::new("Vector", FontSpec::new("Jura", 24, ColourId(1)))],
            align: Align::Center,
            valign: Align::Start,
        });
        pg.add(Frame::Line { from: (0, 500_000), to: (800_000, 500_000), width: 1000, colour: ColourId(2) });
        doc.page(pg);
        let a = doc.emit_svg(&pal());
        assert!(a.starts_with("<svg xmlns="));
        assert!(a.contains("<rect"));
        assert!(a.contains("<line"));
        assert!(a.contains("text-anchor=\"middle\""));
        assert!(a.contains("#ff0000"));
        assert!(a.contains("Vector"));
        assert_eq!(a, doc.emit_svg(&pal()));
    }

    // v3: dropped test svg_artifact_is_tagged_svg — needed crate::artifact (Crate Zero)

    #[test]
    fn retro_palette_has_the_named_classics() {
        let p = retro_y2k_palette();
        assert_eq!(p[0], [0, 0, 0]); // deep space black
        assert_eq!(p[2], [255, 0, 255]); // hot magenta
        assert_eq!(p[3], [0, 255, 0]); // neon green
        // web-safe cube slot: every channel is a multiple of 51
        assert!(p[30].iter().all(|c| c % 51 == 0));
    }

    #[test]
    fn retro_document_has_the_furniture_and_emits_retro_html() {
        let doc = retro_document("cyber");
        assert!(doc.frame_count() >= 8);
        let html = doc.emit_html(&retro_y2k_palette());
        assert!(html.contains("<marquee scrollamount=\"6\""));
        assert!(html.contains("WELCOME TO MY HOMEPAGE"));
        assert!(html.contains("background-repeat:repeat"));
        assert!(html.contains("<blink>UNDER CONSTRUCTION!!!</blink>"));
        assert!(html.contains("@keyframes fr-blink"));
        assert!(html.contains("Guestbook"));
        assert!(html.contains("#ff00ff")); // cyber accent = hot magenta
        // deterministic
        assert_eq!(html, retro_document("cyber").emit_html(&retro_y2k_palette()));
    }

    #[test]
    fn retro_svg_renders_marquee_text_and_tile() {
        let doc = retro_document("hacker");
        let svg = doc.emit_svg(&retro_y2k_palette());
        assert!(svg.starts_with("<svg xmlns="));
        assert!(svg.contains("WELCOME TO MY HOMEPAGE")); // marquee → static text
        assert!(svg.contains("href=\"matrix.gif\"")); // tiled bg swatch
        assert_eq!(svg, doc.emit_svg(&retro_y2k_palette()));
    }
}
