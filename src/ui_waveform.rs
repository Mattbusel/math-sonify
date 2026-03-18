//! Waveform display and note-map widgets, extracted from ui.rs for maintainability.
//!
//! Contains:
//!   - `draw_waveform`  – oscilloscope + spectrum bar display for the WAVEFORM tab.
//!   - `draw_note_map`  – logarithmic frequency axis display for the NOTE MAP tab.
//!   - `hz_to_note_name` – helper converting a frequency in Hz to a note name string.

use egui::{Align2, Color32, Context, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use parking_lot::Mutex;
use std::sync::Arc;

use crate::ui::hue_to_color;

/// Draw an oscilloscope-style waveform with zoom/scroll controls, RMS meter, and simple DFT
/// spectrum bars. Returns (new_zoom, new_offset).
///
/// - `zoom`: 1.0 = full 2s window, 0.1 = 0.2s window.
/// - `offset`: 0.0 = start of buffer, 1.0 = live end of buffer.
pub(crate) fn draw_waveform(
    ui: &mut Ui,
    waveform: &Arc<Mutex<Vec<f32>>>,
    zoom: f32,
    offset: f32,
    ctx: &Context,
) -> (f32, f32) {
    let mut zoom = zoom.clamp(0.05, 1.0);
    let mut offset = offset.clamp(0.0, 1.0);

    let samples = if let Some(wf) = waveform.try_lock() {
        wf.clone()
    } else {
        return (zoom, offset);
    };

    if samples.len() < 2 {
        let avail = ui.available_size();
        let (response, painter) = ui.allocate_painter(avail, Sense::hover());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Audio starting...",
            FontId::proportional(16.0),
            Color32::from_rgb(80, 80, 120),
        );
        return (zoom, offset);
    }

    // ── #15: zoom / scroll controls ─────────────────────────────────────────
    ui.horizontal(|ui| {
        let window_secs = zoom * 2.0;
        ui.label(
            egui::RichText::new(format!("Zoom: {:.2}s", window_secs))
                .size(11.0)
                .color(Color32::from_rgb(120, 160, 200)),
        );
        ui.add(egui::Slider::new(&mut zoom, 0.05..=1.0).show_value(false));
        ui.add_space(8.0);
        ui.add_enabled(
            zoom < 0.999,
            egui::Slider::new(&mut offset, 0.0..=1.0)
                .show_value(false)
                .text("Scroll"),
        );
    });

    // ── #15: apply zoom / offset to pick the displayed slice ────────────────
    let n = samples.len();
    let window_samples = ((zoom * n as f32) as usize).max(2).min(n);
    let max_start = n.saturating_sub(window_samples);
    // offset=1.0 -> live end; offset=0.0 -> beginning
    let start = ((1.0 - offset) * max_start as f32) as usize;
    let start = start.min(max_start);
    let display_samples = &samples[start..start + window_samples];

    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;

    // ── #15: scroll-wheel zoom when waveform is hovered ─────────────────────
    if response.hovered() {
        let scroll_y = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_y.abs() > 0.5 {
            zoom = (zoom - scroll_y * 0.001).clamp(0.05, 1.0);
        }
    }

    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let cy = rect.center().y;

    // Subtle grid lines
    for i in 1..4 {
        let frac = i as f32 / 4.0;
        let gy = rect.top() + frac * rect.height();
        painter.line_segment(
            [Pos2::new(rect.left(), gy), Pos2::new(rect.right(), gy)],
            Stroke::new(0.5, Color32::from_rgba_premultiplied(30, 45, 70, 60)),
        );
    }
    // Zero line
    painter.line_segment(
        [Pos2::new(rect.left(), cy), Pos2::new(rect.right(), cy)],
        Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 80, 130, 120)),
    );

    let ds_len = display_samples.len();
    let w = rect.width();

    let pts: Vec<Pos2> = display_samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let x = rect.left() + (i as f32 / ds_len as f32) * w;
            let y = cy - s.clamp(-1.0, 1.0) * (rect.height() * 0.42);
            Pos2::new(x, y)
        })
        .collect();

    // Glow pass (wide, dim)
    for seg in pts.windows(2) {
        painter.line_segment(
            [seg[0], seg[1]],
            Stroke::new(4.0, Color32::from_rgba_premultiplied(0, 200, 100, 30)),
        );
    }
    // Core pass
    let neon_green = Color32::from_rgb(0, 245, 115);
    for seg in pts.windows(2) {
        painter.line_segment([seg[0], seg[1]], Stroke::new(1.5, neon_green));
    }

    let rms = (display_samples.iter().map(|&s| s * s).sum::<f32>() / ds_len as f32).sqrt();
    let bar_h = (rms * rect.height()).clamp(0.0, rect.height());
    let bar_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 12.0, rect.bottom() - bar_h),
        Vec2::new(10.0, bar_h),
    );
    painter.rect_filled(bar_rect, 0.0, Color32::from_rgb(0, 200, 80));
    painter.text(
        Pos2::new(rect.right() - 6.0, rect.bottom() - bar_h - 4.0),
        Align2::CENTER_BOTTOM,
        format!("{:.2}", rms),
        FontId::proportional(10.0),
        Color32::from_rgb(0, 220, 100),
    );

    if samples.len() >= 64 {
        let n_bins = 32usize;
        let n_dft = display_samples.len().min(2048);
        let bin_h_max = 60.0f32;
        let spec_y = rect.bottom() - bin_h_max - 10.0;

        painter.text(
            Pos2::new(rect.left() + 4.0, spec_y - 4.0),
            Align2::LEFT_BOTTOM,
            "Spectrum",
            FontId::proportional(10.0),
            Color32::from_rgb(100, 120, 160),
        );

        for bin in 0..n_bins {
            let freq_bin = bin + 1;
            let mut re = 0.0f32;
            let mut im = 0.0f32;
            for (k, &s) in display_samples.iter().take(n_dft).enumerate() {
                let angle = -2.0 * std::f32::consts::PI * freq_bin as f32 * k as f32 / n_dft as f32;
                re += s * angle.cos();
                im += s * angle.sin();
            }
            let mag = ((re * re + im * im) / n_dft as f32).sqrt();
            let bar_h2 = (mag * 200.0).clamp(0.0, bin_h_max);
            let bar_w = (rect.width() / n_bins as f32) - 1.0;
            let bx = rect.left() + bin as f32 * (rect.width() / n_bins as f32);
            let bar_rect2 = Rect::from_min_size(
                Pos2::new(bx, spec_y + bin_h_max - bar_h2),
                Vec2::new(bar_w.max(1.0), bar_h2),
            );
            painter.rect_filled(
                bar_rect2,
                0.0,
                hue_to_color(bin as f32 / n_bins as f32, 0.8),
            );
        }
    }

    (zoom, offset)
}

/// Convert a frequency in Hz to a note name string (e.g. "A4", "C#3").
pub(crate) fn hz_to_note_name(hz: f32) -> String {
    if hz < 16.0 {
        return "---".into();
    }
    let note_names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let semitones_from_a4 = 12.0 * (hz / 440.0).log2();
    let midi = (69.0 + semitones_from_a4).round() as i32;
    let octave = (midi / 12) - 1;
    let note = ((midi % 12 + 12) % 12) as usize;
    format!("{}{}", note_names[note], octave)
}

/// Draw the note map: a logarithmic frequency axis showing the active voices and chord tones.
pub(crate) fn draw_note_map(
    ui: &mut Ui,
    freqs: &[f32; 4],
    voice_levels: &[f32; 4],
    chord_intervals: &[f32; 3],
) {
    let avail = ui.available_size();
    let (response, painter) = ui.allocate_painter(avail, Sense::hover());
    let rect = response.rect;

    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 8, 18));

    let voice_colors = [
        Color32::from_rgb(0, 180, 255),
        Color32::from_rgb(0, 255, 140),
        Color32::from_rgb(255, 200, 0),
        Color32::from_rgb(255, 80, 80),
    ];
    let chord_color = Color32::from_rgb(200, 100, 255);

    let freq_min = 50.0f32.ln();
    let freq_max = 4000.0f32.ln();
    let freq_to_x = |f: f32| {
        let ln_f = f.max(20.0).ln();
        rect.left() + ((ln_f - freq_min) / (freq_max - freq_min)) * rect.width()
    };

    for &lf in &[55.0f32, 110.0, 220.0, 440.0, 880.0, 1760.0, 3520.0] {
        let x = freq_to_x(lf);
        if x >= rect.left() && x <= rect.right() {
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                Stroke::new(1.0, Color32::from_rgba_premultiplied(50, 50, 80, 80)),
            );
            painter.text(
                Pos2::new(x, rect.bottom() - 4.0),
                Align2::CENTER_BOTTOM,
                format!("{:.0}Hz", lf),
                FontId::proportional(9.0),
                Color32::from_rgb(80, 80, 120),
            );
        }
    }

    let bar_h = 28.0f32;
    let spacing = 36.0f32;
    let y_start = rect.top() + 20.0;

    for (i, (&freq, &level)) in freqs.iter().zip(voice_levels.iter()).enumerate() {
        if freq < 16.0 {
            continue;
        }
        let x = freq_to_x(freq);
        let y = y_start + i as f32 * spacing;
        let bar_w = (level * 80.0).max(4.0);

        let bar_rect =
            Rect::from_min_size(Pos2::new(x - bar_w * 0.5, y), Vec2::new(bar_w, bar_h * 0.6));
        painter.rect_filled(bar_rect, 3.0, voice_colors[i]);

        painter.text(
            Pos2::new(x, y - 2.0),
            Align2::CENTER_BOTTOM,
            hz_to_note_name(freq),
            FontId::proportional(11.0),
            voice_colors[i],
        );

        if i == 0 {
            for (k, &interval) in chord_intervals.iter().enumerate() {
                if interval.abs() < 0.001 {
                    continue;
                }
                let chord_freq = freq * 2.0f32.powf(interval / 12.0);
                let cx = freq_to_x(chord_freq);
                let cy = y - (k as f32 + 1.0) * (bar_h * 0.5 + 4.0);
                let chord_bar =
                    Rect::from_min_size(Pos2::new(cx - 20.0, cy), Vec2::new(40.0, bar_h * 0.4));
                painter.rect_filled(chord_bar, 2.0, chord_color);
                painter.text(
                    Pos2::new(cx, cy - 2.0),
                    Align2::CENTER_BOTTOM,
                    hz_to_note_name(chord_freq),
                    FontId::proportional(10.0),
                    chord_color,
                );
            }
        }
    }

    painter.text(
        rect.left_top() + Vec2::new(8.0, 4.0),
        Align2::LEFT_TOP,
        "Note Map (log frequency axis)",
        FontId::proportional(11.0),
        Color32::from_rgb(100, 110, 150),
    );
}
