//! Poisson disk sampling via Bridson's algorithm (2007).
//! Generates spatially-distributed points where no two are closer than a minimum distance.
//! Zero-dependency, deterministic implementation suitable for procedural world generation.

/// Seeded, stateless pseudo-random number generator using xorshift64.
struct Rng(u64);

impl Rng {
    /// Construct a new RNG with the given seed. Seed 0 is replaced with a default pattern.
    fn new(seed: u64) -> Self {
        Rng(if seed == 0 { 0xcafe_babe_dead_beef } else { seed })
    }

    /// Advance the RNG state and return the next `u64`.
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Return a uniform random `f64` in [0, 1).
    fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Poisson disk sampling via Bridson's algorithm.
/// Returns points within `width` × `height` where no two points are closer than `min_dist`.
/// `k` = rejection attempts per active point (30 is typical).
/// Uses a deterministic seeded RNG for reproducible layouts.
pub fn poisson_disk(width: f64, height: f64, min_dist: f64, seed: u64, k: u32) -> Vec<(f64, f64)> {
    let mut rng = Rng::new(seed);
    let cell = min_dist / std::f64::consts::SQRT_2;
    let cols = (width / cell).ceil() as usize + 1;
    let rows = (height / cell).ceil() as usize + 1;
    let mut grid: Vec<Option<usize>> = vec![None; cols * rows];
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut active: Vec<usize> = Vec::new();

    // Seed with first point
    let first = (rng.f64() * width, rng.f64() * height);
    grid[(first.1 / cell) as usize * cols + (first.0 / cell) as usize] = Some(0);
    pts.push(first);
    active.push(0);

    while !active.is_empty() {
        let ai = (rng.f64() * active.len() as f64) as usize;
        let base = pts[active[ai]];
        let mut found = false;

        for _ in 0..k {
            let theta = rng.f64() * std::f64::consts::TAU;
            let r = min_dist * (1.0 + rng.f64()); // annulus [min_dist, 2*min_dist]
            let nx = base.0 + theta.cos() * r;
            let ny = base.1 + theta.sin() * r;
            if nx < 0.0 || nx >= width || ny < 0.0 || ny >= height {
                continue;
            }

            let cgx = (nx / cell) as usize;
            let cgy = (ny / cell) as usize;
            let x0 = cgx.saturating_sub(2);
            let x1 = (cgx + 3).min(cols);
            let y0 = cgy.saturating_sub(2);
            let y1 = (cgy + 3).min(rows);
            let mut ok = true;
            'check: for cy in y0..y1 {
                for cx in x0..x1 {
                    if let Some(pi) = grid[cy * cols + cx] {
                        let dx = pts[pi].0 - nx;
                        let dy = pts[pi].1 - ny;
                        if dx * dx + dy * dy < min_dist * min_dist {
                            ok = false;
                            break 'check;
                        }
                    }
                }
            }
            if ok {
                grid[cgy * cols + cgx] = Some(pts.len());
                active.push(pts.len());
                pts.push((nx, ny));
                found = true;
                break;
            }
        }
        if !found {
            active.swap_remove(ai);
        }
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poisson_disk_min_distance_respected() {
        let pts = poisson_disk(100.0, 100.0, 10.0, 42, 30);
        assert!(pts.len() > 5, "too few points: {}", pts.len());
        for i in 0..pts.len() {
            for j in (i + 1)..pts.len() {
                let dx = pts[i].0 - pts[j].0;
                let dy = pts[i].1 - pts[j].1;
                assert!(dx * dx + dy * dy >= 99.99, "points {i},{j} too close");
            }
        }
    }

    #[test]
    fn poisson_disk_within_bounds() {
        for (w, h, r) in [(50.0, 80.0, 5.0), (200.0, 200.0, 20.0)] {
            for (x, y) in poisson_disk(w, h, r, 7, 30) {
                assert!(x >= 0.0 && x < w, "x={x} out of {w}");
                assert!(y >= 0.0 && y < h, "y={y} out of {h}");
            }
        }
    }
}
