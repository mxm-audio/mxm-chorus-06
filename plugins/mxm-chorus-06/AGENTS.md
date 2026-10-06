# AGENTS.md — plugins/mxm-chorus-06

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

The nice-plug shell for **mxm-chorus-06**: the JUNO-106’s bucket-brigade chorus as a standalone
CLAP audio effect, unlocked. Identity, the five parameters (four controls and Rate's tempo sync),
presets, the two layouts and the wrapper's own rules — activity, Off, parking, the tail. The DSP is
[`crates/mxm-poly-06-dsp`](https://github.com/mxm-audio/mxm-poly-06/blob/main/crates/mxm-poly-06-dsp/AGENTS.md)'s `chorus` module, **depended
on in place**; the plan that shaped this is `plans/plan-mxm-chorus-06.md` (`plans/plan-mxm-chorus-06.md` in the private archive),
under `plans/plan-mxm-fx-collection.md` (`plans/plan-mxm-fx-collection.md` in the private archive).

Shared conventions — nice-plug's API, the preset rules, `process()` realtime rules, the editor
contract — live in the parent and are not restated here. This doc holds what is **local to this
plugin**. The history, measurements and reasoning behind each contract are in [NOTES.md](NOTES.md).

# Ownership

`Cargo.toml`, `LICENSE`, `README.md`, `control-map.json`, `presets/`, and `src/` — `lib.rs`,
`params.rs`, `preset.rs`, `telemetry.rs`, and `editor.rs` with `editor/{binding, sections}.rs`.
The brief it is built to is [`docs/briefs/mxm-chorus-06.md`](../../docs/briefs/mxm-chorus-06.md),
which is root-owned and gates the editor.

# Local Contracts

## Permanent identifiers

| What | Value |
|---|---|
| `CLAP_ID` | `dk.mxm.mxm-chorus-06` — assembled from `plugin_name!` in `src/lib.rs`, **not** from `CARGO_PKG_NAME` |
| Parameter `#[id]`s | `rate` `ratesync` `depth` `mix` `noise` |
| Control-map roles | `fx.chorus_rate` `fx.chorus_depth` `fx.chorus_mix` — appended to the standard's page 9 with this plugin; `fx.chorus` deliberately unfilled (stepped, and the circuit's buttons set Rate rather than being a parameter) |

Four controls and Rate's tempo sync. Treat all of it as public interface.
`the_five_ids_are_permanent` pins the list.

**`ratesync` is the collection's one tempo sync**, on `params::LFO_SYNC`, resolved once a block from
the modulated position (`MxmChorus06Params::synced_rate`). Synced, **the circuit's detents do not
apply** ([NOTES.md](NOTES.md#rates-tempo-sync)).

## The circuit is inside the product, to the bit

([NOTES.md](NOTES.md#the-product-is-the-circuit-unlocked-and-the-circuit-is-inside-it))

- **The four controls are the quantities the circuit fixes** (rate, depth, mix, noise), each
  defaulting to the circuit's value (the owner, 2026-09-03).
- **The circuit's three switch positions are the panel's buttons** (`params::POSITIONS`: I, II,
  I + II), each the rate it selects.
- **`at_each_button_the_standalone_is_the_built_in_to_the_bit`**: `mxm-poly-06` with its chorus in
  I, II or Both equals `mxm-poly-06` with its chorus Off feeding this plugin at the matching button,
  sample for sample, at two block sizes. Its conditions: the synth's master **Volume at unity**, a
  **held-note score**, state and seeds reset together, the mono layout. A score with note boundaries
  is not compared under an equality claim; if one ever is, its differing intervals are declared
  first.
- **Where the circuit is exact**: Depth's midpoint, and mix and noise at the top. The three circuit
  rates are **detents**: `params::circuit_rate` snaps a value within `1e-4` of one onto it
  (`the_circuit_rates_are_detents_and_nothing_else_is`).

## Mix at zero is Off, and doing nothing costs nothing

([NOTES.md](NOTES.md#mix-at-zero-is-off-and-doing-nothing-costs-nothing))

- No On switch (the owner, 2026-09-04). `Mix` at exactly zero — the parameter's value, not its
  smoother — is Off.
- Once the mute has closed (and on a stereo track the dry leg is back to pass-through) the core is
  **parked**: lines emptied, no part of it run, a block costs a copy; `ProcessStatus::Normal`.
- When the mix returns the modulator is moved across the skipped time (`Chorus::advance`).
- **One chosen deviation**: the lines are empty when Mix leaves zero, and the wet builds over the
  delay time. Outside the equivalence path. `mix_at_zero_is_off_and_parks_the_core` and
  `unparking_moves_the_modulator_across_the_skipped_time_and_brings_nothing_stale` hold both halves.

## Activity is the input, not a voice

([NOTES.md](NOTES.md#activity-is-the-input-not-a-voice))

- **A block with any input sample that is not exact digital zero is active**; the first all-zero
  block sends the noise floor to zero. No RMS threshold, gate time or sensitivity control.
- The same pass **flushes a subnormal input to zero** and **zeroes a non-finite sample**: both count
  as silence (`a_non_finite_input_sample_is_silence_and_the_render_recovers`). A parked block
  inspects nothing.
- **The wrapper owns the settle clock**: after the input goes quiet it processes silence for the
  core's declared tail, reports `Tail(n)` each block, then `Normal` once drained
  (`activity_is_exact_zero_and_the_tail_is_reported_then_ends_in_exact_silence`). Status is
  never derived from `Chorus::is_quiet()` alone.

## Two layouts, and what stereo costs

([NOTES.md](NOTES.md#two-layouts-and-what-stereo-costs))

- **Mono in, stereo out** is the default, the circuit's topology; the plugin writes both outputs.
- **Stereo in, stereo out**: at Mix zero, true left/right pass-through. Engaged, the two are summed
  to mono and **the dry leg crossfades** over the core's own mute interval
  (`a_mix_sweep_through_zero_on_a_stereo_track_does_not_click`).
- **No stereo-in, mono-out**, and a mono-in, stereo-out main pair is **not an in-place pair**; the
  validator on this bundle is the regression test for the MXM nice-plug fork's fix 5.

"Bit-transparent" is claimed for the mono layout at Mix zero, where it is true, and nowhere else.

## Activation refuses a rate the core cannot run at

`activate` returns `false` for a non-finite sample rate or one below `MIN_SAMPLE_RATE` (3 Hz),
before it touches anything. The floor is derived, not musical, and no ceiling is set
([NOTES.md](NOTES.md#activation-refuses-a-rate-the-core-cannot-run-at)).
`activation_refuses_a_non_finite_rate_and_any_below_the_floor` and
`the_rate_floor_and_the_ordinary_rates_activate_and_process` hold both sides of it.

## Defaults and presets

([NOTES.md](NOTES.md#defaults-and-presets--the-effects-rules-not-the-instruments))

- **The default is the circuit at position I, engaged** — the effects exception to *every amount
  starts at zero* (parent, *An effect starts engaged*). Init has no file.
- **Ten chorus sounds**, designed in the controls' own units in `preset.rs`'s `FACTORY_DESIGN` and
  generated to JSON by the `#[ignore]`d `write_the_factory_presets`; never hand-edit the files.
  `the_factory_files_match_the_design_they_were_generated_from`,
  `no_factory_preset_is_init_or_a_button` and `a_synced_preset_picks_its_division` hold them. Each
  carries `Category::Fx`.
- **Presets are `crates/mxm-preset`'s**, and `editor/binding.rs` re-exports `mxm_preset::binding`.
  What is left here is the `Instrument` impl, `FACTORY_FILES`, and the ten sounds.

## No note port, so no developer channel

An effect declares `MidiConfig::None`, so this plugin **cannot carry** the parent's developer
channel; never give it a note input for a debug facility. Its editor's private surface is covered
by in-process tests alone ([NOTES.md](NOTES.md#no-note-port-so-no-developer-channel)).

## The editor: one card, and the window is the app bar's width

Built to the brief, which owns the decisions
([NOTES.md](NOTES.md#the-editor-one-card-and-the-window-is-the-app-bars-width)):

- **One Effects paging item, key 0, and no bar.** The opening size is derived (`REFERENCE`,
  `the_opening_size_is_the_budget_hugged`); the minimum is the card's floor plus the shell gutters
  (`MINIMUM`, `the_minimum_window_holds_the_card_at_its_floor`).
- **The card is a `mxm_ui::tree`**, described once in `sections::card` and drawn by
  `sections::paint`. **The floor is computed**, with no usability minimum, and the card is exactly
  that wide. The Sweep states its own size (`SWEEP_MIN_WIDTH`, `MIN_SWEEP_BOX`).
  `every_card_passes_the_tree_checks_in_every_state` runs the shared checks.
- **The circuit's buttons set Rate; they are not a parameter** (`sections::positions`). A button is
  lit only while Rate sits on its rate; a press writes Rate as one gesture, tempo sync off first. No
  Off button. `a_button_puts_rate_on_its_rate_and_off_the_tempo`, `a_button_is_lit_only_on_its_rate`.
- **All four knobs one size**: two diameters in one row put the value labels on two lines.
- **The window's width is the app bar's, not the panel's**; the Sweep sits **beside** the knobs.
- **The Sweep is fed by `Chorus::lfo()` through `telemetry.rs`**, once per block, and **nothing is
  published while parked**. Off draws no dot at all.
- **`the_panel_fits_the_editor` pins the height from above and below**, measuring the card's own
  bottom, never `globally_used_rect`.

## Smoothed and unsmoothed

Mix and noise multiply into the audio and are smoothed here, per sample. Rate and depth are handed
to the core as targets once per block and glide inside it (`CONTROL_SLEW_S`); a second smoother
here would be one too many. The rate passes through the detent first.

# Work Guidance

# Verification

```bash
cargo test -p mxm-chorus-06
cargo clippy -p mxm-chorus-06 --all-targets
# The panel, light and dark, for review -> target/layout-tree/mxm-chorus-06/<MXM_PICTURES tag>/
MXM_PICTURES=after cargo test -p mxm-chorus-06 --lib tree_pictures -- --ignored
cargo xtask bundle mxm-chorus-06                  # debug: assert_process_allocs only fires here
clap-validator validate "target/bundled/mxm-chorus-06.clap"
cargo xtask bundle mxm-chorus-06 --release
clap-validator validate "target/bundled/mxm-chorus-06.clap"
cargo test -p mxm-poly-06-dsp                     # the module both products share
cargo test -p mxm-poly-06-host-tests --test golden_audio   # the synth unchanged, chorus on and off
```

`mxm-poly-06`’s four golden scores — dry, I, II and Both — protect the shared module. Bitwig mono
and stereo operation and listening fidelity remain manual gates; automated proof establishes
circuit equality with the instrument, not hardware fidelity.

# Child DOX Index

No child AGENTS.md files.
