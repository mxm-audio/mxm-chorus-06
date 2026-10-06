//! The parameter table and the one card, with the sentence each control's tooltip carries.
//!
//! [`all_parameters`] is also what `preset.rs` walks — capture, Init, resolve and the dirty
//! baseline all iterate it — so a parameter missing from [`ALL_IDS`] would silently fall out of
//! every preset, which is what `every_parameter_is_bound_exactly_once` guards.

use std::collections::HashMap;

use egui::{Rect, Ui};
use mxm_poly_06_dsp::chorus::{DELAY_CENTRE_MS, DEPTH_MAX_MS};
use mxm_ui::control::Size;
use mxm_ui::space::SPACE_5;
use mxm_ui::theme::Tokens;
use mxm_ui::tree::{self, Height, Kind, Node};
use nice_plug::prelude::ParamSetter;

use super::binding::Bound;
use crate::params::MxmChorus06Params;
use crate::telemetry::Telemetry;

/// **One size for all four** — brief §2, revised from the panel.
///
/// `mxm-ui`'s knob is a fixed grid per column: a name box, then a band the diameter tall, then the
/// value. Aligning the tops is what it guarantees, so two diameters in one row put the *values* on
/// two different lines — which reads as a mistake rather than as a hierarchy. With four controls
/// there is little hierarchy to express anyway, so it is carried by the order (the signal chain)
/// and by the display, and the row is left flush. Seen in the first build, 2026-09-04.
const SIZES: &[(&str, Size)] = &[
    ("rate", Size::Primary),
    ("depth", Size::Primary),
    ("mix", Size::Primary),
    ("noise", Size::Primary),
];

/// The narrowest the Sweep is drawn. With the knob row beside it, it sets the card's floor, and
/// brief §10's 640-point minimum window has to hold that floor with its gutters
/// (`editor::tests::the_minimum_window_holds_the_card_at_its_floor`), which leaves the display at
/// most 238 points there; 232 keeps six of them in hand. Its height is the controls', at least
/// [`MIN_SWEEP_BOX`].
pub const SWEEP_MIN_WIDTH: f32 = 232.0;
/// A dot's radius on the delay axis.
const DOT_RADIUS: f32 = 5.0;
/// The size of the display's own small text: the lane names and the caption.
const CAPTION_FONT: f32 = 10.0;
/// A floor under the box, so a future knob size cannot squeeze the lanes into each other.
pub const MIN_SWEEP_BOX: f32 = 88.0;
/// How far the caption sits in from the box's bottom-left and bottom-right corners.
const CAPTION_INSET: f32 = 9.0;
/// The gap between the two lanes' axes.
const LANE_GAP: f32 = 34.0;
/// How far the shaded band reaches above the L axis and below the R axis.
const BAND_REACH: f32 = 15.0;
/// The card's title.
pub const TITLE: &str = "Chorus";

/// What a leaf of the card draws. Hashed by what it names, which keeps its widget ids stable.
#[derive(Clone, Copy, Debug, Hash)]
pub enum Leaf {
    Knob(&'static str, Size),
    /// A control's tempo sync, the quarter note beside it.
    Picture(&'static str),
    /// The circuit's buttons, I, II and both: they set Rate ([`positions`]).
    Positions,
    Sweep,
}

/// The buttons' painted label. The card is titled *Chorus*, so the row needs only the word the
/// hardware's switches are known by (design system §7.1).
pub const POSITIONS_LABEL: &str = "Mode";

/// The buttons' place in the keyboard cursor's order. Not a parameter's id: the buttons write Rate.
pub const POSITIONS_ID: &str = "positions";

/// What each button does, one sentence apiece — the design system's per-option hover rule.
pub const POSITION_DETAILS: [&str; 3] = [
    "The slow rate: a gentle, wide chorus.",
    "A quicker rate: a livelier swirl.",
    "Both buttons at once: the fastest of the three.",
];

/// The parameters drawn as a picture rather than a knob: Rate's tempo sync.
#[cfg(test)]
pub const PICTURES: &[&str] = &["ratesync"];

/// The one card's body, as a tree (plans/plan-layout-tree.md): described once, and that one
/// description is both measured — the card's floor and height — and drawn, leaf by leaf, through
/// the bindings ([`paint`]).
///
/// **Side by side rather than stacked.** The window's width is set by the collection's app bar
/// (`editor::REFERENCE`), which is wider than four knobs need; stacking left the display with a
/// long empty axis and the card with air on its right. Controls left, what they do right, is also
/// the order the eye wants.
///
/// **The buttons go under the knobs, and the Sweep stands beside both** (the owner, 2026-09-28:
/// the circuit's positions are buttons, as on the original). Knobs and buttons are one column, so
/// the Sweep's box spans the controls from the title rule to the card's foot.
///
/// **Top-aligned, because a knob's column is a grid built from the top down** — a name box, the
/// circle, the value — and a row that centres its children would move each half by half its own
/// height. The knobs are the collection's knob row (`tree::knob_row`: equal columns at the one knob
/// column, R1d); `SPACE_5` beyond the row's own spacing, the Sweep takes the rest of the row
/// and the controls' height, so its box spans the knobs and the buttons and its caption lands on
/// the buttons' line — a different knob size moves both halves together.
pub fn card(ui: &Ui, params: &MxmChorus06Params) -> Node<Leaf> {
    let gap = ui.spacing().item_spacing.x;
    let knobs = |sizes: &[(&'static str, Size)]| {
        tree::knob_row(
            ui,
            sizes
                .iter()
                .map(|&(id, size)| {
                    let param = binding_for(id, params).param;
                    // A syncable control's column holds its free readings and its divisions.
                    let widest = if id == "rate" {
                        super::binding::synced_widest(param, crate::params::LFO_SYNC.span)
                    } else {
                        mxm_ui::control::widest_value(|n| param.format(n as f32))
                    };
                    let knob = tree::leaf(
                        Leaf::Knob(id, size),
                        Kind::Knob {
                            name: param.name().to_owned(),
                            widest,
                            size,
                            // In the collection's knob row, which sizes the columns.
                            column: 0.0,
                        },
                    );
                    (size, knob)
                })
                .collect(),
        )
    };
    // **One flat row**: Rate, its tempo sync's quarter note, the other three, then the Sweep a
    // `SPACE_5` further on. A row inside a row keeps the width it is offered, which would push the
    // Sweep away from the knobs; the spacer carries the extra gap instead of a pad, because the
    // Sweep stretches to the row only as a direct child.
    let (rate, rest) = SIZES.split_at(1);
    let controls = tree::stack(vec![
        tree::row_gap(
            gap,
            vec![
                knobs(rate),
                tree::switch_beside_knob(
                    rate[0].1,
                    tree::leaf(Leaf::Picture("ratesync"), Kind::SyncToggle),
                ),
                knobs(rest),
            ],
        ),
        tree::leaf(
            Leaf::Positions,
            Kind::Segmented {
                label: POSITIONS_LABEL.to_owned(),
                options: crate::params::POSITIONS
                    .iter()
                    .map(|(name, _)| (*name).to_owned())
                    .collect(),
                beside: None,
            },
        ),
    ]);
    tree::row_gap(
        gap,
        vec![
            controls,
            Node::Space(egui::vec2((SPACE_5 - gap).max(0.0), 0.0)),
            tree::leaf(
                Leaf::Sweep,
                Kind::Custom {
                    min_width: SWEEP_MIN_WIDTH,
                    height: Height::Row(MIN_SWEEP_BOX),
                    fills: true,
                },
            ),
        ],
    )
}

/// The authored card, its floor computed from its tree in `ui`'s fonts. Also what the keyboard
/// cursor is given: one card means the `Shift` tier has nowhere to go, and the parameter and value
/// tiers work exactly as they do on a paged editor.
pub fn page_items(ui: &Ui, params: &MxmChorus06Params) -> Vec<mxm_ui::paging::Item<'static>> {
    use mxm_ui::{
        flow::Card,
        paging::{Category, Item, Key},
    };
    let floor = tree::card_floor(ui, TITLE, &card(ui, params));
    vec![Item {
        key: Key(0),
        // Exactly as wide as its content: the ceiling is the floor (`plans/plan-editor-standard.md`
        // A1).
        card: Card::new(TITLE, floor).capped(floor),
        category: Category::Effects,
        kind: TITLE,
    }]
}

/// Everything a leaf draws with.
pub struct Live<'a, 'b> {
    pub params: &'a MxmChorus06Params,
    pub telemetry: &'a Telemetry,
    pub setter: &'a ParamSetter<'b>,
    pub text_entry: &'a mut HashMap<&'static str, Option<String>>,
}

/// Draws one leaf, in the `Ui` the tree bounded to `rect`, through the bindings — so the controls,
/// their gestures and their names are exactly what they were.
pub fn paint(ui: &mut Ui, tokens: &Tokens, leaf: &Leaf, rect: Rect, live: &mut Live<'_, '_>) {
    match *leaf {
        // Synced to a tempo, Rate reads its division; the host still reads its hertz.
        Leaf::Knob(id, size) => {
            let params = live.params;
            let bound = binding_for(id, params);
            let division = {
                use nice_plug::prelude::Param as _;
                let synced: Option<(bool, &nice_plug::prelude::FloatParam, mxm_tempo::Ladder)> =
                    match id {
                        "rate" => Some((
                            params.rate_sync.value(),
                            &params.rate,
                            crate::params::LFO_SYNC,
                        )),
                        _ => None,
                    };
                synced
                    .filter(|(on, _, _)| *on)
                    .and_then(|(_, param, ladder)| {
                        ladder.shown(
                            param.unmodulated_normalized_value(),
                            live.telemetry.tempo.get(),
                            f64::from(param.preview_plain(0.0)),
                            f64::from(param.preview_plain(1.0)),
                        )
                    })
            };
            match division {
                Some(division) => bound.knob_with_reading(
                    ui,
                    tokens,
                    live.setter,
                    size,
                    rect.width(),
                    live.text_entry,
                    division.label(),
                ),
                None => bound.knob(ui, tokens, live.setter, size, rect.width(), live.text_entry),
            }
        }
        Leaf::Picture(id) => super::binding::sync_picture(
            ui,
            tokens,
            id,
            binding_for(id, live.params).param,
            live.setter,
        ),
        Leaf::Positions => positions(ui, tokens, live.params, live.setter),
        Leaf::Sweep => sweep(ui, tokens, live.params, live.telemetry, rect.height()),
    }
}

/// **The circuit's buttons**, which set Rate rather than hold a value of their own.
///
/// A button is lit while Rate sits on its rate ([`crate::params::position`]), so the row and the
/// knob cannot disagree: turn the knob away and nothing is lit, press a button and the knob moves
/// there. Pressing one while Rate follows the tempo switches the sync off, because the button means
/// the circuit's rate and not a division. Double-click, or `Command`+`Backspace`, is I — Rate's
/// default.
fn positions(ui: &mut Ui, tokens: &Tokens, params: &MxmChorus06Params, setter: &ParamSetter<'_>) {
    use crate::params::{POSITIONS, position};
    use nice_plug::prelude::Param as _;
    let options = POSITIONS.map(|(name, _)| name);
    let lit = position(
        params.rate.unmodulated_plain_value(),
        params.rate_sync.unmodulated_plain_value(),
    );
    // Nothing lit is one past the last cell.
    let mut selected = lit.unwrap_or(options.len());
    let changed = mxm_ui::navigation::at(ui, POSITIONS_ID, |ui| {
        mxm_ui::control::segmented(
            ui,
            tokens,
            POSITIONS_LABEL,
            &options,
            &mut selected,
            None,
            Some(0),
            &POSITION_DETAILS,
        )
    });
    if changed && let Some(&(_, hz)) = POSITIONS.get(selected) {
        choose(setter, params, hz);
    }
}

/// Puts Rate on `hz`, free of the tempo: each write one complete gesture, as a button press is.
pub fn choose(setter: &ParamSetter<'_>, params: &MxmChorus06Params, hz: f32) {
    use nice_plug::prelude::Param as _;
    if params.rate_sync.unmodulated_plain_value() {
        setter.begin_set_parameter(&params.rate_sync);
        setter.set_parameter(&params.rate_sync, false);
        setter.end_set_parameter(&params.rate_sync);
    }
    setter.begin_set_parameter(&params.rate);
    setter.set_parameter(&params.rate, hz);
    setter.end_set_parameter(&params.rate);
}

/// The whole view: the one card, through the shared paging renderer. Returns the bottom of the
/// card, which is what the fit test measures.
pub fn chorus_card(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmChorus06Params,
    telemetry: &Telemetry,
    setter: &ParamSetter<'_>,
    text_entry: &mut HashMap<&'static str, Option<String>>,
) -> f32 {
    let items = page_items(ui, params);
    let text_editing = text_entry.values().any(Option::is_some);
    let mut live = Live {
        params,
        telemetry,
        setter,
        text_entry,
    };
    let report = mxm_ui::paging::editor::show(
        ui,
        tokens,
        &items,
        &[],
        text_editing,
        &mut |ui, _| card(ui, params),
        &mut |ui, _, leaf, rect| paint(ui, tokens, leaf, rect, &mut live),
    );
    report
        .visible
        .iter()
        .map(|(_, r)| r.bottom())
        .fold(ui.cursor().top(), f32::max)
}

/// The Sweep — brief §8.
///
/// Two lanes on one delay axis, each carrying the dot for one bucket brigade, **in antiphase**:
/// one triangle drives both, so when the left is long the right is short. That is the fact no pair
/// of numbers can state, and it is why this display exists rather than a rate readout.
///
/// The shaded band is the swing in milliseconds, so Depth reads as a distance rather than a
/// percentage; the tick is the circuit's centre delay, which never moves.
///
/// **The position comes from the audio thread** ([`Telemetry::lfo`]), not from the editor's clock:
/// a display with its own oscillator drifts from the sound and keeps sweeping when the plugin is
/// parked, which is the lie a display exists to prevent.
///
/// # Where it sits, which took three tries
///
/// **The box is the controls' whole height, and the moving part is centred in the box** — the
/// owner's rule, 2026-09-04: *the animation should be aligned between the line under the card's
/// title and the bottom of the card, the way the knobs are.* The knobs and buttons fill that band, and the
/// card's tree gives the display its row's height ([`card`]), so `height` **is** the band and the
/// box's middle is the band's middle. Not the same height
/// as the knobs' circles, which sit a little lower because their value line hangs below them —
/// what is being centred is the display in the band, which is what the eye reads.
///
/// The two earlier attempts both centred against something invisible. Measuring from the whole
/// box while the caption sat inside its bottom strip pushed the lanes high by half that strip;
/// taking the strip off the measurement centred them against a line nobody can see. So the caption
/// is **inside** the box, painted in the bottom corner where it costs no layout, and the lanes are
/// measured from `rect.center()` with nothing subtracted.
fn sweep(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmChorus06Params,
    telemetry: &Telemetry,
    height: f32,
) {
    let engaged = params.mix.value() > 0.0;
    let depth = params.depth.value();
    let lfo = telemetry.lfo().clamp(-1.0, 1.0);
    let size = egui::vec2(ui.available_width(), height.max(MIN_SWEEP_BOX));

    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();

    // The axis spans the widest swing the control allows, so the band's width **is** Depth: a
    // display rescaled to the current depth would look identical at every setting.
    let span = DEPTH_MAX_MS;
    let x_for = |ms: f32| {
        let t = (ms - (DELAY_CENTRE_MS - span)) / (2.0 * span);
        rect.left() + 12.0 + t * (rect.width() - 24.0)
    };

    painter.rect_filled(rect, 4.0, tokens.surface_2);

    // Two lanes, so the dots stay separate where they cross — which is the moment worth seeing.
    //
    // **Measured from the box's own centre**, so the band around them is symmetric by
    // construction. The caption is outside the box now: it used to own the bottom strip, which put
    // more air above the moving part than below and read as the display having slipped upwards.
    let middle = rect.center().y;
    let lane_l = middle - LANE_GAP / 2.0;
    let lane_r = middle + LANE_GAP / 2.0;
    let caption_y = rect.bottom() - CAPTION_INSET;

    let band = egui::Rect::from_min_max(
        egui::pos2(x_for(DELAY_CENTRE_MS - depth), lane_l - BAND_REACH),
        egui::pos2(x_for(DELAY_CENTRE_MS + depth), lane_r + BAND_REACH),
    );
    painter.rect_filled(
        band,
        3.0,
        if engaged {
            tokens.accent.gamma_multiply(0.18)
        } else {
            tokens.track
        },
    );

    // The circuit's centre, which no control moves.
    painter.line_segment(
        [
            egui::pos2(x_for(DELAY_CENTRE_MS), lane_l - BAND_REACH - 2.0),
            egui::pos2(x_for(DELAY_CENTRE_MS), lane_r + BAND_REACH + 2.0),
        ],
        egui::Stroke::new(1.0, tokens.border_strong),
    );

    let label_colour = if engaged {
        tokens.text_secondary
    } else {
        tokens.text_disabled
    };
    for (lane, name, sign) in [(lane_l, "L", 1.0_f32), (lane_r, "R", -1.0)] {
        painter.line_segment(
            [
                egui::pos2(x_for(DELAY_CENTRE_MS - span), lane),
                egui::pos2(x_for(DELAY_CENTRE_MS + span), lane),
            ],
            egui::Stroke::new(1.0, tokens.border),
        );
        painter.text(
            egui::pos2(rect.left() + 4.0, lane),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(CAPTION_FONT),
            label_colour,
        );
        // **Off draws no dot at all.** Nothing is being added, and a dot parked at the centre
        // would say the modulator was running and doing nothing, which is a different claim.
        if engaged {
            painter.circle_filled(
                egui::pos2(x_for(DELAY_CENTRE_MS + sign * depth * lfo), lane),
                DOT_RADIUS,
                tokens.accent,
            );
        }
    }

    // Painted in the corner rather than laid out under the box: it states what the shapes cannot
    // — what the axis is, and, when it applies, that the effect is off — and text that took layout
    // would move the moving part off centre, which is the whole subject of this function's doc.
    // Text, not hue: brief §11's second channel.
    painter.text(
        egui::pos2(rect.left() + CAPTION_INSET, caption_y),
        egui::Align2::LEFT_CENTER,
        format!("Delay {DELAY_CENTRE_MS:.1} ms ± {depth:.2}"),
        egui::FontId::proportional(CAPTION_FONT),
        tokens.text_secondary,
    );
    if !engaged {
        painter.text(
            egui::pos2(rect.right() - CAPTION_INSET, caption_y),
            egui::Align2::RIGHT_CENTER,
            "Off",
            egui::FontId::proportional(CAPTION_FONT),
            tokens.text_primary,
        );
    }

    let hover = if engaged {
        "The chorus sweep: left and right move in opposite directions, across the band Depth sets."
    } else {
        "Mix is at zero, so the chorus is off. Turn Mix up to hear it."
    };
    response.on_hover_text(hover);
}

/// One parameter's binding, with the sentence §7.1 requires in its tooltip.
///
/// **The descriptions live here because only the plugin has them.** CLAP carries no such field, so
/// a host cannot supply one — which is the concrete reason an editor belongs to the plugin.
pub fn binding_for<'a>(id: &'static str, p: &'a MxmChorus06Params) -> Bound<'a> {
    let (param, description): (&'a dyn super::binding::ErasedParam, &'static str) = match id {
        "rate" => (
            &p.rate,
            "How fast the chorus sweeps; the buttons below set the three classic speeds.",
        ),
        "depth" => (
            &p.depth,
            "How deep the sweep is; halfway is the classic setting.",
        ),
        "mix" => (
            &p.mix,
            "How much chorus is added to the dry sound; at zero it is off.",
        ),
        "noise" => (&p.noise, "Adds the chorus's gentle hiss."),
        "ratesync" => (&p.rate_sync, super::binding::SYNC_DESCRIPTION),
        other => unreachable!("no binding for parameter `{other}`"),
    };
    Bound::new(id, param, description)
}

/// Every parameter, bound, in the plugin's own order.
pub fn all_parameters(params: &MxmChorus06Params) -> Vec<Bound<'_>> {
    ALL_IDS.iter().map(|id| binding_for(id, params)).collect()
}

/// Every id this plugin has. One list, so the coverage test and the lookup cannot disagree.
pub const ALL_IDS: &[&str] = &["rate", "ratesync", "depth", "mix", "noise"];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every parameter the derive declares is bound exactly once, and nothing is bound that is
    /// not declared.
    #[test]
    fn every_parameter_is_bound_exactly_once() {
        use nice_plug::prelude::Params;
        let params = MxmChorus06Params::default();
        let declared: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        for id in &declared {
            let count = ALL_IDS.iter().filter(|d| *d == id).count();
            assert_eq!(count, 1, "{id} is bound {count} times, expected once");
        }
        assert_eq!(
            ALL_IDS.len(),
            declared.len(),
            "ALL_IDS {ALL_IDS:?} against declared {declared:?}"
        );
    }

    /// Every binding resolves and carries a sentence, which `binding_for`'s `unreachable!` would
    /// otherwise turn into a panic inside a paint call — and a panic there takes the host down.
    #[test]
    fn every_bound_parameter_has_a_sentence() {
        let params = MxmChorus06Params::default();
        for id in ALL_IDS {
            let bound = binding_for(id, &params);
            assert!(!bound.description.is_empty(), "{id} has no description");
            assert!(
                bound.description.ends_with('.'),
                "{id}'s description is not a sentence: {:?}",
                bound.description
            );
        }
    }

    /// Every parameter is on the panel, at a declared size. A control that exists and is not drawn
    /// is reachable only through a host's generic list, which is the thing an editor is for.
    #[test]
    fn every_parameter_is_drawn_at_a_declared_size() {
        for id in ALL_IDS.iter().filter(|id| !PICTURES.contains(id)) {
            assert!(
                SIZES.iter().any(|(drawn, _)| drawn == id),
                "`{id}` is a parameter that the panel never draws"
            );
        }
        assert_eq!(SIZES.len() + PICTURES.len(), ALL_IDS.len());
    }
}
