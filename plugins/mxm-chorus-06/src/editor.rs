//! mxm-chorus-06's editor.
//!
//! Built to `docs/briefs/mxm-chorus-06.md`, which is the gating document — this module implements
//! it and does not re-decide it. In particular the brief owns:
//!
//! - **§6's single view and no view bar.** Four parameters do not divide into a designed panel and
//!   a generic list; a bar offering one destination is an affordance for nothing.
//! - **§2's ranking**: Mix and Rate Primary, Depth and Noise Standard.
//! - **§8's one display**, the Sweep, fed by the value the audio was made with.
//! - **§11's Off state**, carried by a badge and a fill rather than by hue.
//! - **§7's accent**: the collection's, not an identity hue of its own.
//!
//! # It is a panel, not a window
//!
//! [`panel`] takes a `Ui` and draws into it. It does not create a window, run an event loop, or own
//! a swapchain. That is what lets the same code be the plugin's CLAP editor and, later, a
//! standalone harness's contents.
//!
//! # Gestures
//!
//! Every edit is bracketed: `begin_set_parameter`, `set_parameter_normalized`, `end_set_parameter`,
//! in exactly one place — [`binding::Bound::apply`]. An unclosed gesture leaves a host's automation
//! lane latched, and it breaks the player's step editing outright.
//!
//! # No developer channel
//!
//! `plugins/AGENTS.md`'s channel arrives as MIDI CC, and an effect has no note port — see
//! `telemetry.rs` for why inventing one would be a lie about what this plugin is.

pub mod binding;
pub mod sections;

use std::collections::HashMap;
use std::sync::Arc;

use egui::Ui;
use mxm_ui::space::SPACE_5;
use mxm_ui::theme::Tokens;
use nice_plug::context::gui::GuiContext;
use nice_plug::prelude::*;
use nice_plug_egui::{EguiEditorState, NiceEguiApp, create_egui_editor};

use crate::params::MxmChorus06Params;
use crate::telemetry::Telemetry;

/// The size the editor **opens** at — **derived, not chosen**: the quarter-4K budget hugged around
/// its card (`plans/plan-editor-standard.md` F1), which `tests::the_opening_size_is_the_budget_hugged`
/// holds. The app bar compacts to whatever width that is (design system §3.1).
const REFERENCE: (u32, u32) = (616, 306);

/// The narrowest the window may be: the one card at its floor, plus the panel's gutters. Below it
/// the card's controls would overflow, which no arrangement fixes — there is only one card (§4.3).
const MINIMUM: (u32, u32) = (614, 220);

/// Builds the editor. Called from `Plugin::editor`.
pub fn create(
    params: Arc<MxmChorus06Params>,
    telemetry: Arc<Telemetry>,
) -> Option<MxmChorus06Editor> {
    let state = EguiEditorState::from_size(
        nice_plug::editor::dpi::LogicalSize::new(REFERENCE.0, REFERENCE.1),
        1.0,
    );

    create_egui_editor(
        state,
        nice_plug_egui::RepaintNotifier::new(),
        nice_plug_egui::EguiNiceSettings {
            title: "mxm-chorus-06".to_owned(),
            // **Resizable**, as every editor in the collection is (`plugins/AGENTS.md`).
            //
            // There is no flow here and there does not need to be: this effect is **one card**, and
            // a single card has no row to wrap into. What resizing buys it is a window a tiling
            // manager can size, with the card taking whatever width it is given; the floor is the
            // width below which the card's own controls stop being usable (§4.3).
            resize_hint: ResizeHint {
                size_constraints: nice_plug::editor::SizeConstraints::min_logical_size(
                    nice_plug::editor::dpi::LogicalSize::new(MINIMUM.0 as f32, MINIMUM.1 as f32),
                ),
                ..ResizeHint::RESIZABLE
            },
            ..Default::default()
        },
        MxmChorus06App::new(params, telemetry),
    )
}

/// The editor type the plugin exposes.
pub type MxmChorus06Editor = nice_plug_egui::EguiEditor<MxmChorus06App>;

/// Where the panel records the bottom of its content, for the fit test to read.
///
/// **Not `globally_used_rect`**: the central panel fills the window whatever is in it, so that
/// measure only exceeds the window once something is already cut off, and reads as exactly full
/// for any window taller than its content. `apps/mxm-player`'s and poly-06's scar (in mxm-player
/// and mxm-poly-06), avoided here by measuring the card itself.
pub(crate) fn content_bottom_id() -> egui::Id {
    egui::Id::new("mxm-chorus-06-content-bottom")
}

/// The editor's own state: what the plugin does not own and the host does not need.
pub struct MxmChorus06App {
    params: Arc<MxmChorus06Params>,
    telemetry: Arc<Telemetry>,
    /// Set in `build`, because that is where nice-plug hands it over.
    gui_context: Option<GuiContext>,
    /// Open text-entry buffers, keyed by parameter id.
    text_entry: HashMap<&'static str, Option<String>>,
    /// The preset library and everything the browser needs across frames.
    presets: PresetUi,
    /// Where the keyboard is: a card, and a parameter inside it. Transient, like the text
    /// buffers — it is not a parameter and nothing durable reads it.
    nav: mxm_ui::navigation::State,
}

/// The app bar's preset controls and what they need between frames — `mxm-preset`'s, one for
/// every instrument and effect.
pub use mxm_preset::PresetUi;

impl MxmChorus06App {
    pub fn new(params: Arc<MxmChorus06Params>, telemetry: Arc<Telemetry>) -> Self {
        let params_for_presets = Arc::clone(&params);
        Self {
            params,
            telemetry,
            gui_context: None,
            text_entry: HashMap::new(),
            presets: PresetUi::new(params_for_presets.as_ref()),
            nav: mxm_ui::navigation::State::default(),
        }
    }
}

impl NiceEguiApp for MxmChorus06App {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        nice_gui_ctx: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        mxm_ui::theme::apply(&egui_ctx);
        mxm_ui::typography::apply(&egui_ctx);
        // Light by default, overridable with `MXM_EDITOR_THEME`. The reasoning, and why the
        // default is not `System`, lives on `mxm_ui::theme::preference`.
        egui_ctx.set_theme(mxm_ui::theme::preference());
        self.gui_context = Some(nice_gui_ctx);
        Ok(())
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut nice_plug_egui::Frame) {
        let Some(gui_context) = self.gui_context.clone() else {
            return;
        };
        panel(
            ui,
            &self.params,
            &self.telemetry,
            &gui_context.param_setter(),
            &mut self.text_entry,
            &mut self.presets,
            &mut self.nav,
        );
    }

    fn editor_closed(&mut self) {
        self.gui_context = None;
    }
}

/// The whole editor, as a panel.
pub fn panel(
    ui: &mut Ui,
    params: &MxmChorus06Params,
    telemetry: &Telemetry,
    setter: &ParamSetter<'_>,
    text_entry: &mut HashMap<&'static str, Option<String>>,
    presets: &mut PresetUi,
    nav: &mut mxm_ui::navigation::State,
) {
    let tokens = &tokens_for(ui);

    // **The Sweep moves without input, so the frames have to come without input too.** egui
    // repaints when something happens; the modulator happening is not something egui can see. Both
    // mono editors carry the same request, and it stops when the editor closes because nothing
    // runs then.
    ui.ctx().request_repaint();

    let peak = telemetry.take_peak();
    let clipped = telemetry.clipped();
    // One question, and both layers suspend on it: the paging renderer's `hold` and the cursor's
    // `inert` both ask whether another surface owns this frame's keyboard.
    let busy = presets.holds_the_keyboard() || text_entry.values().any(Option::is_some);
    mxm_ui::paging::editor::hold(ui.ctx(), busy);
    // **The cursor moves before anything is drawn**, so a navigation arrow is consumed here rather
    // than also walking egui's own focus ring. It reads the registry and the exact card rectangles
    // the previous frame built, and navigates the paging plan's own order.
    mxm_ui::navigation::paged(ui.ctx(), nav, busy);

    mxm_ui::AppBar::new("mxm-chorus-06").show_with(
        ui,
        tokens,
        |ui| mxm_preset::ui::preset_row(ui, tokens, params, setter, presets),
        |ui| {
            if mxm_ui::shell::level_meter(ui, tokens, peak, clipped) {
                telemetry.clear_clip();
            }
            mxm_ui::shell::zoom_control(ui);

            // §3.1 slot 5, and the same place the player keeps it: at the left end of the bar's
            // right-hand group. What the person picks is remembered for every MXM editor, so the
            // next one to open agrees with this one.
            mxm_ui::shell::editor_theme_control(ui);
        },
    );

    mxm_preset::ui::overlays(ui, tokens, params, setter, presets);

    // **No view bar** — brief §6. One view, and a bar with one cell is an affordance for nothing.
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(tokens.canvas)
                .inner_margin(egui::Margin::same(SPACE_5 as i8)),
        )
        .show(ui, |ui| {
            let bottom = sections::chorus_card(ui, tokens, params, telemetry, setter, text_entry);
            // Recorded rather than derived, for the fit test — see `content_bottom_id`.
            ui.data_mut(|d| d.insert_temp(content_bottom_id(), bottom + SPACE_5));
        });
}

/// The collection's tokens, unchanged.
///
/// **No identity accent** — brief §7. §5.3 gives each *instrument* a hue so a rack of them is
/// tellable apart; an effect is told apart by its name in a chain, and a second accent scheme
/// would have nothing keeping it from colliding with the instruments'.
fn tokens_for(ui: &Ui) -> Tokens {
    if ui.visuals().dark_mode {
        mxm_ui::DARK
    } else {
        mxm_ui::LIGHT
    }
}

/// The paging items as the editor computes them, from a context set up as an editor's is — three
/// passes in, so the weighted font cuts are bound — for tests, which have no editor `Ui` to hand.
#[cfg(test)]
pub(crate) fn test_items(params: &MxmChorus06Params) -> Vec<mxm_ui::paging::Item<'static>> {
    let ctx = egui::Context::default();
    mxm_ui::typography::apply(&ctx);
    mxm_ui::theme::apply(&ctx);
    let mut items = Vec::new();
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            items = sections::page_items(ui, params);
        });
        output.textures_delta.clear();
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::internals::ParamPtr;
    use nice_plug::prelude::{PluginApi, PluginState};

    /// The editor reports edits through a `ParamSetter`; laying it out makes none.
    struct NoHost;

    impl nice_plug::context::gui::GuiContextInner for NoHost {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, _normalized: f32) {}
        unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            PluginState {
                version: String::new(),
                params: Default::default(),
                fields: Default::default(),
            }
        }
        fn set_state(&self, _state: PluginState) {}
    }

    /// Lays the panel out headlessly and returns the context and the bottom of its content.
    fn lay_out(width: f32, height: f32, mix: f32) -> (egui::Context, f32) {
        let ctx = egui::Context::default();
        mxm_ui::theme::apply(&ctx);
        mxm_ui::typography::apply(&ctx);
        ctx.set_theme(egui::ThemePreference::Light);
        ctx.all_styles_mut(|style| style.animation_time = 0.0);

        let params = MxmChorus06Params::default();
        {
            // Straight into the parameter, as `lib.rs`'s tests do: a layout test has no host, and
            // `ParamSetter` is the only other way in. `&FloatParam` rather than a trait object —
            // `InternalParamMut` carries an associated `Plain` that a `dyn` would have to name.
            use nice_plug::params::{InternalParamMut, Param};
            let normalised = params.mix.preview_normalized(mix);
            unsafe {
                let _ = params.mix._internal_set_normalized_value(normalised);
                params.mix._internal_update_smoother(48_000.0, true);
            }
        }
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut text_entry = HashMap::new();
        // A library rooted nowhere: a layout test must never touch the real config directory.
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();

        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            ..Default::default()
        };

        for _ in 0..3 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            });
            output.textures_delta.clear();
        }
        let bottom = ctx
            .memory(|m| m.data.get_temp::<f32>(content_bottom_id()))
            .expect("the panel records where its content ends");
        (ctx, bottom)
    }

    use mxm_plugin_test::keyboard_checks;

    /// What this editor keeps behind a disclosure, opened so the reachability check sees it.
    /// Nothing here: every control is on a card.
    const REVEAL: fn(&egui::Context) = |_| {};

    /// The rollout's own failure mode: a control whose `navigation::at` scope was forgotten paints
    /// exactly as before and is simply unreachable from the keyboard. Nothing else would say so.
    #[test]
    fn the_keyboard_cursor_reaches_and_operates_every_parameter() {
        let params = MxmChorus06Params::default();
        let telemetry = Telemetry::default();
        let host = keyboard_checks::Recorder::default();
        let setter = ParamSetter::new(&host);
        // Every parameter, and the circuit's buttons, which set Rate and are reached on their own.
        let ids: Vec<&str> = sections::all_parameters(&params)
            .iter()
            .map(|bound| bound.id)
            .chain([sections::POSITIONS_ID])
            .collect();
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        keyboard_checks::the_cursor_reaches_and_operates(
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &test_items(&params),
            keyboard_checks::Coverage::Exactly(&ids),
            &REVEAL,
            &host,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    /// A host that keeps every normalised value written to it, in order.
    #[derive(Default)]
    struct Keeper(std::sync::Mutex<Vec<f32>>);

    impl nice_plug::context::gui::GuiContextInner for Keeper {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }
        unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, normalized: f32) {
            self.0.lock().unwrap().push(normalized);
        }
        unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            PluginState {
                version: String::new(),
                params: Default::default(),
                fields: Default::default(),
            }
        }
        fn set_state(&self, _state: PluginState) {}
    }

    /// **A button puts Rate on the circuit's rate, and off the tempo** (the owner, 2026-09-28: the
    /// positions are buttons, as on the original). A pointer press on II, through the whole panel:
    /// free, it writes Rate and nothing else; synced, it switches the sync off first. What it
    /// writes is the rate's own normalised value, which reads back through the detent as the
    /// circuit's rate exactly — the rate `lib.rs` proves the standalone equal to the built-in at.
    #[test]
    fn a_button_puts_rate_on_its_rate_and_off_the_tempo() {
        use crate::params::{POSITIONS, circuit_rate};
        use nice_plug::params::{InternalParamMut, Param};
        for synced in [false, true] {
            let params = MxmChorus06Params::default();
            if synced {
                // SAFETY: the parameters are this test's own and nothing else reads them.
                unsafe {
                    let _ = params.rate_sync._internal_set_normalized_value(1.0);
                }
            }
            let keeper = Keeper::default();
            let setter = ParamSetter::new(&keeper);
            let telemetry = Telemetry::default();
            let mut text_entry = HashMap::new();
            let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
            let mut nav = mxm_ui::navigation::State::default();
            let ctx = egui::Context::default();
            mxm_ui::theme::apply(&ctx);
            mxm_ui::typography::apply(&ctx);
            let size = egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32);
            let mut frame = |events: Vec<egui::Event>| {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                };
                let mut output = ctx.run_ui(input, |ui| {
                    panel(
                        ui,
                        &params,
                        &telemetry,
                        &setter,
                        &mut text_entry,
                        &mut presets,
                        &mut nav,
                    );
                });
                output.textures_delta.clear();
            };
            for _ in 0..3 {
                frame(Vec::new());
            }
            // The row's middle cell: the cells are the row's width and sit on its bottom line.
            let spot = mxm_ui::navigation::spots(&ctx)
                .into_iter()
                .find(|spot| spot.key == sections::POSITIONS_ID)
                .expect("the buttons are on the panel");
            let at = egui::pos2(
                spot.rect.center().x,
                spot.rect.bottom() - mxm_ui::space::MIN_TARGET / 2.0,
            );
            let button = |pressed| egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            frame(vec![egui::Event::PointerMoved(at), button(true)]);
            frame(vec![button(false)]);

            let rate = params.rate.preview_normalized(POSITIONS[1].1);
            let written = keeper.0.lock().unwrap().clone();
            let expected: &[f32] = if synced { &[0.0, rate] } else { &[rate] };
            assert_eq!(written, expected, "synced: {synced}");
            assert_eq!(
                circuit_rate(params.rate.preview_plain(rate)),
                POSITIONS[1].1,
                "II lands on the circuit's rate"
            );
        }
    }

    /// **The scar both mono editors carry, turned into a number.** A height guessed before the
    /// panel existed, and controls cut off the bottom of a window that cannot be resized.
    #[test]
    fn the_panel_fits_the_editor() {
        let (_, used) = lay_out(REFERENCE.0 as f32, REFERENCE.1 as f32, 1.0);
        eprintln!("the panel needs {used} points of height");
        assert!(
            used <= REFERENCE.1 as f32,
            "the panel needs {used} points of height in a {} point window; it will be clipped",
            REFERENCE.1
        );
        assert!(
            used > REFERENCE.1 as f32 - 60.0,
            "the panel needs only {used} points in a {} point window; the window is taller than it has to be",
            REFERENCE.1
        );
    }

    /// The Off state is a *layout* no-op: the display changes what it paints, never how much room
    /// it takes, or the window would resize itself when the mix reached zero.
    #[test]
    fn switching_off_does_not_move_anything() {
        let (_, engaged) = lay_out(REFERENCE.0 as f32, REFERENCE.1 as f32, 1.0);
        let (_, off) = lay_out(REFERENCE.0 as f32, REFERENCE.1 as f32, 0.0);
        assert!(
            (engaged - off).abs() < 0.5,
            "the panel is {engaged} points engaged and {off} off; the Off state must not reflow"
        );
    }

    /// The card holds the knobs **and** a readable display beside them, and brief §10's minimum
    /// window holds the card: its floor — computed from its tree, the knob row, the gap and the
    /// Sweep's [`sections::SWEEP_MIN_WIDTH`] — plus the panel's gutters is no wider than
    /// [`MINIMUM`]. The window's width is the app bar's, so the card spends what it is given.
    #[test]
    fn the_minimum_window_holds_the_card_at_its_floor() {
        let floor = test_items(&MxmChorus06Params::default())[0].card.floor;
        assert!(
            floor + 2.0 * SPACE_5 <= MINIMUM.0 as f32,
            "the card's floor is {floor} points and the minimum window leaves it {}",
            MINIMUM.0 as f32 - 2.0 * SPACE_5
        );
    }

    /// Straight into a parameter, as `lay_out` does: a check has no host.
    fn set(param: &FloatParam, normalised: f32) {
        use nice_plug::params::InternalParamMut;
        unsafe {
            let _ = param._internal_set_normalized_value(normalised);
            param._internal_update_smoother(48_000.0, true);
        }
    }

    /// The card, in every state that changes what it paints, passes the layout tree's checks
    /// (plans/plan-layout-tree.md §4.3, `tree_checks::card`): its computed floor holds its content
    /// with nothing painted outside the card, the content floor is exact, the height its tree states
    /// is the height it draws, and every leaf stays in the room it was given.
    ///
    /// The states are this editor's structural-state matrix. The card has no route, disclosure or
    /// reserved alternative, so only what the Sweep paints changes: the init patch; Off (Mix at
    /// zero), which paints the badge and no dots; and the widest swing — Depth at its top with the
    /// modulator at an extreme — where a dot sits furthest out on its lane.
    #[test]
    fn every_card_passes_the_tree_checks_in_every_state() {
        for state in [
            "init",
            "off",
            "the widest swing",
            "rate synced, no tempo",
            "rate synced to a tempo",
        ] {
            let params = MxmChorus06Params::default();
            let telemetry = Telemetry::default();
            match state {
                "off" => set(&params.mix, 0.0),
                "the widest swing" => {
                    set(&params.depth, 1.0);
                    telemetry.publish_lfo(1.0);
                }
                "rate synced, no tempo" | "rate synced to a tempo" => {
                    // SAFETY: the parameters are this test's own and nothing else reads them.
                    unsafe {
                        use nice_plug::params::InternalParamMut;
                        let _ = params.rate_sync._internal_set_normalized_value(1.0);
                    }
                    if state == "rate synced to a tempo" {
                        telemetry.tempo.publish(Some(120.0));
                    }
                }
                _ => {}
            }
            let floor = test_items(&params)[0].card.floor;
            let host = NoHost;
            let setter = ParamSetter::new(&host);
            let mut text_entry = HashMap::new();
            let mut live = sections::Live {
                params: &params,
                telemetry: &telemetry,
                setter: &setter,
                text_entry: &mut text_entry,
            };
            tree_checks::card(
                &|_| {},
                state,
                sections::TITLE,
                floor,
                &|ui| sections::card(ui, &params),
                &mut |ui, leaf, rect| sections::paint(ui, &mxm_ui::LIGHT, leaf, rect, &mut live),
            );
        }
    }

    use mxm_plugin_test::opening_size;
    use mxm_plugin_test::tree_checks;

    /// **The editor opens at the quarter-4K budget, hugged** (`plans/plan-editor-standard.md` F1):
    /// `REFERENCE` is derived, not typed, and this holds it.
    #[test]
    fn the_opening_size_is_the_budget_hugged() {
        let params = MxmChorus06Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        opening_size::is_the_budget_hugged(
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &REVEAL,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    /// **The app bar holds in the narrowest window**: its `…` menu whole and nothing drawn over
    /// anything else, from `MINIMUM` up (`opening_size::bar_holds_from_the_minimum`).
    #[test]
    fn the_app_bar_holds_in_the_minimum_window() {
        let params = MxmChorus06Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        opening_size::bar_holds_from_the_minimum(
            egui::vec2(MINIMUM.0 as f32, MINIMUM.1 as f32),
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }

    /// The panel at the opening size, light and dark, for the owner's review of the layout-tree
    /// conversion (plans/plan-layout-tree.md §4.3): `target/layout-tree/mxm-chorus-06/<tag>/`, where
    /// `MXM_PICTURES` names the tag — `before` on the unconverted editor, `after` on the tree.
    ///
    /// `MXM_PICTURES=after cargo test -p mxm-chorus-06 --lib tree_pictures -- --ignored`
    #[test]
    #[ignore = "renders through wgpu; run by hand"]
    fn tree_pictures() {
        let tag = std::env::var("MXM_PICTURES").unwrap_or_else(|_| "after".to_owned());
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/layout-tree/mxm-chorus-06")
            .join(tag);
        let params = MxmChorus06Params::default();
        let telemetry = Telemetry::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let mut text_entry = HashMap::new();
        let mut presets = PresetUi::at(crate::preset::Library::at(None), &params);
        let mut nav = mxm_ui::navigation::State::default();
        tree_checks::pictures(
            &|_| {},
            egui::vec2(REFERENCE.0 as f32, REFERENCE.1 as f32),
            &dir,
            &mut |ui| {
                panel(
                    ui,
                    &params,
                    &telemetry,
                    &setter,
                    &mut text_entry,
                    &mut presets,
                    &mut nav,
                );
            },
        );
    }
}
