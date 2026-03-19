//! Arrangement timeline widget, extracted from ui.rs for maintainability.
//!
//! Renders the horizontal timeline bar showing scenes, morph segments, and a playhead
//! in the ARRANGE tab.

use crate::arrangement::{total_duration, Scene};
use egui::{Align2, Color32, FontId, Pos2, Sense, Stroke, Vec2};

pub(crate) fn draw_arrangement_timeline(ui: &mut egui::Ui, scenes: &[Scene], elapsed: f32) {
    let total = total_duration(scenes);
    if total < 0.001 {
        return;
    }

    // Format seconds as M:SS
    let fmt_time = |secs: f32| -> String {
        let s = secs as u32;
        format!("{}:{:02}", s / 60, s % 60)
    };

    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
    let painter = ui.painter();

    painter.rect_filled(rect, 4.0, Color32::from_rgb(15, 15, 25));

    let colors = [
        Color32::from_rgb(0, 100, 180),
        Color32::from_rgb(0, 140, 80),
        Color32::from_rgb(140, 80, 0),
        Color32::from_rgb(120, 0, 140),
        Color32::from_rgb(0, 120, 120),
        Color32::from_rgb(140, 30, 30),
        Color32::from_rgb(80, 80, 0),
        Color32::from_rgb(40, 80, 140),
    ];

    let mut x = rect.left();
    let active: Vec<usize> = (0..scenes.len()).filter(|&i| scenes[i].active).collect();
    for (ord, &idx) in active.iter().enumerate() {
        let scene = &scenes[idx];
        let col = colors[idx % colors.len()];

        // Morph segment (darker), skip for first scene
        if ord > 0 {
            let w = rect.width() * scene.morph_secs / total;
            let r =
                egui::Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()));
            painter.rect_filled(
                r,
                0.0,
                Color32::from_rgba_premultiplied(col.r() / 3, col.g() / 3, col.b() / 3, 200),
            );
            x += w;
        }

        // Hold segment
        let w = rect.width() * scene.hold_secs / total;
        let r = egui::Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w, rect.height()));
        painter.rect_filled(
            r,
            0.0,
            Color32::from_rgba_premultiplied(col.r(), col.g(), col.b(), 200),
        );

        // Scene name label
        painter.text(
            Pos2::new(x + 4.0, rect.center().y),
            Align2::LEFT_CENTER,
            &scene.name,
            FontId::proportional(10.0),
            Color32::WHITE,
        );

        x += w;
    }

    // Playhead
    if elapsed > 0.0 {
        let px = rect.left() + rect.width() * (elapsed / total).min(1.0);
        painter.line_segment(
            [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
            Stroke::new(2.0, Color32::from_rgb(0, 240, 255)),
        );
    }

    // Border
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(50, 50, 80)));

    // Elapsed / total time label below timeline
    let time_str = format!("{} / {}", fmt_time(elapsed.min(total)), fmt_time(total));
    ui.label(
        egui::RichText::new(time_str)
            .size(10.0)
            .color(Color32::from_rgb(140, 160, 200)),
    );
}
