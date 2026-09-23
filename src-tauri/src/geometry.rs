use crate::previews::{self, Mesh};
use serde::Serialize;
use std::path::Path;

pub const ALGORITHM_VERSION: &str = "surface-v1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryAnalysis {
    pub geometry_hash: String,
    pub similarity_key: String,
    pub dimensions_mm: [f64; 3],
    pub surface_area: f64,
    pub volume: Option<f64>,
    pub triangle_count: u64,
    pub radial_moments: [f64; 4],
}

pub fn analyze_file(path: &Path, extension: &str) -> Result<GeometryAnalysis, String> {
    let mesh = previews::load_mesh(path, extension)?;
    analyze_mesh(&mesh)
}

fn analyze_mesh(mesh: &Mesh) -> Result<GeometryAnalysis, String> {
    if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
        return Err("Model contains no displayable geometry".into());
    }

    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for vertex in &mesh.vertices {
        let point = [vertex.x as f64, vertex.y as f64, vertex.z as f64];
        for axis in [0, 1, 2] {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    if !min.iter().chain(max.iter()).all(|value| value.is_finite()) {
        return Err("Model has invalid geometry bounds".into());
    }
    let mut dimensions = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    dimensions.sort_by(f64::total_cmp);
    if dimensions.iter().all(|value| *value <= f64::EPSILON) {
        return Err("Model geometry has no measurable size".into());
    }

    let mut surface_area = 0.0;
    let mut signed_volume = 0.0;
    let mut first_moment = [0.0; 3];
    let mut second_moment = [[0.0; 3]; 3];
    for triangle in &mesh.triangles {
        let a = point(mesh, triangle[0])?;
        let b = point(mesh, triangle[1])?;
        let c = point(mesh, triangle[2])?;
        let ab = sub(b, a);
        let ac = sub(c, a);
        let triangle_cross = cross(ab, ac);
        let area = length(triangle_cross) * 0.5;
        if !area.is_finite() || area <= f64::EPSILON {
            continue;
        }
        surface_area += area;
        signed_volume += dot(a, cross(b, c)) / 6.0;
        let centroid = [
            (a[0] + b[0] + c[0]) / 3.0,
            (a[1] + b[1] + c[1]) / 3.0,
            (a[2] + b[2] + c[2]) / 3.0,
        ];
        for axis in 0..3 {
            first_moment[axis] += centroid[axis] * area;
            for other in 0..3 {
                let diagonal = a[axis] * a[other] + b[axis] * b[other] + c[axis] * c[other];
                let mixed = a[axis] * b[other]
                    + b[axis] * a[other]
                    + a[axis] * c[other]
                    + c[axis] * a[other]
                    + b[axis] * c[other]
                    + c[axis] * b[other];
                second_moment[axis][other] += area * (diagonal / 6.0 + mixed / 12.0);
            }
        }
    }
    if surface_area <= f64::EPSILON {
        return Err("Model contains no non-degenerate triangles".into());
    }
    let surface_centroid = first_moment.map(|value| value / surface_area);
    let mut covariance = [[0.0; 3]; 3];
    for axis in 0..3 {
        for other in 0..3 {
            covariance[axis][other] = second_moment[axis][other] / surface_area
                - surface_centroid[axis] * surface_centroid[other];
        }
    }
    let eigenvalues = symmetric_eigenvalues(covariance).map(|value| value.max(0.0) / surface_area);
    let normalized_volume = signed_volume.abs() / surface_area.powf(1.5);
    let radial_moments = [
        eigenvalues[0],
        eigenvalues[1],
        eigenvalues[2],
        normalized_volume,
    ];
    let volume = signed_volume
        .abs()
        .is_finite()
        .then_some(signed_volume.abs())
        .filter(|value| *value > f64::EPSILON);

    let fine = serde_json::json!({
        "a": logarithmic_bucket(surface_area, 200.0),
        "v": volume.map(|value| logarithmic_bucket(value, 200.0)),
        "m": radial_moments.map(|value| (value * 500.0).round() as i64),
    });
    let coarse = serde_json::json!({
        "a": logarithmic_bucket(surface_area, 40.0),
        "v": volume.map(|value| logarithmic_bucket(value, 40.0)),
        "m": radial_moments.map(|value| (value * 100.0).round() as i64),
    });

    Ok(GeometryAnalysis {
        geometry_hash: blake3::hash(fine.to_string().as_bytes())
            .to_hex()
            .to_string(),
        similarity_key: blake3::hash(coarse.to_string().as_bytes())
            .to_hex()
            .to_string(),
        dimensions_mm: [max[0] - min[0], max[1] - min[1], max[2] - min[2]],
        surface_area,
        volume,
        triangle_count: mesh.triangles.len() as u64,
        radial_moments,
    })
}

fn point(mesh: &Mesh, index: usize) -> Result<[f64; 3], String> {
    let vertex = mesh
        .vertices
        .get(index)
        .ok_or_else(|| "Mesh contains an invalid triangle index".to_string())?;
    Ok([vertex.x as f64, vertex.y as f64, vertex.z as f64])
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(value: [f64; 3]) -> f64 {
    dot(value, value).sqrt()
}

fn logarithmic_bucket(value: f64, precision: f64) -> i64 {
    if value <= f64::EPSILON {
        0
    } else {
        (value.ln() * precision).round() as i64
    }
}

fn symmetric_eigenvalues(mut matrix: [[f64; 3]; 3]) -> [f64; 3] {
    for _ in 0..24 {
        let mut p = 0;
        let mut q = 1;
        for (left, right) in [(0, 1), (0, 2), (1, 2)] {
            if matrix[left][right].abs() > matrix[p][q].abs() {
                p = left;
                q = right;
            }
        }
        if matrix[p][q].abs() < 1e-12 {
            break;
        }
        let angle = 0.5 * (2.0 * matrix[p][q]).atan2(matrix[q][q] - matrix[p][p]);
        let cosine = angle.cos();
        let sine = angle.sin();
        let pp = matrix[p][p];
        let qq = matrix[q][q];
        let pq = matrix[p][q];
        matrix[p][p] = cosine * cosine * pp - 2.0 * sine * cosine * pq + sine * sine * qq;
        matrix[q][q] = sine * sine * pp + 2.0 * sine * cosine * pq + cosine * cosine * qq;
        matrix[p][q] = 0.0;
        matrix[q][p] = 0.0;
        for axis in [0, 1, 2] {
            if axis == p || axis == q {
                continue;
            }
            let ap = matrix[axis][p];
            let aq = matrix[axis][q];
            matrix[axis][p] = cosine * ap - sine * aq;
            matrix[p][axis] = matrix[axis][p];
            matrix[axis][q] = sine * ap + cosine * aq;
            matrix[q][axis] = matrix[axis][q];
        }
    }
    let mut values = [matrix[0][0], matrix[1][1], matrix[2][2]];
    values.sort_by(f64::total_cmp);
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::previews::Vec3;

    fn tetrahedron(vertices: [[f32; 3]; 4]) -> Mesh {
        Mesh {
            vertices: vertices
                .into_iter()
                .map(|point| Vec3::new(point[0], point[1], point[2]))
                .collect(),
            triangles: vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
            vertex_colors: Vec::new(),
        }
    }

    #[test]
    fn fingerprint_ignores_translation_and_axis_rotation() {
        let original = tetrahedron([
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [0.0, 0.0, 4.0],
        ]);
        let transformed = tetrahedron([
            [10.0, 20.0, 30.0],
            [10.0, 22.0, 30.0],
            [7.0, 20.0, 30.0],
            [10.0, 20.0, 34.0],
        ]);
        assert_eq!(
            analyze_mesh(&original).unwrap().geometry_hash,
            analyze_mesh(&transformed).unwrap().geometry_hash
        );
    }

    #[test]
    fn fingerprint_ignores_arbitrary_rotation() {
        let original = tetrahedron([
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [0.0, 0.0, 4.0],
        ]);
        let angle = std::f32::consts::FRAC_PI_4;
        let rotate = |point: [f32; 3]| {
            [
                point[0] * angle.cos() - point[1] * angle.sin() + 10.0,
                point[0] * angle.sin() + point[1] * angle.cos() + 20.0,
                point[2] + 30.0,
            ]
        };
        let transformed = tetrahedron([
            rotate([0.0, 0.0, 0.0]),
            rotate([2.0, 0.0, 0.0]),
            rotate([0.0, 3.0, 0.0]),
            rotate([0.0, 0.0, 4.0]),
        ]);
        assert_eq!(
            analyze_mesh(&original).unwrap().geometry_hash,
            analyze_mesh(&transformed).unwrap().geometry_hash
        );
    }

    #[test]
    fn fingerprint_preserves_scale() {
        let small = tetrahedron([
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ]);
        let large = tetrahedron([
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 2.0],
        ]);
        assert_ne!(
            analyze_mesh(&small).unwrap().geometry_hash,
            analyze_mesh(&large).unwrap().geometry_hash
        );
    }

    #[test]
    fn fingerprint_is_stable_across_planar_tessellations() {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(10.0, 4.0, 0.0),
            Vec3::new(0.0, 4.0, 0.0),
        ];
        let left = Mesh {
            vertices: vertices.clone(),
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            vertex_colors: Vec::new(),
        };
        let right = Mesh {
            vertices,
            triangles: vec![[0, 1, 3], [1, 2, 3]],
            vertex_colors: Vec::new(),
        };
        assert_eq!(
            analyze_mesh(&left).unwrap().geometry_hash,
            analyze_mesh(&right).unwrap().geometry_hash
        );
    }
}
