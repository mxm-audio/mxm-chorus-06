# NOTES.md — plugins/mxm-chorus-06

The detail behind this folder's AGENTS.md: history, measurements, rationale and worked examples.
AGENTS.md is the contract; this file is the reference it links to.

## Rate's tempo sync

**`ratesync` is the collection's one tempo sync** (2026-09-25, `plans/plan-tempo-sync-controls.md`):
the quarter note beside Rate, on `params::LFO_SYNC` (1/32 to four bars, the top the fastest).
`process` resolves it once a block from the modulated position (`MxmChorus06Params::synced_rate`);
synced, the rate is the division and **the circuit's detents do not apply** — they are positions of
the free knob, and a synced rate is not one. `Telemetry::tempo` lets the knob read its division.

## The product is the circuit unlocked, and the circuit is inside it

The owner's ruling (2026-09-03): continuous parameters for flexibility. So the four controls are
the quantities the circuit fixes — the modulator's **rate**, its **depth**, the wet level (**mix**)
and the brigades' **noise** floor — each defaulting to the circuit's value. **The circuit's three
switch positions are the panel's buttons** (the owner, 2026-09-28: *the original has buttons for
that*; until then they were the factory presets `One`, `Two` and `Onetwo`): `params::POSITIONS`,
I, II and I + II, each the rate it selects, which is all the hardware's switches change. The `06`
holds because the copy is inside the product and provably reachable:
`at_each_button_the_standalone_is_the_built_in_to_the_bit` renders `mxm-poly-06` with its chorus in
I, II or Both against `mxm-poly-06` with its chorus Off feeding this plugin with Rate where the
matching button puts it, and they are equal sample for sample, at two block sizes.

**The conditions of that equality**, because each was needed: the synth's master **Volume at
unity** (it sits after the chorus and defaults to a half, where the noise floor is scaled on one
path only); a **held-note score**, nonzero from the first sample to the last, so the two activity
rules below agree throughout; state and seeds reset together; the mono layout. A score with note
boundaries is not compared under an equality claim — the noise floor's ramp differs there by
construction, up to a block at an onset and by the release at an end — and if one ever is, its
differing intervals are declared here first.

**Where the circuit is exact.** Depth's range is exactly twice the circuit's depth, so the circuit
is the linear control's midpoint and a half is exact; mix and noise are `0..=1` with the circuit at
the top. Rate is skewed and its normalised round trip is not exact, so the three circuit rates are
**detents**: `params::circuit_rate` snaps a value within `1e-4` of one onto it, which is how a
button lands. `the_circuit_rates_are_detents_and_nothing_else_is` holds the width.

## Mix at zero is Off, and doing nothing costs nothing

The owner's rulings (2026-09-04): no On switch, and an effect or a synth doing nothing uses no CPU.
The circuit's Off is a wet-mute with the dry untouched, which is exactly what the wet level at zero
is; `Mix` at exactly zero — the parameter's value, not its smoother — is Off. Once the mute has
closed (and, on a stereo track, the dry leg is back to pass-through) the core is **parked**: the
delay lines are emptied, `process` runs no part of it, and a block costs a copy. The host is told
(`ProcessStatus::Normal`, which nice-plug maps to *continue if not quiet*). When the mix returns
the modulator is moved across the skipped time (`Chorus::advance`), so it is where the circuit's
would be.

**One chosen deviation follows**, recorded here: the circuit's brigades keep clocking while muted,
so re-engaging gives wet from the first sample; here the lines are empty when Mix leaves zero and
the wet builds over the delay time — a few milliseconds — because a parked core that kept its
lines would hand back audio from before the park, which is worse. Outside the equivalence path.
`mix_at_zero_is_off_and_parks_the_core` and
`unparking_moves_the_modulator_across_the_skipped_time_and_brings_nothing_stale` hold both halves.

## Activity is the input, not a voice

Inside the synth the noise floor follows voice activity. Here: a block with any input sample that is
not exact digital zero is **active**; the first all-zero block sends the floor to zero. No RMS
threshold, gate time or sensitivity control — nothing chosen. Dither keeps the hiss alive, as the
hardware would; exact silence ends it, as the synth does. The same pass **flushes a subnormal input
to zero**: below anything a converter carries, slow to multiply on x86 (the validator measured
2.6×), and never passed through on the dry leg. So a subnormal counts as silence. **So does a
non-finite sample**, zeroed in the same pass: let through, the pre-filter, the lines and the
reconstruction filters held a NaN until the core parked
(`a_non_finite_input_sample_is_silence_and_the_render_recovers`). A parked block is a copy and
inspects nothing; there is no state there to poison.

The wrapper owns the **settle clock** the core does not have: after the input goes quiet it keeps
processing silence for the core's declared tail (the noise fade plus the lines' drain), reports
`Tail(n)` each block, and reports `Normal` once drained — exact zero follows.
`activity_is_exact_zero_and_the_tail_is_reported_then_ends_in_exact_silence` holds it. Status is
never derived from `Chorus::is_quiet()` alone, which reflects the noise target and stays false at
zero mix under a live input.

## Two layouts, and what stereo costs

- **Mono in, stereo out** — first, so the default: the circuit's topology. nice-plug zero-fills the
  second output; the plugin writes both.
- **Stereo in, stereo out** for hosts that insert on stereo tracks. At Mix zero, true left/right
  pass-through. Engaged, the two are summed to mono inside `process()` on the in-place buffer, the
  core makes the stereo, and **the dry leg crossfades** between the two channels and their sum
  over the core's own mute interval — wrapper-only, stereo-only, outside the equivalence path — so
  a mix sweep through zero is one fade and never a step
  (`a_mix_sweep_through_zero_on_a_stereo_track_does_not_click`). The image collapses only while
  the effect is on; the README says so and why.
- **No stereo-in, mono-out**: nice-plug does not support many-to-few main layouts. And a mono-in,
  stereo-out main pair is **not an in-place pair** — CLAP allows one only between ports of the same
  shape, and upstream nice-plug declared one regardless, which the validator refused to process.
  Fixed in the MXM nice-plug fork (fix 5 in its `PATCHES.md`); the validator on this bundle
  is its regression test.

## Activation refuses a rate the core cannot run at

`activate` returns `false` for a non-finite sample rate or one below `MIN_SAMPLE_RATE` (3 Hz),
before it touches anything. The pre-filter's corner is `f32::clamp`ed to `1 Hz ..= 0.45 × fs` in
`mxm-poly-06-dsp`; the bounds cross below about 2.22 Hz or at a NaN rate, which panicked inside
activation. The floor is derived, not musical, and no ceiling is set.

## Defaults and presets — the effects' rules, not the instruments'

The default is the circuit at position I, engaged — the effects exception to *every amount starts
at zero* (parent, *An effect starts engaged*). The preset system ships under the owner's rule (a
meaningful control beyond level): **ten chorus sounds** (the owner, 2026-09-28: *then 10 chorus
presets*), from `Clean doubler` to `Eighth pulse`, designed in the controls' own units in
`preset.rs`'s `FACTORY_DESIGN` — Rate in hertz, or for the two synced sounds the division its
position picks (`LFO_SYNC.position`) — and generated to JSON by the `#[ignore]`d
`write_the_factory_presets`. `the_factory_files_match_the_design_they_were_generated_from` catches
a stale file, `no_factory_preset_is_init_or_a_button` keeps the circuit's positions out of the set
(the buttons have them), and `a_synced_preset_picks_its_division` holds the synced two. The values
are a first draft by numbers, for the owner's ear. Until 2026-09-28 the set was the circuit's
positions, `One`, `Two` and `Onetwo`; a project saved with one loaded still names it, because the
loaded identity carries its own name and baseline. Init has no file. Each carries `Category::Fx`.

**On `crates/mxm-preset` since 2026-09-04** (mxm-kit's since the split), and its binding since 2026-09-24
(`editor/binding.rs` re-exports `mxm_preset::binding`). `preset.rs` and `editor/binding.rs` were the
sixth verbatim copies, made the same day the extraction began landing from another session; this plugin
moved to the crate as soon as it was committed, which is what the editor's preset row and browser
are. What is left here is the `Instrument` impl, `FACTORY_FILES`, and the ten sounds — the
identity type, the format, the library and the favourites are the crate's.

## No note port, so no developer channel

An effect declares `MidiConfig::None`. The parent's *developer channel in every editor* rides on
control changes, which never reach a plugin without a note port, so this plugin **cannot carry
it** — and the player's `cc` verb reaches the loaded instrument, never the chain after it, so there
would be no route even with one. Giving an effect a note input for a debug facility would be a lie
about what it is in every host that lists ports. Its editor's private surface is covered by
in-process tests alone, as the parent's last section says every editor's is. Recorded as a
deviation rather than left implicit; `telemetry.rs`'s module doc carries the same reasoning where
the request slots would have gone.

## The editor: one card, and the window is the app bar's width

Built to the brief, which owns the decisions; what is worth having here is what the panel found:

- **One Effects paging item, key 0, and no bar.** The shared paging renderer treats this as the
  singleton case; there is no extra Parameters surface or developer MIDI channel. The opening size
  is derived: the budget hugged around the card (`REFERENCE`, held by
  `the_opening_size_is_the_budget_hugged` and, for its height, `the_panel_fits_the_editor`), and the
  app bar compacts to that width (design system §3.1);
  the minimum is the card's control/display floor plus the shell gutters (`MINIMUM`, held by
  `the_minimum_window_holds_the_card_at_its_floor`). An indivisible overflow remains scrollable.
- **The card is a `mxm_ui::tree`** (mxm-kit's [`crates/ui/NOTES.md`](https://github.com/mxm-audio/mxm-kit/blob/main/crates/ui/NOTES.md#a-card-body-as-data--tree), *A card body as data*).
  `sections::card` describes the body once — one row: a column of the knobs (Rate as a knob row, its
  tempo sync's quarter note, the other three as the collection's knob row, `tree::knob_row`) over
  the circuit's buttons, a `Space` making up `SPACE_5` beyond the row's spacing, then the Sweep, as
  tall as the column (a pad would stop the Sweep stretching to the row; the card is exactly its
  floor wide, so the column's fillers have no spare width to push the Sweep with) — and that description is measured for the
  card's floor and height and drawn leaf by leaf through the bindings (`sections::paint`), through
  `paging::editor::show`. **The floor is computed**, the tree's narrowest plus the card's
  chrome, with no usability minimum declared beside it, and the card is exactly that wide (its
  ceiling is its floor, `plans/plan-editor-standard.md` A1).
  The Sweep states its own size: at least `SWEEP_MIN_WIDTH` wide, filling the rest of the row,
  and as tall as the knob row, at least `MIN_SWEEP_BOX`.
  `the_minimum_window_holds_the_card_at_its_floor` holds the
  floor inside brief §10's minimum window, and
  `every_card_passes_the_tree_checks_in_every_state` runs the shared checks
  (`mxm_plugin_test::tree_checks`) at Init, Off and the widest swing.
- **The circuit's buttons set Rate; they are not a parameter** (`sections::positions`, the owner,
  2026-09-28). A button is lit while Rate sits on its rate (`params::position`: through the detent,
  and never while Rate follows the tempo), so the row and the knob cannot disagree; a press writes
  Rate as one gesture, switching its tempo sync off first. No Off button: Mix at zero is Off. The
  row joins the keyboard cursor as `positions`, and with nothing lit an arrow enters it at the near
  end (design system §7.3). It made the window 306 tall (`REFERENCE`); the width stays the app
  bar's. `a_button_puts_rate_on_its_rate_and_off_the_tempo` (a pointer press through the whole
  panel) and `a_button_is_lit_only_on_its_rate` hold it.
- **All four knobs one size.** `mxm-ui`'s knob is a fixed grid per column — name box, a band the
  diameter tall, then the value — so it aligns the *tops* of a row and not the values. Two
  diameters in one row put the value labels on two lines, which reads as a mistake rather than as
  a hierarchy. The brief's §2 was revised from the built panel rather than the panel from §2.
- **The window's width is the app bar's, not the panel's.** The bar is the collection's and carries
  the wordmark, the preset row and the right-hand group; sizing the window to four knobs clipped
  Save and the zoom control clean off it. So the width is what the bar needs, and the card spends
  it by putting the Sweep display **beside** the knobs instead of under them.
- **The Sweep is fed by `Chorus::lfo()` through `telemetry.rs`**, once per block, and **nothing is
  published while parked**: the editor draws Off from the Mix parameter, so a plugin doing nothing
  is never asked to keep a display alive. Off draws no dot at all — a dot parked at the centre
  would claim the modulator was running.
- **`the_panel_fits_the_editor` pins the height from above and below**, measuring the card's own
  bottom rather than `globally_used_rect`, which a central panel makes useless: it fills the window
  whatever is in it. Both mono editors shipped a first build with controls clipped off a window
  that could not be resized; this is that scar as a number.

