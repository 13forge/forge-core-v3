//! song — "one cart one song": an integer PCM synth that emits mono WAV data.
//!
//! Ported 2026-08-17 from `F:\NewRepo\crates\forge-studio\src\song.rs` (~254 LOC).
//! Stripped: `crate::artifact` import and `Song::artifact()` method (artifact module is
//! load-bearing/large; synth core intact). Kept: Bhaskara sine + chiptune oscillators,
//! permyriad ADSR envelope, MIDI→milli-hertz via 12-TET ratio table + octave shift,
//! monophonic i16 PCM rendering, RIFF/WAVE encoding via [`wav_mono16`].
//!
//! Deterministic, float-free (DET-CLOCK law): no allocator in hot path, all integer math.

/// Oscillator shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Osc {
    /// Sine wave (Bhaskara-I approximation).
    Sine,
    /// Square wave.
    Square,
    /// Sawtooth wave.
    Saw,
    /// Triangle wave.
    Triangle,
}

/// Bhaskara-I sine in per-mille (−1000..=1000), argument in degrees. Exact
/// integer, no float, no LUT.
///
/// # Arguments
/// * `deg` - Angle in degrees; will be normalized to [0, 360).
///
/// # Returns
/// Sine value in per-mille (−1000..=1000), where 1000 represents 1.0.
pub fn sine_permille(deg: i64) -> i64 {
    let d = ((deg % 360) + 360) % 360;
    if d <= 180 {
        let x = d * (180 - d);
        (4 * x * 1000) / (40_500 - x)
    } else {
        let e = d - 180;
        let x = e * (180 - e);
        -((4 * x * 1000) / (40_500 - x))
    }
}

impl Osc {
    /// Sample at phase `deg` (0..360), per-mille amplitude (−1000..=1000).
    ///
    /// # Arguments
    /// * `deg` - Phase in degrees; will be normalized to [0, 360).
    ///
    /// # Returns
    /// Sample value in per-mille (−1000..=1000).
    pub fn sample_permille(self, deg: i64) -> i64 {
        let d = ((deg % 360) + 360) % 360;
        match self {
            Osc::Sine => sine_permille(d),
            Osc::Square => {
                if d < 180 {
                    1000
                } else {
                    -1000
                }
            }
            Osc::Saw => (d * 2000) / 360 - 1000,
            Osc::Triangle => {
                if d < 180 {
                    (d * 2000) / 180 - 1000
                } else {
                    1000 - ((d - 180) * 2000) / 180
                }
            }
        }
    }
}

/// A/D/S/R times in milliseconds; sustain level in per-mille.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Adsr {
    /// Attack time in milliseconds.
    pub attack_ms: u32,
    /// Decay time in milliseconds.
    pub decay_ms: u32,
    /// Sustain level in per-mille (0..=1000).
    pub sustain_pm: i64,
    /// Release time in milliseconds.
    pub release_ms: u32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self { attack_ms: 8, decay_ms: 40, sustain_pm: 700, release_ms: 60 }
    }
}

impl Adsr {
    /// Envelope value (per-mille) at `t_ms` into a note of `dur_ms`.
    ///
    /// # Arguments
    /// * `t_ms` - Time in milliseconds since note start.
    /// * `dur_ms` - Total note duration in milliseconds.
    ///
    /// # Returns
    /// Envelope level in per-mille (0..=1000).
    pub fn level_pm(&self, t_ms: u32, dur_ms: u32) -> i64 {
        let a = self.attack_ms.max(1);
        if t_ms < self.attack_ms {
            return (t_ms as i64 * 1000) / a as i64;
        }
        let ad = self.attack_ms + self.decay_ms;
        if t_ms < ad {
            let into = (t_ms - self.attack_ms) as i64;
            let span = self.decay_ms.max(1) as i64;
            return 1000 - ((1000 - self.sustain_pm) * into) / span;
        }
        // release measured from the note end
        let rel_start = dur_ms.saturating_sub(self.release_ms);
        if t_ms >= rel_start {
            let into = (t_ms - rel_start) as i64;
            let span = self.release_ms.max(1) as i64;
            return (self.sustain_pm * (span - into).max(0)) / span;
        }
        self.sustain_pm
    }
}

const RATIO: [i64; 12] = [1000, 1059, 1122, 1189, 1260, 1335, 1414, 1498, 1587, 1682, 1782, 1888];

/// MIDI note → milli-hertz (A4=69 → 440000). 12-TET, integer, octave-shifted.
///
/// # Arguments
/// * `midi` - MIDI note number (0..128).
///
/// # Returns
/// Frequency in milli-hertz (i.e., frequency in Hz × 1000).
pub fn midi_to_mhz(midi: u8) -> i64 {
    let n = midi as i64 - 69;
    let octave = n.div_euclid(12);
    let idx = n.rem_euclid(12) as usize;
    let base = 440_000 * RATIO[idx] / 1000;
    if octave >= 0 {
        base << octave
    } else {
        base >> (-octave)
    }
}

/// One note: pitch, duration, velocity (0..127).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note {
    /// MIDI note number.
    pub midi: u8,
    /// Duration in milliseconds.
    pub dur_ms: u32,
    /// Velocity (0..127).
    pub vel: u8,
}

impl Note {
    /// Create a new note.
    ///
    /// # Arguments
    /// * `midi` - MIDI note number.
    /// * `dur_ms` - Duration in milliseconds.
    /// * `vel` - Velocity (0..127).
    pub fn new(midi: u8, dur_ms: u32, vel: u8) -> Self {
        Self { midi, dur_ms, vel }
    }
}

/// A monophonic sequence rendered at `sample_rate` with one oscillator + ADSR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    /// Sample rate in Hz (typically 44100).
    pub sample_rate: u32,
    /// Oscillator waveform.
    pub osc: Osc,
    /// ADSR envelope.
    pub adsr: Adsr,
    /// Sequence of notes.
    pub notes: Vec<Note>,
}

impl Song {
    /// Create a new song with the given oscillator and default sample rate (44100 Hz).
    ///
    /// # Arguments
    /// * `osc` - The oscillator waveform to use.
    pub fn new(osc: Osc) -> Self {
        Self { sample_rate: 44_100, osc, adsr: Adsr::default(), notes: Vec::new() }
    }

    /// Add a note to the song.
    ///
    /// # Arguments
    /// * `midi` - MIDI note number.
    /// * `dur_ms` - Duration in milliseconds.
    /// * `vel` - Velocity (0..127).
    ///
    /// # Returns
    /// A mutable reference to self for chaining.
    pub fn note(&mut self, midi: u8, dur_ms: u32, vel: u8) -> &mut Self {
        self.notes.push(Note::new(midi, dur_ms, vel));
        self
    }

    /// Total sample count across all notes.
    pub fn sample_count(&self) -> usize {
        self.notes.iter().map(|n| (n.dur_ms as u64 * self.sample_rate as u64 / 1000) as usize).sum()
    }

    /// Render the whole sequence to i16 mono PCM. Deterministic.
    pub fn render_pcm(&self) -> Vec<i16> {
        let sr = self.sample_rate.max(1) as i64;
        let mut pcm = Vec::with_capacity(self.sample_count());
        for note in &self.notes {
            let mhz = midi_to_mhz(note.midi);
            let n = (note.dur_ms as i64 * sr) / 1000;
            for i in 0..n {
                // phase degrees = i * freq * 360 / sr, freq in Hz = mhz/1000.
                let deg = (i * mhz * 360) / (sr * 1000);
                let osc = self.osc.sample_permille(deg); // −1000..1000
                let t_ms = (i * 1000 / sr) as u32;
                let env = self.adsr.level_pm(t_ms, note.dur_ms); // 0..1000
                let vel = note.vel.min(127) as i64;
                // −1000..1000 · env/1000 · vel/127 · 30000/1000 → i16
                let s = osc * env / 1000 * vel / 127 * 30_000 / 1000;
                pcm.push(s.clamp(-32_768, 32_767) as i16);
            }
        }
        pcm
    }

    /// Pack the rendered PCM as a canonical mono 16-bit WAV file.
    pub fn wav_bytes(&self) -> Vec<u8> {
        wav_mono16(&self.render_pcm(), self.sample_rate)
    }
}

/// Encode i16 mono PCM into a RIFF/WAVE (PCM) byte stream.
///
/// # Arguments
/// * `pcm` - Slice of i16 PCM samples.
/// * `sample_rate` - Sample rate in Hz.
///
/// # Returns
/// Complete RIFF/WAVE file as bytes (header + data).
pub fn wav_mono16(pcm: &[i16], sample_rate: u32) -> Vec<u8> {
    let channels: u16 = 1;
    let bits: u16 = 16;
    let block_align: u16 = channels * bits / 8;
    let byte_rate: u32 = sample_rate * block_align as u32;
    let data_len: u32 = (pcm.len() * 2) as u32;
    let riff_len: u32 = 36 + data_len;

    let mut b = Vec::with_capacity(44 + pcm.len() * 2);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&riff_len.to_le_bytes());
    b.extend_from_slice(b"WAVE");
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    b.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&byte_rate.to_le_bytes());
    b.extend_from_slice(&block_align.to_le_bytes());
    b.extend_from_slice(&bits.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_hits_known_points() {
        assert_eq!(sine_permille(0), 0);
        assert_eq!(sine_permille(90), 1000);
        assert_eq!(sine_permille(180), 0);
        assert_eq!(sine_permille(270), -1000);
        // 30° ≈ 0.5
        let s30 = sine_permille(30);
        assert!((s30 - 500).abs() <= 5, "sin30 ~= 500, got {s30}");
    }

    #[test]
    fn oscillators_are_exact_integers() {
        assert_eq!(Osc::Square.sample_permille(10), 1000);
        assert_eq!(Osc::Square.sample_permille(200), -1000);
        assert_eq!(Osc::Saw.sample_permille(0), -1000);
        assert_eq!(Osc::Saw.sample_permille(180), 0);
        assert_eq!(Osc::Triangle.sample_permille(90), 0);
        assert_eq!(Osc::Triangle.sample_permille(0), -1000);
    }

    #[test]
    fn midi_pitch_table() {
        assert_eq!(midi_to_mhz(69), 440_000); // A4
        assert_eq!(midi_to_mhz(81), 880_000); // A5
        assert_eq!(midi_to_mhz(57), 220_000); // A3
        let c4 = midi_to_mhz(60);
        assert!((c4 - 261_600).abs() < 1500, "C4 ~= 261.6 Hz, got {c4} mHz");
    }

    #[test]
    fn adsr_shapes_the_note() {
        let a = Adsr { attack_ms: 10, decay_ms: 10, sustain_pm: 600, release_ms: 10 };
        assert_eq!(a.level_pm(0, 100), 0); // start of attack
        assert_eq!(a.level_pm(10, 100), 1000); // peak
        assert_eq!(a.level_pm(20, 100), 600); // after decay = sustain
        assert_eq!(a.level_pm(50, 100), 600); // sustain plateau
        assert_eq!(a.level_pm(100, 100), 0); // end of release
    }

    #[test]
    fn renders_pcm_and_a_valid_wav() {
        let mut song = Song::new(Osc::Sine);
        song.note(69, 100, 100).note(72, 100, 100).note(76, 100, 100);
        let pcm = song.render_pcm();
        assert_eq!(pcm.len(), song.sample_count());
        assert!(pcm.iter().any(|&s| s.abs() > 1000), "must produce audible signal");

        let wav = song.wav_bytes();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + pcm.len() * 2);
    }
}
