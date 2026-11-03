use std::collections::HashMap;

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
const MAX_INSPECTABLE_OBJECTS: usize = 4_096;

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

pub(crate) fn stl_preview(bytes: &[u8]) -> Result<LocalPreview, String> {
    let mut mesh = parse_stl(bytes)?;
    let triangle_count = u32::try_from(mesh.triangles.len())
        .map_err(|_| "This STL is too detailed for an on-device preview".to_string())?;
    analyze_objects(&mut mesh)?;
    prepare_viewer_mesh(&mut mesh)?;
    Ok(LocalPreview {
        payload: encode_viewer_mesh(&mesh)?,
        triangle_count,
    })
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
    let mut bytes = Vec::with_capacity(
        header_length
            + mesh.vertices.len() * std::mem::size_of::<[f32; 3]>()
            + index_count as usize * std::mem::size_of::<u32>()
            + mesh.triangle_colors.len() * 3
            + mesh.triangle_kinds.len()
            + mesh.triangle_object_ids.len() * std::mem::size_of::<u32>(),
    );
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
}
