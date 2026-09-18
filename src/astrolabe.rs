// Copyright (c) 2026 Sean Morin, Edmonton River Valley, Alberta. All rights reserved.
// SPDX-License-Identifier: MIT

//! # THE ASTROLABE (Crate Zero Primitive)
//!
//! A flat, deterministic, zero-transcendental model of the celestial sphere (~150 BC Hipparchus,
//! al-Battani, Andalusian master metalworkers).
//!
//! Stereographic projection from the south celestial pole onto the plane of the equator:
//! Every celestial circle projects to a Euclidean circle on the plate.
//!
//! ## The Six Classical Parts:
//! 1. **MATER**   - The main dish; the outer rim (the LIMB) graduated in degrees and 24 hours.
//! 2. **TYMPAN**  - The climate plate for local latitude (horizon, meridian, almucantar circles).
//! 3. **RETE**    - Pierced rotating star map with pointers for named catalog stars & the ecliptic ring.
//! 4. **RULE**    - Rotating straight-edge on the front face for coordinate reading.
//! 5. **ALIDADE** - Sighting arm on the reverse side for measuring celestial altitude.
//! 6. **THRONE**  - Suspension shackle at the zenith ($0^\circ$).
//!
//! ## Three Canonical Radii:
//! - **Tropic of Capricorn** (Outer Limb)  : $r = 1.0000 = 10000\text{ pmy}$
//! - **Equator**             (Middle)      : $r = 0.5774 = 5774\text{ pmy}$
//! - **Tropic of Cancer**    (Inner Circle): $r = 0.3333 = 3333\text{ pmy}$

#![deny(unsafe_code)]

/// Permyriad scaling factor (1.0000 = 10,000 pmy).
pub const PMY_ONE: i32 = 10_000;

/// Classical plate radius for Tropic of Capricorn (Outer Limb = 10,000 pmy).
pub const RADIUS_CAPRICORN_PMY: i32 = 10_000;

/// Classical plate radius for Equator ($\tan(45^\circ - 23.44^\circ/2) \approx 5774\text{ pmy}$).
pub const RADIUS_EQUATOR_PMY: i32 = 5_774;

/// Classical plate radius for Tropic of Cancer ($3333\text{ pmy}$).
pub const RADIUS_CANCER_PMY: i32 = 3_333;

/// The full circle in milli-degrees.
pub const CIRCLE_MDEG: i32 = 360_000;

/// Smallest absolute separation between two wheel angles, in milli-degrees.
pub fn angular_distance_mdeg(a_mdeg: i32, b_mdeg: i32) -> i32 {
    let d = (b_mdeg - a_mdeg).rem_euclid(CIRCLE_MDEG);
    d.min(CIRCLE_MDEG - d)
}

/// Canonical Palette from `.forge/hud.html`
pub mod palette {
    /// Gold highlight (`--brasshi`).
    pub const BRASS_HI: u32 = 0xC3A256FF;
    /// Aged bronze (`--brassdim`).
    pub const BRASS_DIM: u32 = 0x5F4A22FF;
    /// Verdigris copper (`--verd`).
    pub const VERD: u32 = 0x6D8A6BFF;
    /// Rust (`--rust`).
    pub const RUST: u32 = 0x8A3A30FF;
    /// Sand ink (`--ink`).
    pub const SAND: u32 = 0xC3B791FF;
}

/// A star pointer anchored to the Rete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarPointer {
    /// Star catalog name.
    pub name: &'static str,
    /// Right Ascension in hundredths of a degree ($[0, 36000]$).
    pub ra_cdeg: u32,
    /// Declination in hundredths of a degree ($[-9000, 9000]$).
    pub dec_cdeg: i32,
    /// Apparent magnitude in permyriad.
    pub mag_pmy: i32,
    /// Resonant audio frequency in millihertz (mHz).
    pub milli_hz: u32,
    /// Spectral emission color (RGBA8).
    pub color_rgba: u32,
}

/// Canonical 16-Star Astrolabe Constellation (Locked Microcanon).
pub const CATALOG_16: [StarPointer; 16] = [
    StarPointer { name: "Sirius", ra_cdeg: 10128, dec_cdeg: -1671, mag_pmy: -14600, milli_hz: 440_000, color_rgba: 0xE0F7FFFF },
    StarPointer { name: "Canopus", ra_cdeg: 9598, dec_cdeg: -5269, mag_pmy: -7400, milli_hz: 415_305, color_rgba: 0xFFF8E7FF },
    StarPointer { name: "Arcturus", ra_cdeg: 21391, dec_cdeg: 1918, mag_pmy: -500, milli_hz: 391_995, color_rgba: 0xFFB347FF },
    StarPointer { name: "Vega", ra_cdeg: 27923, dec_cdeg: 3878, mag_pmy: 300, milli_hz: 369_994, color_rgba: 0xC6E2FFFF },
    StarPointer { name: "Capella", ra_cdeg: 7917, dec_cdeg: 4599, mag_pmy: 800, milli_hz: 349_228, color_rgba: 0xFFE4B5FF },
    StarPointer { name: "Rigel", ra_cdeg: 7863, dec_cdeg: -820, mag_pmy: 1300, milli_hz: 329_628, color_rgba: 0xADD8E6FF },
    StarPointer { name: "Procyon", ra_cdeg: 11482, dec_cdeg: 522, mag_pmy: 3400, milli_hz: 311_127, color_rgba: 0xFFFACDFF },
    StarPointer { name: "Betelgeuse", ra_cdeg: 8879, dec_cdeg: 740, mag_pmy: 5000, milli_hz: 293_665, color_rgba: 0xFF4500FF },
    StarPointer { name: "Achernar", ra_cdeg: 2442, dec_cdeg: -5723, mag_pmy: 4500, milli_hz: 277_183, color_rgba: 0xB0E0E6FF },
    StarPointer { name: "Hadar", ra_cdeg: 21095, dec_cdeg: -6037, mag_pmy: 6100, milli_hz: 261_626, color_rgba: 0x87CEEBFF },
    StarPointer { name: "Altair", ra_cdeg: 29769, dec_cdeg: 886, mag_pmy: 7700, milli_hz: 246_942, color_rgba: 0xE6E6FAFF },
    StarPointer { name: "Acrux", ra_cdeg: 18664, dec_cdeg: -6309, mag_pmy: 7700, milli_hz: 233_082, color_rgba: 0x4682B4FF },
    StarPointer { name: "Aldebaran", ra_cdeg: 6898, dec_cdeg: 1650, mag_pmy: 8500, milli_hz: 220_000, color_rgba: 0xFF6347FF },
    StarPointer { name: "Antares", ra_cdeg: 24735, dec_cdeg: -2643, mag_pmy: 9600, milli_hz: 207_652, color_rgba: 0xDC143CFF },
    StarPointer { name: "Spica", ra_cdeg: 20129, dec_cdeg: -1116, mag_pmy: 9800, milli_hz: 195_998, color_rgba: 0x87CEFAFF },
    StarPointer { name: "Pollux", ra_cdeg: 11632, dec_cdeg: 2802, mag_pmy: 11400, milli_hz: 184_997, color_rgba: 0xFFA07AFF },
];

/// Nostr 3-tuple identity token: a sovereign world-genesis key.
/// pubkey = BIP-340 x-only bytes; lst_cdeg = local sidereal time in cdeg $[0, 36000)$;
/// discipline_mask bit $i$ = sevenfold correspondence row $i$ active (top bit reserved).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenesisToken {
    /// BIP-340 x-only public key bytes.
    pub pubkey: [u8; 32],
    /// Local sidereal time in hundredths of a degree ($[0, 36000)$).
    pub lst_cdeg: u16,
    /// Sevenfold correspondence bits (bit $i$ = row $i$ active; top bit reserved).
    pub discipline_mask: u8,
}

impl GenesisToken {
    /// FNV-1a fold of pubkey || lst_cdeg || discipline_mask into one world seed.
    pub const fn world_seed(&self) -> u64 {
        const PRIME: u64 = 0x0000_0100_0000_01B3;
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        let mut i = 0;
        while i < 32 {
            h = (h ^ self.pubkey[i] as u64).wrapping_mul(PRIME);
            i += 1;
        }
        h = (h ^ self.lst_cdeg as u64).wrapping_mul(PRIME);
        (h ^ self.discipline_mask as u64).wrapping_mul(PRIME)
    }

    /// Natal star dealt by the seed ($0..16$).
    pub const fn natal_star_idx(&self) -> usize {
        (self.world_seed() % CATALOG_16.len() as u64) as usize
    }
}

/// The Astrolabe State Machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Astrolabe {
    /// Local latitude in hundredths of a degree ($[-9000, 9000]$).
    pub latitude_cdeg: i32,
    /// Rete rotation angle (Right Ascension of the meridian) in hundredths of a degree ($[0, 36000)$).
    pub rete_rot_cdeg: u32,
    /// Alidade sighting bar angle in hundredths of a degree ($[0, 36000)$).
    pub alidade_cdeg: u32,
    /// Active selected star index ($0..15$).
    pub active_star_idx: usize,
}

impl Astrolabe {
    /// Construct an Astrolabe calibrated for a specific latitude (e.g. Edmonton River Valley = 53.54° = 5354 cdeg).
    pub const fn new(latitude_cdeg: i32) -> Self {
        Self {
            latitude_cdeg,
            rete_rot_cdeg: 0,
            alidade_cdeg: 0,
            active_star_idx: 0,
        }
    }

    /// Calibrate from a genesis token: LST turns the rete, the seed deals the natal star.
    pub const fn from_token(token: &GenesisToken, latitude_cdeg: i32) -> Self {
        Self {
            latitude_cdeg,
            rete_rot_cdeg: (token.lst_cdeg as u32) % 36000,
            alidade_cdeg: 0,
            active_star_idx: token.natal_star_idx(),
        }
    }

    /// Rotate the Rete star plate by delta hundredths of a degree.
    pub fn rotate_rete(&mut self, delta_cdeg: i32) {
        let current = self.rete_rot_cdeg as i32;
        let next = (current + delta_cdeg).rem_euclid(36000);
        self.rete_rot_cdeg = next as u32;
    }

    /// Set Alidade sighting bar angle ($[0, 36000)$).
    pub fn set_alidade(&mut self, angle_cdeg: u32) {
        self.alidade_cdeg = angle_cdeg % 36000;
    }

    /// Select active star by catalog index.
    pub fn select_star(&mut self, idx: usize) {
        if idx < CATALOG_16.len() {
            self.active_star_idx = idx;
        }
    }

    /// Project a star's celestial coordinates $(\alpha, \delta)$ stereographically onto the plate plane.
    ///
    /// Returns $(x, y)$ in permyriad relative to plate center:
    /// - $r = R_{\text{equator}} \cdot \tan\left(\frac{90^\circ - \delta}{2}\right)$
    /// - $\theta = \alpha + \theta_{\text{rete}}$
    pub fn project_star(&self, star: &StarPointer) -> (i32, i32) {
        // Co-declination in cdeg: (9000 - dec_cdeg) / 2
        let half_codec = (9000 - star.dec_cdeg) / 2; // [0, 9000]
        
        // Fixed-point tan(half_codec) using polynomial approximation in permyriad
        let tan_pmy = tan_half_angle_pmy(half_codec);
        let r_pmy = (RADIUS_EQUATOR_PMY as i64 * tan_pmy as i64 / PMY_ONE as i64) as i32;

        let total_ang_cdeg = ((star.ra_cdeg as i64 + self.rete_rot_cdeg as i64) % 36000) as u32;
        let (sin_val, cos_val) = sin_cos_cdeg(total_ang_cdeg);

        let x = (r_pmy as i64 * cos_val as i64 / PMY_ONE as i64) as i32;
        let y = (r_pmy as i64 * sin_val as i64 / PMY_ONE as i64) as i32;

        (x, y)
    }

    /// Read celestial altitude at current Alidade sighting angle.
    pub fn read_altitude_cdeg(&self) -> i32 {
        let ang = self.alidade_cdeg;
        if ang <= 9000 {
            ang as i32
        } else if ang <= 18000 {
            (18000 - ang) as i32
        } else if ang <= 27000 {
            -((ang - 18000) as i32)
        } else {
            -((36000 - ang) as i32)
        }
    }
}

/// Fixed-point tangent of half-angle in permyriad ($0^\circ \le \theta \le 90^\circ$).
fn tan_half_angle_pmy(cdeg: i32) -> i32 {
    let clamped = cdeg.clamp(0, 8999);
    // tan(theta) approx (theta_rad) + (theta_rad^3)/3
    // In cdeg: 18000 cdeg = pi rad
    let theta_scaled = clamped as i64;
    let rad_pmy = (theta_scaled * 31416) / 18000;
    let rad3_pmy = (rad_pmy * rad_pmy / PMY_ONE as i64 * rad_pmy / PMY_ONE as i64) / 3;
    (rad_pmy + rad3_pmy).min(50_000) as i32
}

/// Fixed-point sine and cosine from hundredths of a degree ($[0, 36000)$).
/// Returns values in permyriad ($[-10000, 10000]$).
pub fn sin_cos_cdeg(cdeg: u32) -> (i32, i32) {
    let norm = (cdeg % 36000) as i32;
    let (quadrant, rem) = (norm / 9000, norm % 9000);
    
    // Rem is in [0, 9000] cdeg -> [0, pi/2]
    // 5-term integer Taylor approximation for sin(x) in permyriad
    let x = (rem as i64 * 31416) / 18000; // x in permyriad radians (0..15708)
    let x2 = (x * x) / 10000;
    let x3 = (x2 * x) / 10000;
    let x5 = (x3 * x2) / 10000;
    let s_raw = (x - (x3 / 6) + (x5 / 120)) as i32;
    let s = s_raw.clamp(0, 10000);

    let c_raw = 10000 - (x2 / 2) as i32 + ((x2 * x2 / 10000) / 24) as i32;
    let c = c_raw.clamp(0, 10000);

    match quadrant {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_astrolabe_projection_sirius() {
        let astro = Astrolabe::new(5354); // Edmonton
        let sirius = &CATALOG_16[0];
        let (x, y) = astro.project_star(sirius);

        // Sirius has negative declination (-16.71°), so it lies outside the equator circle (> 5774 pmy)
        let r_sq = (x as i64 * x as i64 + y as i64 * y as i64) as u64;
        let r = (r_sq as f64).sqrt() as i32;
        assert!(r > RADIUS_EQUATOR_PMY, "Sirius must project outside equator ring, got r={}", r);
        assert!(r <= RADIUS_CAPRICORN_PMY + 2000, "Sirius within plate boundary");
    }

    #[test]
    fn test_sin_cos_cdeg_cardinals() {
        let (s0, c0) = sin_cos_cdeg(0);
        assert_eq!(s0, 0);
        assert_eq!(c0, 10000);

        let (s90, c90) = sin_cos_cdeg(9000);
        assert!((s90 - 10000).abs() <= 10);
        assert_eq!(c90, 0);

        let (s180, c180) = sin_cos_cdeg(18000);
        assert_eq!(s180, 0);
        assert!((c180 - (-10000)).abs() <= 10);
    }

    #[test]
    fn test_genesis_token_seed_and_rete() {
        let a = GenesisToken { pubkey: [0x13; 32], lst_cdeg: 12345, discipline_mask: 0b0101_0101 };
        let b = GenesisToken { pubkey: [0x14; 32], ..a };
        assert_eq!(a.world_seed(), a.world_seed());
        assert_ne!(a.world_seed(), b.world_seed(), "distinct pubkeys must deal distinct seeds");
        assert_ne!(a.world_seed(), GenesisToken { lst_cdeg: 12346, ..a }.world_seed());
        assert_ne!(a.world_seed(), GenesisToken { discipline_mask: 0b0101_0100, ..a }.world_seed());

        let astro = Astrolabe::from_token(&a, 5354);
        assert_eq!(astro.rete_rot_cdeg, 12345);
        assert!(astro.active_star_idx < CATALOG_16.len());

        let wrapped = Astrolabe::from_token(&GenesisToken { lst_cdeg: 36005, ..a }, 5354);
        assert_eq!(wrapped.rete_rot_cdeg, 5);
    }

    #[test]
    fn test_alidade_altitude_reading() {
        let mut astro = Astrolabe::new(5354);
        astro.set_alidade(4500); // 45 degrees
        assert_eq!(astro.read_altitude_cdeg(), 4500);

        astro.set_alidade(13500); // 135 degrees -> 45 degrees back
        assert_eq!(astro.read_altitude_cdeg(), 4500);
    }
}
