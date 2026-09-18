//! ZONE-IDENTITY primitive (Sean 2026-07-24): one seed + 32×32 ZoneId grid.
//! Three lenses (text·2D·3D) for reading zone identity through different views.
//!
//! TODO: This module depends on:
//! 1. forge-zones-v3::marker::Marker (exists at F:\v3\crates\forge-zones-v3, but
//!    forge-core-v3 cannot depend on it — Crate Zero is dependency-sealed).
//! 2. A WorldState type (not present in forge-core-v3; would live downstream
//!    in a studio/game crate).
//! 3. ASP (Answer Set Programming) infrastructure for rule grounding.
//!
//! This module stubs all three and provides the public interface.

/// 1 ZoneId = 1 chunk = 128px; 32×32 grid = 4096px.
pub const ZONE_GRID_N: u32 = 32;
/// Chunk pixel size.
pub const CHUNK_PX: u32 = 128;
/// One stage (full 32×32 grid in pixels).
pub const ZONE_STAGE_PX: u32 = ZONE_GRID_N * CHUNK_PX;

/// The three ways to read a zone identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneLens {
    /// Text/rules view.
    Text,
    /// 2D blockout view.
    Blockout2d,
    /// 3D mesh view.
    Mesh3d,
}

impl ZoneLens {
    /// Unique key for this lens.
    pub fn key(self) -> &'static str {
        match self {
            ZoneLens::Text => "text",
            ZoneLens::Blockout2d => "blockout",
            ZoneLens::Mesh3d => "mesh",
        }
    }
}

/// Stub Atom type (from forge_core::asp in v2).
#[derive(Clone, Debug)]
pub struct Atom {
    functor: String,
    args: Vec<String>,
}

impl Atom {
    /// Create an atom with a functor and arguments.
    pub fn new(functor: &str, args: Vec<&str>) -> Self {
        Self {
            functor: functor.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Convert to Clingo LP syntax.
    pub fn to_lp(&self) -> String {
        if self.args.is_empty() {
            self.functor.clone()
        } else {
            format!("{}({})", self.functor, self.args.join(", "))
        }
    }
}

/// Stub Rule type (from forge_core::asp in v2).
#[derive(Clone, Debug)]
pub struct Rule {
    head: Atom,
    body: Vec<Atom>,
}

impl Rule {
    /// Create a fact (rule with no body).
    pub fn fact(head: Atom) -> Self {
        Self { head, body: Vec::new() }
    }

    /// Create a rule with head and body.
    pub fn when(head: Atom, body: Vec<Atom>) -> Self {
        Self { head, body }
    }

    /// Convert to Clingo LP syntax.
    pub fn to_lp(&self) -> String {
        if self.body.is_empty() {
            format!("{}.", self.head.to_lp())
        } else {
            let body_str = self.body.iter().map(|a| a.to_lp()).collect::<Vec<_>>().join(", ");
            format!("{} :- {}.", self.head.to_lp(), body_str)
        }
    }
}

/// Stub Program type (from forge_core::asp in v2).
#[derive(Clone, Debug)]
pub struct Program {
    rules: Vec<Rule>,
}

impl Program {
    /// Create an empty program.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule to the program.
    pub fn push(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Number of rules in the program.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether the program is empty.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Convert entire program to Clingo LP syntax.
    pub fn to_lp(&self) -> String {
        self.rules.iter().map(|r| r.to_lp()).collect::<Vec<_>>().join("\n")
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub Marker type (from forge_zones::marker in v2).
#[derive(Clone, Debug)]
pub struct Marker {
    /// Marker name.
    pub name: String,
    /// Entity type (e.g. "secret", "boss", "spawn").
    pub entity_type: String,
    /// Additional metadata (hand-rolled key:value pairs — Crate Zero bans serde_json).
    pub metadata: Vec<(String, String)>,
}

impl Marker {
    /// Create a new marker.
    pub fn new(name: &str, _x: f64, _y: f64, _z: f64, entity_type: &str) -> Self {
        Self {
            name: name.to_string(),
            entity_type: entity_type.to_string(),
            metadata: Vec::new(),
        }
    }
}

/// Stub WorldState type (from crate::world in v2 forge-studio).
#[derive(Clone, Default)]
pub struct WorldState {
    /// Random seed.
    pub seed: u64,
    /// Archetype/biome name.
    pub archetype: String,
    /// Era name.
    pub era: String,
    /// World size in pixels (width, height).
    pub size: (u32, u32),
    /// Generated level (if any).
    pub level: Option<String>,
}

impl WorldState {
    /// Create a new world with default values.
    pub fn new() -> Self {
        Self {
            seed: 42,
            archetype: "forest".into(),
            era: "ancient".into(),
            size: (512, 512),
            level: None,
        }
    }

    /// Simulate a 2D level generation.
    pub fn generate(&mut self) -> String {
        self.level = Some(format!("world:{}x{}:seed{:#x}", self.size.0, self.size.1, self.seed));
        self.level.as_ref().unwrap().clone()
    }

    /// World dimensions.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
}

/// Construct the zone's proc-gen rule shadow as a Program: ground facts plus rules.
pub fn zone_program(world: &WorldState) -> Program {
    let mut p = Program::new();
    p.push(Rule::fact(Atom::new("zone_seed", vec![&format!("{:#x}", world.seed)])));
    p.push(Rule::fact(Atom::new("biome", vec![&world.archetype])));
    p.push(Rule::fact(Atom::new("era", vec![&world.era])));
    p.push(Rule::fact(Atom::new("grid", vec![&ZONE_GRID_N.to_string()])));
    p.push(Rule::fact(Atom::new("chunk_px", vec![&CHUNK_PX.to_string()])));
    p.push(Rule::when(
        Atom::new("realizable", vec!["Z"]),
        vec![Atom::new("biome", vec!["Z"]), Atom::new("zone_seed", vec!["S"])],
    ));
    p.push(Rule::when(
        Atom::new("unlocked", vec!["W"]),
        vec![Atom::new("gate", vec!["W", "K"]), Atom::new("have", vec!["K"])],
    ));
    p.push(Rule::when(
        Atom::new("layout_alt", vec!["Z"]),
        vec![Atom::new("boss_defeated", vec!["B"]), Atom::new("alters", vec!["B", "Z"])],
    ));
    p
}

/// Normalize arbitrary text to a Clingo constant: lowercase, non-alnum→`_`, letter-led.
fn atomize(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    if !out.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        out.insert(0, 'm');
    }
    out
}

/// Ground zone markers as ASP facts: spawn→marker fact, secret→gate fact, boss→boss+alters fact.
pub fn ground_markers(p: &mut Program, markers: &[Marker]) {
    for m in markers {
        let name = atomize(&m.name);
        let etype = atomize(&m.entity_type);
        p.push(Rule::fact(Atom::new("marker", vec![name.as_str(), etype.as_str()])));
        match m.entity_type.as_str() {
            "secret" | "gate" => {
                if let Some(k) = m.metadata.iter().find(|(k, _)| k == "key").map(|(_, v)| v.as_str()) {
                    let key = atomize(k);
                    p.push(Rule::fact(Atom::new("gate", vec![name.as_str(), key.as_str()])));
                }
            }
            "boss" => {
                p.push(Rule::fact(Atom::new("boss", vec![name.as_str()])));
                if let Some(z) = m.metadata.iter().find(|(k, _)| k == "alters").map(|(_, v)| v.as_str()) {
                    let zone = atomize(z);
                    p.push(Rule::fact(Atom::new("alters", vec![name.as_str(), zone.as_str()])));
                }
            }
            _ => {}
        }
    }
}

/// Read the current zone through `lens` → `(panel, receipt)`.
/// All three lenses are REAL: 2D=world::generate, text=asp, 3D=forge-zones chunk.
/// In this stub version, calls to generate_preview will fail because worldgen_kit
/// is also stubbed and doesn't have access to the real forge-zones generators.
pub fn realize(world: &mut WorldState, lens: ZoneLens) -> (Option<String>, String) {
    match lens {
        ZoneLens::Blockout2d => {
            let line = world.generate();
            (Some("level_architect".into()), format!("zone[2D]: {line}"))
        }
        ZoneLens::Text => {
            let prog = zone_program(world);
            let (w, h) = world.size;
            let summary = format!(
                "zone[text]: {} · era {} · seed {:#x} · {w}x{h} px · {ZONE_GRID_N}x{ZONE_GRID_N} ZoneId grid \
                 ({CHUNK_PX}px chunks = {ZONE_STAGE_PX}px stage) · {} asp rules",
                world.archetype, world.era, world.seed, prog.len(),
            );
            (Some("book_ledger".into()), summary)
        }
        ZoneLens::Mesh3d => {
            // In real code, this would call crate::worldgen_kit::generate_preview(world.seed)
            // For now, return a stubbed 3D message
            (
                None,
                format!(
                    "zone[3D]: stub (forge-zones not available in core) · seed {:#x}",
                    world.seed
                ),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_ruling_is_one_mmx3_stage() {
        assert_eq!(CHUNK_PX, 128);
        assert_eq!(ZONE_GRID_N, 32);
        assert_eq!(ZONE_STAGE_PX, 4096);
    }

    #[test]
    fn lens_keys_are_stable_and_distinct() {
        assert_eq!(ZoneLens::Text.key(), "text");
        assert_eq!(ZoneLens::Blockout2d.key(), "blockout");
        assert_eq!(ZoneLens::Mesh3d.key(), "mesh");
    }

    #[test]
    fn zone_program_grounds_the_shared_identity() {
        let world = WorldState::default();
        let lp = zone_program(&world).to_lp();
        assert!(lp.contains("zone_seed"), "zone_seed missing from program");
        assert!(lp.contains("biome"), "biome missing from program");
        assert!(lp.contains("realizable"), "realizable rule missing");
        assert!(lp.contains("unlocked"), "unlocked rule missing");
        assert!(lp.contains("layout_alt"), "layout_alt rule missing");
    }

    #[test]
    fn markers_ground_the_trigger_facts() {
        let mut secret = Marker::new("Secret Wall", 0.0, 0.0, 0.0, "secret");
        secret.metadata = vec![("key".to_string(), "Triad Thunder".to_string())];
        let mut boss = Marker::new("Vile", 0.0, 0.0, 0.0, "boss");
        boss.metadata = vec![("alters".to_string(), "factory".to_string())];
        let mut p = Program::new();
        ground_markers(&mut p, &[secret, boss]);
        let lp = p.to_lp();
        assert!(lp.contains("gate(secret_wall"), "weapon gate grounded");
        assert!(lp.contains("boss(vile)"), "boss grounded");
        assert!(lp.contains("alters(vile"), "layout-swap binding grounded");
    }

    #[test]
    fn all_three_lenses_read_one_zone() {
        let mut world = WorldState::default();
        let (panel, r) = realize(&mut world, ZoneLens::Blockout2d);
        assert_eq!(panel.as_deref(), Some("level_architect"));
        assert!(r.starts_with("zone[2D]:"), "2D lens format");

        let (_, rt) = realize(&mut world, ZoneLens::Text);
        assert!(rt.contains("asp rules"), "text lens includes asp rules");

        let (_, r3) = realize(&mut world, ZoneLens::Mesh3d);
        assert!(r3.contains("3D"), "3D lens identified");
    }
}
