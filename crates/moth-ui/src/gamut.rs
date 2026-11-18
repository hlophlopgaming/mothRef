// SPDX-License-Identifier: GPL-3.0-or-later
//! Draws a color gamut wheel: hue around the circle, saturation from the gray center
//! outwards. Three views of the same coverage grid (decision 0028):
//! gamut windows, painter's cells, and a mask with hue shares and marked colors.
use egui::{Color32, Mesh, Painter, Pos2, Shape, Stroke, vec2};

/// What the wheel shows. The grid holds the share of pixels per (hue sector,
/// saturation ring), row-major by hue, 0–255.
pub struct Wheel<'a> {
    pub grid: &'a [u8],
    pub hues: usize,
    pub rings: usize,
    /// 0 windows, 1 cells, 2 mask with shares and marks.
    pub view: u8,
    /// Main colors to mark (view 2): color, hue 0–1, saturation 0–1.
    pub marks: &'a [([u8; 3], f32, f32)],
}

/// Below this a grid cell counts as noise rather than a color the image uses.
const PRESENT: u8 = 3;

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(1.) * 6.;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1. - s), v * (1. - s * f), v * (1. - s * (1. - f)));
    match i as u8 % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}
fn color([r, g, b]: [f32; 3], alpha: f32) -> Color32 {
    let c = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    Color32::from_rgba_unmultiplied(c(r), c(g), c(b), c(alpha))
}
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}
/// The wheel's own color at hue `h`, saturation `s`.
fn wheel_color(h: f32, s: f32) -> [f32; 3] {
    hsv(h, s, 0.95)
}
fn point(center: Pos2, radius: f32, h: f32, s: f32) -> Pos2 {
    let angle = (h * 360. - 90.).to_radians();
    center + vec2(angle.cos(), angle.sin()) * s * radius
}
/// From a screen point back to (hue, saturation).
fn polar(center: Pos2, radius: f32, p: Pos2) -> (f32, f32) {
    let d = p - center;
    let h = (d.y.atan2(d.x).to_degrees() + 90.).rem_euclid(360.) / 360.;
    (h, (d.length() / radius).min(1.))
}

impl Wheel<'_> {
    fn cell(&self, hue: isize, ring: isize) -> u8 {
        if ring < 0 || ring >= self.rings as isize {
            return 0;
        }
        let hue = hue.rem_euclid(self.hues as isize) as usize;
        self.grid
            .get(hue * self.rings + ring as usize)
            .copied()
            .unwrap_or(0)
    }
    /// Presence smoothed over the neighbouring cells, 0–1, so masks have soft,
    /// rounded edges instead of following the grid.
    fn smooth(&self) -> Vec<f32> {
        let mut out = vec![0.; self.hues * self.rings];
        for h in 0..self.hues as isize {
            for r in 0..self.rings as isize {
                let mut sum = 0.;
                let mut weight = 0.;
                for dh in -1..=1isize {
                    for dr in -1..=1isize {
                        let w = if dh == 0 && dr == 0 { 2. } else { 1. };
                        sum += w * f32::from(u8::from(self.cell(h + dh, r + dr) >= PRESENT));
                        weight += w;
                    }
                }
                out[h as usize * self.rings + r as usize] = sum / weight;
            }
        }
        out
    }
    /// Smoothed presence at a grid vertex (between cells), for meshes and outlines.
    fn vertex_values(&self, smooth: &[f32]) -> Vec<f32> {
        let (hues, rings) = (self.hues, self.rings);
        let mut out = vec![0.; hues * (rings + 1)];
        for h in 0..hues {
            for r in 0..=rings {
                let mut sum = 0.;
                let mut n = 0.;
                for (dh, dr) in [(0usize, 0usize), (1, 0), (0, 1), (1, 1)] {
                    let hh = (h + hues - dh) % hues;
                    if r < dr || r - dr >= rings {
                        continue;
                    }
                    sum += smooth[hh * rings + r - dr];
                    n += 1.;
                }
                out[h * (rings + 1) + r] = if n > 0. { sum / n } else { 0. };
            }
        }
        out
    }

    /// Paints the wheel into the circle at `center` with `radius`.
    pub fn paint(&self, painter: &Painter, center: Pos2, radius: f32) {
        if self.grid.len() != self.hues * self.rings || self.hues < 3 || self.rings < 2 {
            return;
        }
        match self.view {
            0 => self.windows(painter, center, radius),
            1 => self.cells(painter, center, radius),
            _ => self.shares(painter, center, radius),
        }
    }

    /// A polar mesh over the whole wheel, coloured per vertex.
    fn disc(
        &self,
        painter: &Painter,
        center: Pos2,
        radius: f32,
        tint: impl Fn(f32, f32, f32) -> Color32,
        values: &[f32],
    ) {
        let (hues, rings) = (self.hues, self.rings);
        // Twice as many hue steps as cells keeps the rim round.
        let steps = hues * 2;
        let mut mesh = Mesh::default();
        for k in 0..=steps {
            let h = k as f32 / steps as f32;
            for r in 0..=rings {
                let s = r as f32 / rings as f32;
                let cell = (k / 2) % hues;
                let next = (cell + (k % 2)) % hues;
                let value = (values[cell * (rings + 1) + r] + values[next * (rings + 1) + r]) / 2.;
                mesh.colored_vertex(point(center, radius, h, s), tint(h, s, value));
            }
        }
        let row = (rings + 1) as u32;
        for k in 0..steps as u32 {
            for r in 0..rings as u32 {
                let a = k * row + r;
                let b = (k + 1) * row + r;
                mesh.add_triangle(a, b, a + 1);
                mesh.add_triangle(b, b + 1, a + 1);
            }
        }
        painter.add(Shape::mesh(mesh));
    }

    /// Marching squares at 0.5 over the vertex grid: the outline of the gamut mask.
    fn outline(
        &self,
        painter: &Painter,
        center: Pos2,
        radius: f32,
        values: &[f32],
        stroke: Stroke,
    ) {
        let (hues, rings) = (self.hues, self.rings);
        let at = |h: usize, r: usize| values[(h % hues) * (rings + 1) + r];
        let pos = |h: f32, r: f32| point(center, radius, h / hues as f32, r / rings as f32);
        for h in 0..hues {
            for r in 0..rings {
                let corners = [
                    (h as f32, r as f32, at(h, r)),
                    (h as f32 + 1., r as f32, at(h + 1, r)),
                    (h as f32 + 1., r as f32 + 1., at(h + 1, r + 1)),
                    (h as f32, r as f32 + 1., at(h, r + 1)),
                ];
                let mut cut = Vec::with_capacity(4);
                for i in 0..4 {
                    let (a, b) = (corners[i], corners[(i + 1) % 4]);
                    if (a.2 >= 0.5) != (b.2 >= 0.5) {
                        let t = (0.5 - a.2) / (b.2 - a.2);
                        cut.push(pos(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                    }
                }
                for pair in cut.as_chunks::<2>().0 {
                    painter.line_segment(*pair, stroke);
                }
            }
        }
    }

    /// 5a: the image's color regions as convex "windows" cut into a darkened wheel.
    fn windows(&self, painter: &Painter, center: Pos2, radius: f32) {
        let smooth = self.smooth();
        let flat = vec![0.; self.hues * (self.rings + 1)];
        self.disc(
            painter,
            center,
            radius,
            |h, s, _| {
                let c = wheel_color(h, s);
                let gray = 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
                color(mix([gray; 3], c, 0.25).map(|v| v * 0.25 + 0.27), 1.)
            },
            &flat,
        );
        for hull in self.hulls(&smooth, center, radius) {
            fill_wheel_polygon(painter, center, radius, &hull);
            let mut closed = hull.clone();
            closed.push(hull[0]);
            painter.add(Shape::line(
                closed,
                Stroke::new(radius / 75., Color32::WHITE),
            ));
        }
    }
    /// Convex hulls of the connected regions the image covers.
    fn hulls(&self, smooth: &[f32], center: Pos2, radius: f32) -> Vec<Vec<Pos2>> {
        let (hues, rings) = (self.hues, self.rings);
        let on = |h: usize, r: usize| smooth[h * rings + r] >= 0.5;
        let mut seen = vec![false; hues * rings];
        let mut hulls = Vec::new();
        for start in 0..hues * rings {
            if seen[start] || !on(start / rings, start % rings) {
                continue;
            }
            seen[start] = true;
            let mut stack = vec![start];
            let mut points = Vec::new();
            while let Some(cell) = stack.pop() {
                let (h, r) = (cell / rings, cell % rings);
                for (dh, dr) in [(0., 0.), (1., 0.), (0., 1.), (1., 1.)] {
                    points.push(point(
                        center,
                        radius,
                        (h as f32 + dh) / hues as f32,
                        (r as f32 + dr) / rings as f32,
                    ));
                }
                let neighbours = [
                    ((h + 1) % hues, r),
                    ((h + hues - 1) % hues, r),
                    (h, r.saturating_sub(1)),
                    (h, (r + 1).min(rings - 1)),
                ];
                for (nh, nr) in neighbours {
                    let index = nh * rings + nr;
                    if !seen[index] && on(nh, nr) {
                        seen[index] = true;
                        stack.push(index);
                    }
                }
            }
            let hull = convex_hull(points);
            if hull.len() >= 3 {
                hulls.push(hull);
            }
        }
        hulls
    }

    /// 5d: the painter's wheel of 24 hues and 5 saturations; the cells the image uses
    /// are solid, the rest faint.
    fn cells(&self, painter: &Painter, center: Pos2, radius: f32) {
        const HUES: usize = 24;
        const RINGS: usize = 5;
        let gap = radius / 120.;
        for h in 0..HUES {
            for r in 0..RINGS {
                let (h0, h1) = (h * self.hues / HUES, (h + 1) * self.hues / HUES);
                let (r0, r1) = (r * self.rings / RINGS, (r + 1) * self.rings / RINGS);
                let used = (h0..h1)
                    .flat_map(|hh| (r0..r1).map(move |rr| (hh, rr)))
                    .any(|(hh, rr)| self.cell(hh as isize, rr as isize) >= PRESENT);
                let c = wheel_color(
                    (h as f32 + 0.5) / HUES as f32,
                    (r as f32 + 0.5) / RINGS as f32,
                );
                let fill = if used { color(c, 1.) } else { color(c, 0.16) };
                let inner = (r as f32 / RINGS as f32 * radius + gap).max(gap * 2.);
                let outer = (r + 1) as f32 / RINGS as f32 * radius - gap;
                let a0 = h as f32 / HUES as f32 + gap / (radius * std::f32::consts::TAU);
                let a1 = (h + 1) as f32 / HUES as f32 - gap / (radius * std::f32::consts::TAU);
                let arc = |radius_at: f32, reverse: bool| {
                    let mut points: Vec<Pos2> = (0..=8)
                        .map(|i| {
                            let t = a0 + (a1 - a0) * i as f32 / 8.;
                            point(center, 1., t, radius_at)
                        })
                        .collect();
                    if reverse {
                        points.reverse();
                    }
                    points
                };
                let outside = arc(outer, false);
                let inside = arc(inner, true);
                let mut mesh = Mesh::default();
                for (o, i) in outside.iter().zip(inside.iter().rev()) {
                    mesh.colored_vertex(*o, fill);
                    mesh.colored_vertex(*i, fill);
                }
                for k in 0..8u32 {
                    let a = k * 2;
                    mesh.add_triangle(a, a + 1, a + 2);
                    mesh.add_triangle(a + 1, a + 3, a + 2);
                }
                painter.add(Shape::mesh(mesh));
                if used {
                    let mut border = outside;
                    border.extend(inside);
                    border.push(border[0]);
                    painter.add(Shape::line(
                        border,
                        Stroke::new(radius / 160., Color32::from_black_alpha(140)),
                    ));
                }
            }
        }
    }

    /// 5c + 5e: the gamut mask, the share of each hue as a band around the wheel,
    /// and the main colors marked with their numbers.
    fn shares(&self, painter: &Painter, center: Pos2, radius: f32) {
        let inner = radius * 0.84;
        let smooth = self.smooth();
        let values = self.vertex_values(&smooth);
        self.disc(
            painter,
            center,
            inner,
            |h, s, m| {
                let full = wheel_color(h, s);
                let dim = full.map(|v| v * 0.18 + 0.78);
                let t = ((m - 0.3) / 0.4).clamp(0., 1.);
                color(mix(dim, full, t * t * (3. - 2. * t)), 1.)
            },
            &values,
        );
        self.outline(
            painter,
            center,
            inner,
            &values,
            Stroke::new(radius / 160., Color32::from_gray(30)),
        );
        // Hue shares: grays have no hue, so a cell counts by its saturation squared.
        let mut weights = vec![0f32; self.hues];
        for (h, weight) in weights.iter_mut().enumerate() {
            for r in 0..self.rings {
                let s = (r as f32 + 0.5) / self.rings as f32;
                if s > 0.15 {
                    *weight += f32::from(self.cell(h as isize, r as isize)) * s * s;
                }
            }
        }
        let most = weights.iter().copied().fold(0., f32::max).max(1e-6);
        let band = radius - inner;
        for (h, weight) in weights.iter().enumerate() {
            if *weight < most * 0.04 {
                continue;
            }
            let length = band * 0.25 + band * 0.65 * (weight / most).sqrt();
            let from = inner + band * 0.08;
            let c = color(hsv((h as f32 + 0.5) / self.hues as f32, 0.85, 0.9), 1.);
            let (a0, a1) = (
                h as f32 / self.hues as f32,
                (h + 1) as f32 / self.hues as f32,
            );
            let mut mesh = Mesh::default();
            for i in 0..=3 {
                let t = a0 + (a1 - a0) * i as f32 / 3.;
                mesh.colored_vertex(point(center, 1., t, from), c);
                mesh.colored_vertex(point(center, 1., t, from + length), c);
            }
            for k in 0..3u32 {
                let a = k * 2;
                mesh.add_triangle(a, a + 1, a + 2);
                mesh.add_triangle(a + 1, a + 3, a + 2);
            }
            painter.add(Shape::mesh(mesh));
        }
        let dot = radius / 15.;
        let font = egui::FontId::proportional(dot * 1.15);
        for (number, (rgb, h, s)) in self.marks.iter().enumerate() {
            let at = point(center, inner, *h, *s * 0.96);
            painter.circle(
                at,
                dot,
                Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                Stroke::new(dot / 3.5, Color32::WHITE),
            );
            let label = at + vec2(dot * 1.25, -dot * 0.7);
            painter.text(
                label + vec2(0.6, 0.6),
                egui::Align2::LEFT_TOP,
                (number + 1).to_string(),
                font.clone(),
                Color32::from_white_alpha(200),
            );
            painter.text(
                label,
                egui::Align2::LEFT_TOP,
                (number + 1).to_string(),
                font.clone(),
                Color32::from_gray(25),
            );
        }
    }
}

/// Fills a convex polygon with the wheel's own colors, subdividing a fan from its
/// centroid so the hues bend around the circle instead of blending straight across.
fn fill_wheel_polygon(painter: &Painter, center: Pos2, radius: f32, hull: &[Pos2]) {
    const LEVELS: u32 = 8;
    let middle = hull.iter().fold(vec2(0., 0.), |a, p| a + p.to_vec2()) / hull.len() as f32;
    let middle = middle.to_pos2();
    let mut mesh = Mesh::default();
    for i in 0..hull.len() {
        let (b, c) = (hull[i], hull[(i + 1) % hull.len()]);
        let base = mesh.vertices.len() as u32;
        // Rows of a triangular grid from the centroid to the edge b–c.
        for row in 0..=LEVELS {
            for col in 0..=row {
                let (u, v) = (
                    row as f32 / LEVELS as f32,
                    if row == 0 {
                        0.
                    } else {
                        col as f32 / row as f32
                    },
                );
                let p = middle + ((b + (c - b) * v) - middle) * u;
                let (h, s) = polar(center, radius, p);
                mesh.colored_vertex(p, color(wheel_color(h, s), 1.));
            }
        }
        let index = |row: u32, col: u32| base + row * (row + 1) / 2 + col;
        for row in 0..LEVELS {
            for col in 0..=row {
                mesh.add_triangle(
                    index(row, col),
                    index(row + 1, col),
                    index(row + 1, col + 1),
                );
                if col < row {
                    mesh.add_triangle(
                        index(row, col),
                        index(row + 1, col + 1),
                        index(row, col + 1),
                    );
                }
            }
        }
    }
    painter.add(Shape::mesh(mesh));
}

fn convex_hull(mut points: Vec<Pos2>) -> Vec<Pos2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01);
    if points.len() < 3 {
        return points;
    }
    let cross = |o: Pos2, a: Pos2, b: Pos2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut lower: Vec<Pos2> = Vec::new();
    for &p in &points {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0. {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Pos2> = Vec::new();
    for &p in points.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0. {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}
