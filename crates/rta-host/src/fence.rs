//! Host fence. The spec sees `fence_ok` only. The legal line is not the trip line.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FenceInput<'a> {
    pub polygon: &'a [[f64; 2]],
    pub vertical_cap_m: f64,
    pub v_max: f64,
    pub a_max: f64,
    pub mode_change_latency_s: f64,
}

pub fn stopping_margin_m(v_max: f64, a_max: f64, latency_s: f64) -> f64 {
    if a_max <= 0.0 || v_max < 0.0 || latency_s < 0.0 {
        return f64::MAX;
    }
    (v_max * v_max) / (2.0 * a_max) + latency_s * v_max
}

pub fn fence_ok(point: [f64; 2], alt_m: f64, fence: FenceInput<'_>) -> bool {
    if fence.polygon.len() < 3 || fence.a_max <= 0.0 {
        return false;
    }
    if alt_m > fence.vertical_cap_m {
        return false;
    }
    if !inside(point, fence.polygon) {
        return false;
    }
    let margin = stopping_margin_m(fence.v_max, fence.a_max, fence.mode_change_latency_s);
    distance_to_boundary(point, fence.polygon) > margin
}

fn inside(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut crossed = false;
    let n = polygon.len();
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        let intersect = ((a[1] > point[1]) != (b[1] > point[1]))
            && (point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]);
        if intersect {
            crossed = !crossed;
        }
    }
    crossed
}

fn distance_to_boundary(point: [f64; 2], polygon: &[[f64; 2]]) -> f64 {
    let n = polygon.len();
    let mut best = f64::MAX;
    for i in 0..n {
        best = best.min(distance_to_segment(point, polygon[i], polygon[(i + 1) % n]));
    }
    best
}

fn distance_to_segment(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [point[0] - a[0], point[1] - a[1]];
    let denom = ab[0] * ab[0] + ab[1] * ab[1];
    if denom == 0.0 {
        return (ap[0] * ap[0] + ap[1] * ap[1]).sqrt();
    }
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / denom).clamp(0.0, 1.0);
    let dx = point[0] - (a[0] + ab[0] * t);
    let dy = point[1] - (a[1] + ab[1] * t);
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> FenceInput<'static> {
        FenceInput {
            polygon: &[[0.0, 0.0], [200.0, 0.0], [200.0, 200.0], [0.0, 200.0]],
            vertical_cap_m: 120.0,
            v_max: 10.0,
            a_max: 2.0,
            mode_change_latency_s: 0.2,
        }
    }

    #[test]
    fn interior_is_ok() {
        let fence = square();
        assert!(fence_ok([100.0, 100.0], 20.0, fence));
    }

    #[test]
    fn margin_band_is_not_ok() {
        let fence = square();
        assert!(!fence_ok([1.0, 100.0], 20.0, fence));
    }

    #[test]
    fn empty_polygon_is_not_ok() {
        let mut fence = square();
        fence.polygon = &[];
        assert!(!fence_ok([100.0, 100.0], 20.0, fence));
    }
}
