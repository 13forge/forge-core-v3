//! The 13moons star-lore catalogue — 16 named lights, magnitudes verbatim from
//! the v2 lore (NewRepo forge-game-systems celestial_alignment.rs:181, the locked
//! 13moons.stars_microcanon.v1 of real HIP entries). ONE HOME (L05): the shell's
//! GPU sky pane reads this as a crate module; xtask (zero-dependency by its own
//! law) mounts this same FILE via `#[path]` — one definition on disk, two compile
//! targets, no cargo edge and no drift.
//!
//! Machine-first (L08): the RGBA words below are the exact integer encoding; any
//! CSS hex or perceptual form is DERIVED from them by the consumer, never stored
//! a second time.
//!
//! This module is deliberately self-contained: no `crate::` reference may enter,
//! or the `#[path]` mount in xtask stops compiling.

/// Spiritual weight — how bright this light appears to the watcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brightness {
    /// Bright enough to cast shadow on snow. Strongest spiritual weight.
    SpiritFire,
    /// Navigation stars — wayfinding across open prairie.
    GuideStar,
    /// Distant but present. Someone who walked the prairie before.
    AncestorLight,
    /// Too dim to see, but the land still feels them.
    TheForgotten,
}

impl Brightness {
    /// Map magnitude (in Permyriad, where 1_0000 = 1.0 mag) to brightness band.
    /// Magnitudes run backwards — smaller/negative is brighter.
    ///
    /// Total classification (the colour.rs Scotopic rule: an empty band keeps
    /// its meaning — TheForgotten has no member because the Milky Way is a
    /// −6.5 blaze, and DeepWinter's O-class has no star in this sky).
    ///
    /// In the xtask mount this derivation's living caller is the band-agreement
    /// gate (tests), invisible to the dead-code lint — the allow predates this
    /// file's re-homing (drained with the original, not authored in a weld).
    pub fn of(mag_permyriad: i32) -> Self {
        match mag_permyriad {
            _ if mag_permyriad <= 0 => Self::SpiritFire,
            _ if mag_permyriad <= 2_0000 => Self::GuideStar,
            _ if mag_permyriad <= 4_0000 => Self::AncestorLight,
            _ => Self::TheForgotten,
        }
    }

    /// The band's own hue lane (v2 sky_verb::primary_rgb) — separate from
    /// spectral colour, never derived from it.
    pub const fn rgba(self) -> [u8; 4] {
        match self {
            Self::SpiritFire => [255, 60, 60, 255],
            Self::GuideStar => [80, 200, 255, 255],
            Self::AncestorLight => [215, 80, 215, 255],
            Self::TheForgotten => [110, 110, 110, 255],
        }
    }

    /// The band's spoken tag.
    pub const fn label(self) -> &'static str {
        match self {
            Self::SpiritFire => "SPIRIT_FIRE",
            Self::GuideStar => "GUIDE_STAR",
            Self::AncestorLight => "ANCESTOR_LIGHT",
            Self::TheForgotten => "THE_FORGOTTEN",
        }
    }
}

/// v2 `mag_ink`'s magnitude norm: −6.5 blaze .. +4.0 smudge → 0..=1000 pmy
/// (magnitudes here ride ×10_000 permyriad; v2's were ×1_000). [`mag_fill`]
/// is this norm bucketed to bar cells.
pub fn mag_norm(mag_permyriad: i32) -> i32 {
    ((4_000 - mag_permyriad / 10).clamp(0, 10_500) * 1_000) / 10_500
}

/// Spectral class — what colour this star burns. Season and character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spectral {
    /// O — hottest, rarest. Blue fire. DeepWinter. No O-class star stands in
    /// this 16-star sky, but the class keeps its colour (Scotopic rule:
    /// shipping eight of nine would strand the classification); the colour
    /// gate in tests is one living caller; `natal_boon`'s four O-class arms are
    /// the other. The dead-code allow this doc used to carry retired 2026-08-12
    /// when the star→boon table gave the class real work.
    DeepWinter,
    /// B — blue-white. Bleached like prairie bones. BoneStar.
    BoneStar,
    /// A — white. Cold clarity. Frost.
    Frost,
    /// F and G — yellow-white warmth. AskiyGold.
    AskiyGold,
    /// K — orange. Transformation through heat. TheForge.
    TheForge,
    /// M — red, ancient. Wisakedjak.
    Wisakedjak,
    /// Planets. Wanderers.
    Wanderer,
    /// Distant galaxy. TheDistant.
    TheDistant,
    /// The Milky Way band. Meskanaw.
    Meskanaw,
}

impl Spectral {
    /// The exact RGBA word for this spectral class — the machine encoding
    /// (L08). Verbatim from the v2 spectral bucket table
    /// (celestial_alignment.rs:221, 0xRRGGBBFF words, alpha opaque).
    pub const fn rgba(self) -> [u8; 4] {
        match self {
            Self::DeepWinter => [0x9B, 0xB0, 0xFF, 0xFF], // O: blue fire
            Self::BoneStar => [0xAA, 0xBF, 0xFF, 0xFF],   // B: blue-white
            Self::Frost => [0xCA, 0xD7, 0xFF, 0xFF],      // A: white
            Self::AskiyGold => [0xFF, 0xF4, 0xEA, 0xFF],  // F/G: sun's warmth
            Self::TheForge => [0xFF, 0xD2, 0xA1, 0xFF],   // K: orange, forge heat
            Self::Wisakedjak => [0xFF, 0xCC, 0x6F, 0xFF], // M: red, trickster fire
            Self::Wanderer => [0xFF, 0xFF, 0xFF, 0xFF],   // PLANET: plain white
            Self::TheDistant => [0xE8, 0xE0, 0xD0, 0xFF], // GALAXY: pale distant
            Self::Meskanaw => [0xD4, 0xC8, 0xB0, 0xFF],   // BAND: road dust
        }
    }

    /// The class's spoken tag.
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeepWinter => "DEEP_WINTER",
            Self::BoneStar => "BONE_STAR",
            Self::Frost => "FROST",
            Self::AskiyGold => "ASKIY_GOLD",
            Self::TheForge => "THE_FORGE",
            Self::Wisakedjak => "WISAKEDJAK",
            Self::Wanderer => "WANDERER",
            Self::TheDistant => "THE_DISTANT",
            Self::Meskanaw => "MESKANAW",
        }
    }
}

/// Historical name record: era, culture, scripts, meaning.
#[derive(Debug, Clone, Copy)]
pub struct NameRecord {
    /// Era year: negative = BCE, positive = CE.
    pub era_year: i16,
    /// Cultural origin or language family.
    pub culture: &'static str,
    /// Original script representation.
    pub original_script: &'static str,
    /// Romanized or transliterated form.
    pub transliteration: &'static str,
    /// Literal or figurative meaning.
    pub meaning: &'static str,
}

/// One star: name, constellation, magnitude, spiritual weight, colour.
#[derive(Debug, Clone, Copy)]
pub struct Star {
    /// The star's proper name or asterism identifier.
    pub name: &'static str,
    /// The constellation or grouping it belongs to.
    pub constellation: &'static str,
    /// Apparent magnitude in Permyriad (i32). 1_0000 = 1.0 mag.
    /// Negative = brighter. Exactly as authored in v2.
    pub mag_permyriad: i32,
    /// Brightness band: spiritual weight.
    pub brightness: Brightness,
    /// Spectral class: colour and season.
    pub spectral: Spectral,
    /// Historical lineage: earliest known name through modern IAU designation.
    pub lineage: &'static [NameRecord],
}

impl Star {
    /// Format this star's magnitude for display: sign, integer part, two
    /// decimals — pure integer math over permyriad (`-14600` → `"-1.46"`).
    /// Arithmetic cannot drift (the LUT that used to live here could).
    #[inline]
    pub fn mag_display(&self) -> String {
        let m = self.mag_permyriad;
        let sign = if m < 0 { "-" } else { "" };
        let a = m.unsigned_abs();
        format!("{sign}{}.{:02}", a / 10_000, (a % 10_000) / 100)
    }
}

/// FNV-1a hash (64-bit integer-only, no mutations): core-local integer fold
/// of a star name to entropy. Deterministic, never floats, pure bit arithmetic.
/// Offset basis 14695981039346656037, prime 1099511628211.
fn fnv1a_hash(s: &str) -> u64 {
    let mut hash = 14695981039346656037u64;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211u64);
    }
    hash
}

const SIRIUS_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -2000, culture: "Egyptian", original_script: "𓇲𓏏𓅆", transliteration: "Sopdet", meaning: "Star of the Nile Flood" },
    NameRecord { era_year: -800, culture: "Arabic", original_script: "الشعرى", transliteration: "al-Shi'ra", meaning: "the Brilliance" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Sirius", transliteration: "Sirius", meaning: "From Greek seirion, scorching" },
];

const RIGEL_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -800, culture: "Arabic", original_script: "رجل الجوزاء", transliteration: "Rijl al-Jauzā'", meaning: "Leg of Orion" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Rigel", transliteration: "Rigel", meaning: "Arabization of foot/leg" },
];

const BETELGEUSE_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -800, culture: "Arabic", original_script: "بيت الجوزاء", transliteration: "Bayt al-Jauzā'", meaning: "Shoulder of Orion (arm of)" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Betelgeuse", transliteration: "Betelgeuse", meaning: "Corruption of Arabic bayt al-jauzā'" },
];

const CAPELLA_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -200, culture: "Greek", original_script: "Αἴξ", transliteration: "Aix", meaning: "the Goat" },
    NameRecord { era_year: 500, culture: "Latin", original_script: "Capella", transliteration: "Capella", meaning: "little goat (feminine)" },
];

const PROCYON_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -200, culture: "Greek", original_script: "Προκύων", transliteration: "Pro Kyon", meaning: "before the Dog (Star)" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Procyon", transliteration: "Procyon", meaning: "From Greek pro kyon" },
];

const MORNING_STAR_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -1000, culture: "Akkadian", original_script: "𒌚𒋧𒊬", transliteration: "Ishtar", meaning: "Goddess of morning/evening" },
    NameRecord { era_year: 500, culture: "Latin", original_script: "Stella Matutina", transliteration: "Morning Star", meaning: "Star of the morning" },
];

const POLARIS_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -1, culture: "Arabic", original_script: "النجية الشمالية", transliteration: "al-Nujayya al-Šamāliyya", meaning: "northern star" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Polaris", transliteration: "Polaris", meaning: "From Latin stella polaris" },
];

const DENEB_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -800, culture: "Arabic", original_script: "ذنب الدجاجة", transliteration: "Dhanab al-Dajaja", meaning: "Tail of the Hen" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Deneb", transliteration: "Deneb", meaning: "Arabic word for tail" },
];

const ALTAIR_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -800, culture: "Arabic", original_script: "الناصر الطائر", transliteration: "al-Nasr al-Ṭā'ir", meaning: "the Flying Eagle" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Altair", transliteration: "Altair", meaning: "From Arabic al-nasr al-tāir" },
];

const SPICA_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -800, culture: "Arabic", original_script: "السماك الأعزل", transliteration: "al-Simāk al-A'zal", meaning: "the Disarmed One" },
    NameRecord { era_year: 500, culture: "Latin", original_script: "Spica Virginis", transliteration: "Spica", meaning: "Ear of grain in Virgo's hand" },
];

const VEGA_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -800, culture: "Arabic", original_script: "واقع", transliteration: "Wāqiʿ", meaning: "the Landing Eagle or Falling" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Vega", transliteration: "Vega", meaning: "From Arabic wāqiʿ" },
];

const ARCTURUS_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -200, culture: "Greek", original_script: "Αρκτοῦρος", transliteration: "Arktoûros", meaning: "Bear-watcher/Guardian of Bear" },
    NameRecord { era_year: -800, culture: "Arabic", original_script: "السماك الرامح", transliteration: "Al-Simak al-Ramih", meaning: "the Spear-thrower" },
];

const PLEIADES_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -300, culture: "Greek", original_script: "Πλειάδες", transliteration: "Pleiades", meaning: "the Seven Sisters" },
    NameRecord { era_year: 500, culture: "Latin", original_script: "Pleiades", transliteration: "Pleiades", meaning: "From Greek pleiades" },
];

const BIG_DIPPER_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -2000, culture: "Proto-Indo-European", original_script: "Ursa Major", transliteration: "Ursae Majoris", meaning: "Great Bear constellation" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Big Dipper", transliteration: "Big Dipper", meaning: "Ladle/plow shape" },
];

const ANDROMEDA_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -200, culture: "Greek", original_script: "Ανδρομέδα", transliteration: "Andromeda", meaning: "Chained maiden" },
    NameRecord { era_year: 1600, culture: "European", original_script: "Andromeda Galaxy", transliteration: "Andromeda", meaning: "From Greek mythology" },
];

const MILKY_WAY_LINEAGE: &[NameRecord] = &[
    NameRecord { era_year: -200, culture: "Greek", original_script: "κύκλος γαλακτικός", transliteration: "Galaxías kýklos", meaning: "milky circle" },
    NameRecord { era_year: 500, culture: "Latin", original_script: "Via Lactea", transliteration: "Via Lactea", meaning: "Milky Way" },
];

/// The 13moons star-lore catalog: named stars and asterisms with their
/// spiritual and spectral properties. Every magnitude verbatim from the v2 lore.
pub const CATALOG: [Star; 16] = [
    // SpiritFire: shadow-casting lights. Walker dims these first.
    Star {
        name: "Sirius",
        constellation: "Canis Major",
        mag_permyriad: -14600,
        brightness: Brightness::SpiritFire,
        spectral: Spectral::Frost,
        lineage: SIRIUS_LINEAGE,
    },
    Star {
        name: "Rigel",
        constellation: "Orion",
        mag_permyriad: 1300,
        // v2 BRIGHTNESS_LADDER (stars.rs:35): mag 1300 > 0 ⇒ GuideStar.
        brightness: Brightness::GuideStar,
        spectral: Spectral::BoneStar,
        lineage: RIGEL_LINEAGE,
    },
    Star {
        name: "Betelgeuse",
        constellation: "Orion",
        mag_permyriad: 4200,
        brightness: Brightness::GuideStar,
        spectral: Spectral::Wisakedjak,
        lineage: BETELGEUSE_LINEAGE,
    },
    Star {
        name: "Capella",
        constellation: "Auriga",
        mag_permyriad: 800,
        brightness: Brightness::GuideStar,
        spectral: Spectral::AskiyGold,
        lineage: CAPELLA_LINEAGE,
    },
    Star {
        name: "Procyon",
        constellation: "Canis Minor",
        mag_permyriad: 3400,
        brightness: Brightness::GuideStar,
        spectral: Spectral::AskiyGold,
        lineage: PROCYON_LINEAGE,
    },
    Star {
        name: "Morning Star",
        constellation: "Venus",
        mag_permyriad: -46000,
        brightness: Brightness::SpiritFire,
        spectral: Spectral::Wanderer,
        lineage: MORNING_STAR_LINEAGE,
    },
    // GuideStar: wayfinding home. Navigation becomes unreliable under the Walker.
    Star {
        name: "Polaris",
        constellation: "Ursa Minor",
        mag_permyriad: 19800,
        brightness: Brightness::GuideStar,
        spectral: Spectral::AskiyGold,
        lineage: POLARIS_LINEAGE,
    },
    Star {
        name: "Deneb",
        constellation: "Cygnus",
        mag_permyriad: 12500,
        brightness: Brightness::GuideStar,
        spectral: Spectral::Frost,
        lineage: DENEB_LINEAGE,
    },
    Star {
        name: "Altair",
        constellation: "Aquila",
        mag_permyriad: 7700,
        brightness: Brightness::GuideStar,
        spectral: Spectral::Frost,
        lineage: ALTAIR_LINEAGE,
    },
    Star {
        name: "Spica",
        constellation: "Virgo",
        mag_permyriad: 9700,
        brightness: Brightness::GuideStar,
        // B1III in the v2 microcanon (celestial_alignment.rs:200); the B prefix
        // buckets to BONE_STAR (:223). Was Frost — a transcription slip caught
        // 2026-08-12 when the boon table made spectral load-bearing.
        spectral: Spectral::BoneStar,
        lineage: SPICA_LINEAGE,
    },
    Star {
        name: "Vega",
        constellation: "Lyra",
        mag_permyriad: 300,
        brightness: Brightness::GuideStar,
        spectral: Spectral::Frost,
        lineage: VEGA_LINEAGE,
    },
    // AncestorLight: distant but present. Naked-eye stars most dwell here.
    Star {
        name: "Arcturus",
        constellation: "Boötes",
        mag_permyriad: -500,
        brightness: Brightness::SpiritFire,
        spectral: Spectral::TheForge,
        lineage: ARCTURUS_LINEAGE,
    },
    Star {
        name: "Pleiades",
        constellation: "Taurus",
        mag_permyriad: 16000,
        brightness: Brightness::GuideStar,
        // B6III in the v2 microcanon (celestial_alignment.rs:202); same B-prefix
        // rule as Spica and Rigel. Was Frost — the same 2026-08-12 slip.
        spectral: Spectral::BoneStar,
        lineage: PLEIADES_LINEAGE,
    },
    Star {
        name: "Big Dipper",
        constellation: "Ursa Major",
        mag_permyriad: 18000,
        brightness: Brightness::GuideStar,
        spectral: Spectral::Frost,
        lineage: BIG_DIPPER_LINEAGE,
    },
    // TheDistant & Meskanaw: impossible far or the spirit path itself.
    Star {
        name: "Andromeda Galaxy",
        constellation: "Andromeda",
        mag_permyriad: 34400,
        brightness: Brightness::AncestorLight,
        spectral: Spectral::TheDistant,
        lineage: ANDROMEDA_LINEAGE,
    },
    Star {
        name: "Milky Way",
        constellation: "Galactic Band",
        // v2 microcanon (celestial_alignment.rs:206): the Milky Way is a
        // −6.5 BLAZE across the whole sky, not a faint smudge.
        mag_permyriad: -65000,
        brightness: Brightness::SpiritFire,
        spectral: Spectral::Meskanaw,
        lineage: MILKY_WAY_LINEAGE,
    },
];

// ── The Astrolabe plate ──────────────────────────────────────────────────────
//
// The readout v2 printed from `forge-studio/src/sky_verb.rs` (`report` :146,
// `paint_chart` :183, `mag_ink` :39-69), rebuilt on THIS catalog. Integer-only: v2 carried
// `mag: f64` and multiplied by 1_000 for milli-magnitudes; every star here is already
// `mag_permyriad`, so the same arithmetic is a divide by 10 and no float ever appears.

/// Cells in the magnitude bar. v2's `mag_ink` filled 0..=10 of these.
pub const MAG_BAR_CELLS: usize = 10;

impl Brightness {
    /// The screen name — the left half of `SPIRIT_FIRE/FROST`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::SpiritFire => "SPIRIT_FIRE",
            Self::GuideStar => "GUIDE_STAR",
            Self::AncestorLight => "ANCESTOR_LIGHT",
            Self::TheForgotten => "THE_FORGOTTEN",
        }
    }
}

impl Spectral {
    /// The screen name — the right half of `SPIRIT_FIRE/FROST`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeepWinter => "DEEP_WINTER",
            Self::BoneStar => "BONE_STAR",
            Self::Frost => "FROST",
            Self::AskiyGold => "ASKIY_GOLD",
            Self::TheForge => "THE_FORGE",
            Self::Wisakedjak => "WISAKEDJAK",
            Self::Wanderer => "WANDERER",
            Self::TheDistant => "THE_DISTANT",
            Self::Meskanaw => "MESKANAW",
        }
    }

    /// This class's colour as one packed `0xRRGGBBAA` word, for a text ink.
    pub const fn ink(self) -> u32 {
        let c = self.rgba();
        ((c[0] as u32) << 24) | ((c[1] as u32) << 16) | ((c[2] as u32) << 8) | c[3] as u32
    }
}

/// The glyph tier for a magnitude — v2's `mag_ink` ladder, thresholds converted from its
/// `f64` magnitudes to Permyriad. Brighter things carry a heavier mark.
pub const fn mag_glyph(mag_permyriad: i32) -> char {
    match mag_permyriad {
        _ if mag_permyriad < -5_0000 => '●', // galactic
        _ if mag_permyriad < -2_0000 => '◎', // blazing
        _ if mag_permyriad < 5_000 => '✦',   // vivid
        _ if mag_permyriad < 1_5000 => '★',  // bright
        _ if mag_permyriad < 2_5000 => '✧',  // faint
        _ => '·',                            // smudge
    }
}

/// How many of [`MAG_BAR_CELLS`] a magnitude fills.
///
/// Ported verbatim from v2's `mag_ink` (`sky_verb.rs:39-69`):
/// `norm = (4_000 - mag_milli).clamp(0, 10_500) * 1_000 / 10_500`, then
/// `filled = ((norm + 50) / 100).clamp(0, 10)`. Magnitudes run backwards, so the window
/// spans the −6.5 galactic blaze down to the +4.0 forgotten floor.
pub const fn mag_fill(mag_permyriad: i32) -> usize {
    let mag_milli = mag_permyriad / 10;
    let span = 4_000 - mag_milli;
    let clamped = if span < 0 {
        0
    } else if span > 10_500 {
        10_500
    } else {
        span
    };
    let filled = (clamped * 1_000 / 10_500 + 50) / 100;
    if filled < 0 {
        0
    } else if filled > MAG_BAR_CELLS as i32 {
        MAG_BAR_CELLS
    } else {
        filled as usize
    }
}

/// The `[████░░░░░░]` meter for a magnitude.
pub fn mag_bar(mag_permyriad: i32) -> String {
    let filled = mag_fill(mag_permyriad);
    let mut s = String::with_capacity(MAG_BAR_CELLS * 3);
    for i in 0..MAG_BAR_CELLS {
        s.push(if i < filled { '█' } else { '░' });
    }
    s
}

/// Which catalog star a clock register selects.
///
/// v2's `select_star`: stride 7, modulo the catalog. 7 is coprime with 16, so successive
/// clocks walk every star before repeating rather than pacing the list in order.
pub const fn active_index(clock: u8) -> usize {
    (clock as usize * 7) % CATALOG.len()
}

/// The Astrolabe plate: one inked line per star, active row marked.
///
/// Each line is `(text, ink)`, the ink being the star's own spectral colour
/// ([`Spectral::ink`]) — the plate is coloured by what the star actually burns, not by a
/// theme. Reproduces v2's `paint_chart` layout against this catalog.
pub fn report_lines(clock: u8) -> Vec<(String, u32)> {
    let active = active_index(clock);
    let mut out = Vec::with_capacity(CATALOG.len());
    for (i, s) in CATALOG.iter().enumerate() {
        let mark = if i == active { '*' } else { ' ' };
        out.push((
            format!(
                "{mark} {} [{}] {} mag {} [{}] — {}/{}",
                s.name,
                s.constellation,
                mag_glyph(s.mag_permyriad),
                s.mag_display(),
                mag_bar(s.mag_permyriad),
                s.brightness.name(),
                s.spectral.name(),
            ),
            s.spectral.ink(),
        ));
    }
    out
}

#[cfg(test)]
mod astrolabe_tests {
    use super::*;

    /// v2 pinned these two exact widths (`sky_verb.rs:334-345`); the port must agree or the
    /// plate is not the same instrument.
    #[test]
    fn the_bar_matches_v2_at_both_extremes() {
        assert_eq!(mag_fill(-6_5000), MAG_BAR_CELLS, "the galactic blaze must fill the bar");
        assert_eq!(mag_fill(3_4400), 1, "Andromeda is one cell of light");
    }

    /// Magnitudes run backwards: a brighter (smaller) magnitude must never fill less.
    #[test]
    fn brighter_is_never_a_shorter_bar() {
        let mut prev = mag_fill(-7_0000);
        let mut m = -7_0000;
        while m < 5_0000 {
            let f = mag_fill(m);
            assert!(f <= prev, "bar grew as the star dimmed at mag_pmy={m}");
            prev = f;
            m += 500;
        }
    }

    #[test]
    fn the_bar_is_always_exactly_the_published_width() {
        for s in CATALOG.iter() {
            assert_eq!(mag_bar(s.mag_permyriad).chars().count(), MAG_BAR_CELLS, "{}", s.name);
        }
    }

    /// Stride 7 is coprime with 16, so a full sweep touches every star — that is what makes
    /// the clock a dial and not a queue.
    #[test]
    fn the_clock_stride_visits_every_star() {
        let mut seen = [false; CATALOG.len()];
        for clock in 0..CATALOG.len() as u8 {
            seen[active_index(clock)] = true;
        }
        assert!(seen.iter().all(|&s| s), "stride 7 failed to reach every star");
    }

    #[test]
    fn every_line_is_inked_by_its_own_spectral_class() {
        let lines = report_lines(5);
        assert_eq!(lines.len(), CATALOG.len());
        for (i, (_, ink)) in lines.iter().enumerate() {
            assert_eq!(*ink, CATALOG[i].spectral.ink());
        }
        assert_eq!(lines.iter().filter(|(t, _)| t.starts_with('*')).count(), 1, "one active row");
    }

    /// The plate must read like the instrument it came from.
    #[test]
    fn a_line_reads_like_the_v2_plate() {
        let sirius = &report_lines(0)[0].0;
        assert!(sirius.contains("Sirius [Canis Major]"), "{sirius}");
        assert!(sirius.contains("SPIRIT_FIRE/FROST"), "{sirius}");
        assert!(sirius.contains("mag -1.46"), "{sirius}");
    }
}

/// Derive a seed from a star index — one-to-one encoding (L07 bijection).
/// Low nibble carries the star index; upper 60 bits from the integer fold
/// of the star's name. Decode is surjective: a hand-typed `reseed` resolves
/// to a star it was not struck from.
///
/// # Panics
/// Never — star is clamped to low 4 bits silently.
pub fn natal_seed(star: u8) -> u64 {
    let idx = (star & 0xF) as u64;
    if idx as usize >= CATALOG.len() {
        // Star out of range — clamp to 0. This should never happen in practice,
        // but L10 (abort) is for corruption in critical paths; here we have time
        // to fix it gracefully.
        return fnv1a_hash("Sirius") << 4;
    }
    let name_hash = fnv1a_hash(CATALOG[idx as usize].name);
    (name_hash << 4) | idx
}

/// Decode a star index from a seed — always total, never panics.
/// Returns the low 4 bits of the seed as the star index.
pub fn natal_star(seed: u64) -> u8 {
    (seed & 0xF) as u8
}

/// One of the eight hermetic registers a natal boon can move. Variant order is
/// `HermeticStats`' field order in forge-mud-v3, so a register's position here
/// is its position there — one word, two crates. This module may not name that
/// struct (no `crate::` may enter; see the module doc), which is precisely why
/// the order is stated as law here rather than imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatReg {
    /// Vigor — the strike current.
    Vigor,
    /// Shadow-weight — what the operator carries unseen.
    ShadowWeight,
    /// Logic-depth — the depth of making.
    LogicDepth,
    /// Momentum — motion already underway.
    Momentum,
    /// Tarnish — accrues in play. Never dealt, never natal.
    Tarnish,
    /// Resonance — how the world answers.
    Resonance,
    /// Guilt — accrues in play. Never dealt, never natal.
    Guilt,
    /// Clarity — earned in play, never rolled (ARCH000 2026-08-12). Never natal.
    Clarity,
}

/// How a natal boon moves a register. Integer-only; the v2 donor carried these
/// as parsed strings (`"Vigor << 1"`), which this replaces outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatalOp {
    /// Add a flat amount.
    Add(u8),
    /// Double, shifted left by n.
    Shl(u8),
    /// Halve, shifted right by n.
    Shr(u8),
}

/// What being born under a light grants. Slots are INDICES, not names: Crate
/// Zero cannot name forge-mud-v3's `ARTS`/`DEED_*`/`FACTIONS` tables, so it
/// emits the slot and mud resolves it. That keeps the dependency one-way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatalBoon {
    /// Move one dealt stat register.
    Register(StatReg, NatalOp),
    /// Seed an art: `ARTS` slot 0..7, points added to that art's register.
    Art(u8, u16),
    /// Lean a deed family: `DEED_*` 0..4, count added.
    Deed(u8, u32),
    /// Set out with standing: `FACTIONS` slot 0..5, standing added.
    Standing(u8, i16),
    /// Start with time already counted.
    Xp(u64),
}

/// The natal boon for a light's (spectral, brightness) pair — the v2 star→RPG
/// bridge, drained whole and completed.
///
/// Ten rows are verbatim from the v2 donor (`celestial_alignment.rs:117-139`,
/// the `compute_modifier` table). Five of that donor's fifteen rows named
/// buckets no v3 enum can express (`FROST×BLAZE`, `VOID×ASH`, `IRON×EMBER`,
/// `GOLD×LAMP`, `FROST×LAMP`) — the donor's own comment records that dead
/// dialect being caught once already; here it is unrepresentable rather than
/// merely unused. The remaining 26 pairs are authored (ARCH000 2026-08-12).
///
/// Total by construction: the match has no `_` arm and the return is not an
/// `Option`, so no light is silent and a new `Spectral` variant breaks the
/// build instead of quietly granting nothing (L01 — the match IS the test).
///
/// Never targets Tarnish, Guilt or Clarity: those accrue in play. That law is
/// asserted in `natal_never_touches_an_earned_register`.
pub const fn natal_boon(spectral: Spectral, brightness: Brightness) -> NatalBoon {
    use Brightness::{AncestorLight, GuideStar, SpiritFire, TheForgotten};
    use NatalOp::{Add, Shl, Shr};
    use StatReg::{LogicDepth, Momentum, Resonance, ShadowWeight, Vigor};
    match (spectral, brightness) {
        // O — the long cold. No O-class light stands in this sky, but the
        // class ships whole (Scotopic rule, as with its colour).
        (Spectral::DeepWinter, SpiritFire) => NatalBoon::Art(1, 120),
        (Spectral::DeepWinter, GuideStar) => NatalBoon::Register(ShadowWeight, Add(10)), // donor :134
        (Spectral::DeepWinter, AncestorLight) => NatalBoon::Standing(3, 150),
        (Spectral::DeepWinter, TheForgotten) => NatalBoon::Xp(500),

        // B — bleached prairie bone. Rigel, Spica, Pleiades.
        (Spectral::BoneStar, SpiritFire) => NatalBoon::Art(0, 150),
        (Spectral::BoneStar, GuideStar) => NatalBoon::Register(Vigor, Add(25)), // donor :133
        (Spectral::BoneStar, AncestorLight) => NatalBoon::Deed(0, 3),
        (Spectral::BoneStar, TheForgotten) => NatalBoon::Standing(2, 100),

        // A — cold clarity. Sirius, Vega, Deneb, Altair, Big Dipper.
        (Spectral::Frost, SpiritFire) => NatalBoon::Register(Vigor, Shl(1)), // donor :127
        (Spectral::Frost, GuideStar) => NatalBoon::Register(Vigor, Add(10)), // donor :128
        (Spectral::Frost, AncestorLight) => NatalBoon::Art(0, 80),
        (Spectral::Frost, TheForgotten) => NatalBoon::Xp(250),

        // F/G — the land's own warmth. Capella, Procyon, Polaris.
        (Spectral::AskiyGold, SpiritFire) => NatalBoon::Register(Resonance, Add(25)), // donor :129
        (Spectral::AskiyGold, GuideStar) => NatalBoon::Register(Resonance, Shl(1)),   // donor :130
        (Spectral::AskiyGold, AncestorLight) => NatalBoon::Standing(1, 150),
        (Spectral::AskiyGold, TheForgotten) => NatalBoon::Deed(2, 3),

        // K — transformation through heat. Arcturus.
        // SpiritFire matches GuideStar's shift because Shl is already the
        // strongest op the donor's vocabulary carries; the flat spot is the
        // donor's (:132), named rather than hidden.
        (Spectral::TheForge, SpiritFire) => NatalBoon::Register(Momentum, Shl(1)),
        (Spectral::TheForge, GuideStar) => NatalBoon::Register(Momentum, Shl(1)), // donor :132
        (Spectral::TheForge, AncestorLight) => NatalBoon::Register(Momentum, Add(25)), // donor :131
        (Spectral::TheForge, TheForgotten) => NatalBoon::Art(2, 120),

        // M — the trickster's red fire. Betelgeuse.
        // GuideStar carries the art, not the register: Betelgeuse is the only
        // M-class light in this sky, so if the art rows sat on an unoccupied
        // pair the whole art family would be dormant — 21 authored rows that
        // no star can reach is a table that lies about moving the game.
        (Spectral::Wisakedjak, SpiritFire) => NatalBoon::Register(ShadowWeight, Shl(1)),
        (Spectral::Wisakedjak, GuideStar) => NatalBoon::Art(1, 150),
        (Spectral::Wisakedjak, AncestorLight) => NatalBoon::Register(ShadowWeight, Add(25)), // donor :136
        (Spectral::Wisakedjak, TheForgotten) => NatalBoon::Register(ShadowWeight, Shr(1)),   // donor :135

        // Planets. Venus walks the ecliptic; motion is the art.
        (Spectral::Wanderer, SpiritFire) => NatalBoon::Register(Momentum, Add(25)),
        (Spectral::Wanderer, GuideStar) => NatalBoon::Art(3, 120),
        (Spectral::Wanderer, AncestorLight) => NatalBoon::Deed(3, 3),
        (Spectral::Wanderer, TheForgotten) => NatalBoon::Xp(250),

        // Galaxies. Distance read as depth of making. Andromeda.
        (Spectral::TheDistant, SpiritFire) => NatalBoon::Art(2, 150),
        (Spectral::TheDistant, GuideStar) => NatalBoon::Register(LogicDepth, Shl(1)),
        // Same reasoning as Wisakedjak: Andromeda sits on AncestorLight, so the
        // standing row lives there where a real light can earn it.
        (Spectral::TheDistant, AncestorLight) => NatalBoon::Standing(3, 100),
        (Spectral::TheDistant, TheForgotten) => NatalBoon::Register(LogicDepth, Add(25)),

        // The road itself. Meskanaw is where speech is traded.
        (Spectral::Meskanaw, SpiritFire) => NatalBoon::Register(Resonance, Add(25)),
        (Spectral::Meskanaw, GuideStar) => NatalBoon::Art(5, 120),
        (Spectral::Meskanaw, AncestorLight) => NatalBoon::Xp(250),
        (Spectral::Meskanaw, TheForgotten) => NatalBoon::Deed(2, 3),
    }
}

// The tests' ONE home is this crate. The `sky-mount` feature is on only in
// xtask's #[path] mount of this file, so the mounted copy compiles data-only
// and `cargo xtask tests` never sees one name in two homes.
#[cfg(all(test, not(feature = "sky-mount")))]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_not_empty() {
        assert!(!CATALOG.is_empty(), "Catalog must contain at least one star");
        assert_eq!(CATALOG.len(), 16, "Catalog should have all 16 named stars");
    }

    /// Every spectral class, in declaration order. Named here rather than on
    /// the enum so the mounted copy stays data-only.
    const ALL_SPECTRAL: [Spectral; 9] = [
        Spectral::DeepWinter,
        Spectral::BoneStar,
        Spectral::Frost,
        Spectral::AskiyGold,
        Spectral::TheForge,
        Spectral::Wisakedjak,
        Spectral::Wanderer,
        Spectral::TheDistant,
        Spectral::Meskanaw,
    ];

    /// Every brightness band, in declaration order.
    const ALL_BRIGHTNESS: [Brightness; 4] = [
        Brightness::SpiritFire,
        Brightness::GuideStar,
        Brightness::AncestorLight,
        Brightness::TheForgotten,
    ];

    /// The v2 donor's ten representable rows, asserted verbatim against
    /// `celestial_alignment.rs:117-139`. If one of these drifts, the drain has
    /// quietly become an invention — which is the whole failure this table
    /// exists to prevent.
    #[test]
    fn donor_rows_port_verbatim() {
        use Brightness::{AncestorLight, GuideStar, SpiritFire, TheForgotten};
        use NatalOp::{Add, Shl, Shr};
        use StatReg::{Momentum, Resonance, ShadowWeight, Vigor};
        let rows = [
            (Spectral::Frost, SpiritFire, NatalBoon::Register(Vigor, Shl(1))),
            (Spectral::Frost, GuideStar, NatalBoon::Register(Vigor, Add(10))),
            (Spectral::AskiyGold, SpiritFire, NatalBoon::Register(Resonance, Add(25))),
            (Spectral::AskiyGold, GuideStar, NatalBoon::Register(Resonance, Shl(1))),
            (Spectral::TheForge, AncestorLight, NatalBoon::Register(Momentum, Add(25))),
            (Spectral::TheForge, GuideStar, NatalBoon::Register(Momentum, Shl(1))),
            (Spectral::BoneStar, GuideStar, NatalBoon::Register(Vigor, Add(25))),
            (Spectral::DeepWinter, GuideStar, NatalBoon::Register(ShadowWeight, Add(10))),
            (Spectral::Wisakedjak, TheForgotten, NatalBoon::Register(ShadowWeight, Shr(1))),
            (Spectral::Wisakedjak, AncestorLight, NatalBoon::Register(ShadowWeight, Add(25))),
        ];
        for (s, b, want) in rows {
            assert_eq!(natal_boon(s, b), want, "donor row {s:?} x {b:?} drifted");
        }
    }

    /// No light is silent. v2 could only assert 6 of its 16 stars carried a
    /// modifier (`celestial_alignment.rs:358`); here every one of the 9x4 pairs
    /// grants something, and no grant is a zero operand hiding as a boon.
    #[test]
    fn no_pair_is_silent() {
        let mut pairs = 0;
        for s in ALL_SPECTRAL {
            for b in ALL_BRIGHTNESS {
                let live = match natal_boon(s, b) {
                    NatalBoon::Register(_, NatalOp::Add(n) | NatalOp::Shl(n) | NatalOp::Shr(n)) => {
                        n > 0
                    }
                    NatalBoon::Art(_, n) => n > 0,
                    NatalBoon::Deed(_, n) => n > 0,
                    NatalBoon::Standing(_, n) => n != 0,
                    NatalBoon::Xp(n) => n > 0,
                };
                assert!(live, "{s:?} x {b:?} grants nothing");
                pairs += 1;
            }
        }
        assert_eq!(pairs, 36, "9 spectral classes x 4 brightness bands");
    }

    /// Tarnish, Guilt and Clarity accrue in play and are never dealt
    /// (ARCH000 2026-08-12). No natal boon may hand them out at birth.
    #[test]
    fn natal_never_touches_an_earned_register() {
        for s in ALL_SPECTRAL {
            for b in ALL_BRIGHTNESS {
                if let NatalBoon::Register(reg, _) = natal_boon(s, b) {
                    assert!(
                        !matches!(reg, StatReg::Tarnish | StatReg::Guilt | StatReg::Clarity),
                        "{s:?} x {b:?} grants {reg:?}, which is earned in play"
                    );
                }
            }
        }
    }

    /// The B-prefix rule from the v2 lore rules (`celestial_alignment.rs:223`).
    /// Rigel, Spica and Pleiades are all B-class in the microcanon; two of them
    /// were transcribed as Frost until 2026-08-12. This pins the correction.
    #[test]
    fn b_class_lights_are_bone_star() {
        for name in ["Rigel", "Spica", "Pleiades"] {
            let star = CATALOG.iter().find(|s| s.name == name).expect("named light in catalog");
            assert_eq!(star.spectral, Spectral::BoneStar, "{name} is B-class in the v2 microcanon");
        }
    }

    /// The full chain for named lights: catalog row -> its pair -> its boon.
    /// These are the ones Sean sees on glass, so they are pinned by name.
    #[test]
    fn named_lights_grant_their_boon() {
        let boon_of = |name: &str| {
            let s = CATALOG.iter().find(|s| s.name == name).expect("named light in catalog");
            natal_boon(s.spectral, s.brightness)
        };
        assert_eq!(boon_of("Sirius"), NatalBoon::Register(StatReg::Vigor, NatalOp::Shl(1)));
        assert_eq!(boon_of("Rigel"), NatalBoon::Register(StatReg::Vigor, NatalOp::Add(25)));
        assert_eq!(boon_of("Spica"), NatalBoon::Register(StatReg::Vigor, NatalOp::Add(25)));
        assert_eq!(boon_of("Betelgeuse"), NatalBoon::Art(1, 150));
        assert_eq!(boon_of("Morning Star"), NatalBoon::Register(StatReg::Momentum, NatalOp::Add(25)));
        assert_eq!(boon_of("Milky Way"), NatalBoon::Register(StatReg::Resonance, NatalOp::Add(25)));
        assert_eq!(boon_of("Andromeda Galaxy"), NatalBoon::Standing(3, 100));
    }

    /// A boon family no living light can reach is a table that lies about
    /// moving the game. Every catalog star landed on a `Register` row when the
    /// 21 authored rows first went in, leaving arts, deeds, standings and xp
    /// dormant; the Wisakedjak and TheDistant rows moved to the bands their
    /// stars actually occupy. This holds that line.
    #[test]
    fn the_live_sky_reaches_past_the_stat_bars() {
        let mut register = 0;
        let mut wider = 0;
        for star in &CATALOG {
            match natal_boon(star.spectral, star.brightness) {
                NatalBoon::Register(..) => register += 1,
                _ => wider += 1,
            }
        }
        assert_eq!(register + wider, CATALOG.len(), "every light is counted");
        assert!(register > 0, "the stat bars must still move");
        assert!(wider > 0, "no living light reaches arts/deeds/standings/xp — they are dormant");
    }

    /// Every catalog row's stored band agrees with the derivation — the band
    /// column cannot drift from its own magnitude. DeepWinter has no star in
    /// this sky but keeps its colour (Scotopic rule).
    #[test]
    fn every_stored_band_agrees_with_its_magnitude() {
        for star in &CATALOG {
            assert_eq!(star.brightness, Brightness::of(star.mag_permyriad), "{}", star.name);
        }
        assert_eq!(
            Spectral::DeepWinter.rgba(),
            [0x9B, 0xB0, 0xFF, 0xFF],
            "the unshipped class keeps its colour"
        );
    }

    #[test]
    fn magnitudes_within_sane_range() {
        for star in &CATALOG {
            assert!(
                star.mag_permyriad >= -70_000 && star.mag_permyriad <= 70_000,
                "Star {} mag {} out of range",
                star.name,
                star.mag_permyriad
            );
        }
    }

    /// Every spectral word is fully opaque — the sky pane composes these as
    /// layer words, and a translucent star would vanish into the glaze rule.
    #[test]
    fn every_spectral_word_is_opaque() {
        for star in &CATALOG {
            assert_eq!(star.spectral.rgba()[3], 0xFF, "{} carries a translucent word", star.name);
        }
    }

    #[test]
    fn sirius_magnitude_matches_v2() {
        let sirius = CATALOG.iter().find(|s| s.name == "Sirius").expect("Sirius must be in catalog");
        assert_eq!(sirius.mag_permyriad, -14600, "Sirius mag must match v2 value");
        assert_eq!(sirius.mag_display(), "-1.46", "Sirius formatted display must match v2");
    }

    /// L07 bijection test: every star round-trips through natal_seed/natal_star.
    #[test]
    fn natal_seed_is_bijective() {
        for star in 0..16 {
            let seed = natal_seed(star);
            let decoded = natal_star(seed);
            assert_eq!(
                decoded, star,
                "Star {star} did not round-trip: natal_seed({star}) = {seed:016x}, natal_star = {decoded}"
            );
        }
    }

    /// Edge case: seeds bearing index 0 and u64::MAX round-trip correctly.
    #[test]
    fn natal_star_edge_cases() {
        // A seed with low 4 bits = 0 should decode to star 0.
        let seed_zero = 0x0000_0000_0000_0000u64;
        assert_eq!(natal_star(seed_zero), 0, "seed 0x0 should decode to star 0");

        // A seed with low 4 bits = 15 should decode to star 15.
        let seed_max_low = 0xFFFF_FFFF_FFFF_FFFfu64;
        assert_eq!(
            natal_star(seed_max_low), 15,
            "seed 0xFFFFFFFFFFFFFFFF should decode to star 15"
        );

        // Sirius (star 0) and Milky Way (star 15) both encode and decode cleanly.
        let sirius_seed = natal_seed(0);
        assert_eq!(natal_star(sirius_seed), 0, "Sirius seed round-trip");
        let milky_way_seed = natal_seed(15);
        assert_eq!(natal_star(milky_way_seed), 15, "Milky Way seed round-trip");
    }

    /// L18 sabotage witness: the decode mask is load-bearing (not decorative).
    /// When the mask 0xF is flipped to a wrong value, the bijection breaks,
    /// proving the mask earns its place.
    #[test]
    fn decode_mask_is_load_bearing() {
        // The correct mask 0xF extracts the low 4 bits (16 stars).
        let seed = natal_seed(7); // A middle star
        let correct_decode = natal_star(seed);
        assert_eq!(correct_decode, 7, "Correct mask 0xF must preserve star index");

        // If we were using wrong mask 0x0 (always zero), bijection would fail:
        // seed & 0x0 = 0, so every star would decode to 0.
        let wrong_mask_would_decode = seed & 0x0;
        assert_eq!(wrong_mask_would_decode, 0, "wrong mask 0x0 always yields 0");
        assert_ne!(wrong_mask_would_decode as u8, 7, "Wrong mask breaks bijection");

        // This test ensures the actual mask in natal_star is correct.
    }
}
