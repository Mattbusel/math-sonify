// Bifurcation parameter sweep module.
//
// Runs a dynamical system across a range of parameter values, records the
// steady-state attractors at each step, exports an SVG bifurcation diagram,
// and writes a per-step WAV audio render concatenated into a single sweep file.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::systems::{DynamicalSystem, Lorenz};

/// Configuration for a bifurcation parameter sweep.
#[derive(Debug, Clone)]
pub struct BifurcationConfig {
    /// Name of the parameter being swept (e.g., "rho", "sigma").
    pub parameter_name: String,
    /// Start value of the swept parameter.
    pub range_start: f64,
    /// End value of the swept parameter.
    pub range_end: f64,
    /// Number of discrete parameter steps.
    pub steps: usize,
    /// Audio duration in milliseconds rendered per step.
    pub duration_per_step_ms: u64,
}

impl Default for BifurcationConfig {
    fn default() -> Self {
        Self {
            parameter_name: "rho".into(),
            range_start: 0.5,
            range_end: 30.0,
            steps: 200,
            duration_per_step_ms: 300,
        }
    }
}

/// Output of a bifurcation sweep.
pub struct BifurcationResult {
    /// Parameter values at each step.
    pub parameter_values: Vec<f64>,
    /// Attractor points sampled in steady state at each step.
    /// Each entry is a Vec of [x, y, z] triplets.
    pub attractor_points: Vec<Vec<[f64; 3]>>,
    /// Path to the rendered audio file (concatenated WAV).
    pub audio_path: PathBuf,
}

// ---------------------------------------------------------------------------
// BifurcationSweeper
// ---------------------------------------------------------------------------

/// Performs bifurcation parameter sweeps over any [`DynamicalSystem`].
pub struct BifurcationSweeper;

impl BifurcationSweeper {
    /// Sweep a parameter range and collect steady-state attractor samples.
    ///
    /// This runs entirely on the calling thread.  For GUI use, call from a
    /// background thread and poll a shared flag for progress.
    ///
    /// # Errors
    /// Returns an error if the output WAV file cannot be created.
    pub fn sweep(
        config: &BifurcationConfig,
        base_cfg: &Config,
        output_dir: &Path,
    ) -> anyhow::Result<BifurcationResult> {
        use anyhow::Context as _;

        if config.steps == 0 {
            anyhow::bail!("BifurcationConfig.steps must be > 0");
        }
        if config.range_end <= config.range_start {
            anyhow::bail!("range_end must be greater than range_start");
        }

        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("creating output directory {}", output_dir.display()))?;

        let step_count = config.steps;
        let mut parameter_values: Vec<f64> = Vec::with_capacity(step_count);
        let mut attractor_points: Vec<Vec<[f64; 3]>> = Vec::with_capacity(step_count);

        // Collect samples for WAV concatenation (mono f32 at 44100 Hz).
        let sample_rate: u32 = 44100;
        let samples_per_step =
            (sample_rate as u64 * config.duration_per_step_ms / 1000).max(1) as usize;
        let mut all_samples: Vec<f32> = Vec::with_capacity(step_count * samples_per_step);

        for step_idx in 0..step_count {
            let t = step_idx as f64 / (step_count - 1).max(1) as f64;
            let param_val = config.range_start + t * (config.range_end - config.range_start);
            parameter_values.push(param_val);

            // Build a Lorenz system with the swept parameter applied.
            let mut system = build_swept_system(base_cfg, &config.parameter_name, param_val);

            // Warm-up: discard transient (500 iterations at dt=0.01).
            let warmup_steps = 500_usize;
            for _ in 0..warmup_steps {
                system.step(0.01);
            }

            // Collect steady-state samples.
            let collect_steps = 200_usize;
            let mut points: Vec<[f64; 3]> = Vec::with_capacity(collect_steps);
            for _ in 0..collect_steps {
                system.step(0.01);
                let s = system.state();
                let x = s.first().copied().unwrap_or(0.0);
                let y = s.get(1).copied().unwrap_or(0.0);
                let z = s.get(2).copied().unwrap_or(0.0);
                points.push([x, y, z]);
            }
            attractor_points.push(points.clone());

            // Audio render: map z variable to amplitude and frequency.
            // z range for Lorenz is roughly [0, 60]; normalize to [0, 1].
            let z_max = 60.0_f64;
            for i in 0..samples_per_step {
                let phase_t = i as f64 / sample_rate as f64;
                // Vary frequency with parameter value to create a sweeping tone.
                let freq = 110.0 * (param_val / config.range_start).max(0.01).ln().abs().max(1.0);
                // Use averaged z position as amplitude modulator.
                let avg_z: f64 = points.iter().map(|p| p[2]).sum::<f64>() / points.len().max(1) as f64;
                let amp = (avg_z / z_max).clamp(0.0, 1.0) as f32 * 0.5;
                let sample = amp * (2.0 * std::f64::consts::PI * freq * phase_t).sin() as f32;
                all_samples.push(sample);
            }
        }

        // Write WAV.
        let audio_filename = format!(
            "bifurcation_sweep_{}.wav",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        let audio_path = output_dir.join(&audio_filename);
        write_wav_mono_f32(&audio_path, &all_samples, sample_rate)
            .with_context(|| format!("writing bifurcation sweep WAV to {}", audio_path.display()))?;

        // Write SVG diagram.
        let svg_path = output_dir.join(format!(
            "bifurcation_{}.svg",
            config.parameter_name
        ));
        if let Err(e) = write_bifurcation_svg(
            &svg_path,
            &parameter_values,
            &attractor_points,
            &config.parameter_name,
        ) {
            tracing::warn!("Could not write bifurcation SVG: {e}");
        }

        Ok(BifurcationResult {
            parameter_values,
            attractor_points,
            audio_path,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a Lorenz system (default) with one parameter overridden.
///
/// Only "rho", "sigma", and "beta" are recognized for the built-in Lorenz.
/// For other systems or parameter names the default Lorenz is returned unchanged.
fn build_swept_system(base: &Config, param: &str, value: f64) -> Lorenz {
    let mut sigma = base.lorenz.sigma;
    let mut rho = base.lorenz.rho;
    let mut beta = base.lorenz.beta;
    match param {
        "rho" => rho = value,
        "sigma" => sigma = value,
        "beta" => beta = value,
        _ => {}
    }
    Lorenz::new(sigma, rho, beta)
}

/// Write a mono f32 WAV file using hound.
fn write_wav_mono_f32(path: &Path, samples: &[f32], sample_rate: u32) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Write a minimal SVG bifurcation diagram (attractors vs parameter).
fn write_bifurcation_svg(
    path: &Path,
    parameter_values: &[f64],
    attractor_points: &[Vec<[f64; 3]>],
    param_name: &str,
) -> anyhow::Result<()> {
    let width = 800_f64;
    let height = 500_f64;
    let margin_l = 60.0_f64;
    let margin_r = 20.0_f64;
    let margin_t = 30.0_f64;
    let margin_b = 50.0_f64;

    let plot_w = width - margin_l - margin_r;
    let plot_h = height - margin_t - margin_b;

    // Determine z-axis bounds across all attractor points.
    let mut z_min = f64::MAX;
    let mut z_max = f64::MIN;
    for pts in attractor_points {
        for p in pts {
            if p[2] < z_min { z_min = p[2]; }
            if p[2] > z_max { z_max = p[2]; }
        }
    }
    if (z_max - z_min).abs() < 1e-12 {
        z_min -= 1.0;
        z_max += 1.0;
    }

    let p_min = parameter_values.first().copied().unwrap_or(0.0);
    let p_max = parameter_values.last().copied().unwrap_or(1.0);
    let p_range = (p_max - p_min).max(1e-12);

    let mut file = std::fs::File::create(path)?;

    // Build SVG content using a String buffer to avoid writeln! format conflicts
    // with SVG attribute syntax (hex colors, quotes).
    let mut svg = String::with_capacity(4096);
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\">\n",
        width, height
    ));
    svg.push_str(&format!(
        "<rect width=\"{}\" height=\"{}\" fill=\"#0d1117\"/>\n",
        width, height
    ));
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"20\" fill=\"#8b949e\" font-size=\"13\" \
         font-family=\"monospace\">math-sonify bifurcation -- z vs {}</text>\n",
        margin_l, param_name
    ));

    // Axis labels.
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" fill=\"#8b949e\" font-size=\"11\" \
         font-family=\"monospace\">{:.2}</text>\n",
        margin_l - 5.0,
        margin_t + plot_h,
        z_min
    ));
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" fill=\"#8b949e\" font-size=\"11\" \
         font-family=\"monospace\">{:.2}</text>\n",
        margin_l - 5.0,
        margin_t,
        z_max
    ));

    // Plot points.
    for (pi, pts) in attractor_points.iter().enumerate() {
        let param = parameter_values.get(pi).copied().unwrap_or(0.0);
        let px = margin_l + (param - p_min) / p_range * plot_w;
        for p in pts {
            let z = p[2];
            let py = margin_t + plot_h - (z - z_min) / (z_max - z_min) * plot_h;
            svg.push_str(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"0.8\" fill=\"#58a6ff\" opacity=\"0.5\"/>\n",
                px,
                py.clamp(margin_t, margin_t + plot_h)
            ));
        }
    }

    // Axis lines.
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#30363d\" stroke-width=\"1\"/>\n",
        margin_l,
        margin_t + plot_h,
        margin_l + plot_w,
        margin_t + plot_h
    ));
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#30363d\" stroke-width=\"1\"/>\n",
        margin_l,
        margin_t,
        margin_l,
        margin_t + plot_h
    ));
    svg.push_str("</svg>\n");

    use std::io::Write as IoWrite;
    file.write_all(svg.as_bytes())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_default_config_fields() {
        let cfg = BifurcationConfig::default();
        assert_eq!(cfg.parameter_name, "rho");
        assert!(cfg.range_start < cfg.range_end);
        assert!(cfg.steps > 0);
    }

    #[test]
    fn test_sweep_produces_correct_step_count() {
        let cfg = BifurcationConfig {
            parameter_name: "rho".into(),
            range_start: 10.0,
            range_end: 30.0,
            steps: 10,
            duration_per_step_ms: 10,
        };
        let base = Config::default();
        let dir = std::env::temp_dir().join("bifurc_test");
        let result = BifurcationSweeper::sweep(&cfg, &base, &dir);
        assert!(result.is_ok(), "sweep should succeed: {:?}", result.err());
        let r = result.unwrap();
        assert_eq!(r.parameter_values.len(), 10);
        assert_eq!(r.attractor_points.len(), 10);
    }

    #[test]
    fn test_sweep_parameter_values_are_monotonic() {
        let cfg = BifurcationConfig {
            parameter_name: "rho".into(),
            range_start: 5.0,
            range_end: 28.0,
            steps: 5,
            duration_per_step_ms: 10,
        };
        let base = Config::default();
        let dir = std::env::temp_dir().join("bifurc_monotone_test");
        let result = BifurcationSweeper::sweep(&cfg, &base, &dir).unwrap();
        let vals = &result.parameter_values;
        for i in 1..vals.len() {
            assert!(
                vals[i] > vals[i - 1],
                "parameter_values should be strictly increasing"
            );
        }
    }

    #[test]
    fn test_sweep_zero_steps_returns_error() {
        let cfg = BifurcationConfig {
            steps: 0,
            ..BifurcationConfig::default()
        };
        let base = Config::default();
        let dir = std::env::temp_dir().join("bifurc_zero_steps");
        assert!(BifurcationSweeper::sweep(&cfg, &base, &dir).is_err());
    }

    #[test]
    fn test_sweep_inverted_range_returns_error() {
        let cfg = BifurcationConfig {
            range_start: 30.0,
            range_end: 5.0,
            steps: 10,
            ..BifurcationConfig::default()
        };
        let base = Config::default();
        let dir = std::env::temp_dir().join("bifurc_inverted");
        assert!(BifurcationSweeper::sweep(&cfg, &base, &dir).is_err());
    }

    #[test]
    fn test_sweep_audio_file_created() {
        let cfg = BifurcationConfig {
            steps: 3,
            duration_per_step_ms: 20,
            range_start: 10.0,
            range_end: 28.0,
            ..BifurcationConfig::default()
        };
        let base = Config::default();
        let dir = std::env::temp_dir().join("bifurc_audio_test");
        let result = BifurcationSweeper::sweep(&cfg, &base, &dir).unwrap();
        assert!(result.audio_path.exists(), "audio WAV should be created");
    }
}
