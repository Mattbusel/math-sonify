//! Sound-design tips panel, extracted from ui.rs for maintainability.
//!
//! Contains the collapsible tips shown in the Simple panel's "SOUND DESIGN TIPS" section.

use crate::ui::{AMBER, GRAY_HINT};
use egui::{CollapsingHeader, RichText, Ui};

pub(crate) fn draw_tips_content(ui: &mut Ui) {
    let tips: &[(&str, &str)] = &[
        (
            "Hear synchronization emerge",
            "Load 'The Synchronization'. Set coupling K to 0.5 (all noise). Slowly drag K to 3.0. You're watching a mathematical phase transition in real time. That's the Kuramoto model.",
        ),
        (
            "Build a 3-layer ambient piece",
            "In the MIXER tab: Layer 0 = 'Midnight Approach' (pad, reverb 0.8). Layer 1 = 'Glass Harp' (melody, reverb 0.5). Layer 2 = 'Monk's Bell' (rhythm, dry). Three different attractors, one mix.",
        ),
        (
            "Find sounds that exist for 15 seconds",
            "In Simple mode, hit AUTO (Ambient mood). Wait for the morph to begin. The 30-second transition between 'Breathing Galaxy' and 'Collapsing Cathedral' exists exactly once. Hit FREEZE in MIXER to capture it.",
        ),
        (
            "The chaos boundary",
            "Load 'The Butterfly's Aria'. Slowly drag ρ (rho) from 24 to 25. The system crosses the Hopf bifurcation at ρ=24.74. Below: stable spiral. Above: chaos. You can hear the moment the universe becomes unpredictable.",
        ),
        (
            "Type your own mathematics",
            "In the PHYSICS ENGINE section, select 'Custom ODE'. Type: dx/dt = y, dy/dt = -x + y*(1-x*x), dz/dt = 0.5*z. That's the Van der Pol oscillator from scratch. Press Apply and hear it.",
        ),
        (
            "Two attractors in conversation",
            "In COUPLED SYSTEMS (Advanced): Source = Rössler, Target = rho, Strength = 0.6. The Rössler's x-output is now modulating the Lorenz's chaos threshold in real time. This coupling behavior has barely been studied.",
        ),
        (
            "Create evolving textures without touching anything",
            "In Simple mode, enable Evolve. Set Walk speed to 0.08. Leave the app running. In 10 minutes you'll have sounds you couldn't have designed manually — the random walk explores regions of parameter space you'd never find.",
        ),
    ];
    for (title, body) in tips.iter() {
        CollapsingHeader::new(RichText::new(*title).color(AMBER).size(11.0).strong())
            .default_open(false)
            .show(ui, |ui| {
                ui.label(RichText::new(*body).color(GRAY_HINT).size(11.0).italics());
            });
        ui.add_space(2.0);
    }
}
