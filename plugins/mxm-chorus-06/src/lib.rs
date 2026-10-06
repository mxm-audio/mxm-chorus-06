//! mxm-chorus-06 — the JUNO-106's bucket-brigade chorus as a standalone effect, unlocked.
//!
//! The circuit is `research:effects/juno-chorus.md`; the DSP is `mxm-poly-06`'s own chorus module,
//! depended on in place rather than copied (`plans/plan-mxm-chorus-06.md` §2), so there is one
//! implementation and the synth's render is provably unchanged. What the circuit fixes — the
//! modulator's rate, its depth, the wet level and the bucket brigades' noise floor — are the four
//! controls here, each starting at the circuit's value, and the circuit's three switch positions
//! are the panel's buttons, which put Rate on their rate (`params::POSITIONS`). Not affiliated with
//! or endorsed by Roland.
//!
//! # Mix at zero is Off, and doing nothing costs nothing
//!
//! There is no On switch (the owner's ruling, 2026-09-04). The circuit's Off is a wet-mute with the
//! dry untouched, which is exactly what the wet level at zero is. Once the mute has closed the core
//! is not run at all: the output is the input, the delay lines are emptied so nothing stale comes
//! back later, and the modulator is moved across the skipped time when the mix returns — the
//! collection's rule that an effect doing nothing uses no CPU, in both its halves: the host is told
//! (`ProcessStatus::Normal`, which under nice-plug lets a host sleep the plugin once its output is
//! quiet) and the plugin does no work while it has none.
//!
//! # Activity is the input, not a voice
//!
//! Inside the synth the noise floor follows voice activity. Here it follows the signal: a block
//! with any sample that is not exact digital zero is active, and the first all-zero block sends
//! the floor to zero. No threshold, so nothing is chosen: dither keeps the hiss alive, as the
//! hardware would, and exact silence ends it, as the synth does.
//!
//! # Two layouts
//!
//! Mono in, stereo out is the circuit's topology and the default. Stereo in, stereo out is offered
//! for hosts that insert on stereo tracks: Off passes both channels through untouched, and
//! engaged, the two are summed to mono before the chorus — as the machine's own voice sum was —
//! with the dry leg crossfaded between the two over the core's own mute interval, so a mix sweep
//! through zero is one fade and never a step. The image collapses only while the effect is on.

/// The plugin's name, and the **only** place it is written in this crate.
///
/// Everything else that names the effect derives from here: [`NAME`], which the host shows, and
/// [`CLAP_ID`], which it remembers. A rename is this line.
///
/// A macro rather than a `const` because [`CLAP_ID`] is built with `concat!`, which takes literals.
macro_rules! plugin_name {
    () => {
        "mxm-chorus-06"
    };
}

/// What the host displays.
pub const NAME: &str = plugin_name!();

/// The permanent CLAP identifier.
///
/// **Deliberately assembled from [`plugin_name!`] and not from `CARGO_PKG_NAME`.** Deriving it from
/// the package name would mean a future `git mv` of this directory silently changed the plugin's
/// permanent identity — no compile error, no failing test, and every preset and saved project
/// written under the old id orphaned. Renaming the plugin is a deliberate act that edits
/// `plugin_name!` above: one line, one decision.
pub const CLAP_ID: &str = concat!("dk.mxm.", plugin_name!());

mod editor;
mod params;
pub mod preset;
mod telemetry;

use mxm_poly_06_dsp::chorus::{Chorus, Mode, NOISE_LEVEL, WET_SWITCH_S};
use mxm_poly_06_dsp::flush;
use nice_plug::prelude::*;
use params::MxmChorus06Params;
use std::sync::Arc;
use telemetry::Telemetry;

/// How the stereo layout's dry leg moves between pass-through and the mono sum: the interval the
/// core's own mute uses, so a sweep through zero is one fade and not two of different lengths.
const SUM_BLEND_S: f32 = WET_SWITCH_S;

/// The lowest host sample rate activation accepts. Below it, or at a rate that is not finite,
/// `activate` refuses.
///
/// **Derived, with a margin.** `mxm-poly-06-dsp` holds the chorus's pre-filter corner to
/// `1 Hz ..= 0.45 × fs` with `f32::clamp`, which panics once the bounds cross, below about
/// 2.22 Hz, or when one of them is NaN. 3 Hz is the first whole number clear of that. Nothing is
/// musical down there: the floor promises only that every rate accepted runs.
pub const MIN_SAMPLE_RATE: f32 = 3.0;

pub struct MxmChorus06 {
    params: Arc<MxmChorus06Params>,
    /// The only channel to the editor: the block's peak and the modulator's position.
    telemetry: Arc<Telemetry>,
    chorus: Chorus,
    sample_rate: f32,
    /// Input channels in the negotiated layout: one or two.
    input_channels: usize,
    /// Samples of silence the wet leg is still owed after the input went quiet: the noise's fade
    /// and the lines' drain. Zero once drained. **The wrapper's settle clock**, which the core does
    /// not have — inside the synth the voice keeps it.
    tail_remaining: u32,
    /// Whether the core is being skipped: Mix at zero, the mute closed, the lines emptied.
    parked: bool,
    /// Samples skipped while parked, handed to the modulator when the mix returns.
    skipped: u32,
    /// The stereo layout's dry leg: `0` pass-through, `1` the mono sum. Smoothed.
    sum_blend: f32,
    sum_coef: f32,
    /// Rate as its sync resolved it for this block, or `None` for the knob and its detents.
    synced_rate_hz: Option<f32>,
}

impl Default for MxmChorus06 {
    fn default() -> Self {
        let mut chorus = Chorus::new();
        // Never `Off`: the mute is driven by the wet level, and `Off` would hold it shut.
        chorus.set_mode(Mode::I);
        Self {
            params: Arc::new(MxmChorus06Params::default()),
            telemetry: Telemetry::shared(),
            chorus,
            sample_rate: 48_000.0,
            input_channels: 1,
            tail_remaining: 0,
            parked: false,
            skipped: 0,
            sum_blend: 0.0,
            sum_coef: 0.0,
            synced_rate_hz: None,
        }
    }
}

impl MxmChorus06 {
    /// What `activate` does, and what a test does in its place: the sample rate, the controls'
    /// targets, and a reset so every glide has landed — a preset applied before activation is the
    /// circuit from the first sample, not twenty milliseconds later.
    fn prepare(&mut self, sample_rate: f32, input_channels: usize) {
        // Forget the last activation's tempo too: nice-plug resets right after activating, and a
        // division resolved from a tempo the host may since have changed would seed the engine.
        self.telemetry.tempo.publish(None);
        // A restored state is resolved afresh by the next block: activation must not seed the
        // engine with the division the previous state was synced to.
        self.synced_rate_hz = None;
        self.sample_rate = sample_rate;
        self.input_channels = input_channels;
        // `set_sample_rate` allocates the delay lines. Here, never in `process`.
        self.chorus.set_sample_rate(sample_rate);
        self.sum_coef = (-1.0 / (SUM_BLEND_S * sample_rate)).exp();
        self.set_targets();
        self.settle();
    }

    /// The two controls the core glides, read once per block.
    fn set_targets(&mut self) {
        let p = &self.params;
        // Synced, the rate is its division (`plans/plan-tempo-sync-controls.md`) and the circuit's
        // detents do not apply: they are positions of the free knob.
        self.chorus.set_rate_hz(
            self.synced_rate_hz
                .unwrap_or_else(|| params::circuit_rate(p.rate.value())),
        );
        self.chorus.set_depth_ms(p.depth.value());
    }

    /// Everything at rest, glides landed, nothing owed, nothing parked.
    fn settle(&mut self) {
        self.chorus.reset();
        self.tail_remaining = 0;
        self.parked = false;
        self.skipped = 0;
        self.sum_blend = if self.params.mix.value() > 0.0 {
            1.0
        } else {
            0.0
        };
    }

    /// One host block, on the channel slices the host handed over.
    ///
    /// **In place**: nice-plug has already copied the main input over the main output, so
    /// `channels[c][i]` is the input until this writes it. Factored out of `process` so a test can
    /// drive it with slices of its own and no host.
    fn process_block(&mut self, channels: &mut [&mut [f32]]) -> ProcessStatus {
        let Some(first) = channels.first() else {
            return ProcessStatus::Normal;
        };
        let n = first.len();
        // The parameter's own value, not its smoother: Mix at exactly zero is Off, and a smoother
        // passing through zero on its way somewhere is not.
        let engaged = self.params.mix.value() > 0.0;
        self.set_targets();

        if self.parked {
            if !engaged {
                // Doing nothing costs nothing. The host's copy of the input is the output; in the
                // mono layout the second channel, which nice-plug zero-filled, takes the first.
                self.skipped = self.skipped.saturating_add(n as u32);
                if self.input_channels == 1 && channels.len() > 1 {
                    let (head, rest) = channels.split_at_mut(1);
                    rest[0][..n].copy_from_slice(&head[0][..n]);
                }
                return ProcessStatus::Normal;
            }
            // The mix has returned. The lines were emptied when the core was parked; the circuit's
            // modulator ran on, so it is moved to where it would be.
            self.chorus.advance(self.skipped);
            self.skipped = 0;
            self.parked = false;
        }

        // The activity rule: any input sample that is not exact digital zero. The same pass
        // flushes a subnormal input to zero — below anything a converter carries, and on x86 a
        // multiply by one is slow enough that the validator measures it — so a subnormal counts
        // as silence, and the dry leg never passes one through. **A non-finite sample is zeroed
        // here too**, before the pre-filter, the lines and the reconstruction filters can hold
        // it: none of them lets go of a NaN until the core is parked.
        let inputs = self.input_channels.min(channels.len());
        let mut active = false;
        for ch in channels[..inputs].iter_mut() {
            for s in ch[..n].iter_mut() {
                if s.abs() < f32::MIN_POSITIVE || !s.is_finite() {
                    *s = 0.0;
                } else {
                    active = true;
                }
            }
        }
        self.chorus.set_active(active);
        if active {
            self.tail_remaining = self.chorus.tail_samples();
        }

        // The block's loudest output sample, for the app bar's meter. Accumulated where the
        // samples are written rather than in a second pass: the wet is added on top of a fixed
        // dry, so this is the only place that can say the sum clipped.
        let mut peak = 0.0f32;

        let blend_target = if engaged { 1.0 } else { 0.0 };
        for i in 0..n {
            let mix = self.params.mix.smoothed.next();
            let noise = self.params.noise.smoothed.next();
            self.chorus.set_wet_level(mix);
            self.chorus.set_noise_level(NOISE_LEVEL * noise);

            if inputs == 1 {
                // The circuit: mono in, the chorus makes the stereo.
                let x = channels[0][i];
                let (l, r) = self.chorus.process(x);
                peak = peak.max(l.abs()).max(r.abs());
                channels[0][i] = l;
                if let Some(right) = channels.get_mut(1) {
                    right[i] = r;
                }
            } else {
                // The compatibility layout: summed into the chorus, with a dry leg of its own that
                // is the two channels while Off and their sum while engaged.
                self.sum_blend = blend_target + (self.sum_blend - blend_target) * self.sum_coef;
                if (self.sum_blend - blend_target).abs() < 1e-4 {
                    self.sum_blend = blend_target;
                }
                let l = channels[0][i];
                let r = channels[1][i];
                let mono = 0.5 * (l + r);
                let (a, b) = self.chorus.wet(mono);
                let dry_l = l + self.sum_blend * (mono - l);
                let dry_r = r + self.sum_blend * (mono - r);
                let (out_l, out_r) = (flush(dry_l + a), flush(dry_r + b));
                peak = peak.max(out_l.abs()).max(out_r.abs());
                channels[0][i] = out_l;
                channels[1][i] = out_r;
            }
        }

        if !active {
            self.tail_remaining = self.tail_remaining.saturating_sub(n as u32);
        }

        // Once per block, and only on the path that ran the modulator. **Nothing is published
        // while parked**: the editor draws Off from the parameter, so the audio thread never has
        // to keep a display alive for a plugin that is doing nothing.
        self.telemetry.publish_peak(peak);
        self.telemetry.publish_lfo(self.chorus.lfo());

        // Off has fully taken: the mute is closed and, on a stereo track, the dry leg is back to
        // pass-through. From here the output is the input to the bit, so the core is parked — its
        // lines emptied now, because by the time the mix returns they would be stale.
        let dry_at_rest = inputs != 2 || self.sum_blend == 0.0;
        if !engaged && self.chorus.is_wet_silent() && dry_at_rest {
            self.chorus.silence();
            self.parked = true;
            self.skipped = 0;
            return ProcessStatus::Normal;
        }

        if active || self.tail_remaining == 0 {
            ProcessStatus::Normal
        } else {
            ProcessStatus::Tail(self.tail_remaining)
        }
    }
}

impl Plugin for MxmChorus06 {
    const NAME: &'static str = crate::NAME;
    const VENDOR: &'static str = "mxm";
    const URL: &'static str = "https://mxm.dk";
    const EMAIL: &'static str = "plugins@mxm.dk";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    /// Mono in, stereo out first: the circuit, and the default. Stereo in, stereo out for hosts
    /// that insert on stereo tracks. No stereo-in, mono-out: nice-plug does not support
    /// many-to-few main layouts, and a mono host can take the mono-in layout.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    /// An effect: no note port. Nothing here is played.
    const MIDI_INPUT: MidiConfig = MidiConfig::None;

    /// The mix and noise smoothers advance per sample, which already removes zipper noise, and
    /// the rate and depth glide inside the core.
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    /// The panel, built to `docs/briefs/mxm-chorus-06.md` — the design system's §14 gate, which
    /// this plugin passed before a line of it was drawn.
    type Editor = editor::MxmChorus06Editor;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(self.params.clone(), self.telemetry.clone())
    }

    fn activate(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        // Refused before anything is touched, so a refusal leaves the plugin as it was.
        if !buffer_config.sample_rate.is_finite() || buffer_config.sample_rate < MIN_SAMPLE_RATE {
            return false;
        }
        let inputs = audio_io_layout
            .main_input_channels
            .map_or(1, |c| c.get() as usize);
        self.prepare(buffer_config.sample_rate, inputs);
        true
    }

    fn reset(&mut self) {
        // A host resets without a callback between (a bypass, a transport restart), and a parameter
        // flush may have moved a sync meanwhile: re-resolve every sync from the parameters as they
        // stand and the last tempo seen, so nothing is seeded from the previous division.
        self.synced_rate_hz = self.params.synced_rate(self.telemetry.tempo.get());
        self.set_targets();
        self.settle();
    }

    /// **A project saved before the tempo syncs** restores each Off rather than keeping this
    /// instance's, and a loaded preset's baseline gains it, so the preset stays clean
    /// (`mxm_preset::add_switches_off`).
    fn filter_state(state: &mut PluginState) {
        mxm_preset::add_switches_off(state, crate::preset::TEMPO_SYNC_IDS);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Rate's tempo sync, once a block, and the tempo in force for the editor's reading.
        let tempo = context.transport().tempo;
        self.synced_rate_hz = self.params.synced_rate(tempo);
        self.telemetry.tempo.publish(tempo);
        self.process_block(buffer.as_slice())
    }
}

impl ClapPlugin for MxmChorus06 {
    /// Permanent. Reverse DNS of a domain the project owns. Changing it breaks every saved project
    /// using the plugin.
    const CLAP_ID: &'static str = CLAP_ID;
    const CLAP_DESCRIPTION: Option<&'static str> = Some(
        "A bucket-brigade (BBD) stereo chorus, unlocked: rate, depth, mix and hiss, with the three classic settings as buttons",
    );
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Chorus,
        ClapFeature::Stereo,
    ];
}

nice_export_clap!(MxmChorus06);

/// The plugin's name, checked where it escapes this crate.
#[cfg(test)]
mod identity {
    use super::{CLAP_ID, NAME};

    /// The id is built from the name, so it cannot drift from it.
    #[test]
    fn the_id_is_the_name_under_the_project_domain() {
        assert_eq!(CLAP_ID, format!("dk.mxm.{NAME}"));
    }

    /// `bundler.toml` names the same plugin this crate does.
    ///
    /// **The one place the plugin's name is duplicated outside this crate**, and nothing else would
    /// catch a disagreement: `bundler.toml` is read by `xtask` at bundle time, never by the plugin,
    /// so a stale display name there produces a correctly-built bundle under the wrong filename.
    #[test]
    fn the_bundle_is_named_after_this_plugin() {
        mxm_plugin_test::bundle::is_named(env!("CARGO_MANIFEST_DIR"), env!("CARGO_PKG_NAME"), NAME);
    }
}

#[cfg(test)]
mod defaults {
    use super::params::{MxmChorus06Params, circuit_rate};
    use mxm_poly_06_dsp::chorus::{DELAY_DEPTH_MS, RATE_BOTH_HZ, RATE_I_HZ, RATE_II_HZ};
    use nice_plug::prelude::*;

    /// **The default is the circuit, engaged** — the effects exception to *every amount starts at
    /// zero*: an inserted effect demonstrates the effect it is named after. Recorded in the
    /// plugin's AGENTS.md.
    #[test]
    fn the_defaults_are_the_circuit_at_its_first_position() {
        let p = MxmChorus06Params::default();
        assert_eq!(p.rate.value(), RATE_I_HZ);
        assert_eq!(p.depth.value(), DELAY_DEPTH_MS);
        assert_eq!(p.mix.value(), 1.0, "the circuit's wet level");
        assert_eq!(p.noise.value(), 1.0, "the circuit's floor");
    }

    /// The three circuit rates are detents: a knob or a preset within a hair of one lands on it
    /// exactly, and anything further off is left alone.
    #[test]
    fn the_circuit_rates_are_detents_and_nothing_else_is() {
        for c in [RATE_I_HZ, RATE_II_HZ, RATE_BOTH_HZ] {
            assert_eq!(circuit_rate(c), c);
            assert_eq!(circuit_rate(c * (1.0 + 5e-5)), c);
            assert_eq!(circuit_rate(c * (1.0 - 5e-5)), c);
            assert_ne!(circuit_rate(c * 1.01), c);
            assert_ne!(circuit_rate(c * 0.99), c);
        }
    }

    /// The ids are the public interface, and these four are it.
    #[test]
    fn the_five_ids_are_permanent() {
        let ids: Vec<String> = MxmChorus06Params::default()
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        // `ratesync`, Rate's tempo sync, joined the four on 2026-09-25.
        assert_eq!(ids, ["rate", "ratesync", "depth", "mix", "noise"]);
    }
}

/// The wrapper's own rules — activity, Off, parking, the tail, the stereo dry leg — and the one
/// test that makes the `06` true: at each of the circuit's buttons the standalone is the built-in,
/// to the bit.
#[cfg(test)]
mod processing {
    use super::*;
    use mxm_poly_06_dsp::chorus::{DELAY_DEPTH_MS, RATE_BOTH_HZ, RATE_I_HZ, RATE_II_HZ};
    use mxm_poly_06_dsp::poly::{Assign, Key, Patch, Synth};
    // `InternalParamMut` for writing a parameter without a host — see `set_normalised`.
    use nice_plug::params::InternalParamMut;

    const FS: f32 = 48_000.0;

    /// What the wrapper does at activation, so the smoothers hold their parameters' values rather
    /// than the zero they are constructed with.
    fn activate_smoothers(plugin: &MxmChorus06) {
        for (_, ptr, _) in plugin.params.param_map() {
            // SAFETY: the same call `nice_plug`'s CLAP wrapper makes in `activate`, on pointers
            // that came from this plugin's own `Params` and outlive the call.
            unsafe { ptr._internal_update_smoother(FS, true) };
        }
    }

    /// Writes one parameter through the normalised route a preset takes, without a host.
    fn set_normalised(plugin: &MxmChorus06, id: &str, v: f32) {
        let p = &plugin.params;
        // SAFETY: the call the wrapper makes when a host writes a parameter; a unit test has no
        // `ParamSetter` to go through.
        // The `bool` says whether the value *changed*, which a test writing the default does not
        // care about. The smoother is then pointed at the new value without a reset, which is what
        // the wrapper does after a host's parameter change.
        if id == "ratesync" {
            // SAFETY: as below.
            unsafe {
                let _changed = p.rate_sync._internal_set_normalized_value(v);
            }
            return;
        }
        let param: &FloatParam = match id {
            "rate" => &p.rate,
            "depth" => &p.depth,
            "mix" => &p.mix,
            "noise" => &p.noise,
            other => panic!("`{other}` is not a parameter"),
        };
        unsafe {
            let _changed = param._internal_set_normalized_value(v);
            param._internal_update_smoother(FS, false);
        }
    }

    /// A plugin activated in the given layout, with Rate put on `rate` first if given — through
    /// the normalised route a button's press takes (`ParamSetter::set_parameter` is
    /// `preview_normalized` of the plain value).
    fn plugin(inputs: usize, rate: Option<f32>) -> MxmChorus06 {
        let mut plugin = MxmChorus06::default();
        if let Some(hz) = rate {
            set_normalised(&plugin, "rate", plugin.params.rate.preview_normalized(hz));
        }
        activate_smoothers(&plugin);
        plugin.prepare(FS, inputs);
        plugin
    }

    /// Runs `input` through the plugin in the mono layout, `block` samples at a time, and returns
    /// the two output channels and the status after the last block.
    fn run_mono(
        plugin: &mut MxmChorus06,
        input: &[f32],
        block: usize,
    ) -> (Vec<(f32, f32)>, ProcessStatus) {
        let mut out = Vec::with_capacity(input.len());
        let mut status = ProcessStatus::Normal;
        for chunk in input.chunks(block) {
            let mut left = chunk.to_vec();
            // nice-plug zero-fills the output channel the layout has no input for.
            let mut right = vec![0.0f32; chunk.len()];
            let mut channels: [&mut [f32]; 2] = [&mut left, &mut right];
            status = plugin.process_block(&mut channels);
            out.extend(left.iter().zip(&right).map(|(l, r)| (*l, *r)));
        }
        (out, status)
    }

    /// The same in the stereo layout.
    fn run_stereo(
        plugin: &mut MxmChorus06,
        input: &[(f32, f32)],
        block: usize,
    ) -> (Vec<(f32, f32)>, ProcessStatus) {
        let mut out = Vec::with_capacity(input.len());
        let mut status = ProcessStatus::Normal;
        for chunk in input.chunks(block) {
            let mut left: Vec<f32> = chunk.iter().map(|(l, _)| *l).collect();
            let mut right: Vec<f32> = chunk.iter().map(|(_, r)| *r).collect();
            let mut channels: [&mut [f32]; 2] = [&mut left, &mut right];
            status = plugin.process_block(&mut channels);
            out.extend(left.iter().zip(&right).map(|(l, r)| (*l, *r)));
        }
        (out, status)
    }

    fn saw(i: usize) -> f32 {
        2.0 * ((i as f32 * 110.0 / FS) % 1.0) - 1.0
    }

    /// `mxm-poly-06` holding one note, its chorus in `mode`, master Volume at unity.
    ///
    /// Volume sits **after** the chorus in the synth and defaults to a half, where the noise
    /// floor would be scaled on one path and not the other; at unity the multiply is exact.
    fn built_in(mode: Mode, n: usize) -> Vec<(f32, f32)> {
        let mut synth = Synth::new();
        synth.set_sample_rate(FS);
        let patch = Patch {
            chorus: mode,
            volume: 1.0,
            ..Patch::default()
        };
        synth.prepare(&patch);
        synth.note_on(
            Key {
                channel: 0,
                note: 60,
            },
            None,
            Assign::Poly1,
            0.8,
        );
        (0..n).map(|_| synth.process(&patch)).collect()
    }

    /// **The `06` is true.** The synth with its chorus in I, II or Both, against the synth with
    /// its chorus Off feeding this plugin at the matching button: sample for sample, the
    /// same, on a held note so the two activity rules agree from the first sample to the last.
    /// At two block sizes, because the wrapper's activity rule is block-granular and must not
    /// show.
    #[test]
    fn at_each_button_the_standalone_is_the_built_in_to_the_bit() {
        let n = (2.0 * FS) as usize;
        let dry = built_in(Mode::Off, n);
        assert!(
            dry.iter().all(|(l, r)| l == r),
            "the premise: the synth with its chorus off is dual mono"
        );
        let source: Vec<f32> = dry.iter().map(|(l, _)| *l).collect();
        assert!(
            source.iter().any(|&s| s != 0.0),
            "the premise: the synth is sounding"
        );

        let modes = [Mode::I, Mode::II, Mode::Both];
        for ((name, hz), mode) in params::POSITIONS.into_iter().zip(modes) {
            let reference = built_in(mode, n);
            for block in [64usize, 512] {
                let mut standalone = plugin(1, Some(hz));
                let (rendered, _) = run_mono(&mut standalone, &source, block);
                let first_difference = reference.iter().zip(&rendered).position(|(a, b)| a != b);
                assert_eq!(
                    first_difference, None,
                    "{name} in blocks of {block} first differs from the built-in {mode:?} at sample {first_difference:?}"
                );
            }
        }
    }

    /// Each button lands exactly on the circuit's values, through the same normalised route its
    /// press takes — and the buttons are the circuit's three rates, in the panel's order.
    #[test]
    fn every_button_lands_on_the_circuit_exactly() {
        let rates: Vec<f32> = params::POSITIONS.iter().map(|(_, hz)| *hz).collect();
        assert_eq!(rates, [RATE_I_HZ, RATE_II_HZ, RATE_BOTH_HZ]);
        for (name, rate) in params::POSITIONS {
            let p = plugin(1, Some(rate));
            assert_eq!(params::circuit_rate(p.params.rate.value()), rate, "{name}");
            assert_eq!(p.params.depth.value(), DELAY_DEPTH_MS, "{name}");
            assert_eq!(p.params.mix.value(), 1.0, "{name}");
            assert_eq!(p.params.noise.value(), 1.0, "{name}");
        }
    }

    /// **Mix at zero is Off**: bit-exact dry in the mono layout, both channels; true pass-through
    /// in stereo. And once it has taken, the core is parked and stays parked.
    #[test]
    fn mix_at_zero_is_off_and_parks_the_core() {
        let mut mono = plugin(1, None);
        set_normalised(&mono, "mix", 0.0);
        activate_smoothers(&mono);
        let input: Vec<f32> = (0..24_000).map(saw).collect();
        let (out, status) = run_mono(&mut mono, &input, 256);
        // The mute closes over its interval — five milliseconds to a snap four orders down, so
        // a tenth of a second is comfortably past it; after that the output is the input, on both
        // sides.
        let settled = 4_800;
        for (i, (l, r)) in out.iter().enumerate().skip(settled) {
            assert_eq!((*l, *r), (input[i], input[i]), "at {i}");
        }
        assert_eq!(status, ProcessStatus::Normal);
        assert!(mono.parked, "the core is parked once Off has taken");
        let phase_before = mono.chorus.lfo();
        let skipped_before = mono.skipped;
        run_mono(&mut mono, &input, 256);
        assert_eq!(
            mono.chorus.lfo(),
            phase_before,
            "a parked block runs nothing: the modulator did not move"
        );
        assert_eq!(
            mono.skipped - skipped_before,
            input.len() as u32,
            "and the time is counted"
        );

        let mut stereo = plugin(2, None);
        set_normalised(&stereo, "mix", 0.0);
        activate_smoothers(&stereo);
        let input: Vec<(f32, f32)> = (0..24_000).map(|i| (saw(i), 0.3 * saw(i + 7))).collect();
        let (out, _) = run_stereo(&mut stereo, &input, 256);
        for (i, (l, r)) in out.iter().enumerate().skip(settled) {
            assert_eq!((*l, *r), input[i], "pass-through at {i}");
        }
        assert!(stereo.parked);
    }

    /// When the mix returns, the modulator is where the circuit's would be and the lines are
    /// empty: nothing stale from before the park comes back.
    #[test]
    fn unparking_moves_the_modulator_across_the_skipped_time_and_brings_nothing_stale() {
        let mut p = plugin(1, None);
        let loud: Vec<f32> = (0..12_000).map(saw).collect();
        run_mono(&mut p, &loud, 256);
        set_normalised(&p, "mix", 0.0);
        run_mono(&mut p, &loud, 256);
        assert!(p.parked);
        let parked_at = p.chorus.lfo();
        let skipped_before = p.skipped;
        let skip = 48_000usize;
        run_mono(&mut p, &vec![0.0; skip], 256);
        assert_eq!(p.skipped - skipped_before, skip as u32);
        let skip = p.skipped as usize;

        set_normalised(&p, "mix", 1.0);
        // The first sample of the next block unparks. Expected phase: advanced by the skip at the
        // modulator's rate.
        let (out, _) = run_mono(&mut p, &vec![0.0; 256], 256);
        assert!(!p.parked);
        let expected = {
            let mut c = Chorus::new();
            c.set_sample_rate(FS);
            c.set_mode(Mode::I);
            // put it at the parked phase, then advance
            while (c.lfo() - parked_at).abs() > 1e-3 {
                c.process(0.0);
            }
            c.advance(skip as u32);
            c.lfo()
        };
        assert!(
            (p.chorus.lfo() - expected).abs() < 0.05,
            "{} against {expected}",
            p.chorus.lfo()
        );
        // Silence in on the way back out: the emptied lines carry nothing of the earlier saw.
        assert!(
            out.iter().all(|(l, r)| l.abs() < 1e-3 && r.abs() < 1e-3),
            "stale content came back: {:?}",
            out.iter().take(8).collect::<Vec<_>>()
        );
    }

    /// The input rule is exact zero: a sample no converter could carry still counts, and only an
    /// all-zero block does not. Then the tail: reported while draining, and exact silence follows.
    #[test]
    fn activity_is_exact_zero_and_the_tail_is_reported_then_ends_in_exact_silence() {
        let mut p = plugin(1, None);
        let loud: Vec<f32> = (0..4_800).map(saw).collect();
        let (_, status) = run_mono(&mut p, &loud, 256);
        assert_eq!(
            status,
            ProcessStatus::Normal,
            "active: the host keeps calling"
        );

        let mut whisper = vec![0.0f32; 256];
        whisper[100] = 1.0e-30;
        let (_, status) = run_mono(&mut p, &whisper, 256);
        assert_eq!(
            status,
            ProcessStatus::Normal,
            "one nonzero sample is activity"
        );

        let (_, status) = run_mono(&mut p, &vec![0.0; 256], 256);
        let ProcessStatus::Tail(owed) = status else {
            panic!("silence in with the wet leg open reports a tail, not {status:?}");
        };
        assert!(owed > 0);

        let tail = p.chorus.tail_samples() as usize;
        let (out, status) = run_mono(&mut p, &vec![0.0; tail * 2], 256);
        assert_eq!(status, ProcessStatus::Normal, "drained");
        let last = &out[out.len() - 4_800..];
        assert!(
            last.iter().all(|&(l, r)| l == 0.0 && r == 0.0),
            "exact silence after the tail"
        );
    }

    /// **A non-finite input sample is silence, not state.** Let through, one NaN or infinity sits
    /// in the pre-filter, the lines and the reconstruction filters until the core is parked, and
    /// every sample after it is NaN. Rejected in the input pass it is an exact zero: not activity,
    /// not on the dry leg, and the finite input after it renders as though it had never arrived.
    #[test]
    fn a_non_finite_input_sample_is_silence_and_the_render_recovers() {
        // A saw, a silence long enough to hold a whole quiet block, then the saw again. The bad
        // sample lands inside the silence, so counting it as activity would show too.
        let n = 14_400;
        let bad_at = 6_000;
        let source = |i: usize| {
            if (4_800..8_000).contains(&i) {
                0.0
            } else {
                saw(i)
            }
        };
        let first_difference = |a: &[(f32, f32)], b: &[(f32, f32)]| {
            a.iter()
                .zip(b)
                .position(|(x, y)| x.0.to_bits() != y.0.to_bits() || x.1.to_bits() != y.1.to_bits())
        };

        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let clean: Vec<f32> = (0..n).map(source).collect();
            let mut poisoned = clean.clone();
            poisoned[bad_at] = bad;
            let (expected, _) = run_mono(&mut plugin(1, None), &clean, 256);
            let (rendered, _) = run_mono(&mut plugin(1, None), &poisoned, 256);
            let broken = rendered
                .iter()
                .filter(|(l, r)| !l.is_finite() || !r.is_finite());
            assert_eq!(broken.count(), 0, "mono: {bad} left non-finite output");
            let at = first_difference(&expected, &rendered);
            assert_eq!(at, None, "mono: after {bad} the render differs at {at:?}");

            // Stereo, on one side only: the sum into the core and that side's dry leg both see it.
            let clean: Vec<(f32, f32)> = (0..n).map(|i| (source(i), 0.5 * source(i))).collect();
            let mut poisoned = clean.clone();
            poisoned[bad_at].0 = bad;
            let (expected, _) = run_stereo(&mut plugin(2, None), &clean, 256);
            let (rendered, _) = run_stereo(&mut plugin(2, None), &poisoned, 256);
            let broken = rendered
                .iter()
                .filter(|(l, r)| !l.is_finite() || !r.is_finite());
            assert_eq!(broken.count(), 0, "stereo: {bad} left non-finite output");
            let at = first_difference(&expected, &rendered);
            assert_eq!(at, None, "stereo: after {bad} the render differs at {at:?}");
        }
    }

    /// The context `activate` is handed. Nothing here calls it.
    struct TestActivateContext;

    impl ActivateContext<MxmChorus06> for TestActivateContext {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }

        fn execute(&self, _task: ()) {}

        fn set_latency_samples(&self, _samples: u32) {}

        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    /// Activates through the host's entry point, in the layout at `layout`, at `sample_rate`.
    fn activate_at(plugin: &mut MxmChorus06, layout: usize, sample_rate: f32) -> bool {
        plugin.activate(
            &MxmChorus06::AUDIO_IO_LAYOUTS[layout],
            &BufferConfig {
                sample_rate,
                min_buffer_size: Some(1),
                max_buffer_size: 256,
                process_mode: ProcessMode::Realtime,
            },
            &mut TestActivateContext,
        )
    }

    /// **A rate that is not finite, or one below the floor, is refused**, and the refusal leaves
    /// the plugin as it was. Accepted, a NaN rate or one under about 2.2 Hz crossed the
    /// pre-filter's `clamp` bounds and panicked inside activation.
    #[test]
    fn activation_refuses_a_non_finite_rate_and_any_below_the_floor() {
        for layout in 0..MxmChorus06::AUDIO_IO_LAYOUTS.len() {
            for rate in [
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
                2.0,
                0.0,
                -48_000.0,
                MIN_SAMPLE_RATE.next_down(),
            ] {
                let mut refused = MxmChorus06::default();
                assert!(
                    !activate_at(&mut refused, layout, rate),
                    "layout {layout} accepted {rate} Hz"
                );
                assert_eq!(
                    refused.sample_rate, 48_000.0,
                    "refusing {rate} Hz changed the plugin"
                );
            }
        }
    }

    /// **Every accepted rate is a promise**: the floor itself and the ordinary rates activate in
    /// both layouts and turn a saw into finite, sounding output.
    #[test]
    fn the_rate_floor_and_the_ordinary_rates_activate_and_process() {
        for layout in 0..MxmChorus06::AUDIO_IO_LAYOUTS.len() {
            for rate in [MIN_SAMPLE_RATE, 8_000.0, 44_100.0, 96_000.0, 192_000.0] {
                let mut p = MxmChorus06::default();
                for (_, ptr, _) in p.params.param_map() {
                    // SAFETY: as in `activate_smoothers`, at this rate.
                    unsafe { ptr._internal_update_smoother(rate, true) };
                }
                assert!(
                    activate_at(&mut p, layout, rate),
                    "layout {layout} refused {rate} Hz"
                );
                assert_eq!(p.sample_rate, rate);
                let out = if layout == 0 {
                    let input: Vec<f32> = (0..4_096).map(saw).collect();
                    run_mono(&mut p, &input, 256).0
                } else {
                    let input: Vec<(f32, f32)> = (0..4_096).map(|i| (saw(i), saw(i + 7))).collect();
                    run_stereo(&mut p, &input, 256).0
                };
                assert!(
                    out.iter().all(|(l, r)| l.is_finite() && r.is_finite()),
                    "layout {layout} at {rate} Hz is not finite"
                );
                assert!(
                    out.iter().any(|(l, r)| *l != 0.0 || *r != 0.0),
                    "layout {layout} at {rate} Hz is silent"
                );
            }
        }
    }

    /// A mix sweep through zero on a stereo track: the dry leg crossfades between the two channels
    /// and their sum, and nothing steps.
    #[test]
    fn a_mix_sweep_through_zero_on_a_stereo_track_does_not_click() {
        let mut p = plugin(2, None);
        set_normalised(&p, "noise", 0.0);
        activate_smoothers(&p);
        let n = FS as usize;
        let sine = |hz: f32, i: usize| 0.5 * (std::f32::consts::TAU * hz * i as f32 / FS).sin();
        let (mut worst, mut prev) = (0.0f32, (0.0f32, 0.0f32));
        for i in 0..4 * n {
            // Down to zero over a second, hold at zero a second, back up, hold.
            let mix = if i < n {
                1.0 - i as f32 / n as f32
            } else if i < 2 * n {
                0.0
            } else if i < 3 * n {
                (i - 2 * n) as f32 / n as f32
            } else {
                1.0
            };
            set_normalised(&p, "mix", mix);
            let input = [(sine(220.0, i), sine(330.0, i))];
            let (out, _) = run_stereo(&mut p, &input, 1);
            let (l, r) = out[0];
            if i > 1_000 {
                worst = worst.max((l - prev.0).abs()).max((r - prev.1).abs());
            }
            prev = (l, r);
        }
        // A 330 Hz sine at 0.5 moves at most about 0.022 per sample, the wet up to 1.2 times as
        // much again. Anything past that is a step.
        assert!(worst < 0.08, "a step of {worst} in the sweep");
    }
}

/// **A reset re-resolves the tempo syncs**: a sync turned off while the host held the effect
/// unprocessed does not seed the reset from the previous division, and one still on stays on it.
#[cfg(test)]
mod reset_resolves_the_syncs {
    use super::*;
    use nice_plug::params::InternalParamMut;

    #[test]
    fn a_reset_re_resolves_the_tempo_syncs() {
        let mut plugin = MxmChorus06 {
            synced_rate_hz: Some(9.0),
            ..Default::default()
        };
        plugin.reset();
        assert_eq!(
            plugin.synced_rate_hz, None,
            "sync off: the knob and its detents"
        );
        plugin.telemetry.tempo.publish(Some(120.0));
        unsafe {
            let _ = plugin.params.rate_sync._internal_set_normalized_value(1.0);
        }
        plugin.reset();
        assert_eq!(
            plugin.synced_rate_hz,
            plugin.params.synced_rate(Some(120.0)),
            "sync on: the division at the last tempo"
        );
    }

    /// **Reactivation forgets the old tempo**: nice-plug resets right after activating, and that
    /// reset must not resolve from the tempo the host reported before it was deactivated — the first
    /// callback's tempo is the first one used.
    #[test]
    fn reactivation_forgets_the_previous_tempo() {
        let mut plugin = MxmChorus06::default();
        plugin.telemetry.tempo.publish(Some(120.0));
        unsafe {
            let _ = plugin.params.rate_sync._internal_set_normalized_value(1.0);
        }
        plugin.prepare(48_000.0, 1);
        plugin.reset();
        assert_eq!(plugin.synced_rate_hz, None);
    }
}

/// What a player reads — on hover in the editor, and in a host's plugin browser — speaks to the
/// player about the sound, never about the machine or the code (`mxm_plugin_test::hover_text`).
#[cfg(test)]
mod speaks_to_the_player {
    #[test]
    fn hover_text() {
        mxm_plugin_test::hover_text::speaks_to_the_player(env!("CARGO_MANIFEST_DIR"));
    }

    #[test]
    fn host_description() {
        mxm_plugin_test::hover_text::host_description_speaks_to_the_player(env!(
            "CARGO_MANIFEST_DIR"
        ));
    }
}
