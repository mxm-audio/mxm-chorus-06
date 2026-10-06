//! Parameter definitions.
//!
//! Every `#[id]` here is **permanent**. Changing one breaks every saved project that used the
//! plugin, so ids are part of the public interface.
//!
//! # Four controls, each the circuit's fixed quantity opened
//!
//! Rate, depth, mix and noise: what the JUNO-106's chorus circuit fixes (`research:effects/juno-chorus.md`
//! §*Say what is fixed*), unlocked by the owner's ruling (2026-09-03) for the standalone. Each
//! defaults to the circuit's value, so a chorus nobody has adjusted **is** the circuit. The three
//! switch positions are the panel's buttons ([`POSITIONS`]), which set Rate rather than being a
//! second control for it. There is no On: Mix at zero is Off (the owner's ruling, 2026-09-04).
//!
//! # Where the circuit is exact
//!
//! A preset that says "the circuit" must reach the circuit's numbers to the bit, because the
//! standalone is proved equal to the built-in at those numbers. Depth's range is exactly twice
//! the circuit's depth, so the circuit is the linear control's midpoint and a half is exact in
//! `f32`; mix and noise are `0..=1` with the circuit at the top, and the top is exact. The rate is
//! a skewed control whose normalised round trip is not exact, so the three circuit rates are
//! **detents** — [`circuit_rate`] snaps a value within a hair of one onto it — and a button lands.
//!
//! # Smoothing
//!
//! Mix and noise multiply into the audio and are smoothed here, per sample. Rate and depth are
//! handed to the core as targets and glide inside it (`chorus.rs`, [`CONTROL_SLEW_S`]), so they
//! carry no smoother of their own — two smoothers in series would be one too many.
//!
//! [`CONTROL_SLEW_S`]: mxm_poly_06_dsp::chorus::CONTROL_SLEW_S

use mxm_poly_06_dsp::chorus::{DELAY_DEPTH_MS, DEPTH_MAX_MS, RATE_BOTH_HZ, RATE_I_HZ, RATE_II_HZ};
use mxm_preset::PresetIdentity;
use nice_plug::prelude::*;
use std::sync::{Arc, RwLock};

/// The rate control's ends. **Chosen**: below the circuit's slowest by a decade, and up to where a
/// chorus becomes a vibrato, with the circuit's three positions in the middle of the travel.
pub const RATE_MIN_HZ: f32 = 0.05;
pub const RATE_MAX_HZ: f32 = 10.0;

/// The three circuit rates as detents.
///
/// A value within `1e-4` of a circuit rate, relatively, is that rate. Inaudible as a snap — a
/// hundredth of a percent of the rate — and it is what lets a preset written as a normalised value
/// through a skewed range reach the circuit exactly.
pub fn circuit_rate(hz: f32) -> f32 {
    for circuit in [RATE_I_HZ, RATE_II_HZ, RATE_BOTH_HZ] {
        if (hz - circuit).abs() <= circuit * 1e-4 {
            return circuit;
        }
    }
    hz
}

/// **The circuit's buttons**: I, II and both together, each the rate it selects — the only thing
/// the hardware's switches change (`research:effects/juno-chorus.md`). On the panel they are a row
/// of buttons that put Rate on that rate (the owner, 2026-09-28: *the original has buttons for
/// that*); they are not a parameter, so what is saved and automated is Rate itself.
pub const POSITIONS: [(&str, f32); 3] = [
    ("I", RATE_I_HZ),
    ("II", RATE_II_HZ),
    ("I + II", RATE_BOTH_HZ),
];

/// Which of [`POSITIONS`] the Rate knob is on, if any: through the detent, and never while the
/// rate follows the tempo, where the knob picks a division rather than the circuit's rate.
pub fn position(rate_hz: f32, synced: bool) -> Option<usize> {
    if synced {
        return None;
    }
    let rate = circuit_rate(rate_hz);
    POSITIONS.iter().position(|(_, hz)| *hz == rate)
}

/// Formats a parameter value for display.
type ValueToString = Arc<dyn Fn(f32) -> String + Send + Sync>;
/// Parses a typed-in value, returning `None` if it cannot be understood.
type StringToValue = Arc<dyn Fn(&str) -> Option<f32> + Send + Sync>;

/// Show a `0..=1` control as a percentage.
fn v2s_percent() -> ValueToString {
    Arc::new(|v| format!("{:.0} %", v * 100.0))
}

fn s2v_percent() -> StringToValue {
    Arc::new(|text| {
        text.trim()
            .trim_end_matches('%')
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v / 100.0)
    })
}

/// **The LFO rate's tempo sync** (`plans/plan-tempo-sync-controls.md`): every LFO's ladder, 1/32 to
/// four bars, the top the fastest.
pub const LFO_SYNC: mxm_tempo::Ladder =
    mxm_tempo::Ladder::new(mxm_tempo::Span::LFO, mxm_tempo::Direction::Rate);

#[derive(Params)]
pub struct MxmChorus06Params {
    /// The one triangle's rate. The circuit's three positions are detents.
    #[id = "rate"]
    pub rate: FloatParam,
    /// Rate's tempo sync: its position picks a division of the host's tempo, past the detents.
    #[id = "ratesync"]
    pub rate_sync: BoolParam,
    /// How far the triangle swings the delay either side of its centre. The circuit is halfway.
    #[id = "depth"]
    pub depth: FloatParam,
    /// The wet level, as a fraction of the circuit's. **Zero is Off.**
    #[id = "mix"]
    pub mix: FloatParam,
    /// The bucket brigades' noise floor, as a fraction of the circuit's.
    #[id = "noise"]
    pub noise: FloatParam,

    /// Which preset is loaded, and what it looked like when it was.
    ///
    /// **Persisted with the patch, not beside it.** nice-plug carries non-parameter state through
    /// the `Params` derive's `#[persist]`, so it belongs here rather than as a field on the plugin
    /// struct — and it has its own version number, because nice-plug's state version is
    /// `Plugin::VERSION`, which moves for unrelated reasons.
    ///
    /// The shape is `mxm-preset`'s, shared with every instrument: what is loaded, and the values
    /// it was loaded with, which is what makes *modified* a comparison rather than a flag somebody
    /// has to remember to set.
    #[persist = "preset"]
    pub preset: RwLock<PresetIdentity>,
}

impl Default for MxmChorus06Params {
    /// The init patch, which is also the set of CLAP `default_value`s: **the circuit, engaged, at
    /// its first position.** An effects exception to the instruments' *every amount starts at
    /// zero* — an inserted effect demonstrates the effect it is named after — recorded in the
    /// plugin's AGENTS.md.
    fn default() -> Self {
        Self {
            rate: FloatParam::new(
                "Rate",
                RATE_I_HZ,
                FloatRange::Skewed {
                    min: RATE_MIN_HZ,
                    max: RATE_MAX_HZ,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            rate_sync: BoolParam::new("Rate sync", false),
            depth: FloatParam::new(
                "Depth",
                DELAY_DEPTH_MS,
                FloatRange::Linear {
                    min: 0.0,
                    max: DEPTH_MAX_MS,
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            mix: FloatParam::new("Mix", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_value_to_string(v2s_percent())
                .with_string_to_value(s2v_percent()),
            noise: FloatParam::new("Noise", 1.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(10.0))
                .with_value_to_string(v2s_percent())
                .with_string_to_value(s2v_percent()),

            preset: RwLock::new(PresetIdentity::none()),
        }
    }
}

impl MxmChorus06Params {
    /// The rate while its sync follows the host, or `None` for its free value: the modulated
    /// position picks a division on [`LFO_SYNC`]. Resolved once a buffer by the plugin.
    pub fn synced_rate(&self, tempo: Option<f64>) -> Option<f32> {
        let param = &self.rate;
        LFO_SYNC
            .resolve(
                self.rate_sync.value(),
                tempo,
                param.modulated_normalized_value(),
                f64::from(param.preview_plain(0.0)),
                f64::from(param.preview_plain(1.0)),
            )
            .map(|hz| hz as f32)
    }
}

#[cfg(test)]
mod sync_tests {
    use super::*;

    /// **A button is lit only while Rate sits on its rate** ([`position`]): each of the circuit's
    /// rates, and a hair off it through the detent, lights its own; a rate between them lights
    /// none, and so does any rate while it follows the tempo.
    #[test]
    fn a_button_is_lit_only_on_its_rate() {
        for (index, (name, hz)) in POSITIONS.iter().enumerate() {
            assert_eq!(position(*hz, false), Some(index), "{name}");
            assert_eq!(
                position(hz * (1.0 + 5e-5), false),
                Some(index),
                "{name}, through the detent"
            );
            assert_eq!(position(*hz, true), None, "{name}, synced");
        }
        for between in [0.62, 1.0, RATE_MIN_HZ, RATE_MAX_HZ] {
            assert_eq!(position(between, false), None, "{between} Hz");
        }
    }

    /// **The rate sync picks a division and is inert without a tempo**
    /// (`plans/plan-tempo-sync-controls.md`): off, or with no tempo, the knob's own hertz stand; on
    /// at 120 bpm the ends are the ladder's ends that the range can hold, the top the fastest.
    #[test]
    fn rate_sync_picks_a_division_and_is_inert_without_a_tempo() {
        use nice_plug::params::InternalParamMut;
        fn set<P: InternalParamMut>(param: &P, normalized: f32) {
            unsafe {
                let _ = param._internal_set_normalized_value(normalized);
            }
        }
        let p = MxmChorus06Params::default();
        set(&p.rate, 1.0);
        assert_eq!(p.synced_rate(Some(120.0)), None, "off is the free rate");
        set(&p.rate_sync, 1.0);
        assert_eq!(p.synced_rate(None), None, "no tempo is the free rate");

        let top = p.synced_rate(Some(120.0)).expect("synced at a tempo");
        set(&p.rate, 0.0);
        let bottom = p.synced_rate(Some(120.0)).expect("synced at a tempo");
        let (lo, hi) = (
            f64::from(p.rate.preview_plain(0.0)),
            f64::from(p.rate.preview_plain(1.0)),
        );
        assert!(
            top > bottom,
            "the top of a rate is the fastest: {bottom} to {top}"
        );
        let reach = LFO_SYNC.reachable(120.0, lo, hi).divisions();
        let fastest = reach[0].hz(120.0) as f32;
        let slowest = reach[reach.len() - 1].hz(120.0) as f32;
        assert!((top - fastest).abs() < 1e-4, "{top} against {fastest}");
        assert!(
            (bottom - slowest).abs() < 1e-4,
            "{bottom} against {slowest}"
        );
    }
}
