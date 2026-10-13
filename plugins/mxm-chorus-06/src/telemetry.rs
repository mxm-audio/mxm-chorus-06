//! The **only** channel from the audio thread to the editor.
//!
//! Atomics, written once per block, read whenever the editor happens to look. No locks, no
//! allocation, and the UI may drop as many frames as it likes — a display that made the audio
//! thread wait would be a display that could cause a dropout.
//!
//! **The meter is `mxm_telemetry`'s**, held as `meter`, with one policy for every plugin: a peak
//! is max-combined and reset when the UI reads it, a clip latches until acknowledged (design
//! system §5.4), and a non-finite peak latches the clip and is never the level. What follows here
//! is this plugin's own.
//!
//! # The modulator is this plugin's own display
//!
//! The brief's §8 asks for one thing the numbers cannot say: **the two delay lines riding one
//! triangle in antiphase**. So the block's modulator value travels here — [`Chorus::lfo`]'s own
//! output, the value the audio was made with, never a triangle recomputed in the editor against
//! the editor's clock. A display driven by its own clock would drift from the sound and would keep
//! sweeping while the plugin was doing nothing, which is the exact lie a display exists to prevent.
//!
//! **Nothing is published while the plugin is parked**, because a parked plugin runs no modulator:
//! Mix is at zero, and the editor draws the Off state from the parameter rather than from a value
//! the audio thread would have to keep inventing.
//!
//! # No developer channel here, and that is deliberate
//!
//! `plugins/AGENTS.md`'s developer channel arrives as MIDI CC. **An effect has no note port** — it
//! is what makes it an effect — and the player's `cc` verb reaches the loaded instrument, not the
//! chain after it. So there is no route, and inventing one would mean giving an effect a note
//! input purely for debugging: a lie about what the plugin is, in every host that lists ports.
//! The editor it would drive has one view and no expander, so what is lost is the preset browser's
//! request and nothing else.
//!
//! [`Chorus::lfo`]: mxm_poly_06_dsp::chorus::Chorus::lfo

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug)]
pub struct Telemetry {
    /// The output level and its clip latch: the collection's one meter policy
    /// (`mxm_telemetry::PeakMeter`).
    pub meter: mxm_telemetry::PeakMeter,
    /// The modulating triangle, `-1..=1`, once per block. Not written while parked.
    lfo: AtomicU32,
    /// The host tempo in force, so a synced Rate reads its division.
    pub tempo: mxm_tempo::TempoCell,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            meter: mxm_telemetry::PeakMeter::new(),
            lfo: AtomicU32::new(0),
            tempo: mxm_tempo::TempoCell::new(),
        }
    }

    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    // ---- audio thread ----

    /// Publish the block's modulator position, `-1..=1`.
    pub fn publish_lfo(&self, value: f32) {
        self.lfo.store(value.to_bits(), Ordering::Relaxed);
    }

    // ---- editor thread ----

    /// Where the modulating triangle stood at the end of the last block it ran.
    pub fn lfo(&self) -> f32 {
        f32::from_bits(self.lfo.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modulator_round_trips_including_its_sign() {
        let t = Telemetry::new();
        assert_eq!(t.lfo(), 0.0, "a fresh instance sits at the centre");
        for value in [-1.0, -0.5, 0.0, 0.25, 1.0] {
            t.publish_lfo(value);
            assert_eq!(t.lfo(), value);
        }
    }
}
