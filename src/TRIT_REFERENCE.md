# Trit Inference Reference

## Why Trits, Not Floats (Sean, 2026-08-28)

Continuous math (`f32`/`f64`, Fredholm integrals, smooth curves) is the illusion drawn
on top of hardware that never actually holds a continuum — every physical substrate this
repo touches collapses to a three-state switch under load:

| Layer | −1 (Inhibit) | 0 (Hold) | +1 (Fire) |
|---|---|---|---|
| SIMD/gates | Tikhonov dampening, branch rejection | zero ALU execution, deterministic replay (`R=0`) | saliency spike, workgroup activation, memory write |
| Neuro-circuit | GABAergic hyperpolarization | resting potential, no ATP spent | all-or-nothing depolarization past `N × IPR` |
| Behavior | aversion / fight-flight | habit / freeze / flow | dopaminergic approach / seek |
| Invention #56 Flux (`F`) | negative delta, back-step | null frame, zero-byte wire cost | impulse trigger, input event |

`TritArray32` (`trit_bijection.rs`) is this pattern already PROVEN in this tree — not an
analogy for it. It quantizes a continuous 32-channel `PentaractField` (±10000 permyriad)
down to `{-1, 0, +1}³²` via a threshold, and `verify_round_trip_canonical()` proves the
map is lossless on the canonical lattice 𝒮₃ (see `_proof/TRIT_BIJECTION_VERIFIED.md`).
The per-channel table below is that same collapse applied to one domain (sense/mood
channels); the table above is the same collapse applied to hardware/bio/behavior. One
mechanism, several coatings.


All 32 channels are signed permyriad i32 in [-10000, +10000].
Trit inference: default threshold ±3000.

```
q > +3000  →  trit = +1  (aligned / present / vital form)
q ∈ [-3000, +3000]  →  trit = 0  (neutral / latent / neither)
q < -3000  →  trit = -1  (corrupted / reversed / hostile form)
```

**Use `trit_from_permyriad(q)` to infer, or `trit_from_permyriad_threshold(q, threshold)` for custom thresholds.**

---

## Per-Channel Meanings & Example Thresholds

| Ch# | Channel | +1 (Aligned) | 0 (Neutral) | -1 (Corrupted) | Note |
|-----|---------|-------------|-----------|----------------|------|
| 0 | HeatGradient | burning (+8k) | stale (0) | unnatural_cold (-8k) | Δ from ambient |
| 1 | UvFlux | holy_light (+6k) | none (0) | profane_ir (-7k) | Stars vs. forbidden radiation |
| 2 | LuxZero | sees_dark (+5k) | blind (0) | lost_in_darkness (-6k) | Darkvision acuity |
| 3 | LumensMultiplier | receptive (+6k) | inert (0) | rejects_light (-7k) | Torch/starlight gain |
| 4 | GlamourPhase | true_sight (+8k) | clouded (0) | lost_in_lies (-5k) | Illusion resistance |
| 5 | RefractionDelta | pierces_veil (+6k) | blind (-0) | veiled (-4k) | See invisible |
| 6 | VeilDensity | thin_veil (+2k) | none (0) | rift_open (+9k) | Ethereal bleed-through |
| 7 | ShadowDepth | solid (+0) | none (0) | shadow_doors (+10k) | Shadow permeability |
| 8 | VibrationHz | stable (0) | healthy_motion (+6k) | sick_thrumming (-6k) | Seismic shocks |
| 9 | EchoDelay | sound_serves (+7k) | mute (0) | sound_betrays (-5k) | Acoustics |
| 10 | MasonryStress | light (+2k) | none (0) | crushing_weight (+9k) | Load on stone |
| 11 | GeomagneticYaw | aligned_north (+4k) | random (0) | inverted_north (-3k) | Compass truth |
| 12 | SapVelocity | life_flows (+6k) | stagnant (0) | death_flows (-7k) | Root/leaf fluid |
| 13 | AtmospherePa | pressure_natural (+2k) | stable (0) | unnatural_void (-6k) | Air pressure |
| 14 | FluidDisplacement | water_obeys (+5k) | still (0) | water_rebels (-9k) | Wave motion |
| 15 | ParticulateFlux | air_clean (+0) | clear (0) | air_poisoned (+9k) | Dust/ash/spore |
| 16 | ScentAge | old_trail (-1) | current (0) | fresh (+1) | t-axis (i64, stored separately) |
| 17 | FerrumPpm | blood_rich (+8k) | none (0) | blood_profane (-7k) | Iron/hemoglobin PPM |
| 18 | VitalityLux | life_present (+8k) | empty (0) | death_present (-7k) | Living organism presence |
| 19 | NecroticDecay | life_wins (-2k) | neutral (0) | death_wins (+10k) | Disease/entropy |
| 20 | SoulMass | soul_whole (+7k) | uncertain (0) | soul_broken (-8k) | Purity of bound soul |
| 21 | HormoneBias | calm_joy (+2k) | neutral (0) | rage_despair (-6k) | Emotional broadcast |
| 22 | PathogenCount | clean (-1k) | healthy (0) | infected (+9k) | Viral load/infestation |
| 23 | SporeDensity | symbiotic (+3k) | none (0) | parasitic (-10k) | Fungal network |
| 24 | WeaveFlux | magic_active (+5k) | none (0) | profane_magic (+8k) | Spell resonances (both +1) |
| 25 | ManaDensity | mana_pure (+6k) | depleted (0) | mana_profane (-9k) | Ley-line energy |
| 26 | HateVector | love (-1k) | indifference (0) | hate (+10k) | Hostility aimed |
| 27 | EthosBias | righteous (+9k) | amoral (0) | corrupt (-10k) | **SIGN FLIPS ON CORRUPTION** |
| 28 | NeuralHz | clarity_peace (+3k) | dormant (0) | chaos_madness (-9k) | Synaptic firing |
| 29 | ResidualTrauma | healed (-2k) | untouched (0) | wounded (+10k) | Historical pain |
| 30 | PlanarTear | stable (-1k) | normal (0) | torn (+9k) | Spatial distortion |
| 31 | PietyCharge | holy (+10k) | secular (0) | profaned (-9k) | **SIGN FLIPS ON BLASPHEMY** |

---

## The Priest ↔ Bloodmage Inversion

A cell with both a priest and a bloodmage reads as inverted on moral/spiritual axes:

| Channel | Priest | Bloodmage | △ |
|---------|--------|-----------|---|
| EthosBias | +9k (+1) | -10k (-1) | ↔ |
| PietyCharge | +9k (+1) | -8k (-1) | ↔ |
| HateVector | -2k (-1) | +10k (+1) | ↔ |
| NecroticDecay | -1k (-1) | +8k (+1) | ↔ |
| SoulMass | +8k (+1) | -9k (-1) | ↔ |
| VitalityLux | +7k (+1) | -6k (-1) | ↔ |
| PathogenCount | -1k (-1) | +8k (+1) | ↔ |

Same magnitude of *presence* (noise), opposite moral vector (direction).
A body reads "this place is LOUD and SACRED" vs. "this place is LOUD and PROFANED."

---

## Custom Thresholds by Channel

Most channels use default ±3000. Some may deserve tuning:

- **EthosBias, PietyCharge**: ±5000 threshold (moral absolutes are rare; ±3k = weakly moral)
- **HateVector**: ±2000 threshold (hostility is acute; even ±3k = mild)
- **NeuralHz**: ±4000 threshold (fractured minds are obvious; ±3k = confused-but-sane)
- **VeilDensity**: ±1000 threshold (ethereal bleed is subtle; ±3k = only obvious rifts register)

Use `trit_from_permyriad_threshold(q, 5_000)` to apply custom thresholds in game logic.

---

## Authoring Checklist

When you add signed channel production to a system:

✓ Allow negative values in the upstream state (weather, haunt, biome, etc.)  
✓ Remove `.clamp(0, 10_000)` and replace with `.clamp(-10_000, 10_000)`  
✓ Test that blessed and corrupted versions read with opposite trits  
✓ Update the golden hash test to reflect the new field topology  
✓ Document the corruption triggers in your system (e.g., "haunt becomes -NecroticDecay when undead outnumber living")  

---

## In Code

```rust
use forge_core_v3::pentaract_field::{
    trit_from_permyriad, trit_from_permyriad_threshold,
    PentaractField, SenseChannel,
};

// Infer trit from a channel reading
let ethics = field[SenseChannel::EthosBias];
let ethics_trit = trit_from_permyriad(ethics);
match ethics_trit {
    1 => println!("This place radiates righteousness"),
    0 => println!("Morally ambiguous"),
    -1 => println!("Corruption hangs heavy"),
    _ => unreachable!(),
}

// Custom threshold for a picky channel
let hostility = field[SenseChannel::HateVector];
let hostility_trit = trit_from_permyriad_threshold(hostility, 2_000);
if hostility_trit > 0 {
    println!("Danger is here");
}
```
