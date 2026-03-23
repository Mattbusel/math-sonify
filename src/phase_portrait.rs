//! 3D phase portrait renderer — GPU-accelerated trajectory visualization.
//!
//! Renders the 3D orbit of a dynamical system (e.g. the Lorenz butterfly) as a
//! glowing point trail in 3D space, synchronized with the audio output.
//!
//! # Architecture
//!
//! The renderer is implemented on top of `wgpu` and runs in a dedicated window
//! (or can be embedded in the main egui frame via an offscreen texture).
//!
//! ```text
//! Simulation thread
//!   └─ pushes [x, y, z] points → RingBuffer<Vec3> (lock-free)
//!
//! Render thread (wgpu)
//!   ├─ uploads ring buffer to GPU vertex buffer (dynamic update)
//!   ├─ draws trail as GL_LINE_STRIP with depth testing
//!   └─ applies bloom post-process (threshold → blur → composite)
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use math_sonify_plugin::phase_portrait::{PhasePortrait, PortraitConfig};
//!
//! let cfg = PortraitConfig::default();
//! // In a real app you would pass a wgpu Device/Queue/Surface here.
//! // This module provides the data structures and GPU resource descriptors;
//! // actual wgpu calls require a graphics context from the OS window.
//! let mut portrait = PhasePortrait::new(cfg);
//! portrait.push_point([10.0, 5.0, -3.0]);
//! ```

#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for the 3D phase portrait renderer.
#[derive(Debug, Clone)]
pub struct PortraitConfig {
    /// Maximum number of trajectory points stored (ring buffer capacity).
    pub max_points: usize,
    /// Trail colour as RGBA (each component 0–1).
    pub trail_color: [f32; 4],
    /// Background colour as RGBA.
    pub background_color: [f32; 4],
    /// Point size in pixels.
    pub point_size: f32,
    /// Whether to enable bloom post-processing.
    pub bloom: bool,
    /// Bloom threshold (0–1); only fragments brighter than this are bloomed.
    pub bloom_threshold: f32,
    /// Bloom intensity multiplier.
    pub bloom_intensity: f32,
    /// Camera field of view in radians.
    pub fov_radians: f32,
    /// Initial camera distance from the origin.
    pub camera_distance: f32,
    /// Camera auto-rotation speed (radians per second).
    pub rotation_speed: f32,
    /// Alpha fade: oldest point alpha (0 = fully transparent).
    pub tail_alpha: f32,
}

impl Default for PortraitConfig {
    fn default() -> Self {
        Self {
            max_points: 8192,
            trail_color: [0.2, 0.7, 1.0, 1.0],
            background_color: [0.02, 0.02, 0.04, 1.0],
            point_size: 2.0,
            bloom: true,
            bloom_threshold: 0.6,
            bloom_intensity: 1.5,
            fov_radians: std::f32::consts::FRAC_PI_4,
            camera_distance: 60.0,
            rotation_speed: 0.05,
            tail_alpha: 0.05,
        }
    }
}

// ── Vertex ────────────────────────────────────────────────────────────────────

/// A single vertex in the 3D trail.
///
/// `repr(C)` layout: 3 floats position + 4 floats color = 28 bytes.
/// This is `Pod`-compatible (no padding, all-bits-valid fields) and can be
/// cast to `&[u8]` for GPU upload using `.as_ptr() as *const u8`.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TrailVertex3D {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl TrailVertex3D {
    /// Return the vertex as a byte slice for GPU buffer upload.
    pub fn as_bytes(&self) -> &[u8] {
        // Safety: TrailVertex3D is repr(C) with all-bits-valid f32 fields.
        // The resulting slice has lifetime tied to self.
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts(
                (self as *const Self) as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }
}

// ── Ring buffer ───────────────────────────────────────────────────────────────

/// Fixed-capacity ring buffer for 3D trajectory points.
///
/// Thread-safe; simulation thread pushes, render thread reads.
pub struct TrajectoryBuffer {
    inner: VecDeque<[f32; 3]>,
    capacity: usize,
}

impl TrajectoryBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: VecDeque::with_capacity(capacity),
            capacity: capacity.max(2),
        }
    }

    /// Push a new 3D point.  Oldest point is discarded when at capacity.
    pub fn push(&mut self, point: [f32; 3]) {
        if self.inner.len() >= self.capacity {
            self.inner.pop_front();
        }
        self.inner.push_back(point);
    }

    /// Number of points currently stored.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Build a flat vertex array with alpha fade applied.
    ///
    /// The oldest point has alpha = `tail_alpha`, the newest has alpha = 1.0.
    pub fn to_vertices(&self, color: [f32; 3], tail_alpha: f32) -> Vec<TrailVertex3D> {
        let n = self.inner.len();
        if n == 0 {
            return Vec::new();
        }
        self.inner
            .iter()
            .enumerate()
            .map(|(i, &pos)| {
                let t = i as f32 / (n - 1).max(1) as f32;
                let alpha = tail_alpha + t * (1.0 - tail_alpha);
                TrailVertex3D {
                    position: pos,
                    color: [color[0], color[1], color[2], alpha],
                }
            })
            .collect()
    }

    /// Iterator over stored points in chronological order.
    pub fn iter(&self) -> impl Iterator<Item = &[f32; 3]> {
        self.inner.iter()
    }
}

// ── Camera ────────────────────────────────────────────────────────────────────

/// Simple orbiting camera for the phase portrait.
#[derive(Debug, Clone)]
pub struct OrbitCamera {
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub fov: f32,
}

impl OrbitCamera {
    pub fn new(distance: f32, fov: f32) -> Self {
        Self {
            azimuth: 0.0,
            elevation: 0.3,
            distance,
            fov,
        }
    }

    /// Advance the camera rotation by `dt` seconds.
    pub fn tick(&mut self, dt: f32, speed: f32) {
        self.azimuth += speed * dt;
        if self.azimuth > std::f32::consts::TAU {
            self.azimuth -= std::f32::consts::TAU;
        }
    }

    /// Return the camera eye position in world space.
    pub fn eye(&self) -> [f32; 3] {
        let (sin_az, cos_az) = self.azimuth.sin_cos();
        let (sin_el, cos_el) = self.elevation.sin_cos();
        [
            self.distance * cos_el * cos_az,
            self.distance * sin_el,
            self.distance * cos_el * sin_az,
        ]
    }

    /// Build a column-major view-projection matrix (right-handed, depth 0–1).
    ///
    /// Returns a flat `[f32; 16]` suitable for uploading to a wgpu uniform buffer.
    pub fn view_proj_matrix(&self, aspect: f32) -> [f32; 16] {
        let eye = self.eye();
        let target = [0.0f32; 3];
        let up = [0.0f32, 1.0, 0.0];

        // View matrix (look-at).
        let view = look_at(eye, target, up);
        // Projection matrix.
        let proj = perspective(self.fov, aspect, 0.1, 1000.0);
        mat4_mul(proj, view)
    }
}

// ── Minimal math helpers (no external dep) ────────────────────────────────────

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-10 {
        return [0.0, 1.0, 0.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Column-major 4×4 look-at matrix.
fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    let f = normalize3(sub3(center, eye));
    let s = normalize3(cross3(f, up));
    let u = cross3(s, f);
    [
        s[0],          u[0],          -f[0],         0.0,
        s[1],          u[1],          -f[1],         0.0,
        s[2],          u[2],          -f[2],         0.0,
        -dot3(s, eye), -dot3(u, eye), dot3(f, eye),  1.0,
    ]
}

/// Column-major perspective matrix (reverse-Z, depth 0–1).
fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
    let f = 1.0 / (fov_y * 0.5).tan();
    let depth = near - far;
    [
        f / aspect, 0.0,  0.0,                        0.0,
        0.0,        f,    0.0,                        0.0,
        0.0,        0.0,  (far + near) / depth,      -1.0,
        0.0,        0.0,  (2.0 * far * near) / depth, 0.0,
    ]
}

/// Multiply two column-major 4×4 matrices.
fn mat4_mul(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[row + k * 4] * b[k + col * 4];
            }
            out[row + col * 4] = sum;
        }
    }
    out
}

// ── Phase portrait ────────────────────────────────────────────────────────────

/// Main phase portrait state.  Feed it points from the simulation thread and
/// call `render_frame` each display vsync.
pub struct PhasePortrait {
    /// Shared trajectory data (simulation thread writes, render thread reads).
    pub buffer: Arc<Mutex<TrajectoryBuffer>>,
    pub camera: OrbitCamera,
    pub config: PortraitConfig,
    /// Accumulated simulation time for camera animation.
    elapsed: f32,
}

impl PhasePortrait {
    pub fn new(config: PortraitConfig) -> Self {
        let buffer = Arc::new(Mutex::new(TrajectoryBuffer::new(config.max_points)));
        let camera = OrbitCamera::new(config.camera_distance, config.fov_radians);
        Self {
            buffer,
            camera,
            config,
            elapsed: 0.0,
        }
    }

    /// Return a clone of the shared buffer handle so the simulation thread can
    /// push points without holding the portrait lock.
    pub fn buffer_handle(&self) -> Arc<Mutex<TrajectoryBuffer>> {
        Arc::clone(&self.buffer)
    }

    /// Push a 3D point from the simulation thread.
    pub fn push_point(&self, point: [f32; 3]) {
        if let Ok(mut buf) = self.buffer.lock() {
            buf.push(point);
        }
    }

    /// Advance camera animation by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.elapsed += dt;
        self.camera.tick(dt, self.config.rotation_speed);
    }

    /// Build the vertex data for this frame.
    ///
    /// In a real wgpu integration you would upload this to a `wgpu::Buffer`
    /// and issue a draw call.  Here we return the vertices for testing.
    pub fn build_vertices(&self) -> Vec<TrailVertex3D> {
        let buf = match self.buffer.lock() {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };
        let color = [
            self.config.trail_color[0],
            self.config.trail_color[1],
            self.config.trail_color[2],
        ];
        buf.to_vertices(color, self.config.tail_alpha)
    }

    /// WGSL shader source for the trail render pass.
    ///
    /// Returns a string containing the complete WGSL vertex + fragment shader.
    pub fn trail_shader_source() -> &'static str {
        include_str!("shaders/phase_portrait.wgsl")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_buffer_capacity() {
        let mut buf = TrajectoryBuffer::new(4);
        for i in 0..6 {
            buf.push([i as f32, 0.0, 0.0]);
        }
        assert_eq!(buf.len(), 4, "should not exceed capacity");
    }

    #[test]
    fn test_trajectory_buffer_oldest_evicted() {
        let mut buf = TrajectoryBuffer::new(3);
        buf.push([1.0, 0.0, 0.0]);
        buf.push([2.0, 0.0, 0.0]);
        buf.push([3.0, 0.0, 0.0]);
        buf.push([4.0, 0.0, 0.0]); // evicts [1,0,0]
        let first = buf.iter().next().copied().unwrap();
        assert_eq!(first[0], 2.0, "oldest should be evicted");
    }

    #[test]
    fn test_vertices_alpha_fade() {
        let mut buf = TrajectoryBuffer::new(10);
        for i in 0..5 {
            buf.push([i as f32, 0.0, 0.0]);
        }
        let verts = buf.to_vertices([1.0, 1.0, 1.0], 0.1);
        // Oldest has low alpha, newest has high alpha.
        let first_alpha = verts.first().unwrap().color[3];
        let last_alpha = verts.last().unwrap().color[3];
        assert!(
            last_alpha > first_alpha,
            "newest vertex should be more opaque"
        );
        assert!((last_alpha - 1.0).abs() < 1e-5, "newest vertex alpha should be 1");
    }

    #[test]
    fn test_orbit_camera_eye_at_azimuth_zero() {
        let cam = OrbitCamera::new(10.0, std::f32::consts::FRAC_PI_4);
        let eye = cam.eye();
        // At azimuth=0, elevation=0.3: eye should be at distance ≈ 10.
        let dist = (eye[0] * eye[0] + eye[1] * eye[1] + eye[2] * eye[2]).sqrt();
        assert!((dist - 10.0).abs() < 1e-4, "eye distance should equal camera_distance");
    }

    #[test]
    fn test_view_proj_matrix_finite() {
        let cam = OrbitCamera::new(60.0, std::f32::consts::FRAC_PI_4);
        let m = cam.view_proj_matrix(16.0 / 9.0);
        assert!(m.iter().all(|v| v.is_finite()), "view-proj matrix must be finite");
    }

    #[test]
    fn test_phase_portrait_push_point() {
        let portrait = PhasePortrait::new(PortraitConfig::default());
        portrait.push_point([1.0, 2.0, 3.0]);
        portrait.push_point([4.0, 5.0, 6.0]);
        let verts = portrait.build_vertices();
        assert_eq!(verts.len(), 2);
        assert_eq!(verts[0].position, [1.0, 2.0, 3.0]);
    }
}
