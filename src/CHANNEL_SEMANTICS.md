# 32 SenseChannel Trit Semantics: ±10k Permyriad

Each channel is a signed i32 in [-10000, +10000]. Trit inference (default threshold ±3000):
- `q < -3000` → trit = -1 (corrupted/reversed/hostile form)
- `-3000 ≤ q ≤ +3000` → trit = 0 (neutral/latent/neither)
- `q > +3000` → trit = +1 (aligned/present/vital form)

Per-channel meanings and examples (priest → bloodmage corruption):

## Optical Band (0-7): Light & Vision

**0: HeatGradient** — Thermal departure from ambient. Priest cell cool/stable (-2k) vs. bloodmage chamber radiates profane heat (+9k). `trit`: cool=-1, stale=0, burning=+1.

**1: UvFlux** — High-frequency radiation (stars, UV lamps, celestial). Priest shrine: zero (no arcane sources); bloodmage sanctum: -8k (inverted/forbidden radiation, black-light). `trit`: holy_uv=+1, none=0, profane_ir=-1.

**2: LuxZero** — Darkvision-grade acuity in pitch black. Priest: +5k (sees by faith); bloodmage: +7k (sees by hunger). Both are +1 (heightened), but the character's alignment flips elsewhere.

**3: LumensMultiplier** — Gain boost for torchlight/starlight. Priest: +6k (receives grace); bloodmage: -7k (light stings, rejects it). `trit`: receptive=+1, inert=0, hostile_to_light=-1.

**4: GlamourPhase** — Resistance to illusion/polymorph. Priest: +8k (true seeing); bloodmage: -5k (drowns in false visions). `trit`: sees_truth=+1, clouded=0, lost_in_lies=-1.

**5: RefractionDelta** — Detects invisible entities. Priest: +6k (halo reveals phantoms); bloodmage: -4k (shadows hide everything). `trit`: pierces_veil=+1, blind=0, veiled=-1.

**6: VeilDensity** — Overlapping Ethereal Plane density. Priest chapel: +2k (thin veil, one foot in faith); bloodmage ritual site: +9k (veil torn open, many-worlds bleeding through). Both +1, same layer, different purposes.

**7: ShadowDepth** — Permeability of shadow pools. Priest shrine: zero (light dispels shadow); bloodmage hideout: +10k (shadows are doors). `trit`: shadow_absent=0, shadow_deep=+1.

## Kinetic Band (8-15): Motion & Structure

**8: VibrationHz** — Seismic shockwave frequency. Priest standing: zero; bloodmage on a cursed chasm floor: -6k (wrongness thrums). `trit`: stable=0, healthy_motion=+1, sick_vibration=-1.

**9: EchoDelay** — Acoustic geometry. Priest vast cathedral: +7k (echoes carry prayer); bloodmage cramped tomb: -5k (echoes corrupt/contradict). `trit`: sound_serves=+1, mute=0, sound_betrays=-1.

**10: MasonryStress** — Load bearing & worked stone. Priest ancient temple +3k; bloodmage cursed keep +8k (stones groan with weight). Both +1 (old, laden), but reader's dread differs by context.

**11: GeomagneticYaw** — Alignment with cardinal north. Priest: +4k (compass true); bloodmage: -3k (north is wrong/lies). `trit`: aligned=+1, random=0, inverted=-1.

**12: SapVelocity** — Fluid flow in roots/leaves. Priest orchard: +6k (life rises); bloodmage blighted forest: -7k (sap rots/flows backward). `trit`: life_flows=+1, stagnant=0, death_flows=-1.

**13: AtmospherePa** — Air pressure (weather systems). Priest fair skies: +2k (stable); bloodmage storm: +8k (pressure crushing) or -6k (vacuum, wrongness). `trit`: pressure_natural=+1, stable=0, pressure_unnatural=-1.

**14: FluidDisplacement** — Wave/pressure in water. Priest still well: -1k (undisturbed); bloodmage tainted pool: -9k (waves move against gravity). `trit`: water_obeys=+1, still=0, water_rebels=-1.

**15: ParticulateFlux** — Dust, spores, ash suspension. Priest clean: -2k (wind carries dust away); bloodmage blight: +9k (choking corruption-ash). `trit`: air_clean=+1, clear=0, air_poisoned=-1.

## Anima Band (16-23): Life, Death & Soul

**16: ScentAge** — Time-since-last-passage on t-axis. Always a bare i64 (t-position), NOT permyriad. Stored separately. Priest chapel yesterday: t=14; bloodmage lair now: t=0 (fresh kill). `trit`: old_trail=-1, current=0, fresh=+1.

**17: FerrumPpm** — Iron/hemoglobin in air. Priest: +1k (blood present but stable); bloodmage feast: +10k (iron reek). `trit`: blood_rich=+1, none=0, blood_profane=-1? (same high value, different moral vector).

**18: VitalityLux** — Living organism presence. Priest cathedral full: +8k (congregation); bloodmage tomb alone: -7k (undead/absence of life-feeling). `trit`: life_present=+1, empty=0, death_present=-1.

**19: NecroticDecay** — Disease, death, entropy. Priest: -2k (blessed, decay held back); bloodmage: +10k (rot feeds power). `trit`: life_wins=-1, neither=0, death_wins=+1.

**20: SoulMass** — Purity/weight of bound soul. Priest: +7k (soul intact, radiant); bloodmage: -8k (soul fragmented/corrupted). `trit`: soul_whole=+1, uncertain=0, soul_broken=-1.

**21: HormoneBias** — Emotional state via pheromone. Priest calm: +2k (peace); bloodmage frenzied: -6k (rage/hunger/madness). `trit`: calm_joy=+1, neutral=0, rage_despair=-1.

**22: PathogenCount** — Viral load, infestation. Priest healthy: -1k (no infection); bloodmage plague-bearer: +9k (disease radiates). `trit`: clean=-1, healthy=0, infected=+1.

**23: SporeDensity** — Fungal network. Priest orchard: +3k (healthy mycelium aids root); bloodmage death-forest: +10k (spore-cloud strangles). `trit`: symbiotic=+1, none=0, parasitic=-1.

## Arcane Band (24-31): Magic & Intent

**24: WeaveFlux** — Active spell resonances. Priest at prayer +5k (holy magic); bloodmage chanting +8k (profane magic). Both +1 (magic present), same magnitude, opposite moral weight.

**25: ManaDensity** — Raw unshaped ley-line energy. Priest shrine +6k (blessed ley-line); bloodmage ritual site +10k (ley-line screams, corrupted). `trit`: mana_pure=+1, depleted=0, mana_profane=-1.

**26: HateVector** — Hostility aimed at observer. Priest: -1k (love radiates); bloodmage: +10k (hunger/malice directed at intruder). `trit`: love=−1, indifference=0, hate=+1.

**27: EthosBias** — Moral & ethical charge. **PRIEST**: +9k (holy/righteous); **BLOODMAGE**: -10k (corrupt/evil). `trit`: righteous=+1, amoral=0, corrupt=−1. [THE CORRUPTION SIGN FLIPS].

**28: NeuralHz** — Synaptic firing, conscious thought. Priest serene +3k (meditative); bloodmage obsessive: +9k (obsessed with forbidden knowledge) or -8k (mind shattered). `trit`: clarity_peace=+1, dormant=0, chaos_madness=−1.

**29: ResidualTrauma** — Historical pain imprinted on place. Priest: -2k (trauma cleared/healed); bloodmage pyre: +10k (screams still echo). `trit`: healed=-1, untouched=0, wounded=+1.

**30: PlanarTear** — Spatial distortion from gates. Priest shrine: -1k (plane boundaries solid); bloodmage summoning circle: +9k (reality bends, gates yawn). `trit`: stable=-1, normal=0, torn=+1.

**31: PietyCharge** — Proximity to holy/divine. Priest temple: +10k (consecrated ground); bloodmage desecrated altar: -9k (piety inverted/blasphemed). `trit`: holy=+1, secular=0, profaned=−1.

## Priest → Bloodmage Example

A priestess stands in her cathedral. Then she falls to corruption. Same cell, different readings:

| Channel | Priest | Bloodmage | Trit △ |
|---------|--------|-----------|--------|
| EthosBias | +9k | -10k | +1 → -1 |
| PietyCharge | +9k | -8k | +1 → -1 |
| HateVector | -2k | +10k | -1 → +1 |
| NecroticDecay | -1k | +8k | -1 → +1 |
| SoulMass | +8k | -9k | +1 → -1 |
| ManaDensity | +5k | +9k | +1 → +1 (same, redirected) |
| WeaveFlux | +4k | +7k | +1 → +1 (same, redirected) |
| VitalityLux | +7k | -6k | +1 → -1 (drains life) |
| PathogenCount | -1k | +8k | -1 → +1 (plague-bearer) |
| NeuralHz | +2k | -9k | 0 → -1 (shattered mind) |

**Prose result**: The priestess-become-bloodmage reads as a mirror: where holiness blazed (+), corruption now bleeds (-). Same magnitude of *presence*, opposite moral pole. She is NOT weak; she is *inverted*. A room full of both priest and bloodmage would create contradictory sensory reports — each body would read the space as a battlefield of opposing forces.

---

## Authoring Guidelines

1. **Pick a cell/scene concept** (priest chapel, lich tomb, werewolf lair, mortal tavern).
2. **For each of 32 channels, decide its trit**: Is it holy (+1), cursed (-1), or neutral (0)?
3. **Assign permyriad value** matching the trit:
   - Trit +1: q ∈ [+4k, +10k], typical ~+6k to +8k
   - Trit 0: q ∈ [-1k, +1k], typical zero
   - Trit -1: q ∈ [-10k, -4k], typical ~-6k to -8k
4. **Write prose** that a body senses: djinn reads pressure, lich reads decay, werewolf reads scent-age + the field's band-shape.
5. **Test byte-stability**: same cell, same form, same commands → same prose (hash oracle guards this).

Corruption is not weakness — it is *inversion*. A bloodmage at +10k ManaDensity is not less magical than a priest; she is *differently* magical, and every body in the room will know which pole she stands on.
