use image::{codecs::png::PngEncoder, ColorType, ImageEncoder, Rgba, RgbaImage};
use quick_xml::{events::Event, Reader};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::Path,
    sync::Mutex,
};
use zip::ZipArchive;

const WIDTH: u32 = 480;
const HEIGHT: u32 = 360;
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
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
const MAX_VIEWER_OBJECTS: usize = 4_096;
static STEP_TESSELLATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Vec3 {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) z: f32,
}

impl Vec3 {
    pub(crate) fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    fn normalized(self) -> Self {
        let length = self.dot(self).sqrt();
        if length > f32::EPSILON {
            Self::new(self.x / length, self.y / length, self.z / length)
        } else {
            Self::default()
        }
    }

    fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds3 {
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
}

#[derive(Clone, Debug)]
pub(crate) struct MeshObject {
    id: u32,
    triangle_count: u32,
    bounds: Bounds3,
    name: Option<String>,
}

#[derive(Default)]
pub(crate) struct Mesh {
    pub(crate) vertices: Vec<Vec3>,
    pub(crate) triangles: Vec<[usize; 3]>,
    pub(crate) vertex_colors: Vec<[u8; 3]>,
    pub(crate) triangle_colors: Vec<[u8; 3]>,
    pub(crate) triangle_kinds: Vec<u8>,
    pub(crate) triangle_object_ids: Vec<u32>,
    pub(crate) objects: Vec<MeshObject>,
    pub(crate) source_bounds: Option<Bounds3>,
}

#[derive(Clone, Copy)]
struct Projected {
    x: f32,
    y: f32,
    depth: f32,
}

pub fn render_thumbnail(path: &Path, extension: &str) -> Result<Vec<u8>, String> {
    let mesh = load_mesh(path, extension)?;
    rasterize(&mesh)
}

pub(crate) fn load_mesh(path: &Path, extension: &str) -> Result<Mesh, String> {
    match extension {
        "stl" => parse_stl(&std::fs::read(path).map_err(|error| error.to_string())?),
        "obj" => parse_obj(&std::fs::read(path).map_err(|error| error.to_string())?),
        "3mf" => parse_3mf(&std::fs::read(path).map_err(|error| error.to_string())?),
        "step" | "stp" => parse_step(path),
        "zip" => parse_archive(path),
        _ => Err(format!("No geometry loader for {extension}")),
    }
}

fn parse_step(path: &Path) -> Result<Mesh, String> {
    const MAX_STEP_BYTES: u64 = 512 * 1024 * 1024;
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_STEP_BYTES {
        return Err("STEP file exceeds the 512 MB safety limit".into());
    }
    let _guard = STEP_TESSELLATION_LOCK
        .lock()
        .map_err(|_| "STEP tessellator lock is unavailable".to_string())?;
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let solids = cadrum::Solid::read_step(&mut file)
        .map_err(|error| format!("Unable to read STEP geometry: {error}"))?;
    if solids.is_empty() {
        return Err("STEP file contains no solid geometry".into());
    }
    let source = cadrum::Solid::mesh(
        &solids,
        cadrum::Tessellation {
            deflection_linear: 0.0025,
            deflection_angular: 0.35,
            relative_linear: true,
        },
    )
    .map_err(|error| format!("Unable to tessellate STEP geometry: {error}"))?;
    if source.indices.len() > 30_000_000 || source.vertices.len() > 10_000_000 {
        return Err("STEP tessellation exceeds the geometry safety limit".into());
    }
    let vertices = source
        .vertices
        .iter()
        .map(|point| Vec3::new(point.x as f32, point.y as f32, point.z as f32))
        .collect::<Vec<_>>();
    let triangles = source
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        .collect::<Vec<_>>();
    ensure_mesh(Mesh {
        vertices,
        triangles,
        ..Mesh::default()
    })
}

pub fn viewer_mesh(
    path: &Path,
    extension: &str,
    plate_index: Option<u32>,
) -> Result<Vec<u8>, String> {
    let mut mesh = match extension {
        "stl" => parse_stl(&std::fs::read(path).map_err(|error| error.to_string())?)?,
        "obj" => parse_obj(&std::fs::read(path).map_err(|error| error.to_string())?)?,
        "3mf" => {
            let file = File::open(path).map_err(|error| error.to_string())?;
            parse_3mf_archive(
                ZipArchive::new(file).map_err(|error| error.to_string())?,
                plate_index
                    .map(PlateSelection::Index)
                    .unwrap_or(PlateSelection::All),
            )?
        }
        "step" | "stp" => parse_step(path)?,
        "zip" => parse_archive(path)?,
        _ => return Err(format!("No interactive renderer for {extension}")),
    };
    prepare_viewer_mesh(&mut mesh)?;
    encode_viewer_mesh(&mesh)
}

fn parse_archive(path: &Path) -> Result<Mesh, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    validate_archive(&mut archive)?;
    for preferred in ["3mf", "stl", "obj"] {
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
            if !entry
                .name()
                .map_err(|error| error.to_string())?
                .to_ascii_lowercase()
                .ends_with(&format!(".{preferred}"))
            {
                continue;
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            return match preferred {
                "3mf" => parse_3mf(&bytes),
                "stl" => parse_stl(&bytes),
                _ => parse_obj(&bytes),
            };
        }
    }
    Err("Archive has no thumbnail-compatible model".into())
}

fn validate_archive<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Result<(), String> {
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("Archive contains too many entries".into());
    }
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().map_err(|error| error.to_string())?;
        let path = Path::new(name.as_ref());
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err("Unsafe archive path rejected".into());
        }
        total = total.saturating_add(entry.size());
        if total > MAX_ARCHIVE_BYTES {
            return Err("Archive expands beyond the safety limit".into());
        }
    }
    Ok(())
}

fn parse_stl(bytes: &[u8]) -> Result<Mesh, String> {
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        let expected = 84_usize.saturating_add(count.saturating_mul(50));
        if count > 0 && expected <= bytes.len() && expected.saturating_add(2) >= bytes.len() {
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
            return Ok(mesh);
        }
    }
    let source = std::str::from_utf8(bytes).map_err(|_| "Invalid STL data".to_string())?;
    let mut mesh = Mesh::default();
    let mut pending = Vec::with_capacity(3);
    for line in source.lines() {
        let values: Vec<_> = line.split_whitespace().collect();
        if values.first().copied() != Some("vertex") || values.len() < 4 {
            continue;
        }
        let point = Vec3::new(
            values[1].parse().map_err(|_| "Invalid STL vertex")?,
            values[2].parse().map_err(|_| "Invalid STL vertex")?,
            values[3].parse().map_err(|_| "Invalid STL vertex")?,
        );
        let index = mesh.vertices.len();
        mesh.vertices.push(point);
        pending.push(index);
        if pending.len() == 3 {
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
        let values: Vec<_> = line.split_whitespace().collect();
        match values.first().copied() {
            Some("v") if values.len() >= 4 => {
                let point = Vec3::new(
                    values[1].parse().map_err(|_| "Invalid OBJ vertex")?,
                    values[2].parse().map_err(|_| "Invalid OBJ vertex")?,
                    values[3].parse().map_err(|_| "Invalid OBJ vertex")?,
                );
                if point.finite() {
                    mesh.vertices.push(point);
                }
            }
            Some("f") if values.len() >= 4 => {
                let mut face = Vec::with_capacity(values.len() - 1);
                for value in &values[1..] {
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
                for index in 1..face.len() - 1 {
                    mesh.triangles.push([face[0], face[index], face[index + 1]]);
                }
            }
            _ => {}
        }
    }
    ensure_mesh(mesh)
}

#[derive(Clone, Copy)]
struct Transform([f32; 12]);

impl Transform {
    fn identity() -> Self {
        Self([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0])
    }

    fn parse(value: Option<String>) -> Self {
        let Some(value) = value else {
            return Self::identity();
        };
        let values: Vec<f32> = value
            .split_whitespace()
            .filter_map(|item| item.parse().ok())
            .collect();
        if values.len() != 12 || !values.iter().all(|value| value.is_finite()) {
            return Self::identity();
        }
        Self([
            values[0], values[3], values[6], values[9], values[1], values[4], values[7],
            values[10], values[2], values[5], values[8], values[11],
        ])
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

    fn translation(self) -> Vec3 {
        Vec3::new(self.0[3], self.0[7], self.0[11])
    }
}

#[derive(Default)]
struct ThreeMfObject {
    name: Option<String>,
    mesh: Mesh,
    triangle_paint: Vec<Option<String>>,
    components: Vec<ThreeMfInstance>,
}

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
    let archive = ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
    parse_3mf_archive(archive, PlateSelection::First)
}

#[derive(Clone, Copy)]
enum PlateSelection {
    All,
    First,
    Index(u32),
}

#[derive(Default)]
struct ThreeMfProject {
    filament_colors: Vec<[u8; 3]>,
    object_extruders: HashMap<u32, usize>,
    object_names: HashMap<u32, String>,
    plate_objects: BTreeMap<u32, HashSet<u32>>,
    plate_bounds: Option<PlateBounds>,
}

#[derive(Clone, Copy, Debug)]
struct PlateBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl PlateBounds {
    fn width(self) -> f32 {
        self.max_x - self.min_x
    }

    fn height(self) -> f32 {
        self.max_y - self.min_y
    }

    fn center(self) -> (f32, f32) {
        (
            (self.min_x + self.max_x) * 0.5,
            (self.min_y + self.max_y) * 0.5,
        )
    }
}

impl ThreeMfProject {
    fn object_color(&self, object_id: u32) -> Option<[u8; 3]> {
        let extruder = *self.object_extruders.get(&object_id)?;
        self.filament_colors.get(extruder.checked_sub(1)?).copied()
    }

    fn selected_objects(&self, selection: PlateSelection) -> Result<Option<&HashSet<u32>>, String> {
        match selection {
            PlateSelection::All => Ok(None),
            PlateSelection::First => Ok(self
                .plate_objects
                .values()
                .find(|objects| !objects.is_empty())),
            PlateSelection::Index(index) => self
                .plate_objects
                .get(&index)
                .map(Some)
                .ok_or_else(|| format!("3MF plate {index} was not found")),
        }
    }
}

fn parse_3mf_archive<R: Read + Seek>(
    mut archive: ZipArchive<R>,
    selection: PlateSelection,
) -> Result<Mesh, String> {
    validate_archive(&mut archive)?;
    let archive_paths: Vec<String> = (0..archive.len())
        .filter_map(|index| {
            archive
                .by_index(index)
                .ok()
                .and_then(|entry| entry.name().ok().map(|name| name.into_owned()))
        })
        .collect();
    let model_paths = archive_paths
        .iter()
        .filter(|name| name.to_ascii_lowercase().ends_with(".model"))
        .cloned()
        .collect::<Vec<_>>();
    if model_paths.is_empty() {
        return Err("3MF has no model document".into());
    }
    let root_path = model_paths
        .iter()
        .find(|path| path.eq_ignore_ascii_case("3D/3dmodel.model"))
        .or_else(|| {
            model_paths
                .iter()
                .find(|path| !path.to_ascii_lowercase().contains("/objects/"))
        })
        .unwrap_or(&model_paths[0])
        .clone();
    let mut parts = HashMap::new();
    for path in &model_paths {
        let entry = archive.by_name(path).map_err(|error| error.to_string())?;
        parts.insert(normalize_3mf_path(path), parse_3mf_part(entry)?);
    }
    let has_face_paint = parts.values().any(|part| {
        part.objects
            .values()
            .any(|object| object.triangle_paint.iter().any(|paint| paint.is_some()))
    });
    let project = parse_3mf_project(&mut archive, &archive_paths)?;
    let selected_objects = project.selected_objects(selection)?;

    let root_path = normalize_3mf_path(&root_path);
    let mut output = Mesh::default();
    let mut stack = HashSet::new();
    if let Some(root) = parts.get(&root_path) {
        emit_build_plates(&project, selection, &root.build, &mut output)?;
    }
    let plate_triangle_count = output.triangles.len();
    let mut target = ThreeMfEmitTarget {
        stack: &mut stack,
        output: &mut output,
        face_colors: has_face_paint,
    };
    if let Some(root) = parts.get(&root_path) {
        for item in &root.build {
            if selected_objects.is_some_and(|objects| !objects.contains(&item.object_id)) {
                continue;
            }
            let viewer_object_id = u32::try_from(target.output.objects.len() + 1)
                .map_err(|_| "3MF contains too many objects for the viewer")?;
            let triangle_start = target.output.triangles.len();
            let object_name = project
                .object_names
                .get(&item.object_id)
                .cloned()
                .or_else(|| {
                    root.objects
                        .get(&item.object_id)
                        .and_then(|object| object.name.clone())
                });
            emit_3mf_object(
                &parts,
                &project,
                &root_path,
                item.object_id,
                viewer_object_id,
                item.transform,
                None,
                &mut target,
            )?;
            finish_viewer_object(target.output, viewer_object_id, triangle_start, object_name)?;
        }
    }
    if target.output.triangles.len() == plate_triangle_count && selected_objects.is_none() {
        for (path, part) in &parts {
            for (&object_id, object) in &part.objects {
                if !object.mesh.triangles.is_empty() {
                    let viewer_object_id = u32::try_from(target.output.objects.len() + 1)
                        .map_err(|_| "3MF contains too many objects for the viewer")?;
                    let triangle_start = target.output.triangles.len();
                    let object_name = project
                        .object_names
                        .get(&object_id)
                        .cloned()
                        .or_else(|| object.name.clone());
                    emit_3mf_object(
                        &parts,
                        &project,
                        path,
                        object_id,
                        viewer_object_id,
                        Transform::identity(),
                        None,
                        &mut target,
                    )?;
                    finish_viewer_object(
                        target.output,
                        viewer_object_id,
                        triangle_start,
                        object_name,
                    )?;
                }
            }
        }
    }
    if target.output.objects.len() > MAX_VIEWER_OBJECTS {
        target.output.objects.clear();
        target.output.triangle_object_ids.fill(0);
    }
    ensure_mesh(output)
}

fn finish_viewer_object(
    output: &mut Mesh,
    object_id: u32,
    triangle_start: usize,
    name: Option<String>,
) -> Result<(), String> {
    let triangles = output
        .triangles
        .get(triangle_start..)
        .ok_or_else(|| "3MF object triangle range is invalid".to_string())?;
    if triangles.is_empty() {
        return Ok(());
    }
    let bounds = Bounds3::from_points(
        triangles
            .iter()
            .flat_map(|triangle| triangle.iter())
            .filter_map(|index| output.vertices.get(*index).copied()),
    )
    .ok_or_else(|| "3MF object bounds are invalid".to_string())?;
    let triangle_count = u32::try_from(triangles.len())
        .map_err(|_| "3MF object has too many triangles for the viewer")?;
    output.objects.push(MeshObject {
        id: object_id,
        triangle_count,
        bounds,
        name,
    });
    Ok(())
}

fn parse_3mf_project<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    paths: &[String],
) -> Result<ThreeMfProject, String> {
    let mut project = ThreeMfProject::default();
    if let Some(path) = find_archive_path(paths, "metadata/project_settings.config") {
        let mut source = String::new();
        archive
            .by_name(path)
            .map_err(|error| error.to_string())?
            .take(10 * 1024 * 1024)
            .read_to_string(&mut source)
            .map_err(|error| error.to_string())?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&source) {
            project.filament_colors = value
                .get("filament_colour")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|value| value.as_str().and_then(parse_hex_color))
                .collect();
            project.plate_bounds = printable_area_bounds(&value);
        }
    }
    if let Some(path) = find_archive_path(paths, "metadata/model_settings.config") {
        let entry = archive.by_name(path).map_err(|error| error.to_string())?;
        parse_bambu_model_settings(entry, &mut project)?;
    }
    Ok(project)
}

fn printable_area_bounds(value: &serde_json::Value) -> Option<PlateBounds> {
    let points = value.get("printable_area")?.as_array()?;
    let coordinates = points
        .iter()
        .filter_map(|point| {
            let (x, y) = point.as_str()?.split_once('x')?;
            Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?))
        })
        .collect::<Vec<_>>();
    let min_x = coordinates.iter().map(|point| point.0).reduce(f32::min)?;
    let max_x = coordinates.iter().map(|point| point.0).reduce(f32::max)?;
    let min_y = coordinates.iter().map(|point| point.1).reduce(f32::min)?;
    let max_y = coordinates.iter().map(|point| point.1).reduce(f32::max)?;
    let bounds = PlateBounds {
        min_x,
        min_y,
        max_x,
        max_y,
    };
    (bounds.width().is_finite()
        && bounds.height().is_finite()
        && bounds.width() > 0.0
        && bounds.height() > 0.0)
        .then_some(bounds)
}

fn find_archive_path<'a>(paths: &'a [String], wanted: &str) -> Option<&'a str> {
    paths
        .iter()
        .find(|path| path.replace('\\', "/").eq_ignore_ascii_case(wanted))
        .map(String::as_str)
}

fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 && value.len() != 8 {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}

fn parse_bambu_model_settings<R: Read>(
    source: R,
    project: &mut ThreeMfProject,
) -> Result<(), String> {
    let mut reader = Reader::from_reader(BufReader::new(source));
    reader.config_mut().trim_text(true);
    let mut current_object = None;
    let mut current_part = None;
    let mut current_plate: Option<(Option<u32>, HashSet<u32>)> = None;
    let mut in_model_instance = false;
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => match xml_local_name(event.name().as_ref()) {
                "object" => {
                    current_object = xml_attribute(&event, "id").and_then(|id| id.parse().ok())
                }
                "part" => current_part = xml_attribute(&event, "id").and_then(|id| id.parse().ok()),
                "plate" => current_plate = Some((None, HashSet::new())),
                "model_instance" => in_model_instance = true,
                "metadata" => apply_bambu_metadata(
                    &event,
                    current_object,
                    current_part,
                    in_model_instance,
                    current_plate.as_mut(),
                    project,
                ),
                _ => {}
            },
            Ok(Event::Empty(event)) => {
                if xml_local_name(event.name().as_ref()) == "metadata" {
                    apply_bambu_metadata(
                        &event,
                        current_object,
                        current_part,
                        in_model_instance,
                        current_plate.as_mut(),
                        project,
                    );
                }
            }
            Ok(Event::End(event)) => match xml_local_name(event.name().as_ref()) {
                "part" => current_part = None,
                "object" => {
                    current_part = None;
                    current_object = None;
                }
                "model_instance" => in_model_instance = false,
                "plate" => {
                    if let Some((Some(index), objects)) = current_plate.take() {
                        project.plate_objects.insert(index, objects);
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("Invalid 3MF model settings: {error}")),
            _ => {}
        }
        buffer.clear();
    }
    Ok(())
}

fn apply_bambu_metadata(
    event: &quick_xml::events::BytesStart<'_>,
    current_object: Option<u32>,
    current_part: Option<u32>,
    in_model_instance: bool,
    current_plate: Option<&mut (Option<u32>, HashSet<u32>)>,
    project: &mut ThreeMfProject,
) {
    let Some(key) = xml_attribute(event, "key") else {
        return;
    };
    let Some(value) = xml_attribute(event, "value") else {
        return;
    };
    if key == "extruder" {
        if let (Some(object_id), Ok(extruder)) = (current_part.or(current_object), value.parse()) {
            project.object_extruders.insert(object_id, extruder);
        }
    }
    if key == "name" && current_part.is_none() {
        if let (Some(object_id), Some(name)) = (current_object, viewer_object_name(&value)) {
            project.object_names.insert(object_id, name);
        }
    }
    let Some((plate_index, objects)) = current_plate else {
        return;
    };
    if key == "plater_id" {
        *plate_index = value.parse().ok();
    } else if in_model_instance && key == "object_id" {
        if let Ok(object_id) = value.parse() {
            objects.insert(object_id);
        }
    }
}

fn parse_3mf_part<R: Read>(source: R) -> Result<ThreeMfPart, String> {
    let mut reader = Reader::from_reader(BufReader::new(source));
    reader.config_mut().trim_text(true);
    let mut part = ThreeMfPart::default();
    let mut current: Option<(u32, ThreeMfObject)> = None;
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let binding = event.name();
                if xml_local_name(binding.as_ref()) == "object" {
                    if let Some(id) = xml_attribute(&event, "id").and_then(|id| id.parse().ok()) {
                        current = Some((
                            id,
                            ThreeMfObject {
                                name: xml_attribute(&event, "name")
                                    .as_deref()
                                    .and_then(viewer_object_name),
                                ..ThreeMfObject::default()
                            },
                        ));
                    }
                }
            }
            Ok(Event::Empty(event)) => {
                let binding = event.name();
                match xml_local_name(binding.as_ref()) {
                    "vertex" => {
                        if let Some((_, object)) = current.as_mut() {
                            let point = Vec3::new(
                                xml_number(&event, "x").unwrap_or_default(),
                                xml_number(&event, "y").unwrap_or_default(),
                                xml_number(&event, "z").unwrap_or_default(),
                            );
                            if point.finite() {
                                object.mesh.vertices.push(point);
                            }
                        }
                    }
                    "triangle" => {
                        if let Some((_, object)) = current.as_mut() {
                            if let (Some(v1), Some(v2), Some(v3)) = (
                                xml_index(&event, "v1"),
                                xml_index(&event, "v2"),
                                xml_index(&event, "v3"),
                            ) {
                                object.mesh.triangles.push([v1, v2, v3]);
                                object.triangle_paint.push(
                                    xml_attribute(&event, "paint_color")
                                        .or_else(|| xml_attribute(&event, "mmu_segmentation")),
                                );
                            }
                        }
                    }
                    "component" => {
                        if let Some((_, object)) = current.as_mut() {
                            if let Some(instance) = parse_3mf_instance(&event) {
                                object.components.push(instance);
                            }
                        }
                    }
                    "item" => {
                        if let Some(instance) = parse_3mf_instance(&event) {
                            part.build.push(instance);
                        }
                    }
                    "object" => {
                        if let Some(id) = xml_attribute(&event, "id").and_then(|id| id.parse().ok())
                        {
                            part.objects.insert(
                                id,
                                ThreeMfObject {
                                    name: xml_attribute(&event, "name")
                                        .as_deref()
                                        .and_then(viewer_object_name),
                                    ..ThreeMfObject::default()
                                },
                            );
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(event)) => {
                let binding = event.name();
                if xml_local_name(binding.as_ref()) == "object" {
                    if let Some((id, object)) = current.take() {
                        part.objects.insert(id, object);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(format!("Invalid 3MF XML: {error}")),
            _ => {}
        }
        buffer.clear();
    }
    Ok(part)
}

fn parse_3mf_instance(event: &quick_xml::events::BytesStart<'_>) -> Option<ThreeMfInstance> {
    Some(ThreeMfInstance {
        object_id: xml_attribute(event, "objectid")?.parse().ok()?,
        path: xml_attribute(event, "path").map(|path| normalize_3mf_path(&path)),
        transform: Transform::parse(xml_attribute(event, "transform")),
    })
}

fn emit_build_plates(
    project: &ThreeMfProject,
    selection: PlateSelection,
    build: &[ThreeMfInstance],
    output: &mut Mesh,
) -> Result<(), String> {
    let Some(bounds) = project.plate_bounds else {
        return Ok(());
    };
    let width = bounds.width();
    let height = bounds.height();
    let plates = match selection {
        PlateSelection::All => project
            .plate_objects
            .values()
            .filter(|objects| !objects.is_empty())
            .collect::<Vec<_>>(),
        PlateSelection::First => project
            .plate_objects
            .values()
            .find(|objects| !objects.is_empty())
            .into_iter()
            .collect(),
        PlateSelection::Index(index) => vec![project
            .plate_objects
            .get(&index)
            .ok_or_else(|| format!("3MF plate {index} was not found"))?],
    };
    let (base_center_x, base_center_y) = bounds.center();
    let spacing = plate_grid_spacing(width, height);
    if plates.is_empty() {
        if !build.is_empty() {
            emit_build_plate(
                output,
                base_center_x,
                base_center_y,
                width,
                height,
                -0.55,
                spacing,
            );
        }
        return Ok(());
    }
    let step_x = width * 1.2;
    let step_y = height * 1.2;
    for objects in plates {
        let Some(anchor) = build.iter().find(|item| objects.contains(&item.object_id)) else {
            continue;
        };
        let translation = anchor.transform.translation();
        let tile_x = ((translation.x - base_center_x) / step_x).round();
        let tile_y = ((translation.y - base_center_y) / step_y).round();
        let center_x = base_center_x + tile_x * step_x;
        let center_y = base_center_y + tile_y * step_y;
        emit_build_plate(output, center_x, center_y, width, height, -0.55, spacing);
    }
    Ok(())
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
    output
        .vertex_colors
        .extend(std::iter::repeat_n(color, outline.len() + 1));
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
    output
        .vertex_colors
        .extend(std::iter::repeat_n(color, outer.len() + inner.len()));
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
    output.vertex_colors.extend(std::iter::repeat_n(color, 4));
    output
        .triangles
        .extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
    output.triangle_colors.extend([color, color]);
    output.triangle_kinds.extend([kind, kind]);
    output.triangle_object_ids.extend([0, 0]);
}

const MAX_PAINT_DEPTH: usize = 64;
const MAX_PAINT_LEAVES: usize = 100_000;

struct PaintedLeaf {
    vertices: [Vec3; 3],
    state: usize,
}

struct PaintedTriangle {
    split: bool,
    leaves: Vec<PaintedLeaf>,
}

fn decode_painted_triangle(value: &str, vertices: [Vec3; 3]) -> Option<PaintedTriangle> {
    if value.is_empty() {
        return None;
    }
    let codes = value
        .bytes()
        .rev()
        .map(paint_hex_digit)
        .collect::<Option<Vec<_>>>()?;
    let split = codes.first().is_some_and(|code| code & 0b11 != 0);
    let mut cursor = 0;
    let mut leaves = Vec::new();
    decode_paint_node(&codes, &mut cursor, vertices, 0, &mut leaves)?;
    (cursor == codes.len()).then_some(PaintedTriangle { split, leaves })
}

fn decode_paint_leaf_state(value: &str) -> Option<usize> {
    let mut codes = value.bytes().rev().map(paint_hex_digit);
    let code = codes.next()??;
    if code & 0b11 != 0 {
        return None;
    }
    let state = if code & 0b1100 == 0b1100 {
        let mut state = 3_usize;
        loop {
            let next = usize::from(codes.next()??);
            if next == 15 {
                state = state.checked_add(15)?;
            } else {
                break state.checked_add(next)?;
            }
        }
    } else {
        usize::from(code >> 2)
    };
    codes.next().is_none().then_some(state)
}

fn paint_hex_digit(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

fn decode_paint_node(
    codes: &[u8],
    cursor: &mut usize,
    vertices: [Vec3; 3],
    depth: usize,
    leaves: &mut Vec<PaintedLeaf>,
) -> Option<()> {
    if depth > MAX_PAINT_DEPTH || leaves.len() >= MAX_PAINT_LEAVES {
        return None;
    }
    let code = *codes.get(*cursor)?;
    *cursor += 1;
    let split_sides = usize::from(code & 0b11);
    if split_sides == 0 {
        let state = if code & 0b1100 == 0b1100 {
            let mut state = 3_usize;
            loop {
                let next = usize::from(*codes.get(*cursor)?);
                *cursor += 1;
                if next == 15 {
                    state = state.checked_add(15)?;
                } else {
                    break state.checked_add(next)?;
                }
            }
        } else {
            usize::from(code >> 2)
        };
        leaves.push(PaintedLeaf { vertices, state });
        return Some(());
    }

    let special_side = usize::from(code >> 2);
    if special_side >= 3 || (split_sides == 3 && special_side != 0) {
        return None;
    }
    let children = split_painted_triangle(vertices, split_sides, special_side)?;
    for child in children.into_iter().rev() {
        decode_paint_node(codes, cursor, child, depth + 1, leaves)?;
    }
    Some(())
}

fn split_painted_triangle(
    vertices: [Vec3; 3],
    split_sides: usize,
    special_side: usize,
) -> Option<Vec<[Vec3; 3]>> {
    let a = vertices[special_side];
    let b = vertices[(special_side + 1) % 3];
    let c = vertices[(special_side + 2) % 3];
    let midpoint = |left: Vec3, right: Vec3| {
        Vec3::new(
            (left.x + right.x) * 0.5,
            (left.y + right.y) * 0.5,
            (left.z + right.z) * 0.5,
        )
    };
    match split_sides {
        1 => {
            let bc = midpoint(b, c);
            Some(vec![[a, b, bc], [bc, c, a]])
        }
        2 => {
            let ab = midpoint(a, b);
            let ac = midpoint(a, c);
            Some(vec![[a, ab, ac], [ab, b, ac], [b, c, ac]])
        }
        3 => {
            let ab = midpoint(a, b);
            let bc = midpoint(b, c);
            let ac = midpoint(a, c);
            Some(vec![[a, ab, ac], [ab, b, bc], [bc, c, ac], [ab, bc, ac]])
        }
        _ => None,
    }
}

struct ThreeMfEmitTarget<'a> {
    stack: &'a mut HashSet<(String, u32)>,
    output: &'a mut Mesh,
    face_colors: bool,
}

fn emit_3mf_object(
    parts: &HashMap<String, ThreeMfPart>,
    project: &ThreeMfProject,
    part_path: &str,
    object_id: u32,
    viewer_object_id: u32,
    transform: Transform,
    inherited_color: Option<[u8; 3]>,
    target: &mut ThreeMfEmitTarget<'_>,
) -> Result<(), String> {
    let key = (part_path.to_string(), object_id);
    if !target.stack.insert(key.clone()) {
        return Err("3MF contains a component cycle".into());
    }
    let part = find_3mf_part(parts, part_path)
        .ok_or_else(|| format!("3MF component part is missing: {part_path}"))?;
    let object = part
        .objects
        .get(&object_id)
        .ok_or_else(|| format!("3MF object is missing: {object_id}"))?;
    let color = project.object_color(object_id).or(inherited_color);
    let default_color = color.unwrap_or([216, 200, 170]);
    let base = target.output.vertices.len();
    target.output.vertices.extend(
        object
            .mesh
            .vertices
            .iter()
            .copied()
            .map(|point| transform.apply(point)),
    );
    target.output.vertex_colors.extend(std::iter::repeat_n(
        default_color,
        object.mesh.vertices.len(),
    ));
    for (triangle_index, triangle) in object.mesh.triangles.iter().enumerate() {
        if triangle
            .iter()
            .any(|index| *index >= object.mesh.vertices.len())
        {
            continue;
        }
        if !target.face_colors {
            target.output.triangles.push([
                base + triangle[0],
                base + triangle[1],
                base + triangle[2],
            ]);
            target.output.triangle_kinds.push(TRIANGLE_KIND_MODEL);
            target.output.triangle_object_ids.push(viewer_object_id);
            continue;
        }

        let paint = object
            .triangle_paint
            .get(triangle_index)
            .and_then(Option::as_deref);
        if let Some(state) = paint.and_then(decode_paint_leaf_state) {
            target.output.triangles.push([
                base + triangle[0],
                base + triangle[1],
                base + triangle[2],
            ]);
            target
                .output
                .triangle_colors
                .push(paint_state_color(project, state, default_color));
            target.output.triangle_kinds.push(TRIANGLE_KIND_MODEL);
            target.output.triangle_object_ids.push(viewer_object_id);
            continue;
        }
        let painted = paint.and_then(|value| {
            decode_painted_triangle(
                value,
                [
                    object.mesh.vertices[triangle[0]],
                    object.mesh.vertices[triangle[1]],
                    object.mesh.vertices[triangle[2]],
                ],
            )
        });
        if painted.as_ref().is_some_and(|painted| painted.split) {
            let painted = painted.expect("painted triangle was checked above");
            for leaf in painted.leaves {
                let leaf_color = paint_state_color(project, leaf.state, default_color);
                let leaf_base = target.output.vertices.len();
                target.output.vertices.extend(
                    leaf.vertices
                        .into_iter()
                        .map(|point| transform.apply(point)),
                );
                target
                    .output
                    .vertex_colors
                    .extend(std::iter::repeat_n(leaf_color, 3));
                target
                    .output
                    .triangles
                    .push([leaf_base, leaf_base + 1, leaf_base + 2]);
                target.output.triangle_colors.push(leaf_color);
                target.output.triangle_kinds.push(TRIANGLE_KIND_MODEL);
                target.output.triangle_object_ids.push(viewer_object_id);
            }
        } else {
            let state = painted
                .and_then(|painted| painted.leaves.first().map(|leaf| leaf.state))
                .unwrap_or_default();
            target.output.triangles.push([
                base + triangle[0],
                base + triangle[1],
                base + triangle[2],
            ]);
            target
                .output
                .triangle_colors
                .push(paint_state_color(project, state, default_color));
            target.output.triangle_kinds.push(TRIANGLE_KIND_MODEL);
            target.output.triangle_object_ids.push(viewer_object_id);
        }
    }
    for component in &object.components {
        let component_path = component.path.as_deref().unwrap_or(part_path);
        emit_3mf_object(
            parts,
            project,
            component_path,
            component.object_id,
            viewer_object_id,
            transform.then(component.transform),
            color,
            target,
        )?;
    }
    target.stack.remove(&key);
    Ok(())
}

fn paint_state_color(project: &ThreeMfProject, state: usize, default_color: [u8; 3]) -> [u8; 3] {
    state
        .checked_sub(1)
        .and_then(|index| project.filament_colors.get(index))
        .copied()
        .unwrap_or(default_color)
}

fn find_3mf_part<'a>(
    parts: &'a HashMap<String, ThreeMfPart>,
    path: &str,
) -> Option<&'a ThreeMfPart> {
    parts.get(path).or_else(|| {
        parts
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(path))
            .map(|(_, part)| part)
    })
}

fn normalize_3mf_path(path: &str) -> String {
    path.trim_start_matches('/').replace('\\', "/")
}

fn xml_local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn xml_attribute(event: &quick_xml::events::BytesStart<'_>, name: &str) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        (xml_local_name(attribute.key.as_ref()) == name)
            .then(|| attribute.value.as_ref().to_string())
    })
}

fn xml_number(event: &quick_xml::events::BytesStart<'_>, name: &str) -> Option<f32> {
    xml_attribute(event, name)?.parse().ok()
}

fn xml_index(event: &quick_xml::events::BytesStart<'_>, name: &str) -> Option<usize> {
    xml_attribute(event, name)?.parse().ok()
}

fn viewer_object_name(value: &str) -> Option<String> {
    let unescaped = quick_xml::escape::unescape(value).ok()?;
    let name = unescaped
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(160)
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn prepare_viewer_mesh(mesh: &mut Mesh) -> Result<(), String> {
    if mesh.triangle_kinds.len() != mesh.triangles.len() {
        mesh.triangle_kinds = vec![TRIANGLE_KIND_MODEL; mesh.triangles.len()];
    }
    if !mesh.objects.is_empty() && mesh.triangle_object_ids.len() != mesh.triangles.len() {
        return Err("Model object metadata is invalid".into());
    }
    let source_bounds = model_bounds(mesh).ok_or_else(|| "Model bounds are invalid".to_string())?;
    let source_size = source_bounds.size();
    if !source_size.finite() || source_size.x < 0.0 || source_size.y < 0.0 || source_size.z < 0.0 {
        return Err("Model bounds are invalid".into());
    }
    mesh.source_bounds = Some(source_bounds);

    if mesh.triangle_colors.len() != mesh.triangles.len() {
        mesh.triangle_colors = mesh
            .triangles
            .iter()
            .map(|triangle| {
                triangle
                    .first()
                    .and_then(|index| mesh.vertex_colors.get(*index))
                    .copied()
                    .unwrap_or([216, 200, 170])
            })
            .collect();
    }

    if !mesh
        .triangle_kinds
        .iter()
        .any(|kind| *kind != TRIANGLE_KIND_MODEL)
    {
        emit_fallback_build_plate(mesh, source_bounds);
    }
    if mesh.objects.is_empty() {
        mesh.triangle_object_ids.clear();
    } else if mesh.triangle_object_ids.len() != mesh.triangles.len() {
        return Err("Model object metadata is invalid".into());
    }
    Ok(())
}

fn model_bounds(mesh: &Mesh) -> Option<Bounds3> {
    Bounds3::from_points(
        mesh.triangles
            .iter()
            .zip(&mesh.triangle_kinds)
            .filter(|(_, kind)| **kind == TRIANGLE_KIND_MODEL)
            .flat_map(|(triangle, _)| triangle)
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

fn encode_viewer_mesh(mesh: &Mesh) -> Result<Vec<u8>, String> {
    let vertex_count = u32::try_from(mesh.vertices.len())
        .map_err(|_| "Model has too many vertices for the viewer".to_string())?;
    let index_count = mesh
        .triangles
        .len()
        .checked_mul(3)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or_else(|| "Model has too many triangles for the viewer".to_string())?;
    let has_triangle_colors = mesh.triangle_colors.len() == mesh.triangles.len();
    let has_vertex_colors = mesh.vertex_colors.len() == mesh.vertices.len();
    let has_semantics = mesh.source_bounds.is_some()
        && has_triangle_colors
        && mesh.triangle_kinds.len() == mesh.triangles.len();
    if has_semantics {
        let bounds = mesh
            .source_bounds
            .expect("viewer bounds were checked above");
        let has_objects = !mesh.objects.is_empty()
            && mesh.objects.len() <= MAX_VIEWER_OBJECTS
            && mesh.triangle_object_ids.len() == mesh.triangles.len();
        if has_objects {
            let object_count = u32::try_from(mesh.objects.len())
                .map_err(|_| "Model has too many objects for the viewer".to_string())?;
            let header_length = 40 + mesh.objects.len() * 32;
            let object_name_bytes = mesh
                .objects
                .iter()
                .map(|object| object.name.as_deref().unwrap_or_default().len())
                .sum::<usize>();
            let has_object_names = object_name_bytes > 0;
            let mut bytes = Vec::with_capacity(
                header_length
                    + mesh.vertices.len() * std::mem::size_of::<[f32; 3]>()
                    + index_count as usize * std::mem::size_of::<u32>()
                    + mesh.triangle_colors.len() * 3
                    + mesh.triangle_kinds.len()
                    + mesh.triangle_object_ids.len() * std::mem::size_of::<u32>()
                    + usize::from(has_object_names) * (8 + mesh.objects.len() * 4)
                    + object_name_bytes,
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
            for (index, object) in mesh.objects.iter().enumerate() {
                if object.id != index as u32 + 1 || object.triangle_count == 0 {
                    return Err("Model object metadata is invalid".into());
                }
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
            append_viewer_geometry(&mut bytes, mesh)?;
            for object_id in &mesh.triangle_object_ids {
                if *object_id > object_count {
                    return Err("Model object metadata is invalid".into());
                }
                bytes.extend_from_slice(&object_id.to_le_bytes());
            }
            if has_object_names {
                bytes.extend_from_slice(b"VNM1");
                bytes.extend_from_slice(&object_count.to_le_bytes());
                for object in &mesh.objects {
                    let name = object.name.as_deref().unwrap_or_default().as_bytes();
                    let name_length = u32::try_from(name.len())
                        .map_err(|_| "3MF object name is too long for the viewer")?;
                    if name_length > 1024 {
                        return Err("3MF object name is too long for the viewer".into());
                    }
                    bytes.extend_from_slice(&name_length.to_le_bytes());
                    bytes.extend_from_slice(name);
                }
            }
            return Ok(bytes);
        }
        let mut bytes = Vec::with_capacity(
            36 + mesh.vertices.len() * std::mem::size_of::<[f32; 3]>()
                + index_count as usize * std::mem::size_of::<u32>()
                + mesh.triangle_colors.len() * 3
                + mesh.triangle_kinds.len(),
        );
        bytes.extend_from_slice(b"VLM4");
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
        append_viewer_geometry(&mut bytes, mesh)?;
        return Ok(bytes);
    }
    let mut bytes = Vec::with_capacity(
        12 + mesh.vertices.len() * std::mem::size_of::<[f32; 3]>()
            + index_count as usize * std::mem::size_of::<u32>()
            + if has_triangle_colors {
                mesh.triangle_colors.len() * 3
            } else if has_vertex_colors {
                mesh.vertex_colors.len() * 3
            } else {
                0
            },
    );
    bytes.extend_from_slice(if has_triangle_colors {
        b"VLM3"
    } else if has_vertex_colors {
        b"VLM2"
    } else {
        b"VLM1"
    });
    bytes.extend_from_slice(&vertex_count.to_le_bytes());
    bytes.extend_from_slice(&index_count.to_le_bytes());
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
    if has_triangle_colors {
        for color in &mesh.triangle_colors {
            bytes.extend_from_slice(color);
        }
    } else if has_vertex_colors {
        for color in &mesh.vertex_colors {
            bytes.extend_from_slice(color);
        }
    }
    Ok(bytes)
}

fn append_viewer_geometry(bytes: &mut Vec<u8>, mesh: &Mesh) -> Result<(), String> {
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
    Ok(())
}

fn ensure_mesh(mesh: Mesh) -> Result<Mesh, String> {
    if mesh.vertices.is_empty() || mesh.triangles.is_empty() {
        Err("Model contains no renderable triangles".into())
    } else {
        Ok(mesh)
    }
}

fn rasterize(mesh: &Mesh) -> Result<Vec<u8>, String> {
    let forward = Vec3::new(1.35, -1.8, 1.1).normalized();
    let right = Vec3::new(-forward.y, forward.x, 0.0).normalized();
    let up = forward.cross(right).normalized();
    let raw: Vec<Projected> = mesh
        .vertices
        .iter()
        .map(|point| Projected {
            x: point.dot(right),
            y: point.dot(up),
            depth: point.dot(forward),
        })
        .collect();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
    );
    for point in &raw {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    let model_width = max_x - min_x;
    let model_height = max_y - min_y;
    if !model_width.is_finite()
        || !model_height.is_finite()
        || model_width <= f32::EPSILON
        || model_height <= f32::EPSILON
    {
        return Err("Model bounds are invalid".into());
    }
    let scale = ((WIDTH as f32 * 0.76) / model_width).min((HEIGHT as f32 * 0.72) / model_height);
    let center_x = (min_x + max_x) * 0.5;
    let center_y = (min_y + max_y) * 0.5;
    let projected: Vec<Projected> = raw
        .into_iter()
        .map(|point| Projected {
            x: WIDTH as f32 * 0.5 + (point.x - center_x) * scale,
            y: HEIGHT as f32 * 0.49 - (point.y - center_y) * scale,
            depth: point.depth,
        })
        .collect();
    let mut image = RgbaImage::new(WIDTH, HEIGHT);
    let mut depth_buffer = vec![f32::NEG_INFINITY; WIDTH as usize * HEIGHT as usize];
    let light = Vec3::new(-0.45, -0.75, 1.0).normalized();
    for (triangle_index, triangle) in mesh.triangles.iter().enumerate() {
        let [a_index, b_index, c_index] = *triangle;
        if [a_index, b_index, c_index]
            .iter()
            .any(|index| *index >= mesh.vertices.len())
        {
            continue;
        }
        let a = projected[a_index];
        let b = projected[b_index];
        let c = projected[c_index];
        let normal = mesh.vertices[b_index]
            .sub(mesh.vertices[a_index])
            .cross(mesh.vertices[c_index].sub(mesh.vertices[a_index]))
            .normalized();
        let diffuse = normal.dot(light).abs();
        let facing = normal.dot(forward).abs();
        let brightness = (0.46 + diffuse * 0.42 + facing * 0.12).clamp(0.38, 1.0);
        let base_color = mesh
            .triangle_colors
            .get(triangle_index)
            .copied()
            .or_else(|| mesh.vertex_colors.get(a_index).copied())
            .unwrap_or([214, 202, 180]);
        let color = [
            (base_color[0] as f32 * brightness) as u8,
            (base_color[1] as f32 * brightness) as u8,
            (base_color[2] as f32 * brightness) as u8,
            255,
        ];
        draw_triangle(&mut image, &mut depth_buffer, a, b, c, color);
    }
    let mut output = Vec::new();
    PngEncoder::new(&mut output)
        .write_image(image.as_raw(), WIDTH, HEIGHT, ColorType::Rgba8.into())
        .map_err(|error| error.to_string())?;
    Ok(output)
}

fn draw_triangle(
    image: &mut RgbaImage,
    depth_buffer: &mut [f32],
    a: Projected,
    b: Projected,
    c: Projected,
    color: [u8; 4],
) {
    let area = edge(a.x, a.y, b.x, b.y, c.x, c.y);
    if area.abs() < 0.01 {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as u32;
    let max_x = a.x.max(b.x).max(c.x).ceil().min(WIDTH as f32 - 1.0) as u32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as u32;
    let max_y = a.y.max(b.y).max(c.y).ceil().min(HEIGHT as f32 - 1.0) as u32;
    let positive = area > 0.0;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = edge(b.x, b.y, c.x, c.y, px, py);
            let w1 = edge(c.x, c.y, a.x, a.y, px, py);
            let w2 = edge(a.x, a.y, b.x, b.y, px, py);
            let inside = if positive {
                w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
            } else {
                w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
            };
            if !inside {
                continue;
            }
            let depth = (w0 * a.depth + w1 * b.depth + w2 * c.depth) / area;
            let offset = y as usize * WIDTH as usize + x as usize;
            if depth > depth_buffer[offset] {
                depth_buffer[offset] = depth;
                image.put_pixel(x, y, Rgba(color));
            }
        }
    }
}

fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (px - ax) * (by - ay) - (py - ay) * (bx - ax)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn renders_an_obj_to_png() {
        let mesh = parse_obj(
            b"v 0 0 0\nv 10 0 0\nv 0 10 0\nv 0 0 10\nf 1 2 3\nf 1 4 2\nf 1 3 4\nf 2 4 3\n",
        )
        .unwrap();
        let png = rasterize(&mesh).unwrap();
        assert!(png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]));
        assert!(png.len() > 1_000);
    }

    #[test]
    fn reads_and_tessellates_step_geometry() {
        let solid = cadrum::Solid::cube(cadrum::DVec3::ZERO, cadrum::DVec3::new(10.0, 20.0, 30.0));
        let mut file = tempfile::NamedTempFile::new().unwrap();
        cadrum::Solid::write_step([&solid], &mut file).unwrap();
        let mesh = parse_step(file.path()).unwrap();
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.triangles.is_empty());
        let analysis = crate::geometry::analyze_file(file.path(), "step").unwrap();
        assert_eq!(analysis.dimensions_mm, [10.0, 20.0, 30.0]);
        assert!((analysis.volume.unwrap() - 6000.0).abs() < 0.1);
    }

    #[test]
    fn builds_a_rounded_grid_plate_without_an_opaque_underside() {
        let mut mesh = Mesh::default();
        emit_build_plate(&mut mesh, 0.0, 0.0, 100.0, 80.0, -0.55, 10.0);
        assert!(mesh.triangles.len() > 100);
        assert!(mesh.vertex_colors.contains(&BUILD_PLATE_SURFACE));
        assert!(mesh.vertex_colors.contains(&BUILD_PLATE_GRID));
        assert!(mesh.vertex_colors.contains(&BUILD_PLATE_GRID_MAJOR));
        assert!(mesh.vertex_colors.contains(&BUILD_PLATE_BORDER));
        assert!(mesh.vertices.iter().all(|vertex| vertex.z >= -0.55));
        assert!(!mesh
            .vertices
            .iter()
            .any(|vertex| vertex.x == 50.0 && vertex.y == 40.0));
    }

    #[test]
    fn anchors_a1_grid_lines_to_physical_ten_millimeter_steps() {
        let lines = metric_grid_lines(128.0, 1.7 * 1.35, 10.0);
        assert_eq!(lines.first(), Some(&(-118.0, 1)));
        assert_eq!(lines.last(), Some(&(122.0, 25)));
        assert_eq!(
            lines
                .iter()
                .filter_map(|(_, index)| (index % 5 == 0).then_some(*index))
                .collect::<Vec<_>>(),
            vec![5, 10, 15, 20, 25]
        );
    }

    #[test]
    fn lays_out_an_a1_mini_as_eighteen_physical_cells() {
        let lines = metric_grid_lines(90.0, 1.7 * 1.35, 10.0);
        assert_eq!(lines.len(), 17);
        assert_eq!(lines.first(), Some(&(-80.0, 1)));
        assert_eq!(lines.last(), Some(&(80.0, 17)));
        assert_eq!(
            lines
                .iter()
                .filter_map(|(_, index)| (index % 5 == 0).then_some(*index))
                .collect::<Vec<_>>(),
            vec![5, 10, 15]
        );
    }

    #[test]
    fn preserves_odd_plate_remainders_without_stretching_the_grid() {
        let lines = metric_grid_lines(61.5, 1.7 * 1.35, 10.0);
        assert_eq!(lines.first(), Some(&(-51.5, 1)));
        assert_eq!(lines.last(), Some(&(58.5, 12)));
        assert!(lines
            .windows(2)
            .all(|pair| (pair[1].0 - pair[0].0 - 10.0).abs() < 0.0001));
    }

    #[test]
    fn preserves_the_origin_and_shape_of_printable_area_metadata() {
        let value = serde_json::json!({
            "printable_area": ["-10x5", "170x5", "170x125", "-10x125"]
        });
        let bounds = printable_area_bounds(&value).unwrap();
        assert_eq!(bounds.width(), 180.0);
        assert_eq!(bounds.height(), 120.0);
        assert_eq!(bounds.center(), (80.0, 65.0));
    }

    #[test]
    fn scales_grid_spacing_only_for_extreme_plate_sizes() {
        assert_eq!(plate_grid_spacing(180.0, 180.0), 10.0);
        assert_eq!(plate_grid_spacing(256.0, 256.0), 10.0);
        assert_eq!(plate_grid_spacing(12_000.0, 8_000.0), 100.0);
    }

    #[test]
    fn adds_a_square_fallback_plate_without_changing_source_bounds() {
        let mut mesh = Mesh {
            vertices: vec![
                Vec3::new(10.0, 20.0, 7.0),
                Vec3::new(30.0, 20.0, 7.0),
                Vec3::new(10.0, 50.0, 9.0),
            ],
            triangles: vec![[0, 1, 2]],
            ..Mesh::default()
        };
        prepare_viewer_mesh(&mut mesh).unwrap();

        let bounds = mesh.source_bounds.unwrap();
        assert_eq!(
            (bounds.min.x, bounds.min.y, bounds.min.z),
            (10.0, 20.0, 7.0)
        );
        assert_eq!(
            (bounds.max.x, bounds.max.y, bounds.max.z),
            (30.0, 50.0, 9.0)
        );
        assert!(mesh
            .triangle_kinds
            .iter()
            .any(|kind| *kind == TRIANGLE_KIND_PLATE_SURFACE));
        let plate_vertices = mesh.vertices.iter().skip(3).collect::<Vec<_>>();
        assert!(plate_vertices.iter().all(|vertex| vertex.z < 7.0));
        let plate_min_x = plate_vertices
            .iter()
            .map(|vertex| vertex.x)
            .reduce(f32::min)
            .unwrap();
        let plate_max_x = plate_vertices
            .iter()
            .map(|vertex| vertex.x)
            .reduce(f32::max)
            .unwrap();
        assert!((plate_min_x + 5.0).abs() < 0.001);
        assert!((plate_max_x - 45.0).abs() < 0.001);
        assert_eq!(&encode_viewer_mesh(&mesh).unwrap()[..4], b"VLM4");
    }

    #[test]
    fn resolves_external_3mf_components_and_build_transforms() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("3D/3dmodel.model", options).unwrap();
        writer.write_all(br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06"><resources><object id="2" name="Assembly &amp; mount"><components><component objectid="1" p:path="/3D/Objects/object_1.model"/></components></object></resources><build><item objectid="2" transform="1 0 0 0 1 0 0 0 1 10 20 30"/></build></model>"#).unwrap();
        writer
            .start_file("3D/Objects/object_1.model", options)
            .unwrap();
        writer.write_all(br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#).unwrap();
        let cursor = writer.finish().unwrap();
        let mesh = parse_3mf(cursor.get_ref()).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(mesh.objects[0].name.as_deref(), Some("Assembly & mount"));
        assert_eq!(
            (mesh.vertices[0].x, mesh.vertices[0].y, mesh.vertices[0].z),
            (10.0, 20.0, 30.0)
        );
        assert_eq!(encode_viewer_mesh(&mesh).unwrap()[..4], *b"VLM2");
    }

    #[test]
    fn selects_a_bambu_plate_and_preserves_its_filament_color() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("3D/3dmodel.model", options).unwrap();
        writer.write_all(br#"<model><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object><object id="2"><mesh><vertices><vertex x="10" y="0" z="0"/><vertex x="11" y="0" z="0"/><vertex x="10" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources><build><item objectid="1"/><item objectid="2" transform="1 0 0 0 1 0 0 0 1 20 0 0"/></build></model>"#).unwrap();
        writer
            .start_file("Metadata/project_settings.config", options)
            .unwrap();
        writer
            .write_all(br##"{"filament_colour":["#ff0000","#00ff00"]}"##)
            .unwrap();
        writer
            .start_file("Metadata/model_settings.config", options)
            .unwrap();
        writer.write_all(br#"<config><object id="1"><metadata key="name" value="Dock"/><metadata key="extruder" value="1"/></object><object id="2"><metadata key="name" value="Right rail"/><metadata key="extruder" value="2"/></object><plate><metadata key="plater_id" value="1"/><model_instance><metadata key="object_id" value="1"/></model_instance></plate><plate><metadata key="plater_id" value="2"/><model_instance><metadata key="object_id" value="2"/></model_instance></plate></config>"#).unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let mut mesh = parse_3mf_archive(archive, PlateSelection::Index(2)).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(mesh.vertices[0].x, 30.0);
        assert!(mesh.vertex_colors.iter().all(|color| *color == [0, 255, 0]));
        assert_eq!(encode_viewer_mesh(&mesh).unwrap()[..4], *b"VLM2");
        assert_eq!(mesh.objects.len(), 1);
        assert_eq!(mesh.objects[0].triangle_count, 1);
        assert_eq!(mesh.objects[0].name.as_deref(), Some("Right rail"));
        assert_eq!(mesh.triangle_object_ids, vec![1]);
        prepare_viewer_mesh(&mut mesh).unwrap();
        let payload = encode_viewer_mesh(&mesh).unwrap();
        assert_eq!(&payload[..4], b"VLM5");
        assert_eq!(u32::from_le_bytes(payload[36..40].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(payload[40..44].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(payload[44..48].try_into().unwrap()), 1);
        assert!(payload
            .windows(b"Right rail".len())
            .any(|value| value == b"Right rail"));

        let archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut all = parse_3mf_archive(archive, PlateSelection::All).unwrap();
        assert_eq!(all.objects.len(), 2);
        assert_eq!(all.objects[0].name.as_deref(), Some("Dock"));
        assert_eq!(all.objects[1].name.as_deref(), Some("Right rail"));
        assert_eq!(all.triangle_object_ids, vec![1, 2]);
        prepare_viewer_mesh(&mut all).unwrap();
        let payload = encode_viewer_mesh(&all).unwrap();
        assert_eq!(&payload[..4], b"VLM5");
        assert_eq!(u32::from_le_bytes(payload[36..40].try_into().unwrap()), 2);
    }

    #[test]
    fn decodes_bambu_paint_states_and_subdivisions() {
        let vertices = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
        ];
        let filament_two = decode_painted_triangle("8", vertices).unwrap();
        assert!(!filament_two.split);
        assert_eq!(filament_two.leaves[0].state, 2);
        let filament_three = decode_painted_triangle("0C", vertices).unwrap();
        assert_eq!(filament_three.leaves[0].state, 3);

        let split = decode_painted_triangle("481", vertices).unwrap();
        assert!(split.split);
        assert_eq!(split.leaves.len(), 2);
        assert_eq!(split.leaves[0].state, 2);
        assert_eq!(split.leaves[1].state, 1);
        assert!(split.leaves.iter().any(|leaf| {
            leaf.vertices
                .iter()
                .any(|vertex| vertex.x == 1.0 && vertex.y == 1.0)
        }));
        assert!(decode_painted_triangle("C", vertices).is_none());
        assert!(decode_painted_triangle("lowercase", vertices).is_none());
    }

    #[test]
    fn preserves_bambu_and_prusa_face_paint_as_triangle_colors() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("3D/3dmodel.model", options).unwrap();
        writer.write_all(br#"<model xmlns:slic3rpe="http://schemas.slic3r.org/3mf/2017/06"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/><vertex x="1" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2" paint_color="8"/><triangle v1="1" v2="3" v3="2" slic3rpe:mmu_segmentation="0C"/></triangles></mesh></object></resources><build><item objectid="1"/></build></model>"#).unwrap();
        writer
            .start_file("Metadata/project_settings.config", options)
            .unwrap();
        writer
            .write_all(br##"{"filament_colour":["#ff0000","#00ff00","#0000ff"]}"##)
            .unwrap();
        writer
            .start_file("Metadata/model_settings.config", options)
            .unwrap();
        writer
            .write_all(
                br#"<config><object id="1"><metadata key="extruder" value="1"/></object></config>"#,
            )
            .unwrap();
        let cursor = writer.finish().unwrap();
        let mesh = parse_3mf(cursor.get_ref()).unwrap();
        assert_eq!(mesh.triangles.len(), 2);
        assert_eq!(mesh.triangle_colors, vec![[0, 255, 0], [0, 0, 255]]);
        assert_eq!(encode_viewer_mesh(&mesh).unwrap()[..4], *b"VLM3");
    }
}
