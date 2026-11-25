use quick_xml::{events::Event, Reader};
use std::{
    collections::{HashMap, HashSet},
    io::{BufReader, Cursor, Read},
};
use zip::ZipArchive;

const BUILD_PLATE_SURFACE: [u8; 3] = [54, 55, 64];
const BUILD_PLATE_GRID: [u8; 3] = [83, 85, 96];
const BUILD_PLATE_GRID_MAJOR: [u8; 3] = [108, 111, 123];
const BUILD_PLATE_BORDER: [u8; 3] = [137, 140, 151];
const BUILD_PLATE_GRID_SPACING: f32 = 10.0;
const MAX_GRID_LINES_PER_AXIS: f32 = 200.0;
const TRIANGLE_KIND_MODEL: u8 = 0;
const TRIANGLE_KIND_PLATE_SURFACE: u8 = 1;
const TRIANGLE_KIND_PLATE_GRID: u8 = 2;
const TRIANGLE_KIND_PLATE_GRID_MAJOR: u8 = 3;
const TRIANGLE_KIND_PLATE_BORDER: u8 = 4;
pub(crate) const MAX_PREVIEW_TRIANGLES: usize = 1_250_000;
const MAX_VIEWER_PAYLOAD_BYTES: usize = 30 * 1024 * 1024;
const MAX_INSPECTABLE_OBJECTS: usize = 4_096;
const MAX_3MF_ENTRIES: usize = 10_000;
const MAX_3MF_EXPANDED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_3MF_COMPONENT_DEPTH: usize = 256;
const MAX_3MF_COMPONENT_INSTANCES: usize = MAX_PREVIEW_TRIANGLES;

#[derive(Clone, Copy, Debug, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds3 {
    min: Vec3,
    max: Vec3,
}

impl Bounds3 {
    fn from_points(points: impl IntoIterator<Item = Vec3>) -> Option<Self> {
        let mut bounds: Option<Self> = None;
        for point in points {
            if !point.finite() {
                continue;
            }
            if let Some(current) = &mut bounds {
                current.min.x = current.min.x.min(point.x);
                current.min.y = current.min.y.min(point.y);
                current.min.z = current.min.z.min(point.z);
                current.max.x = current.max.x.max(point.x);
                current.max.y = current.max.y.max(point.y);
                current.max.z = current.max.z.max(point.z);
            } else {
                bounds = Some(Self {
                    min: point,
                    max: point,
                });
            }
        }
        bounds
    }

    fn size(self) -> Vec3 {
        self.max.sub(self.min)
    }

    fn center(self) -> Vec3 {
        Vec3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    fn include(&mut self, other: Self) {
        self.min.x = self.min.x.min(other.min.x);
        self.min.y = self.min.y.min(other.min.y);
        self.min.z = self.min.z.min(other.min.z);
        self.max.x = self.max.x.max(other.max.x);
        self.max.y = self.max.y.max(other.max.y);
        self.max.z = self.max.z.max(other.max.z);
    }
}

#[derive(Default)]
struct Mesh {
    vertices: Vec<Vec3>,
    triangles: Vec<[usize; 3]>,
    triangle_colors: Vec<[u8; 3]>,
    triangle_kinds: Vec<u8>,
    triangle_object_ids: Vec<u32>,
    objects: Vec<MeshObject>,
    source_bounds: Option<Bounds3>,
}

#[derive(Clone, Copy, Debug)]
struct MeshObject {
    id: u32,
    triangle_count: u32,
    bounds: Bounds3,
}

struct UnionFind {
    parent: Vec<u32>,
    size: Vec<u32>,
}

impl UnionFind {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count as u32).collect(),
            size: vec![1; count],
        }
    }

    fn find(&mut self, value: u32) -> u32 {
        let index = value as usize;
        let parent = self.parent[index];
        if parent != value {
            self.parent[index] = self.find(parent);
        }
        self.parent[index]
    }

    fn union(&mut self, left: u32, right: u32) {
        let mut left_root = self.find(left);
        let mut right_root = self.find(right);
        if left_root == right_root {
            return;
        }
        if self.size[left_root as usize] < self.size[right_root as usize] {
            std::mem::swap(&mut left_root, &mut right_root);
        }
        self.parent[right_root as usize] = left_root;
        self.size[left_root as usize] += self.size[right_root as usize];
    }
}

#[derive(Clone, Copy)]
struct ObjectAccumulator {
    root: u32,
    triangle_count: u32,
    bounds: Bounds3,
}

pub(crate) struct LocalPreview {
    pub(crate) payload: Vec<u8>,
    pub(crate) triangle_count: u32,
}

pub(crate) fn model_preview(bytes: &[u8], extension: &str) -> Result<LocalPreview, String> {
    let mut mesh = match extension {
        "stl" => parse_stl(bytes)?,
        "obj" => parse_obj(bytes)?,
        "3mf" => parse_3mf(bytes)?,
        "step" | "stp" => {
            return Err(
                "STEP previews are prepared by Volum Desktop. You can still save this file.".into(),
            )
        }
        _ => return Err(format!("No on-device preview is available for {extension}")),
    };
    let triangle_count = u32::try_from(mesh.triangles.len())
        .map_err(|_| "This model is too detailed for an on-device preview".to_string())?;
    analyze_objects(&mut mesh)?;
    prepare_viewer_mesh(&mut mesh)?;
    Ok(LocalPreview {
        payload: encode_viewer_mesh(&mesh)?,
        triangle_count,
    })
}

#[cfg(test)]
fn stl_preview(bytes: &[u8]) -> Result<LocalPreview, String> {
    model_preview(bytes, "stl")
}

fn analyze_objects(mesh: &mut Mesh) -> Result<(), String> {
    let bounds = model_bounds(mesh).ok_or_else(|| "Model bounds are invalid".to_string())?;
    let size = bounds.size();
    let largest = size.x.max(size.y).max(size.z).max(0.0);
    let coordinate_scale = [
        bounds.min.x.abs(),
        bounds.min.y.abs(),
        bounds.min.z.abs(),
        bounds.max.x.abs(),
        bounds.max.y.abs(),
        bounds.max.z.abs(),
    ]
    .into_iter()
    .fold(largest, f32::max);
    let tolerance = (largest * 0.000_001)
        .max(coordinate_scale * f32::EPSILON * 8.0)
        .clamp(0.000_001, 0.01) as f64;
    let mut union_find = UnionFind::new(mesh.triangles.len());
    let mut first_triangle_by_vertex = HashMap::<u64, (u32, u32)>::new();
    first_triangle_by_vertex.reserve(mesh.triangles.len().min(250_000));

    for (triangle_index, triangle) in mesh.triangles.iter().enumerate() {
        let triangle_index = triangle_index as u32;
        for vertex_index in triangle {
            let point = mesh
                .vertices
                .get(*vertex_index)
                .copied()
                .ok_or_else(|| "Model has an invalid vertex index".to_string())?;
            let key = quantized_vertex(point, tolerance);
            let mut hash = quantized_vertex_hash(key);
            loop {
                if let Some((stored_vertex, other_triangle)) =
                    first_triangle_by_vertex.get(&hash).copied()
                {
                    let stored_point = mesh
                        .vertices
                        .get(stored_vertex as usize)
                        .copied()
                        .ok_or_else(|| "Model has an invalid vertex index".to_string())?;
                    if quantized_vertex(stored_point, tolerance) == key {
                        union_find.union(triangle_index, other_triangle);
                        break;
                    }
                    hash = hash.wrapping_add(0x9e37_79b9_7f4a_7c15);
                } else {
                    let vertex_index = u32::try_from(*vertex_index)
                        .map_err(|_| "Model has too many vertices for object analysis")?;
                    first_triangle_by_vertex.insert(hash, (vertex_index, triangle_index));
                    break;
                }
            }
        }
    }
    drop(first_triangle_by_vertex);

    let mut accumulator_by_root = HashMap::<u32, usize>::new();
    let mut accumulators = Vec::<ObjectAccumulator>::new();
    for (triangle_index, triangle) in mesh.triangles.iter().enumerate() {
        let root = union_find.find(triangle_index as u32);
        let triangle_bounds = Bounds3::from_points(
            triangle
                .iter()
                .filter_map(|index| mesh.vertices.get(*index).copied()),
        )
        .ok_or_else(|| "Model bounds are invalid".to_string())?;
        if let Some(index) = accumulator_by_root.get(&root).copied() {
            let accumulator = &mut accumulators[index];
            accumulator.triangle_count += 1;
            accumulator.bounds.include(triangle_bounds);
        } else {
            accumulator_by_root.insert(root, accumulators.len());
            accumulators.push(ObjectAccumulator {
                root,
                triangle_count: 1,
                bounds: triangle_bounds,
            });
        }
    }
    if accumulators.len() > MAX_INSPECTABLE_OBJECTS {
        mesh.triangle_object_ids = vec![0; mesh.triangles.len()];
        return Ok(());
    }
    accumulators.sort_by(|left, right| {
        right
            .triangle_count
            .cmp(&left.triangle_count)
            .then_with(|| left.bounds.min.x.total_cmp(&right.bounds.min.x))
            .then_with(|| left.bounds.min.y.total_cmp(&right.bounds.min.y))
            .then_with(|| left.bounds.min.z.total_cmp(&right.bounds.min.z))
    });
    let object_id_by_root = accumulators
        .iter()
        .enumerate()
        .map(|(index, object)| (object.root, index as u32 + 1))
        .collect::<HashMap<_, _>>();
    mesh.triangle_object_ids = (0..mesh.triangles.len())
        .map(|triangle| {
            let root = union_find.find(triangle as u32);
            object_id_by_root.get(&root).copied().unwrap_or_default()
        })
        .collect();
    mesh.objects = accumulators
        .into_iter()
        .enumerate()
        .map(|(index, object)| MeshObject {
            id: index as u32 + 1,
            triangle_count: object.triangle_count,
            bounds: object.bounds,
        })
        .collect();
    Ok(())
}

fn quantized_vertex(point: Vec3, tolerance: f64) -> (i64, i64, i64) {
    let quantize = |value: f32| (f64::from(value) / tolerance).round() as i64;
    (quantize(point.x), quantize(point.y), quantize(point.z))
}

fn quantized_vertex_hash(point: (i64, i64, i64)) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in point
        .0
        .to_le_bytes()
        .into_iter()
        .chain(point.1.to_le_bytes())
        .chain(point.2.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn parse_stl(bytes: &[u8]) -> Result<Mesh, String> {
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        let expected = 84_usize.saturating_add(count.saturating_mul(50));
        if count > 0 && expected <= bytes.len() && expected.saturating_add(2) >= bytes.len() {
            if count > MAX_PREVIEW_TRIANGLES {
                return Err("This STL is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
            }
            let mut mesh = Mesh {
                vertices: Vec::with_capacity(count.saturating_mul(3)),
                triangles: Vec::with_capacity(count),
                ..Mesh::default()
            };
            for triangle in 0..count {
                let facet = 84 + triangle * 50;
                let base = mesh.vertices.len();
                for vertex in 0..3 {
                    let offset = facet + 12 + vertex * 12;
                    let point = Vec3::new(
                        f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
                        f32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
                        f32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()),
                    );
                    if !point.finite() {
                        return Err("STL contains invalid coordinates".into());
                    }
                    mesh.vertices.push(point);
                }
                mesh.triangles.push([base, base + 1, base + 2]);
            }
            return ensure_mesh(mesh);
        }
    }

    let source = std::str::from_utf8(bytes).map_err(|_| "Invalid STL data".to_string())?;
    let mut mesh = Mesh::default();
    let mut pending = Vec::with_capacity(3);
    for line in source.lines() {
        let values = line.split_whitespace().collect::<Vec<_>>();
        if values.first().copied() != Some("vertex") || values.len() < 4 {
            continue;
        }
        let point = Vec3::new(
            values[1].parse().map_err(|_| "Invalid STL vertex")?,
            values[2].parse().map_err(|_| "Invalid STL vertex")?,
            values[3].parse().map_err(|_| "Invalid STL vertex")?,
        );
        if !point.finite() {
            return Err("STL contains invalid coordinates".into());
        }
        let index = mesh.vertices.len();
        mesh.vertices.push(point);
        pending.push(index);
        if pending.len() == 3 {
            if mesh.triangles.len() >= MAX_PREVIEW_TRIANGLES {
                return Err("This STL is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
            }
            mesh.triangles.push([pending[0], pending[1], pending[2]]);
            pending.clear();
        }
    }
    ensure_mesh(mesh)
}

fn parse_obj(bytes: &[u8]) -> Result<Mesh, String> {
    let source = std::str::from_utf8(bytes).map_err(|_| "OBJ is not valid UTF-8".to_string())?;
    let mut mesh = Mesh::default();
    for line in source.lines() {
        let mut values = line
            .split('#')
            .next()
            .unwrap_or_default()
            .split_whitespace();
        match values.next() {
            Some("v") => {
                let (Some(x), Some(y), Some(z)) = (values.next(), values.next(), values.next())
                else {
                    continue;
                };
                if mesh.vertices.len() >= MAX_PREVIEW_TRIANGLES * 3 {
                    return Err("This OBJ is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
                }
                let point = Vec3::new(
                    x.parse().map_err(|_| "Invalid OBJ vertex")?,
                    y.parse().map_err(|_| "Invalid OBJ vertex")?,
                    z.parse().map_err(|_| "Invalid OBJ vertex")?,
                );
                if !point.finite() {
                    return Err("OBJ contains invalid coordinates".into());
                }
                mesh.vertices.push(point);
            }
            Some("f") => {
                let face_limit = MAX_PREVIEW_TRIANGLES - mesh.triangles.len() + 2;
                let mut face = Vec::new();
                for value in values {
                    if face.len() == face_limit {
                        return Err("This OBJ is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
                    }
                    let raw: i64 = value
                        .split('/')
                        .next()
                        .unwrap_or_default()
                        .parse()
                        .map_err(|_| "Invalid OBJ face")?;
                    let index = if raw < 0 {
                        mesh.vertices.len() as i64 + raw
                    } else {
                        raw - 1
                    };
                    if index < 0 || index as usize >= mesh.vertices.len() {
                        return Err("OBJ face references a missing vertex".into());
                    }
                    face.push(index as usize);
                }
                if face.len() < 3 {
                    continue;
                }
                triangulate_obj_face(&face, &mesh.vertices, &mut mesh.triangles)?;
            }
            _ => {}
        }
    }
    ensure_mesh(mesh)
}

fn triangulate_obj_face(
    face: &[usize],
    vertices: &[Vec3],
    triangles: &mut Vec<[usize; 3]>,
) -> Result<(), String> {
    if face.len() == 3 {
        triangles.push([face[0], face[1], face[2]]);
        return Ok(());
    }
    let position = |index: usize| {
        let p = vertices[face[index]];
        [f64::from(p.x), f64::from(p.y), f64::from(p.z)]
    };
    let mut normal = [0.0_f64; 3];
    for index in 0..face.len() {
        let p = position(index);
        let q = position((index + 1) % face.len());
        for (axis, component) in normal.iter_mut().enumerate() {
            let a = (axis + 1) % 3;
            let b = (axis + 2) % 3;
            *component += p[a] * q[b] - p[b] * q[a];
        }
    }
    let axis = (0..3)
        .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
        .unwrap();
    if normal[axis] == 0.0 {
        return Err("OBJ polygon has no area".into());
    }
    let winding = normal[axis].signum();
    let turn = |a, b, c| {
        let [p, q, r] = [position(a), position(b), position(c)];
        let x = (axis + 1) % 3;
        let y = (axis + 2) % 3;
        winding * ((q[x] - p[x]) * (r[y] - p[y]) - (q[y] - p[y]) * (r[x] - p[x]))
    };
    if (0..face.len()).all(|i| turn(i, (i + 1) % face.len(), (i + 2) % face.len()) >= 0.0) {
        triangles.extend((1..face.len() - 1).map(|i| [face[0], face[i], face[i + 1]]));
        return Ok(());
    }
    let mut corners = (0..face.len()).collect::<Vec<_>>();
    let mut checks = 0;
    while corners.len() > 3 {
        let mut ear = None;
        for i in 0..corners.len() {
            let a = corners[(i + corners.len() - 1) % corners.len()];
            let b = corners[i];
            let c = corners[(i + 1) % corners.len()];
            if turn(a, b, c) <= 0.0 {
                continue;
            }
            checks += corners.len();
            if checks > MAX_PREVIEW_TRIANGLES * 4 {
                return Err("OBJ polygon is too complex for on-device preview".into());
            }
            if !corners.iter().copied().any(|p| {
                p != a
                    && p != b
                    && p != c
                    && turn(a, b, p) >= 0.0
                    && turn(b, c, p) >= 0.0
                    && turn(c, a, p) >= 0.0
            }) {
                ear = Some((i, [face[a], face[b], face[c]]));
                break;
            }
        }
        let Some((index, triangle)) = ear else {
            return Err("OBJ polygon could not be triangulated".into());
        };
        triangles.push(triangle);
        corners.remove(index);
    }
    triangles.push([face[corners[0]], face[corners[1]], face[corners[2]]]);
    Ok(())
}

#[derive(Clone, Copy)]
struct Transform([f32; 12]);

impl Transform {
    fn identity() -> Self {
        Self([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0])
    }

    fn parse(value: Option<String>, translation_scale: f32) -> Result<Self, String> {
        let Some(value) = value else {
            return Ok(Self::identity());
        };
        let values = value
            .split_whitespace()
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "3MF contains an invalid transform".to_string())?;
        if values.len() != 12 || !values.iter().all(|value| value.is_finite()) {
            return Err("3MF contains an invalid transform".into());
        }
        Ok(Self([
            values[0],
            values[3],
            values[6],
            values[9] * translation_scale,
            values[1],
            values[4],
            values[7],
            values[10] * translation_scale,
            values[2],
            values[5],
            values[8],
            values[11] * translation_scale,
        ]))
    }

    fn apply(self, point: Vec3) -> Vec3 {
        let m = self.0;
        Vec3::new(
            m[0] * point.x + m[1] * point.y + m[2] * point.z + m[3],
            m[4] * point.x + m[5] * point.y + m[6] * point.z + m[7],
            m[8] * point.x + m[9] * point.y + m[10] * point.z + m[11],
        )
    }

    fn then(self, child: Self) -> Self {
        let a = self.0;
        let b = child.0;
        let mut c = [0.0; 12];
        for row in 0..3 {
            for column in 0..3 {
                c[row * 4 + column] = (0..3)
                    .map(|axis| a[row * 4 + axis] * b[axis * 4 + column])
                    .sum();
            }
            c[row * 4 + 3] =
                a[row * 4] * b[3] + a[row * 4 + 1] * b[7] + a[row * 4 + 2] * b[11] + a[row * 4 + 3];
        }
        Self(c)
    }
}

#[derive(Default)]
struct ThreeMfObject {
    mesh: Mesh,
    components: Vec<ThreeMfInstance>,
}

#[derive(Clone)]
struct ThreeMfInstance {
    object_id: u32,
    path: Option<String>,
    transform: Transform,
}

#[derive(Default)]
struct ThreeMfPart {
    objects: HashMap<u32, ThreeMfObject>,
    build: Vec<ThreeMfInstance>,
}

fn parse_3mf(bytes: &[u8]) -> Result<Mesh, String> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|_| "Invalid 3MF archive".to_string())?;
    if archive.len() > MAX_3MF_ENTRIES {
        return Err("3MF contains too many files".into());
    }
    let mut expanded_bytes = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        expanded_bytes = expanded_bytes.saturating_add(entry.size());
        if expanded_bytes > MAX_3MF_EXPANDED_BYTES {
            return Err("3MF expands beyond the on-device preview safety limit".into());
        }
    }
    let paths = (0..archive.len())
        .filter_map(|index| {
            archive
                .by_index(index)
                .ok()
                .and_then(|entry| entry.name().ok().map(|name| name.into_owned()))
        })
        .filter(|name| name.to_ascii_lowercase().ends_with(".model"))
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("3MF has no model document".into());
    }
    let declared_root = match archive.by_name("_rels/.rels") {
        Ok(relationships) => three_mf_start_part(relationships)?,
        Err(zip::result::ZipError::FileNotFound) => None,
        Err(error) => return Err(error.to_string()),
    };
    let root_path = declared_root.unwrap_or_else(|| {
        paths
            .iter()
            .find(|path| {
                path.trim_start_matches('/')
                    .replace('\\', "/")
                    .eq_ignore_ascii_case("3D/3dmodel.model")
            })
            .or_else(|| {
                paths.iter().find(|path| {
                    !path
                        .replace('\\', "/")
                        .to_ascii_lowercase()
                        .contains("/objects/")
                })
            })
            .unwrap_or(&paths[0])
            .trim_start_matches('/')
            .replace('\\', "/")
    });
    let mut parts = HashMap::new();
    let mut source_vertices = 0_usize;
    let mut source_triangles = 0_usize;
    for path in &paths {
        let entry = archive.by_name(path).map_err(|error| error.to_string())?;
        let part = parse_3mf_part(entry)?;
        source_vertices = source_vertices
            .checked_add(
                part.objects
                    .values()
                    .map(|object| object.mesh.vertices.len())
                    .sum(),
            )
            .ok_or_else(|| "3MF contains too many vertices".to_string())?;
        source_triangles = source_triangles
            .checked_add(
                part.objects
                    .values()
                    .map(|object| object.mesh.triangles.len())
                    .sum(),
            )
            .ok_or_else(|| "3MF contains too many triangles".to_string())?;
        if source_vertices > MAX_PREVIEW_TRIANGLES * 3 || source_triangles > MAX_PREVIEW_TRIANGLES {
            return Err("This 3MF is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
        }
        let normalized = path.trim_start_matches('/').replace('\\', "/");
        if parts.insert(normalized, part).is_some() {
            return Err("3MF contains duplicate model documents".into());
        }
    }
    let find_part = |path: &str| {
        parts.get_key_value(path).or_else(|| {
            parts
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(path))
        })
    };
    let (root_path, root) = find_part(&root_path)
        .or_else(|| super::percent_decode_filename(&root_path).and_then(|path| find_part(&path)))
        .ok_or_else(|| "3MF root model is missing".to_string())?;
    let instances = if root.build.is_empty() {
        root.objects
            .keys()
            .map(|id| ThreeMfInstance {
                object_id: *id,
                path: None,
                transform: Transform::identity(),
            })
            .collect()
    } else {
        root.build.clone()
    };
    let mut mesh = Mesh::default();
    let mut stack = HashSet::new();
    let mut remaining_instances = MAX_3MF_COMPONENT_INSTANCES;
    for instance in instances {
        emit_3mf_object(
            &parts,
            root_path,
            &instance,
            Transform::identity(),
            &mut stack,
            &mut remaining_instances,
            &mut mesh,
        )?;
    }
    ensure_mesh(mesh)
}

fn three_mf_start_part(source: impl Read) -> Result<Option<String>, String> {
    let mut reader = Reader::from_reader(BufReader::new(source));
    let mut buffer = Vec::new();
    let mut target = None;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event) | Event::Empty(event))
                if xml_name(event.name().as_ref()) == "Relationship"
                    && xml_attr(&event, "Type").as_deref()
                        == Some("http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel") =>
            {
                if target.is_some() || xml_attr(&event, "TargetMode").as_deref() == Some("External")
                {
                    return Err("3MF has an invalid StartPart relationship".into());
                }
                let path = xml_attr(&event, "Target")
                    .and_then(|path| {
                        quick_xml::escape::unescape(&path)
                            .ok()
                            .map(|path| path.trim_start_matches('/').to_owned())
                    })
                    .filter(|path| !path.is_empty())
                    .ok_or_else(|| "3MF StartPart has no model path".to_string())?;
                target = Some(path);
            }
            Ok(Event::Eof) => return Ok(target),
            Err(error) => return Err(format!("Invalid 3MF relationships: {error}")),
            _ => {}
        }
        buffer.clear();
    }
}

fn parse_3mf_part<R: Read>(source: R) -> Result<ThreeMfPart, String> {
    let mut reader = Reader::from_reader(BufReader::new(source));
    reader.config_mut().trim_text(true);
    reader.config_mut().expand_empty_elements = true;
    let mut part = ThreeMfPart::default();
    let mut current: Option<(u32, ThreeMfObject)> = None;
    let mut unit_scale = 1.0_f32;
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match xml_name(event.name().as_ref()) {
                "model" => {
                    unit_scale = three_mf_unit_scale(xml_attr(&event, "unit").as_deref())?;
                }
                "object" => {
                    if current.is_some() {
                        return Err("3MF contains nested objects".into());
                    }
                    let id = xml_attr(&event, "id")
                        .ok_or_else(|| "3MF object is missing its id".to_string())?
                        .parse()
                        .map_err(|_| "3MF object has an invalid id".to_string())?;
                    current = Some((id, ThreeMfObject::default()));
                }
                "vertex" => {
                    if let Some((_, object)) = current.as_mut() {
                        let point = Vec3::new(
                            xml_float(&event, "x")? * unit_scale,
                            xml_float(&event, "y")? * unit_scale,
                            xml_float(&event, "z")? * unit_scale,
                        );
                        if !point.finite() {
                            return Err("3MF contains invalid coordinates".into());
                        }
                        if object.mesh.vertices.len() >= MAX_PREVIEW_TRIANGLES * 3 {
                            return Err("This 3MF is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
                        }
                        object.mesh.vertices.push(point);
                    }
                }
                "triangle" => {
                    if let Some((_, object)) = current.as_mut() {
                        if object.mesh.triangles.len() >= MAX_PREVIEW_TRIANGLES {
                            return Err("This 3MF is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
                        }
                        object.mesh.triangles.push([
                            xml_usize(&event, "v1")?,
                            xml_usize(&event, "v2")?,
                            xml_usize(&event, "v3")?,
                        ]);
                    }
                }
                "component" => {
                    if let Some((_, object)) = current.as_mut() {
                        object
                            .components
                            .push(parse_3mf_instance(&event, unit_scale)?);
                    }
                }
                "item" => {
                    part.build.push(parse_3mf_instance(&event, unit_scale)?);
                }
                _ => {}
            },
            Ok(Event::End(event)) if xml_name(event.name().as_ref()) == "object" => {
                if let Some((id, object)) = current.take() {
                    if part.objects.insert(id, object).is_some() {
                        return Err(format!("3MF contains duplicate object id {id}"));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("Invalid 3MF XML: {error}")),
            _ => {}
        }
        buffer.clear();
    }
    if current.is_some() {
        return Err("3MF object is not closed".into());
    }
    Ok(part)
}

fn parse_3mf_instance(
    event: &quick_xml::events::BytesStart<'_>,
    unit_scale: f32,
) -> Result<ThreeMfInstance, String> {
    Ok(ThreeMfInstance {
        object_id: xml_attr(event, "objectid")
            .ok_or_else(|| "3MF component is missing its object id".to_string())?
            .parse()
            .map_err(|_| "3MF component has an invalid object id".to_string())?,
        path: xml_attr(event, "path").map(|path| path.trim_start_matches('/').replace('\\', "/")),
        transform: Transform::parse(xml_attr(event, "transform"), unit_scale)?,
    })
}

fn emit_3mf_object(
    parts: &HashMap<String, ThreeMfPart>,
    part_path: &str,
    instance: &ThreeMfInstance,
    parent: Transform,
    stack: &mut HashSet<(String, u32)>,
    remaining_instances: &mut usize,
    output: &mut Mesh,
) -> Result<(), String> {
    *remaining_instances = remaining_instances
        .checked_sub(1)
        .ok_or_else(|| "3MF contains too many component instances to preview".to_string())?;
    let path = instance.path.as_deref().unwrap_or(part_path);
    let key = (path.to_string(), instance.object_id);
    if stack.len() >= MAX_3MF_COMPONENT_DEPTH {
        return Err("3MF component nesting exceeds the preview safety limit".into());
    }
    if !stack.insert(key.clone()) {
        return Err("3MF contains a component cycle".into());
    }
    let part = parts
        .get(path)
        .or_else(|| {
            parts
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(path))
                .map(|(_, part)| part)
        })
        .ok_or_else(|| format!("3MF component part is missing: {path}"))?;
    let object = part
        .objects
        .get(&instance.object_id)
        .ok_or_else(|| format!("3MF object is missing: {}", instance.object_id))?;
    let transform = parent.then(instance.transform);
    if output
        .vertices
        .len()
        .checked_add(object.mesh.vertices.len())
        .is_none_or(|count| count > MAX_PREVIEW_TRIANGLES * 3)
    {
        return Err("This 3MF is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
    }
    let base = output.vertices.len();
    for point in object.mesh.vertices.iter().copied() {
        let point = transform.apply(point);
        if !point.finite() {
            return Err("3MF transform produces invalid coordinates".into());
        }
        output.vertices.push(point);
    }
    for triangle in &object.mesh.triangles {
        if triangle
            .iter()
            .any(|index| *index >= object.mesh.vertices.len())
        {
            return Err("3MF triangle references a missing vertex".into());
        }
        if output.triangles.len() >= MAX_PREVIEW_TRIANGLES {
            return Err("This 3MF is too detailed for an on-device preview. You can still save it to Volum Desktop.".into());
        }
        output
            .triangles
            .push([base + triangle[0], base + triangle[1], base + triangle[2]]);
    }
    for component in &object.components {
        emit_3mf_object(
            parts,
            path,
            component,
            transform,
            stack,
            remaining_instances,
            output,
        )?;
    }
    stack.remove(&key);
    Ok(())
}

fn xml_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}
fn xml_attr(event: &quick_xml::events::BytesStart<'_>, name: &str) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        (xml_name(attribute.key.as_ref()) == name).then(|| attribute.value.as_ref().to_string())
    })
}
fn xml_float(event: &quick_xml::events::BytesStart<'_>, name: &str) -> Result<f32, String> {
    let value = xml_attr(event, name)
        .ok_or_else(|| format!("3MF vertex is missing its {name} coordinate"))?
        .parse::<f32>()
        .map_err(|_| format!("3MF vertex has an invalid {name} coordinate"))?;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| format!("3MF vertex has an invalid {name} coordinate"))
}
fn xml_usize(event: &quick_xml::events::BytesStart<'_>, name: &str) -> Result<usize, String> {
    xml_attr(event, name)
        .ok_or_else(|| format!("3MF triangle is missing index {name}"))?
        .parse()
        .map_err(|_| format!("3MF triangle has an invalid index {name}"))
}

fn three_mf_unit_scale(unit: Option<&str>) -> Result<f32, String> {
    match unit.unwrap_or("millimeter").to_ascii_lowercase().as_str() {
        "micron" => Ok(0.001),
        "millimeter" => Ok(1.0),
        "centimeter" => Ok(10.0),
        "inch" => Ok(25.4),
        "foot" => Ok(304.8),
        "meter" => Ok(1_000.0),
        value => Err(format!("3MF uses an unsupported unit: {value}")),
    }
}

fn prepare_viewer_mesh(mesh: &mut Mesh) -> Result<(), String> {
    if mesh.triangle_object_ids.len() != mesh.triangles.len() {
        return Err("Model object analysis is invalid".into());
    }
    mesh.triangle_kinds = vec![TRIANGLE_KIND_MODEL; mesh.triangles.len()];
    let source_bounds = model_bounds(mesh).ok_or_else(|| "Model bounds are invalid".to_string())?;
    let source_size = source_bounds.size();
    if !source_size.finite() || source_size.x < 0.0 || source_size.y < 0.0 || source_size.z < 0.0 {
        return Err("Model bounds are invalid".into());
    }
    mesh.source_bounds = Some(source_bounds);
    mesh.triangle_colors = vec![[216, 200, 170]; mesh.triangles.len()];
    emit_fallback_build_plate(mesh, source_bounds);
    Ok(())
}

fn model_bounds(mesh: &Mesh) -> Option<Bounds3> {
    Bounds3::from_points(
        mesh.triangles
            .iter()
            .flatten()
            .filter_map(|index| mesh.vertices.get(*index).copied()),
    )
}

fn emit_fallback_build_plate(mesh: &mut Mesh, bounds: Bounds3) {
    let size = bounds.size();
    let footprint = size.x.max(size.y).max(0.0);
    let padding = (footprint * 0.1).max(BUILD_PLATE_GRID_SPACING);
    let required = (footprint + padding * 2.0).max(BUILD_PLATE_GRID_SPACING * 5.0);
    let initial_spacing = plate_grid_spacing(required, required);
    let initial_major = initial_spacing * 5.0;
    let initial_side = (required / initial_major).ceil().max(1.0) * initial_major;
    let spacing = plate_grid_spacing(initial_side, initial_side);
    let major = spacing * 5.0;
    let side = (required / major).ceil().max(1.0) * major;
    let center = bounds.center();
    emit_build_plate(
        mesh,
        center.x,
        center.y,
        side,
        side,
        bounds.min.z - spacing * 0.055,
        spacing,
    );
}

fn emit_build_plate(
    output: &mut Mesh,
    center_x: f32,
    center_y: f32,
    width: f32,
    height: f32,
    surface: f32,
    spacing: f32,
) {
    let radius = (width.min(height) * 0.028)
        .clamp(spacing * 0.2, spacing * 0.8)
        .min(width * 0.45)
        .min(height * 0.45);
    let outline = rounded_rectangle(center_x, center_y, width, height, radius, surface);
    emit_colored_fan(
        output,
        Vec3::new(center_x, center_y, surface),
        &outline,
        BUILD_PLATE_SURFACE,
        TRIANGLE_KIND_PLATE_SURFACE,
    );

    let border_width = (spacing * 0.17).min(radius * 0.4);
    let border_z = surface + spacing * 0.004;
    let grid_z = surface + spacing * 0.007;
    let inner = rounded_rectangle(
        center_x,
        center_y,
        width - border_width * 2.0,
        height - border_width * 2.0,
        radius - border_width,
        border_z,
    );
    emit_colored_ring(
        output,
        &outline,
        &inner,
        BUILD_PLATE_BORDER,
        TRIANGLE_KIND_PLATE_BORDER,
    );

    let half_width = width * 0.5;
    let half_height = height * 0.5;
    let grid_inset = border_width * 1.35;
    for (local_x, index) in metric_grid_lines(half_width, grid_inset, spacing) {
        let major = index % 5 == 0;
        let thickness = spacing * if major { 0.062 } else { 0.034 };
        let extent = rounded_axis_extent(local_x, half_width, half_height, radius) - grid_inset;
        emit_colored_quad(
            output,
            [
                Vec3::new(
                    center_x + local_x - thickness * 0.5,
                    center_y - extent,
                    grid_z,
                ),
                Vec3::new(
                    center_x + local_x + thickness * 0.5,
                    center_y - extent,
                    grid_z,
                ),
                Vec3::new(
                    center_x + local_x + thickness * 0.5,
                    center_y + extent,
                    grid_z,
                ),
                Vec3::new(
                    center_x + local_x - thickness * 0.5,
                    center_y + extent,
                    grid_z,
                ),
            ],
            if major {
                BUILD_PLATE_GRID_MAJOR
            } else {
                BUILD_PLATE_GRID
            },
            if major {
                TRIANGLE_KIND_PLATE_GRID_MAJOR
            } else {
                TRIANGLE_KIND_PLATE_GRID
            },
        );
    }
    for (local_y, index) in metric_grid_lines(half_height, grid_inset, spacing) {
        let major = index % 5 == 0;
        let thickness = spacing * if major { 0.062 } else { 0.034 };
        let extent = rounded_axis_extent(local_y, half_height, half_width, radius) - grid_inset;
        emit_colored_quad(
            output,
            [
                Vec3::new(
                    center_x - extent,
                    center_y + local_y - thickness * 0.5,
                    grid_z,
                ),
                Vec3::new(
                    center_x + extent,
                    center_y + local_y - thickness * 0.5,
                    grid_z,
                ),
                Vec3::new(
                    center_x + extent,
                    center_y + local_y + thickness * 0.5,
                    grid_z,
                ),
                Vec3::new(
                    center_x - extent,
                    center_y + local_y + thickness * 0.5,
                    grid_z,
                ),
            ],
            if major {
                BUILD_PLATE_GRID_MAJOR
            } else {
                BUILD_PLATE_GRID
            },
            if major {
                TRIANGLE_KIND_PLATE_GRID_MAJOR
            } else {
                TRIANGLE_KIND_PLATE_GRID
            },
        );
    }
}

fn metric_grid_lines(half_extent: f32, inset: f32, spacing: f32) -> Vec<(f32, i32)> {
    let extent = half_extent * 2.0;
    if !extent.is_finite() || !spacing.is_finite() || extent <= spacing {
        return Vec::new();
    }
    let max_position = half_extent - inset;
    let count = (extent / spacing).ceil() as i32;
    (1..count)
        .filter_map(|index| {
            let position = -half_extent + index as f32 * spacing;
            (position <= max_position).then_some((position, index))
        })
        .collect()
}

fn plate_grid_spacing(width: f32, height: f32) -> f32 {
    let required = width.max(height) / MAX_GRID_LINES_PER_AXIS;
    if !required.is_finite() || required <= BUILD_PLATE_GRID_SPACING {
        BUILD_PLATE_GRID_SPACING
    } else {
        nice_metric_step(required)
    }
}

fn nice_metric_step(minimum: f32) -> f32 {
    let magnitude = 10_f32.powf(minimum.log10().floor());
    for multiplier in [1.0, 2.0, 5.0, 10.0] {
        let candidate = magnitude * multiplier;
        if candidate >= minimum {
            return candidate;
        }
    }
    magnitude * 10.0
}

fn rounded_rectangle(
    center_x: f32,
    center_y: f32,
    width: f32,
    height: f32,
    radius: f32,
    z: f32,
) -> Vec<Vec3> {
    let half_width = width * 0.5;
    let half_height = height * 0.5;
    let corners = [
        (half_width - radius, -half_height + radius, -90.0_f32),
        (half_width - radius, half_height - radius, 0.0),
        (-half_width + radius, half_height - radius, 90.0),
        (-half_width + radius, -half_height + radius, 180.0),
    ];
    let mut points = Vec::with_capacity(32);
    for (corner_x, corner_y, start_degrees) in corners {
        for segment in 0..8 {
            let angle = (start_degrees + segment as f32 * 90.0 / 7.0).to_radians();
            points.push(Vec3::new(
                center_x + corner_x + radius * angle.cos(),
                center_y + corner_y + radius * angle.sin(),
                z,
            ));
        }
    }
    points
}

fn rounded_axis_extent(position: f32, half_axis: f32, half_other: f32, radius: f32) -> f32 {
    let corner_offset = (position.abs() - (half_axis - radius)).max(0.0);
    half_other - radius
        + (radius * radius - corner_offset * corner_offset)
            .max(0.0)
            .sqrt()
}

fn emit_colored_fan(output: &mut Mesh, center: Vec3, outline: &[Vec3], color: [u8; 3], kind: u8) {
    let base = output.vertices.len();
    output.vertices.push(center);
    output.vertices.extend_from_slice(outline);
    for index in 0..outline.len() {
        output.triangles.push([
            base,
            base + 1 + index,
            base + 1 + (index + 1) % outline.len(),
        ]);
        output.triangle_colors.push(color);
        output.triangle_kinds.push(kind);
        output.triangle_object_ids.push(0);
    }
}

fn emit_colored_ring(output: &mut Mesh, outer: &[Vec3], inner: &[Vec3], color: [u8; 3], kind: u8) {
    if outer.len() != inner.len() || outer.is_empty() {
        return;
    }
    let base = output.vertices.len();
    output.vertices.extend_from_slice(outer);
    output.vertices.extend_from_slice(inner);
    for index in 0..outer.len() {
        let next = (index + 1) % outer.len();
        output.triangles.extend([
            [base + index, base + next, base + outer.len() + next],
            [
                base + index,
                base + outer.len() + next,
                base + outer.len() + index,
            ],
        ]);
        output.triangle_colors.extend([color, color]);
        output.triangle_kinds.extend([kind, kind]);
        output.triangle_object_ids.extend([0, 0]);
    }
}

fn emit_colored_quad(output: &mut Mesh, vertices: [Vec3; 4], color: [u8; 3], kind: u8) {
    let base = output.vertices.len();
    output.vertices.extend(vertices);
    output
        .triangles
        .extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
    output.triangle_colors.extend([color, color]);
    output.triangle_kinds.extend([kind, kind]);
    output.triangle_object_ids.extend([0, 0]);
}

fn encode_viewer_mesh(mesh: &Mesh) -> Result<Vec<u8>, String> {
    let bounds = mesh
        .source_bounds
        .ok_or_else(|| "Model bounds are invalid".to_string())?;
    let vertex_count = u32::try_from(mesh.vertices.len())
        .map_err(|_| "Model has too many vertices for the viewer".to_string())?;
    let index_count = mesh
        .triangles
        .len()
        .checked_mul(3)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or_else(|| "Model has too many triangles for the viewer".to_string())?;
    if mesh.triangle_colors.len() != mesh.triangles.len()
        || mesh.triangle_kinds.len() != mesh.triangles.len()
        || mesh.triangle_object_ids.len() != mesh.triangles.len()
    {
        return Err("Model viewer metadata is invalid".into());
    }
    let object_count = u32::try_from(mesh.objects.len())
        .map_err(|_| "Model has too many objects for the viewer".to_string())?;
    let header_length = 40 + mesh.objects.len() * 32;
    let payload_size = [
        mesh.vertices
            .len()
            .checked_mul(std::mem::size_of::<[f32; 3]>()),
        (index_count as usize).checked_mul(std::mem::size_of::<u32>()),
        mesh.triangle_colors.len().checked_mul(3),
        Some(mesh.triangle_kinds.len()),
        mesh.triangle_object_ids
            .len()
            .checked_mul(std::mem::size_of::<u32>()),
    ]
    .into_iter()
    .try_fold(header_length, |total, length| {
        total.checked_add(length?)
    })
    .filter(|size| *size <= MAX_VIEWER_PAYLOAD_BYTES)
    .ok_or_else(|| {
        "This model is too detailed for an on-device preview. You can still save it to Volum Desktop."
            .to_string()
    })?;
    let mut bytes = Vec::with_capacity(payload_size);
    bytes.extend_from_slice(b"VLM5");
    bytes.extend_from_slice(&vertex_count.to_le_bytes());
    bytes.extend_from_slice(&index_count.to_le_bytes());
    for value in [
        bounds.min.x,
        bounds.min.y,
        bounds.min.z,
        bounds.max.x,
        bounds.max.y,
        bounds.max.z,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&object_count.to_le_bytes());
    for object in &mesh.objects {
        bytes.extend_from_slice(&object.id.to_le_bytes());
        bytes.extend_from_slice(&object.triangle_count.to_le_bytes());
        for value in [
            object.bounds.min.x,
            object.bounds.min.y,
            object.bounds.min.z,
            object.bounds.max.x,
            object.bounds.max.y,
            object.bounds.max.z,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for vertex in &mesh.vertices {
        bytes.extend_from_slice(&vertex.x.to_le_bytes());
        bytes.extend_from_slice(&vertex.y.to_le_bytes());
        bytes.extend_from_slice(&vertex.z.to_le_bytes());
    }
    for triangle in &mesh.triangles {
        for index in triangle {
            let index = u32::try_from(*index)
                .map_err(|_| "Model has an invalid vertex index".to_string())?;
            bytes.extend_from_slice(&index.to_le_bytes());
        }
    }
    for color in &mesh.triangle_colors {
        bytes.extend_from_slice(color);
    }
    bytes.extend_from_slice(&mesh.triangle_kinds);
    for object_id in &mesh.triangle_object_ids {
        bytes.extend_from_slice(&object_id.to_le_bytes());
    }
    Ok(bytes)
}

fn ensure_mesh(mesh: Mesh) -> Result<Mesh, String> {
    if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
        Err("Model contains no renderable triangles".into())
    } else {
        Ok(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binary_triangle() -> Vec<u8> {
        binary_triangles(&[[0.0, 0.0, 0.0, 20.0, 0.0, 0.0, 0.0, 10.0, 2.0]])
    }

    fn binary_triangles(triangles: &[[f32; 9]]) -> Vec<u8> {
        let mut bytes = vec![0_u8; 84 + triangles.len() * 50];
        bytes[80..84].copy_from_slice(&(triangles.len() as u32).to_le_bytes());
        for (triangle, vertices) in triangles.iter().enumerate() {
            for (index, value) in vertices.iter().enumerate() {
                let offset = 84 + triangle * 50 + 12 + index * 4;
                bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }

    #[test]
    fn produces_selectable_vlm5_with_source_bounds_and_fallback_plate() {
        let preview = stl_preview(&binary_triangle()).unwrap();
        assert_eq!(&preview.payload[..4], b"VLM5");
        assert_eq!(preview.triangle_count, 1);
        let values = (0..6)
            .map(|index| {
                f32::from_le_bytes(
                    preview.payload[12 + index * 4..16 + index * 4]
                        .try_into()
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(values, vec![0.0, 0.0, 0.0, 20.0, 10.0, 2.0]);
        assert!(u32::from_le_bytes(preview.payload[8..12].try_into().unwrap()) > 3);
        assert_eq!(
            u32::from_le_bytes(preview.payload[36..40].try_into().unwrap()),
            1
        );
    }

    #[test]
    fn identifies_connected_and_disconnected_stl_objects() {
        let preview = stl_preview(&binary_triangles(&[
            [0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 10.0, 0.0],
            [10.0, 0.0, 0.0, 10.0, 10.0, 0.0, 0.0, 10.0, 0.0],
            [30.0, 0.0, 0.0, 35.0, 0.0, 0.0, 30.0, 5.0, 2.0],
        ]))
        .unwrap();
        assert_eq!(
            u32::from_le_bytes(preview.payload[36..40].try_into().unwrap()),
            2
        );
        assert_eq!(
            u32::from_le_bytes(preview.payload[40..44].try_into().unwrap()),
            1
        );
        assert_eq!(
            u32::from_le_bytes(preview.payload[44..48].try_into().unwrap()),
            2
        );
        let first_bounds = (0..6)
            .map(|index| {
                f32::from_le_bytes(
                    preview.payload[48 + index * 4..52 + index * 4]
                        .try_into()
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(first_bounds, vec![0.0, 0.0, 0.0, 10.0, 10.0, 0.0]);
        assert_eq!(
            u32::from_le_bytes(preview.payload[72..76].try_into().unwrap()),
            2
        );
        assert_eq!(
            u32::from_le_bytes(preview.payload[76..80].try_into().unwrap()),
            1
        );
    }

    #[test]
    fn preserves_preview_when_there_are_too_many_shells_to_inspect() {
        let triangles = (0..=MAX_INSPECTABLE_OBJECTS)
            .map(|index| {
                let x = index as f32 * 3.0;
                [x, 0.0, 0.0, x + 1.0, 0.0, 0.0, x, 1.0, 0.0]
            })
            .collect::<Vec<_>>();
        let preview = stl_preview(&binary_triangles(&triangles)).unwrap();
        assert_eq!(preview.triangle_count, MAX_INSPECTABLE_OBJECTS as u32 + 1);
        assert_eq!(
            u32::from_le_bytes(preview.payload[36..40].try_into().unwrap()),
            0
        );
    }

    #[test]
    fn accepts_ascii_stl_and_rejects_empty_geometry() {
        let ascii = b"solid test\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid";
        assert_eq!(stl_preview(ascii).unwrap().triangle_count, 1);
        assert!(stl_preview(b"solid empty\nendsolid").is_err());
    }

    #[test]
    fn previews_obj_faces_and_negative_indices() {
        let source = b"v 0 0 0\nv 10 0 0\nv 10 10 0\nv 0 10 0\nf -4 -3 -2 -1\n";
        let preview = model_preview(source, "obj").unwrap();
        assert_eq!(preview.triangle_count, 2);
        assert_eq!(&preview.payload[..4], b"VLM5");
    }

    #[test]
    fn triangulates_concave_obj_faces_without_crossing_the_notch() {
        let source = b"v 0 0 0\nv 4 0 0\nv 4 4 0\nv 2 1 0\nv 0 4 0\nf 1 2 3 4 5\n";
        let mesh = parse_obj(source).unwrap();
        assert_eq!(mesh.triangles.len(), 3);
        let doubled_area: f32 = mesh
            .triangles
            .iter()
            .map(|triangle| {
                let [a, b, c] = triangle.map(|index| mesh.vertices[index]);
                let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
                assert!(area > 0.0);
                area
            })
            .sum();
        assert_eq!(doubled_area, 20.0);
    }

    #[test]
    fn previews_the_3mf_start_part_instead_of_the_default_filename() {
        use std::io::Write as _;

        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("_rels/.rels", options).unwrap();
        writer.write_all(br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Target="/3d/primary%20part.MODEL" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>"#).unwrap();
        writer.start_file("3D/3dmodel.model", options).unwrap();
        writer.write_all(b"<model/>").unwrap();
        writer.start_file("3D/Primary Part.model", options).unwrap();
        writer.write_all(br#"<model><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources><build><item objectid="1"/></build></model>"#).unwrap();
        let archive = writer.finish().unwrap();
        assert_eq!(parse_3mf(archive.get_ref()).unwrap().triangles.len(), 1);
    }

    #[test]
    fn parses_3mf_geometry_and_instances_with_separate_end_tags() {
        let model = br#"<model><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"></vertex><vertex x="1" y="0" z="0"></vertex><vertex x="0" y="1" z="0"></vertex></vertices><triangles><triangle v1="0" v2="1" v3="2"></triangle></triangles></mesh></object><object id="2"><components><component objectid="1"></component></components></object></resources><build><item objectid="2"></item></build></model>"#;
        let part = parse_3mf_part(model.as_slice()).unwrap();
        assert_eq!(part.objects[&1].mesh.vertices.len(), 3);
        assert_eq!(part.objects[&1].mesh.triangles, [[0, 1, 2]]);
        assert_eq!(part.objects[&2].components[0].object_id, 1);
        assert_eq!(part.build[0].object_id, 2);
    }

    #[test]
    fn previews_3mf_components_with_build_transforms() {
        use std::io::Write as _;

        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("3D/3dmodel.model", options).unwrap();
        writer.write_all(br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06"><resources><object id="2"><components><component objectid="1" p:path="/3D/Objects/part.model"/></components></object></resources><build><item objectid="2" transform="1 0 0 0 1 0 0 0 1 10 20 30"/></build></model>"#).unwrap();
        writer.start_file("3D/Objects/part.model", options).unwrap();
        writer.write_all(br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#).unwrap();
        let archive = writer.finish().unwrap();

        let preview = model_preview(archive.get_ref(), "3mf").unwrap();
        assert_eq!(preview.triangle_count, 1);
        let bounds = (0..6)
            .map(|index| {
                f32::from_le_bytes(
                    preview.payload[12 + index * 4..16 + index * 4]
                        .try_into()
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(bounds, vec![10.0, 20.0, 30.0, 11.0, 21.0, 30.0]);
    }

    #[test]
    fn converts_3mf_units_and_transform_translations_to_millimeters() {
        use std::io::Write as _;

        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file("3D/3dmodel.model", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(br#"<model unit="inch" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources><build><item objectid="1" transform="1 0 0 0 1 0 0 0 1 1 0 0"/></build></model>"#).unwrap();
        let archive = writer.finish().unwrap();

        let preview = model_preview(archive.get_ref(), "3mf").unwrap();
        let bounds = (0..6)
            .map(|index| {
                f32::from_le_bytes(
                    preview.payload[12 + index * 4..16 + index * 4]
                        .try_into()
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let expected = [25.4, 0.0, 0.0, 50.8, 25.4, 0.0];
        for (actual, expected) in bounds.iter().zip(expected) {
            assert!((actual - expected).abs() < 0.0001);
        }
    }

    #[test]
    fn rejects_malformed_3mf_geometry_and_transforms() {
        use std::io::Write as _;

        for model in [
            br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1"><mesh><vertices><vertex x="nope" y="0" z="0"/></vertices></mesh></object></resources></model>"#.as_slice(),
            br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources><build><item objectid="1" transform="not a transform"/></build></model>"#.as_slice(),
            br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#.as_slice(),
        ] {
            let cursor = Cursor::new(Vec::new());
            let mut writer = zip::ZipWriter::new(cursor);
            writer
                .start_file(
                    "3D/3dmodel.model",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(model).unwrap();
            let archive = writer.finish().unwrap();
            assert!(model_preview(archive.get_ref(), "3mf").is_err());
        }
    }

    #[test]
    fn bounds_3mf_component_expansion_independently_of_geometry() {
        let path = "3D/3dmodel.model";
        let model = br#"<model><resources><object id="1"/><object id="2"><components><component objectid="1"/><component objectid="1"/></components></object></resources><build><item objectid="2"/></build></model>"#;
        let part = parse_3mf_part(model.as_slice()).unwrap();
        let instance = part.build[0].clone();
        let parts = HashMap::from([(path.to_string(), part)]);
        let result = emit_3mf_object(
            &parts,
            path,
            &instance,
            Transform::identity(),
            &mut HashSet::new(),
            &mut 2,
            &mut Mesh::default(),
        );
        assert_eq!(
            result.unwrap_err(),
            "3MF contains too many component instances to preview"
        );
    }

    #[test]
    fn explains_that_step_previewing_happens_on_desktop() {
        let error = match model_preview(b"ISO-10303-21;", "step") {
            Ok(_) => panic!("STEP should use the desktop preview fallback"),
            Err(error) => error,
        };
        assert!(error.contains("Volum Desktop"));
    }
}
