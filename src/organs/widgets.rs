//! widgets — the retro furniture kit: classic 90s components as Frame builders.
//!
//! Ported 2026-08-17 from `F:\NewRepo\crates\forge-studio\src\widgets.rs` (C06 donor cite).
//! V3 adaptation: changed import from `use crate::page_layout` to `use super::page_layout`.
//!
//! 88×31 buttons, hit counters, webring nav, "best viewed in" badges, sign-my-
//! guestbook links, under-construction banners. Each returns Frames positioned
//! at an anchor so a 6-year-old (or 60-year-old) drops one in with one click.
//! Pure, deterministic, integer geometry.

use super::page_layout::{Align, ColourId, FontSpec, Frame, Mil, Rect, TextRun};

/// The iconic 88×31 web button, as a bordered box + a centred link.
pub fn button_88x31(x: Mil, y: Mil, label: &str, href: &str, fg: ColourId, bg: ColourId) -> Vec<Frame> {
    let rect = Rect::new(x, y, 88_000, 31_000);
    vec![
        Frame::Box { rect, fill: bg, radius: 0, border: Some((1_000, fg)) },
        Frame::Link {
            rect: rect.inset(3_000),
            text: label.to_string(),
            href: href.to_string(),
            font: FontSpec::new("MS Sans Serif", 8, fg).weight(700),
        },
    ]
}

/// A "You are visitor #N" hit counter (8-digit zero-padded, monospace).
pub fn hit_counter(x: Mil, y: Mil, n: u64, fg: ColourId) -> Frame {
    Frame::Text {
        rect: Rect::new(x, y, 400_000, 36_000),
        runs: vec![TextRun::new(
            &format!("You are visitor #{n:08}"),
            FontSpec::new("Courier New", 16, fg).weight(700),
        )],
        align: Align::Start,
        valign: Align::Center,
    }
}

/// A blinking "UNDER CONSTRUCTION!!!" banner.
pub fn under_construction(x: Mil, y: Mil, w: Mil, fg: ColourId) -> Frame {
    Frame::Blink {
        rect: Rect::new(x, y, w, 44_000),
        text: "UNDER CONSTRUCTION!!!".to_string(),
        font: FontSpec::new("Comic Sans MS", 28, fg).weight(800),
    }
}

/// A webring nav row: `<< prev   [ random ]   next >>` as three links.
pub fn webring(x: Mil, y: Mil, ring: &str, fg: ColourId) -> Vec<Frame> {
    let font = FontSpec::new("MS Sans Serif", 12, fg).weight(600);
    vec![
        Frame::Link { rect: Rect::new(x, y, 120_000, 30_000), text: "<< prev".into(), href: format!("{ring}?prev"), font: font.clone() },
        Frame::Link { rect: Rect::new(x + 130_000, y, 120_000, 30_000), text: "[ random ]".into(), href: format!("{ring}?random"), font: font.clone() },
        Frame::Link { rect: Rect::new(x + 260_000, y, 120_000, 30_000), text: "next >>".into(), href: format!("{ring}?next"), font },
    ]
}

/// A "Best viewed in Netscape Navigator" badge (bordered box + caption).
pub fn best_viewed_badge(x: Mil, y: Mil, fg: ColourId, bg: ColourId) -> Vec<Frame> {
    let rect = Rect::new(x, y, 200_000, 40_000);
    vec![
        Frame::Box { rect, fill: bg, radius: 0, border: Some((2_000, fg)) },
        Frame::Text {
            rect: rect.inset(6_000),
            runs: vec![TextRun::new("Best viewed in Netscape", FontSpec::new("MS Sans Serif", 11, fg).weight(600))],
            align: Align::Center,
            valign: Align::Center,
        },
    ]
}

/// A "Sign My Guestbook!" 88×31 button linking to `guestbook.html`.
pub fn guestbook(x: Mil, y: Mil, fg: ColourId, bg: ColourId) -> Vec<Frame> {
    button_88x31(x, y, "Sign Guestbook", "guestbook.html", fg, bg)
}

/// A scrolling marquee banner across `w`.
pub fn marquee_banner(x: Mil, y: Mil, w: Mil, text: &str, fg: ColourId) -> Frame {
    Frame::Marquee {
        rect: Rect::new(x, y, w, 40_000),
        speed: 6,
        text: text.to_string(),
        font: FontSpec::new("Comic Sans MS", 20, fg).weight(700),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_is_88_by_31_and_links() {
        let f = button_88x31(10_000, 20_000, "Home", "index.html", ColourId(1), ColourId(0));
        assert_eq!(f.len(), 2);
        if let Frame::Box { rect, .. } = &f[0] {
            assert_eq!((rect.w, rect.h), (88_000, 31_000));
        } else {
            panic!("first frame is the button box");
        }
        assert!(matches!(&f[1], Frame::Link { href, .. } if href == "index.html"));
    }

    #[test]
    fn hit_counter_zero_pads() {
        let c = hit_counter(0, 0, 42, ColourId(3));
        if let Frame::Text { runs, .. } = c {
            assert_eq!(runs[0].text, "You are visitor #00000042");
        } else {
            panic!("hit counter is text");
        }
    }

    #[test]
    fn webring_is_three_links() {
        let w = webring(0, 0, "webring", ColourId(2));
        assert_eq!(w.len(), 3);
        assert!(w.iter().all(|f| matches!(f, Frame::Link { .. })));
        // spaced left→right
        let xs: Vec<i64> = w.iter().map(|f| f.bounds().x).collect();
        assert!(xs[0] < xs[1] && xs[1] < xs[2]);
    }

    #[test]
    fn badges_and_banners_build() {
        assert_eq!(best_viewed_badge(0, 0, ColourId(1), ColourId(0)).len(), 2);
        assert_eq!(guestbook(0, 0, ColourId(1), ColourId(0)).len(), 2);
        assert!(matches!(under_construction(0, 0, 100_000, ColourId(6)), Frame::Blink { .. }));
        assert!(matches!(marquee_banner(0, 0, 100_000, "hi", ColourId(2)), Frame::Marquee { .. }));
    }
}
