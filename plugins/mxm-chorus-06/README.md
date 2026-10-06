# mxm-chorus-06

The JUNO-106's bucket-brigade chorus as a standalone effect, unlocked. The circuit is the one
`mxm-poly-06` carries inside it, and this plugin runs that same code — depended on, not copied —
with the quantities the circuit fixes opened as controls and the circuit's three switch positions
kept as buttons. The name is not Roland's and neither is the interface. Not affiliated with or
endorsed by Roland.

## What it is

| | |
|---|---|
| The chorus | Two bucket brigades clocked in antiphase from one triangle, a wet path rolled off above 10 kHz and a dry path that is not, the brigades' own noise floor injected where the chips are. Read off the JUNO-106 service notes; see `research:effects/juno-chorus.md` |
| **Rate** | The triangle's speed. The circuit's three positions — I, II and both together — are the buttons under the knobs, which put Rate on that rate and light while it is there, and detents on the knob |
| **Depth** | How far the sweep moves the delay. The circuit's depth is the knob's centre |
| **Mix** | The wet level against a fixed dry, with the circuit at the top. **At zero the chorus is off**: the signal passes untouched and the plugin does no work |
| **Noise** | The floor, from none to the circuit's. Present while anything is playing, gone to exact silence when nothing is |

Init is the circuit at position I, engaged: an inserted effect demonstrates the effect it is named
after.

## Two layouts, and what stereo costs

**Mono in, stereo out** is the circuit's own topology and the default: the chorus makes the stereo.
**Stereo in, stereo out** is offered for hosts that insert on stereo tracks. Off passes both channels
through untouched. Engaged, the two channels are **summed to mono before the chorus**, as the
machine's own voice sum was, so the track's stereo image collapses while the effect is on — the
faithful answer, and the one `juno-chorus.md` §6.1 gives. The change from one to the other is a
short crossfade, never a step.

## Doing nothing costs nothing

With Mix at zero, once the mute has closed, the chorus is not run at all: no delay lines, no
filters, no modulator. The host is told it may sleep the plugin, and if it keeps calling, a block
costs a copy. When the mix returns, the modulator is where the circuit's would have been.

## Presets

Ten chorus sounds, from a clean doubler to a tempo-synced pulse. The circuit's own three positions
are the buttons, not presets: at each of them the plugin is proved equal to the built-in chorus,
sample for sample. **Init is not a file**: it is generated from the
parameter defaults, so it cannot be deleted and cannot drift from them. Your own presets are saved
as readable JSON under the platform config directory.

## Status

**Built and tested**: the four parameters, the circuit's buttons, ten presets, state, both layouts, and the
editor — one card with the four knobs and a display of the two delay lines riding the modulator in
antiphase, which is the one thing about this circuit that numbers cannot show. `clap-validator`
reports 33 passed and nothing failed. Not yet run in a commercial host.

**Fidelity is UNVERIFIED.** No hardware was measured, here or in the instrument this chorus comes
from; the delay range, depth, noise level and the I + II rate are chosen, and `crates/mxm-poly-06-dsp`'s
`AGENTS.md` lists each. What *is* proved is that this plugin at each of the three buttons is that
instrument's chorus to the bit.

## Building

```bash
cargo xtask bundle mxm-chorus-06 --release
clap-validator validate "target/bundled/mxm-chorus-06.clap"
```

MIT licensed — see [LICENSE](LICENSE). All code is original.
