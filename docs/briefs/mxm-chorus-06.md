# mxm-chorus-06 — UI design brief

Required by `MXM_DESIGN_SYSTEM.md` §14, written before implementation. Answers the ten questions in
order, then records the deliberate deviations.

**Plugin:** the JUNO-106's bucket-brigade chorus as a standalone effect, with the four quantities
the circuit fixes opened as controls. Audio in, audio out; no notes.

**This is the collection's first effect, so it is also where the effect editor's shape is decided.**
Everything below that is not about a chorus in particular — the app bar, one view, the Off state,
the size — is meant to be the pattern the spring reverb and the rest inherit.

---

## 1. Primary sound-design task

**Deciding how wide a sound is, and hearing what widening costs.** Unlike an instrument, nobody
opens this to build a sound from nothing: a signal is already playing and the question is how much
motion to put on it before it stops sounding like the source. So the task is a comparison, made
with one hand on Mix, and the editor's job is to make the comparison quick and the state obvious.

The second task, which the hardware could not do at all, is **leaving the circuit**. The three
switch positions are three rates; opening rate and depth turns a three-position switch into a
range, and the editor has to make the circuit's own positions findable inside that range — or the
flexibility costs the machine its identity.

## 2. The three to five parameters users reach for most

There are only four, which is itself the design:

1. **Mix** — the whole comparison, and the on/off. Performed.
2. **Rate** — the character: slow shimmer against fast vibrato. Performed.
3. **Depth** — set once per sound, mostly.
4. **Noise** — set once, and usually left at the circuit's value or at zero.

Rate carries the collection's tempo sync beside it (`ratesync`, 2026-09-25): a quarter note, not a
fifth control to rank. Under the knobs are **the circuit's own buttons, I, II and I + II**
(2026-09-28): they put Rate on the circuit's three rates and are lit while it sits there — a way to
reach Rate, not a fifth control (§9).

**All four take Primary sizing**, and the ranking above is carried by the order they sit in and by
the display rather than by their size. `mxm-ui`'s knob is a fixed grid per column — a name box, a
band the diameter tall, then the value — so it aligns the tops of a row and *not* the values: two
diameters in one row put the value labels on two lines, which reads as a mistake rather than as a
hierarchy. Revised after looking at the first build (2026-09-04); the original said Mix and Rate
Primary, Depth and Noise Standard.

The ranking is also why there is no advanced zone: with four controls, hiding one would leave
three.

## 3. Signal flow that must be visible

```
in ─┬─────────────── dry ──────────────┬─► L
    │                                  │
    └─► BBD 1 ─► delay ────► wet ──────┤
        BBD 2 ─► delay ────► wet ──────┴─► R
             ▲
        one triangle, the two in antiphase
```

Three facts, and each is something people get wrong about this circuit:

- **One triangle drives both lines, in antiphase.** The width is not two independent modulators; it
  is one modulator subtracted from itself, which is why the effect collapses to nothing in mono.
- **The dry is fixed and only the wet moves.** Mix is a wet level, not a crossfade, so the source
  never gets quieter — turning it up adds rather than trades.
- **Zero mix is off**, and off means the signal is untouched and nothing is computed. There is no
  On switch to disagree with the knob.

The Sweep display (§8) carries the first two; the Off badge carries the third.

## 4. Which controls belong in Play view

**Not applicable — no `Play` view.** See §6.

## 5. Advanced controls and their disclosure

**None, and that is a decision rather than an omission.** Four controls do not divide into primary
and advanced; a disclosure here would hide a quarter of the plugin behind a click to save one row
of space. `mxm-mono-03`'s reasoning at the other end of the same scale: hiding a third of a small
instrument costs more than it saves.

**Consequence for the developer channel**: CC 118 opens an expander this editor does not have, so
it does nothing, exactly as `plugins/AGENTS.md` provides for. CC 117 and CC 119 work.

## 6. Views

**One indivisible Effects card**, the singleton case of design-system §3.2's dynamic paging.
No bar and no duplicate Parameters list. An oversized card retains scrolling rather than losing
controls. There is no developer CC channel because an effect has no note port.

## 7. Identity accent

**Teal**, the collection accent, unchanged in both themes.

**Deliberately not its own hue.** §5.3 gives each *instrument* an identity accent so a rack of them
is tellable apart at a glance. An effect is not a rack member in that sense: it sits in a chain
beside other effects, and what has to be tellable apart there is *which effect*, which the name in
the app bar and the strip in the player's rail both say. Spending a hue here would also start a
second scheme — one for instruments, one for effects — with nothing to keep them from colliding.

The accent is used for the arc, the marker and the selected fill; **no text is drawn in it**, so the
contrast requirement that applies is the non-text one, which the shipped tokens already meet — they
are the same tokens every other editor draws its controls with, measured in `crates/ui`.

## 8. Live visualizations

**One: the Sweep.** A horizontal delay axis with the centre marked, the depth's swing shaded, and
two dots — L and R — riding the triangle in antiphase.

It earns its space because it is the only way to see three things the numbers cannot say:

- **Antiphase.** Two dots crossing at the centre is the fact §3 is about, and no pair of numbers
  shows it.
- **What depth means in time.** The shaded band is the swing in milliseconds, so Depth stops being
  a percentage and becomes a distance.
- **That the modulator never stops.** The circuit's LFO free-runs; the plugin's does too, including
  while the effect is parked. A dot that keeps moving with no signal is the honest picture of that,
  and it is also how you see the rate you have dialled without waiting for a note.

Fed by `Telemetry::lfo`, published once per block from the DSP's own `Chorus::lfo()` — **the value
the audio used**, not a copy of the triangle recomputed in the editor. An animation driven by the
editor's own clock would drift from the sound and would keep sweeping when the plugin was not,
which is exactly the lie a display exists to prevent.

A **level meter** in the app bar, as every other editor has: a wet level added on top of a fixed dry
can clip, and this is the only place that says so.

## 9. What is removed from the source hardware layout, and why

The source layout is **two buttons**, I and II, side by side, with both-down as a third state.

- **The buttons stay, as a way to reach Rate** (the owner, 2026-09-28: *the original has buttons
  for that* — reversing this section's first answer, which moved the positions into the factory
  presets `One`, `Two` and `Onetwo`). The worry was a second control for one quantity, disagreeing
  with the knob the moment anyone touched it. They cannot: a button holds nothing, it puts Rate on
  its rate, and it is lit only while Rate sits there — turn the knob away and nothing is lit.
- **The hardware's Off is not among them.** Mix at zero is Off (§11), so an Off button would be a
  second control for that.
- **The rate detents make them exact.** Each circuit rate is a detent on the Rate knob
  (`params::circuit_rate`), so a button lands on the circuit's number exactly, and the positions can
  be dialled as well as pressed.
- **Nothing is added that the circuit did not have.** No feedback, no stereo width, no tone
  control: those make a different effect, and `plugins/AGENTS.md`'s rule for the collection is that
  an effect ships what its original had.

## 10. Minimum size and 200% scale

**Resizable: the editor's `REFERENCE` and `MINIMUM`, derived and held by its tests**, the minimum
including the control/display floor and shell gutters. One Effects item uses the shared paging
renderer without a bar. Existing painted label, display and panel-fit tests remain. Zoom is
independently chosen at **75–200%**; an indivisible overflow scrolls. Keep physical window size
fixed for §15's DPI/zoom gate rather than doubling the window to claim fit. Native-window, real-DAW
and owner inspection remain open.

## 11. The Off state

Mix at zero is Off, so the editor has to show a state the parameters alone imply:

- The Sweep display **greys and its dots stop being drawn**, because nothing is being added.
- A badge reading **Off** sits beside the display's caption, with the sentence *Mix is at zero, so
  the chorus is bypassed and costs no CPU* on hover.
- The Mix knob keeps its normal appearance. It is not disabled — it is the way out.

Two channels, never hue alone: the badge is text, the display's change is a fill.

## Deliberate deviations from the design system

| § | Rule | Deviation | Why |
|---|---|---|---|
| §5.3 | Each instrument has an identity accent | The collection accent is kept | §7 above: an effect is told apart by name in a chain, and a second accent scheme would collide with the instruments' |
| §6 | Views bar | No view bar | §6 above: one view, and a bar with one cell is an affordance for nothing |
| §14.5 | Advanced controls and disclosure | No advanced zone | §5 above: four controls do not divide |

## Sign-off checklist

- [x] §14's ten questions answered before implementation.
- [x] The one visualization is fed by the audio thread's own value, not recomputed.
- [x] The Off state is carried by two channels, not hue.
- [x] The size is pinned by a test rather than chosen.
- [x] The removals from the hardware layout are argued, not silent.
