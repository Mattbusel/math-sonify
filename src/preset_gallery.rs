//! Preset Gallery
//!
//! Organises all named presets into a browsable, in-memory catalogue with
//! rich metadata: mood tags, per-system labels, complexity ratings,
//! favorites tracking, play-count history, and a "random discovery" mode
//! that weights selection toward less-played entries.
//!
//! ## Quick usage
//!
//! ```
//! use math_sonify_plugin::preset_gallery::PresetGallery;
//!
//! let mut gallery = PresetGallery::with_builtin_presets();
//!
//! // Browse by mood
//! let atmospheric = gallery.by_mood("atmospheric");
//!
//! // Play something you haven't heard yet
//! if let Some(p) = gallery.random_discovery() {
//!     println!("Discover: {}", p.name);
//!     gallery.record_play(&p.name.clone());
//! }
//! ```

use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Data types
// ─────────────────────────────────────────────────────────────────────────────

/// Rich metadata record for one preset.
#[derive(Debug, Clone)]
pub struct PresetMetadata {
    /// Display name (matches the key used in `presets::load_preset`).
    pub name: String,
    /// Name of the underlying dynamical system (e.g. `"Lorenz"`, `"Rossler"`).
    pub system: String,
    /// Mood / aesthetic tags (lower-case, e.g. `"atmospheric"`, `"rhythmic"`).
    pub mood: Vec<String>,
    /// One or two sentence description of the sonic character.
    pub description: String,
    /// Typical BPM range this preset sounds good at `(min, max)`.
    pub bpm_range: (f32, f32),
    /// Subjective complexity rating from 1 (minimal) to 5 (dense).
    pub complexity: u8,
    /// Whether the user has marked this preset as a favorite.
    pub is_favorite: bool,
    /// Total number of times this preset has been played in this session.
    pub play_count: u32,
}

impl PresetMetadata {
    fn new(
        name: &str,
        system: &str,
        mood: &[&str],
        description: &str,
        bpm_range: (f32, f32),
        complexity: u8,
    ) -> Self {
        Self {
            name: name.to_string(),
            system: system.to_string(),
            mood: mood.iter().map(|s| s.to_string()).collect(),
            description: description.to_string(),
            bpm_range,
            complexity: complexity.clamp(1, 5),
            is_favorite: false,
            play_count: 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PresetGallery
// ─────────────────────────────────────────────────────────────────────────────

/// Browsable catalogue of all built-in presets with metadata, favorites,
/// play history, and discovery helpers.
pub struct PresetGallery {
    presets: Vec<PresetMetadata>,
    /// History of recently played preset names (newest at the back).
    history: Vec<String>,
    /// Maximum number of entries kept in `history`.
    history_limit: usize,
}

impl Default for PresetGallery {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetGallery {
    /// Create an empty gallery.
    pub fn new() -> Self {
        Self {
            presets: Vec::new(),
            history: Vec::new(),
            history_limit: 20,
        }
    }

    /// Create a gallery pre-loaded with all built-in preset metadata.
    pub fn with_builtin_presets() -> Self {
        let mut g = Self::new();
        g.presets = builtin_presets();
        g
    }

    // ── Filtering ──────────────────────────────────────────────────────────

    /// Return all presets that carry the given mood tag (case-insensitive).
    pub fn by_mood(&self, mood: &str) -> Vec<&PresetMetadata> {
        let q = mood.to_lowercase();
        self.presets.iter().filter(|p| p.mood.iter().any(|m| m == &q)).collect()
    }

    /// Return all presets for the given dynamical system name (case-insensitive).
    pub fn by_system(&self, system: &str) -> Vec<&PresetMetadata> {
        let q = system.to_lowercase();
        self.presets.iter().filter(|p| p.system.to_lowercase() == q).collect()
    }

    /// Return all favorite presets.
    pub fn favorites(&self) -> Vec<&PresetMetadata> {
        self.presets.iter().filter(|p| p.is_favorite).collect()
    }

    /// Full-text search across name and description (case-insensitive substring).
    pub fn search(&self, query: &str) -> Vec<&PresetMetadata> {
        let q = query.to_lowercase();
        self.presets
            .iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&q) || p.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    // ── Discovery ──────────────────────────────────────────────────────────

    /// Return a random preset weighted toward entries with the lowest play
    /// count (inverse-count weighting).  Returns `None` only if the gallery
    /// is empty.
    ///
    /// Uses a simple linear congruential RNG seeded from the current time so
    /// there is no dependency on the `rand` crate.
    pub fn random_discovery(&self) -> Option<&PresetMetadata> {
        if self.presets.is_empty() {
            return None;
        }
        // Build weight vector: weight = 1 / (play_count + 1)
        // Represented as integer weights to avoid floating-point dependency.
        let max_count = self.presets.iter().map(|p| p.play_count).max().unwrap_or(0);
        let weights: Vec<u32> = self
            .presets
            .iter()
            .map(|p| (max_count - p.play_count + 1))
            .collect();
        let total: u32 = weights.iter().sum();

        // Simple LCG for reproducibility without an external crate.
        let seed = lcg_seed();
        let pick = seed % u64::from(total);
        let mut acc: u64 = 0;
        for (i, &w) in weights.iter().enumerate() {
            acc += u64::from(w);
            if pick < acc {
                return Some(&self.presets[i]);
            }
        }
        self.presets.last()
    }

    // ── Mutation ───────────────────────────────────────────────────────────

    /// Toggle the favorite flag for the preset with the given name.
    /// Does nothing if the name is not found.
    pub fn toggle_favorite(&mut self, name: &str) {
        if let Some(p) = self.presets.iter_mut().find(|p| p.name == name) {
            p.is_favorite = !p.is_favorite;
        }
    }

    /// Increment the play count for `name` and append it to the history.
    /// Does nothing if the name is not found.
    pub fn record_play(&mut self, name: &str) {
        if let Some(p) = self.presets.iter_mut().find(|p| p.name == name) {
            p.play_count = p.play_count.saturating_add(1);
        }
        // Append to history, trimming the oldest entry if needed.
        self.history.push(name.to_string());
        if self.history.len() > self.history_limit {
            self.history.remove(0);
        }
    }

    // ── Queries ────────────────────────────────────────────────────────────

    /// Return the history slice (oldest to newest).
    pub fn recent(&self) -> &[String] {
        &self.history
    }

    /// Return a sorted, deduplicated list of all mood tags across all presets.
    pub fn all_moods(&self) -> Vec<String> {
        let mut moods: Vec<String> = self
            .presets
            .iter()
            .flat_map(|p| p.mood.iter().cloned())
            .collect();
        moods.sort();
        moods.dedup();
        moods
    }

    /// Return a sorted, deduplicated list of all system names across all presets.
    pub fn all_systems(&self) -> Vec<String> {
        let mut systems: Vec<String> = self.presets.iter().map(|p| p.system.clone()).collect();
        systems.sort();
        systems.dedup();
        systems
    }

    /// Return a reference to the full preset list.
    pub fn all(&self) -> &[PresetMetadata] {
        &self.presets
    }

    /// Look up a preset by exact name.
    pub fn get(&self, name: &str) -> Option<&PresetMetadata> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Return the number of presets in the gallery.
    pub fn len(&self) -> usize {
        self.presets.len()
    }

    /// Return `true` if the gallery contains no presets.
    pub fn is_empty(&self) -> bool {
        self.presets.is_empty()
    }

    /// Rebuild a `HashMap<String, u32>` of `name -> play_count` for
    /// serialisation / persistence.
    pub fn play_counts(&self) -> HashMap<String, u32> {
        self.presets.iter().map(|p| (p.name.clone(), p.play_count)).collect()
    }

    /// Restore play counts from a previously persisted map.
    pub fn restore_play_counts(&mut self, counts: &HashMap<String, u32>) {
        for p in &mut self.presets {
            if let Some(&c) = counts.get(&p.name) {
                p.play_count = c;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Built-in preset metadata catalogue
// ─────────────────────────────────────────────────────────────────────────────

fn builtin_presets() -> Vec<PresetMetadata> {
    vec![
        PresetMetadata::new(
            "Lorenz Ambience",
            "Lorenz",
            &["atmospheric", "meditative", "melodic"],
            "Slow, breathy drift through the butterfly attractor. \
             The pentatonic scale and long portamento give an organic, floating quality \
             reminiscent of early Brian Eno ambient works.",
            (60.0, 90.0),
            2,
        ),
        PresetMetadata::new(
            "Pendulum Rhythm",
            "Double Pendulum",
            &["rhythmic", "percussive", "energetic"],
            "Chaotic double-pendulum dynamics drive a granular percussion engine. \
             Sudden flips between the two pendulum arms create unpredictable but \
             rhythmically satisfying bursts.",
            (100.0, 140.0),
            3,
        ),
        PresetMetadata::new(
            "Torus Drone",
            "Geodesic Torus",
            &["atmospheric", "meditative", "drone"],
            "Irrational winding on a flat torus; the pitch never exactly repeats. \
             Deep just-intonation harmonics and a long reverb tail create a \
             cathedral-like drone that evolves over minutes.",
            (40.0, 70.0),
            2,
        ),
        PresetMetadata::new(
            "Kuramoto Sync",
            "Kuramoto",
            &["experimental", "evolving", "hypnotic"],
            "Eight phase-coupled oscillators start detuned and gradually \
             phase-lock as the coupling constant rises. The moment of synchronisation \
             is audible as a sudden tonal coalescence.",
            (80.0, 120.0),
            3,
        ),
        PresetMetadata::new(
            "Three-Body Jazz",
            "Three-Body",
            &["melodic", "rhythmic", "complex"],
            "The figure-8 gravitational orbit mapped to spectral synthesis and a \
             dom7 chord produces cascading melodic fragments with jazz-like \
             voice-leading accidents.",
            (90.0, 130.0),
            4,
        ),
        PresetMetadata::new(
            "Rössler Drift",
            "Rossler",
            &["atmospheric", "melodic", "meditative"],
            "The gentle spiral of the Rössler attractor filtered through a \
             microtonal scale. Sus2 voicings and long portamento produce a \
             shimmering, slightly-out-of-tune quality that rewards headphone listening.",
            (55.0, 85.0),
            2,
        ),
        PresetMetadata::new(
            "FM Chaos",
            "Lorenz",
            &["experimental", "electronic", "energetic"],
            "Two-operator FM synthesis driven by the butterfly attractor. \
             Carrier and modulation index track the x and z coordinates respectively, \
             generating metallic, unpredictable timbres that shift between bell-like \
             and harsh.",
            (100.0, 160.0),
            4,
        ),
        PresetMetadata::new(
            "Pendulum Meditation",
            "Double Pendulum",
            &["meditative", "atmospheric", "drone"],
            "A slow double-pendulum run at reduced speed, mapped to orbital \
             sonification and pure just-intonation ratios. Extended reverb and a \
             low base frequency create a Tibetan-bowl quality.",
            (40.0, 65.0),
            2,
        ),
        PresetMetadata::new(
            "Thomas Labyrinth",
            "Thomas",
            &["atmospheric", "experimental", "eerie"],
            "The cyclically-symmetric Thomas attractor wandering through vowel \
             formant space. The voice-like timbres shift between /a/ and /o/ \
             as the trajectory loops through its labyrinthine channels.",
            (60.0, 90.0),
            3,
        ),
        PresetMetadata::new(
            "Neural Burst",
            "Hindmarsh-Rose",
            &["rhythmic", "percussive", "experimental"],
            "Hindmarsh-Rose neuron bursting at high drive current. Each spike \
             cluster triggers a grain spray; the inter-burst silence creates a \
             breathable, biological rhythm.",
            (110.0, 170.0),
            4,
        ),
        PresetMetadata::new(
            "Chemical Wave",
            "Oregonator",
            &["atmospheric", "evolving", "hypnotic"],
            "The Belousov-Zhabotinsky chemical oscillator in spectral mode. \
             32 additive partials track the stiff ODE's slow oscillation cycles, \
             creating a pulsating spectral landscape.",
            (60.0, 100.0),
            3,
        ),
        PresetMetadata::new(
            "Sprott Minimal",
            "Sprott E",
            &["experimental", "electronic", "minimalist"],
            "Sprott's algebraically minimal five-term chaotic system mapped to \
             amplitude modulation. The system's sparse nonlinearity produces a \
             clean, almost crystalline texture interrupted by sudden AM sweeps.",
            (80.0, 120.0),
            2,
        ),
        PresetMetadata::new(
            "Substorm Pulse",
            "WINDMI",
            &["rhythmic", "atmospheric", "electronic"],
            "The WINDMI ionospheric substorm model drives a Karplus-Strong \
             waveguide string. The exponential nonlinearity creates sudden \
             energy-release events that sound like plucked bass pulses.",
            (90.0, 130.0),
            3,
        ),
        PresetMetadata::new(
            "Market Collapse",
            "Finance",
            &["experimental", "eerie", "complex"],
            "Macroeconomic chaos in spectral mode over a Locrian scale. \
             The dim7 chord voicing and tense Locrian tonality give a \
             distinctly unsettling, horror-soundtrack quality.",
            (100.0, 140.0),
            4,
        ),
        PresetMetadata::new(
            "Hyperdimensional",
            "Hyperchaos",
            &["experimental", "complex", "electronic"],
            "Chen-Li hyperchaos (two positive Lyapunov exponents) run through \
             FM synthesis on an octatonic scale. The extra degree of instability \
             makes the timbre changes faster and less predictable than ordinary \
             chaos.",
            (110.0, 160.0),
            5,
        ),
        PresetMetadata::new(
            "Magyar Trance",
            "Dadras",
            &["meditative", "melodic", "atmospheric"],
            "The Dadras attractor, with its rich bifurcation structure, \
             routed through vocal formant mapping on the Hungarian minor scale. \
             Heavy reverb and slow portamento evoke eastern European folk tradition.",
            (70.0, 100.0),
            3,
        ),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// A minimal LCG (linear congruential generator) seeded from the system clock.
/// Avoids a dependency on `rand` while providing non-deterministic picks.
fn lcg_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345) as u64;
    // LCG step with Knuth multiplier
    nanos.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn gallery() -> PresetGallery {
        PresetGallery::with_builtin_presets()
    }

    // ── Construction ──────────────────────────────────────────────────────

    #[test]
    fn builtin_has_sixteen_presets() {
        assert_eq!(gallery().len(), 16);
    }

    #[test]
    fn all_preset_names_non_empty() {
        for p in gallery().all() {
            assert!(!p.name.is_empty());
        }
    }

    #[test]
    fn all_preset_systems_non_empty() {
        for p in gallery().all() {
            assert!(!p.system.is_empty(), "system empty for '{}'", p.name);
        }
    }

    #[test]
    fn all_preset_descriptions_non_empty() {
        for p in gallery().all() {
            assert!(!p.description.is_empty(), "description empty for '{}'", p.name);
        }
    }

    #[test]
    fn all_preset_moods_non_empty() {
        for p in gallery().all() {
            assert!(!p.mood.is_empty(), "no moods for '{}'", p.name);
        }
    }

    #[test]
    fn complexity_in_range() {
        for p in gallery().all() {
            assert!((1..=5).contains(&p.complexity), "complexity out of range for '{}'", p.name);
        }
    }

    // ── by_mood ───────────────────────────────────────────────────────────

    #[test]
    fn by_mood_atmospheric_non_empty() {
        assert!(!gallery().by_mood("atmospheric").is_empty());
    }

    #[test]
    fn by_mood_unknown_empty() {
        assert!(gallery().by_mood("zzyxqq").is_empty());
    }

    #[test]
    fn by_mood_case_insensitive() {
        let g = gallery();
        let lower = g.by_mood("atmospheric").len();
        let upper = g.by_mood("ATMOSPHERIC").len();
        assert_eq!(lower, upper);
    }

    // ── by_system ─────────────────────────────────────────────────────────

    #[test]
    fn by_system_lorenz_non_empty() {
        assert!(!gallery().by_system("Lorenz").is_empty());
    }

    #[test]
    fn by_system_case_insensitive() {
        let g = gallery();
        assert_eq!(g.by_system("lorenz").len(), g.by_system("LORENZ").len());
    }

    // ── search ────────────────────────────────────────────────────────────

    #[test]
    fn search_butterfly_finds_lorenz() {
        let results = gallery().search("butterfly");
        assert!(!results.is_empty(), "search for 'butterfly' should find Lorenz presets");
    }

    #[test]
    fn search_no_match_empty() {
        assert!(gallery().search("zzzzzzzzz").is_empty());
    }

    // ── toggle_favorite ───────────────────────────────────────────────────

    #[test]
    fn toggle_favorite_sets_and_clears() {
        let mut g = gallery();
        assert!(!g.get("Lorenz Ambience").unwrap().is_favorite);
        g.toggle_favorite("Lorenz Ambience");
        assert!(g.get("Lorenz Ambience").unwrap().is_favorite);
        g.toggle_favorite("Lorenz Ambience");
        assert!(!g.get("Lorenz Ambience").unwrap().is_favorite);
    }

    #[test]
    fn toggle_favorite_unknown_name_no_panic() {
        let mut g = gallery();
        g.toggle_favorite("does not exist"); // must not panic
    }

    #[test]
    fn favorites_returns_marked_presets() {
        let mut g = gallery();
        g.toggle_favorite("Lorenz Ambience");
        g.toggle_favorite("FM Chaos");
        let favs: Vec<_> = g.favorites().iter().map(|p| p.name.as_str()).collect();
        assert!(favs.contains(&"Lorenz Ambience"));
        assert!(favs.contains(&"FM Chaos"));
        assert_eq!(favs.len(), 2);
    }

    // ── record_play ───────────────────────────────────────────────────────

    #[test]
    fn record_play_increments_count() {
        let mut g = gallery();
        g.record_play("Lorenz Ambience");
        g.record_play("Lorenz Ambience");
        assert_eq!(g.get("Lorenz Ambience").unwrap().play_count, 2);
    }

    #[test]
    fn record_play_adds_to_history() {
        let mut g = gallery();
        g.record_play("Lorenz Ambience");
        g.record_play("FM Chaos");
        assert_eq!(g.recent(), &["Lorenz Ambience", "FM Chaos"]);
    }

    #[test]
    fn history_trims_to_limit() {
        let mut g = gallery();
        g.history_limit = 3;
        for name in ["Lorenz Ambience", "FM Chaos", "Torus Drone", "Neural Burst"] {
            g.record_play(name);
        }
        assert_eq!(g.recent().len(), 3);
        assert_eq!(g.recent()[0], "FM Chaos");
    }

    // ── all_moods / all_systems ───────────────────────────────────────────

    #[test]
    fn all_moods_sorted_deduped() {
        let moods = gallery().all_moods();
        let mut sorted = moods.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(moods, sorted);
    }

    #[test]
    fn all_systems_contains_lorenz() {
        assert!(gallery().all_systems().contains(&"Lorenz".to_string()));
    }

    // ── random_discovery ──────────────────────────────────────────────────

    #[test]
    fn random_discovery_some() {
        assert!(gallery().random_discovery().is_some());
    }

    #[test]
    fn random_discovery_empty_gallery_none() {
        assert!(PresetGallery::new().random_discovery().is_none());
    }

    #[test]
    fn random_discovery_in_catalogue() {
        let g = gallery();
        let picked = g.random_discovery().unwrap();
        assert!(g.all().iter().any(|p| p.name == picked.name));
    }

    // ── play_counts / restore_play_counts ─────────────────────────────────

    #[test]
    fn play_counts_restore_round_trip() {
        let mut g = gallery();
        g.record_play("Lorenz Ambience");
        g.record_play("Lorenz Ambience");
        g.record_play("FM Chaos");

        let snapshot = g.play_counts();

        let mut g2 = PresetGallery::with_builtin_presets();
        g2.restore_play_counts(&snapshot);
        assert_eq!(g2.get("Lorenz Ambience").unwrap().play_count, 2);
        assert_eq!(g2.get("FM Chaos").unwrap().play_count, 1);
    }

    // ── get ───────────────────────────────────────────────────────────────

    #[test]
    fn get_known_name() {
        assert!(gallery().get("Lorenz Ambience").is_some());
    }

    #[test]
    fn get_unknown_name_none() {
        assert!(gallery().get("nonexistent preset xyz").is_none());
    }
}
