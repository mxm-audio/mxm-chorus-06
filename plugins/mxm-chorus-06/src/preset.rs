//! Presets: this plugin's factory set, and what the collection's preset crate needs of it.
//!
//! The format, the library on disk, favourites, the loaded identity and the app-bar controls are
//! `mxm-preset`'s — one crate for every instrument and effect, extracted from the verbatim copies
//! this file used to be one of (`plugins/AGENTS.md`, *A preset is parameter values*; since the
//! split that section is mxm-kit's `docs/plugin-conventions.md`, and `plugins/AGENTS.md` keeps its
//! one-line contract). What is left here is what only this plugin knows: its id, its parameters,
//! and its ten sounds.
//!
//! # Ten chorus sounds, and the circuit's positions are not among them
//!
//! The circuit's three switch positions are the panel's buttons (`params::POSITIONS`), so the
//! factory set is free to be what the opened controls can do beyond them (the owner, 2026-09-28:
//! *"then 10 chorus presets"*). Until then it was those positions, `One`, `Two` and `Onetwo`; a
//! project saved with one of them loaded still names it, because the loaded identity carries its own
//! name and baseline.

use std::sync::RwLock;

pub use mxm_preset::{
    Category, Entry, INIT_NAME, Library, Loaded, Origin, Preset, PresetIdentity, Refused, Value,
    factory, loaded, mark_loaded, mark_none, read_favourites, snapshot, write_favourites,
};

use crate::params::MxmChorus06Params;

/// **The tempo syncs this plugin gained on 2026-09-25** (`plans/plan-tempo-sync-controls.md`). A
/// preset file written before them was written unsynced, so each loads off rather than keeping the
/// instance's sync, and without reporting a missing control.
pub(crate) const TEMPO_SYNC_IDS: &[&str] = &["ratesync"];

impl mxm_preset::Instrument for MxmChorus06Params {
    fn clap_id(&self) -> &'static str {
        crate::CLAP_ID
    }

    /// In declaration order, from the one list the editor draws from.
    fn parameters(&self) -> Vec<(&'static str, &dyn mxm_preset::ErasedParam)> {
        crate::editor::sections::all_parameters(self)
            .into_iter()
            .map(|bound| (bound.id, bound.param))
            .collect()
    }

    fn identity(&self) -> &RwLock<PresetIdentity> {
        &self.preset
    }

    fn factory_files(&self) -> &'static [(&'static str, &'static str)] {
        FACTORY_FILES
    }

    fn default_missing_legacy_parameter(&self, id: &str) -> bool {
        TEMPO_SYNC_IDS.contains(&id)
    }
}

/// The factory set, compiled in.
///
/// **Ten files, from the subtlest to the strongest, and Init is not one of them** — see
/// [`Preset::init`], which is the circuit at I. Files rather than code, like every factory set: a
/// sound nobody can read is a sound nobody can learn from.
pub const FACTORY_FILES: &[(&str, &str)] = &[
    (
        "Clean doubler",
        include_str!("../presets/clean-doubler.json"),
    ),
    ("Gentle width", include_str!("../presets/gentle-width.json")),
    ("Slow drift", include_str!("../presets/slow-drift.json")),
    (
        "Deep ensemble",
        include_str!("../presets/deep-ensemble.json"),
    ),
    ("Wide pad", include_str!("../presets/wide-pad.json")),
    ("Fast shimmer", include_str!("../presets/fast-shimmer.json")),
    ("Warble", include_str!("../presets/warble.json")),
    ("Rotary swirl", include_str!("../presets/rotary-swirl.json")),
    ("Bar sweep", include_str!("../presets/bar-sweep.json")),
    ("Eighth pulse", include_str!("../presets/eighth-pulse.json")),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// **A project saved before the tempo syncs restores them Off** (`mxm_preset::add_switches_off`),
    /// whatever this instance had.
    #[test]
    fn an_older_state_restores_the_tempo_syncs_off() {
        use nice_plug::prelude::Plugin as _;
        let mut state = nice_plug::prelude::PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        };
        crate::MxmChorus06::filter_state(&mut state);
        for id in TEMPO_SYNC_IDS {
            assert!(
                matches!(
                    state.params.get(*id),
                    Some(nice_plug::plugin::ParamValue::Bool(false))
                ),
                "{{id}} was not restored off"
            );
        }
    }

    /// **A preset saved before the tempo syncs loads them off, and cleanly** ([`TEMPO_SYNC_IDS`]).
    #[test]
    fn a_preset_from_before_the_tempo_syncs_loads_them_off() {
        let params = crate::params::MxmChorus06Params::default();
        let mut old = mxm_preset::Preset::init(&params);
        for id in TEMPO_SYNC_IDS {
            old.params.remove(*id);
        }
        let (writes, problems) = old.resolve(&params);
        assert!(problems.is_empty(), "{{problems:?}}");
        for id in TEMPO_SYNC_IDS {
            assert!(
                writes.iter().any(|(w, _, v)| w == id && *v == 0.0),
                "{{id}} was not written off"
            );
        }
    }

    use mxm_preset::user_root;
    use nice_plug::params::Param;

    fn params() -> MxmChorus06Params {
        MxmChorus06Params::default()
    }

    /// Prints every parameter as eleven `normalised=formatted` steps.
    ///
    /// A facility, not a test: designing a factory preset means choosing values, and choosing them
    /// blind is how a preset ends up somewhere nobody meant.
    ///
    /// ```text
    /// cargo test -p mxm-chorus-06 --lib the_mapping_table -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "prints what each normalised value means, for preset design"]
    fn the_mapping_table() {
        let params = params();
        for bound in crate::editor::sections::all_parameters(&params) {
            let steps: Vec<String> = (0..=10)
                .map(|i| {
                    let v = i as f32 / 10.0;
                    format!("{v:.1}={}", bound.param.format(v))
                })
                .collect();
            eprintln!("{:<12} {}", bound.id, steps.join("  "));
        }
    }

    /// What a design sets a parameter to: a plain value in the parameter's own unit — hertz,
    /// milliseconds, a fraction — or, for a synced Rate, the division its position picks.
    #[derive(Clone, Copy, Debug)]
    enum Set {
        Plain(f32),
        Synced(mxm_tempo::Division),
    }

    use Set::{Plain, Synced};
    use mxm_tempo::Division;

    /// One designed preset: its name, its category, and the values that make it.
    type Design = (&'static str, Category, &'static [(&'static str, Set)]);

    /// The ten sounds, **in the controls' own units**: Rate in hertz (or a division, synced),
    /// Depth in milliseconds either side of the centre — the circuit's is 1.2, the most 2.4 — and
    /// Mix and Noise as fractions of the circuit's. Everything not named is the default, which
    /// is the circuit. A first draft by numbers, to be tuned by ear.
    const FACTORY_DESIGN: &[Design] = &[
        // Barely moving, and no hiss: a short second voice.
        (
            "Clean doubler",
            Category::Fx,
            &[
                ("rate", Plain(0.2)),
                ("depth", Plain(0.3)),
                ("mix", Plain(0.7)),
                ("noise", Plain(0.0)),
            ],
        ),
        // A little width without an audible sweep.
        (
            "Gentle width",
            Category::Fx,
            &[
                ("rate", Plain(0.35)),
                ("depth", Plain(0.8)),
                ("mix", Plain(0.6)),
                ("noise", Plain(0.4)),
            ],
        ),
        // A long, slow sweep.
        (
            "Slow drift",
            Category::Fx,
            &[
                ("rate", Plain(0.1)),
                ("depth", Plain(2.0)),
                ("noise", Plain(0.6)),
            ],
        ),
        // The slow button at twice the depth, hiss and all.
        (
            "Deep ensemble",
            Category::Fx,
            &[("rate", Plain(0.5)), ("depth", Plain(2.4))],
        ),
        (
            "Wide pad",
            Category::Fx,
            &[
                ("rate", Plain(0.9)),
                ("depth", Plain(1.5)),
                ("noise", Plain(0.3)),
            ],
        ),
        // Quick and shallow: a shimmer rather than a wobble.
        (
            "Fast shimmer",
            Category::Fx,
            &[
                ("rate", Plain(3.0)),
                ("depth", Plain(0.4)),
                ("mix", Plain(0.8)),
                ("noise", Plain(0.5)),
            ],
        ),
        // Quick and deep: the pitch wobbles.
        (
            "Warble",
            Category::Fx,
            &[
                ("rate", Plain(5.0)),
                ("depth", Plain(1.8)),
                ("noise", Plain(0.7)),
            ],
        ),
        (
            "Rotary swirl",
            Category::Fx,
            &[
                ("rate", Plain(7.0)),
                ("depth", Plain(0.8)),
                ("noise", Plain(0.5)),
            ],
        ),
        // One sweep a bar, following the host's tempo.
        (
            "Bar sweep",
            Category::Fx,
            &[
                ("ratesync", Plain(1.0)),
                ("rate", Synced(Division::Whole)),
                ("depth", Plain(2.0)),
                ("noise", Plain(0.6)),
            ],
        ),
        // A pulse on every eighth note.
        (
            "Eighth pulse",
            Category::Fx,
            &[
                ("ratesync", Plain(1.0)),
                ("rate", Synced(Division::Eighth)),
                ("depth", Plain(0.6)),
                ("mix", Plain(0.9)),
                ("noise", Plain(0.4)),
            ],
        ),
    ];

    /// A design's value through the named parameter's own range: a synced Rate is the position
    /// that picks its division (`LFO_SYNC.position`), the same at every tempo that reaches it.
    fn normalised_for(params: &MxmChorus06Params, id: &str, set: Set) -> f32 {
        match (id, set) {
            ("rate", Synced(division)) => crate::params::LFO_SYNC.position(division),
            ("rate", Plain(hz)) => params.rate.preview_normalized(hz),
            ("depth", Plain(ms)) => params.depth.preview_normalized(ms),
            ("mix", Plain(v)) => params.mix.preview_normalized(v),
            ("noise", Plain(v)) => params.noise.preview_normalized(v),
            ("ratesync", Plain(on)) => on,
            (other, set) => panic!("`{other}` cannot be set to {set:?}"),
        }
    }

    /// Writes the ten factory presets to `plugins/mxm-chorus-06/presets/`.
    ///
    /// A facility, not a test — and the *only* thing that writes those files, so the numbers in
    /// `FACTORY_DESIGN` stay the readable statement of each preset and the JSON stays generated
    /// output. `every_factory_preset_covers_every_parameter` is what catches a file that has fallen
    /// behind a new parameter.
    ///
    /// ```text
    /// cargo test -p mxm-chorus-06 --lib write_the_factory_presets -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "writes the factory preset files"]
    fn write_the_factory_presets() {
        let params = params();

        for (name, category, overrides) in FACTORY_DESIGN {
            let preset = generated(&params, name, *category, overrides);

            // From the manifest directory, not the working one: a test's cwd is the crate root
            // and not the workspace root, which is the sort of thing that only says so once.
            let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("presets")
                .join(format!("{}.json", name.to_lowercase().replace(' ', "-")));
            std::fs::write(&file, preset.to_json()).expect("write the preset");
            eprintln!("wrote {}", file.display());
        }
    }

    #[test]
    fn every_designed_preset_names_real_parameters() {
        // Runs by default, unlike the generator: a typo in `FACTORY_DESIGN` would otherwise only
        // surface the next time somebody regenerated the files.
        let params = params();
        let bindings = crate::editor::sections::all_parameters(&params);
        for (name, _category, overrides) in FACTORY_DESIGN {
            for (id, set) in *overrides {
                assert!(
                    bindings.iter().any(|bound| bound.id == *id),
                    "{name:?} names `{id}`, which is not a parameter of this plugin"
                );
                let v = normalised_for(&params, id, *set);
                assert!(
                    (0.0..=1.0).contains(&v),
                    "{name:?} sets `{id}` to {set:?}, outside its range"
                );
            }
        }
    }

    /// What the generator would write for one design, in memory.
    fn generated(
        params: &MxmChorus06Params,
        name: &str,
        category: Category,
        overrides: &[(&str, Set)],
    ) -> Preset {
        let bindings = crate::editor::sections::all_parameters(params);
        let mut preset = Preset::init(params);
        preset.name = name.to_owned();
        preset.category = category;
        for (id, set) in overrides {
            let bound = bindings
                .iter()
                .find(|bound| bound.id == *id)
                .unwrap_or_else(|| panic!("{name:?} names `{id}`, which is not a parameter"));
            let v = normalised_for(params, id, *set);
            preset.params.insert(
                (*id).to_owned(),
                Value {
                    v,
                    text: bound.param.format(v),
                },
            );
        }
        preset
    }

    #[test]
    fn the_factory_files_match_the_design_they_were_generated_from() {
        // The generator is `#[ignore]`d, so nothing forces it to have been run. This is what says
        // the shipped files are the current design rather than a stale one — the same class of
        // mistake as a stale `.clap` bundle, and just as quiet.
        //
        // **The whole preset, `text` included**, not only the overridden values: a file generated
        // before a formatter changed would otherwise ship with stale text in it. Never read, but
        // exactly the drift this test exists to see.
        let params = params();
        for (name, category, overrides) in FACTORY_DESIGN {
            let (_, text) = FACTORY_FILES
                .iter()
                .find(|(file_name, _)| file_name == name)
                .unwrap_or_else(|| panic!("no factory file for {name:?}"));
            let shipped = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let expected = generated(&params, name, *category, overrides);
            assert_eq!(
                shipped, expected,
                "{name:?} on disk is not what the design generates — regenerate the files"
            );
        }
    }

    #[test]
    fn the_user_root_is_under_this_plugins_own_id() {
        // Namespaced by CLAP id so an instrument's presets cannot appear in this effect's list.
        let Some(root) = user_root(crate::CLAP_ID) else {
            return;
        };
        assert!(root.ends_with("presets"));
        assert!(root.to_string_lossy().contains(crate::CLAP_ID));
    }

    /// **No factory preset is Init, nor one of the circuit's buttons** — those are a click away
    /// already (the owner, 2026-09-28). A button's sound is Init with Rate on its rate.
    #[test]
    fn no_factory_preset_is_init_or_a_button() {
        // The values alone: what a preset sounds like, not how its file spells them.
        let values = |preset: &Preset| -> Vec<(String, f32)> {
            preset
                .params
                .iter()
                .map(|(id, value)| (id.clone(), value.v))
                .collect()
        };
        let params = params();
        let init = Preset::init(&params);
        for (name, text) in FACTORY_FILES {
            let sound = values(&Preset::parse(text, crate::CLAP_ID).expect("parses"));
            assert_ne!(sound, values(&init), "{name:?} is Init");
            for (button, hz) in crate::params::POSITIONS {
                let mut at_button = init.clone();
                at_button.params.get_mut("rate").expect("Rate").v =
                    params.rate.preview_normalized(hz);
                assert_ne!(sound, values(&at_button), "{name:?} is the {button} button");
            }
        }
    }

    /// **A synced preset's Rate picks its division**, at the position the design names, with the
    /// sync on — so *one bar* means one bar at every tempo that reaches it.
    #[test]
    fn a_synced_preset_picks_its_division() {
        for (name, _, overrides) in FACTORY_DESIGN {
            let Some(division) = overrides.iter().find_map(|(id, set)| match (*id, set) {
                ("rate", Synced(division)) => Some(*division),
                _ => None,
            }) else {
                continue;
            };
            let (_, text) = FACTORY_FILES.iter().find(|(n, _)| n == name).unwrap();
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            assert_eq!(preset.params["ratesync"].v, 1.0, "{name:?} is synced");
            assert_eq!(
                crate::params::LFO_SYNC.pick(preset.params["rate"].v),
                division,
                "{name:?}"
            );
        }
    }

    #[test]
    fn no_two_factory_presets_are_the_same_sound() {
        // A copied-and-edited design could lose its edit unnoticed.
        for (index, (name, text)) in FACTORY_FILES.iter().enumerate() {
            let a = Preset::parse(text, crate::CLAP_ID).expect("parses");
            for (other, text) in &FACTORY_FILES[index + 1..] {
                let b = Preset::parse(text, crate::CLAP_ID).expect("parses");
                assert_ne!(a.params, b.params, "{name:?} and {other:?} are identical");
            }
        }
    }

    #[test]
    fn every_factory_preset_has_a_category() {
        // A sound is saved with its category (the owner's rule, 2026-09-04), and the factory set
        // is where a person first sees what the categories mean. *Uncategorised* is for files
        // written before the field existed, not for sounds this plugin ships.
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            assert_ne!(
                preset.category,
                Category::Uncategorised,
                "factory preset {name:?} has no category"
            );
        }
    }

    #[test]
    fn every_factory_preset_parses_and_is_for_this_instrument() {
        // A malformed factory preset is a build mistake, not a user's, so it is caught here rather
        // than skipped quietly in the browser.
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID)
                .unwrap_or_else(|e| panic!("factory preset {name:?} does not parse: {e}"));
            assert_eq!(preset.name, *name, "the file's name must match its listing");
        }
    }

    #[test]
    fn every_factory_preset_covers_every_parameter() {
        // The one that catches a factory preset written before a parameter existed: it would load
        // and quietly leave that parameter wherever the last patch left it.
        let params = params();
        for (name, text) in FACTORY_FILES {
            let preset = Preset::parse(text, crate::CLAP_ID).expect("parses");
            let (_, problems) = preset.resolve(&params);
            assert!(
                problems.is_empty(),
                "factory preset {name:?} is incomplete: {problems:?}"
            );
        }
    }

    #[test]
    fn the_factory_list_begins_with_init() {
        let params = params();
        let all = factory(&params);
        assert_eq!(all[0].name, INIT_NAME);
        assert_eq!(all.len(), FACTORY_FILES.len() + 1);
    }
}
