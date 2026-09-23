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

#[derive(Default)]
pub(crate) struct Mesh {
    pub(crate) vertices: Vec<Vec3>,
    pub(crate) triangles: Vec<[usize; 3]>,
    pub(crate) vertex_colors: Vec<[u8; 3]>,
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
        vertex_colors: Vec::new(),
    })
}

pub fn viewer_mesh(
    path: &Path,
    extension: &str,
    plate_index: Option<u32>,
) -> Result<Vec<u8>, String> {
    let mesh = match extension {
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
        let path = Path::new(entry.name());
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
                vertex_colors: Vec::new(),
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
    mesh: Mesh,
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
    plate_objects: BTreeMap<u32, HashSet<u32>>,
    plate_size: Option<[f32; 2]>,
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
                .map(|entry| entry.name().to_string())
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
    };
    if let Some(root) = parts.get(&root_path) {
        for item in &root.build {
            if selected_objects.is_some_and(|objects| !objects.contains(&item.object_id)) {
                continue;
            }
            emit_3mf_object(
                &parts,
                &project,
                &root_path,
                item.object_id,
                item.transform,
                None,
                &mut target,
            )?;
        }
    }
    if target.output.triangles.len() == plate_triangle_count && selected_objects.is_none() {
        for (path, part) in &parts {
            for (&object_id, object) in &part.objects {
                if !object.mesh.triangles.is_empty() {
                    emit_3mf_object(
                        &parts,
                        &project,
                        path,
                        object_id,
                        Transform::identity(),
                        None,
                        &mut target,
                    )?;
                }
            }
        }
    }
    ensure_mesh(output)
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
            project.plate_size = printable_area_size(&value);
        }
    }
    if let Some(path) = find_archive_path(paths, "metadata/model_settings.config") {
        let entry = archive.by_name(path).map_err(|error| error.to_string())?;
        parse_bambu_model_settings(entry, &mut project)?;
    }
    Ok(project)
}

fn printable_area_size(value: &serde_json::Value) -> Option<[f32; 2]> {
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
    let width = max_x - min_x;
    let height = max_y - min_y;
    (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
        .then_some([width, height])
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
                b"object" => {
                    current_object = xml_attribute(&event, b"id").and_then(|id| id.parse().ok())
                }
                b"part" => {
                    current_part = xml_attribute(&event, b"id").and_then(|id| id.parse().ok())
                }
                b"plate" => current_plate = Some((None, HashSet::new())),
                b"model_instance" => in_model_instance = true,
                b"metadata" => apply_bambu_metadata(
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
                if xml_local_name(event.name().as_ref()) == b"metadata" {
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
                b"part" => current_part = None,
                b"object" => {
                    current_part = None;
                    current_object = None;
                }
                b"model_instance" => in_model_instance = false,
                b"plate" => {
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
    let Some(key) = xml_attribute(event, b"key") else {
        return;
    };
    let Some(value) = xml_attribute(event, b"value") else {
        return;
    };
    if key == "extruder" {
        if let (Some(object_id), Ok(extruder)) = (current_part.or(current_object), value.parse()) {
            project.object_extruders.insert(object_id, extruder);
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
                if xml_local_name(binding.as_ref()) == b"object" {
                    if let Some(id) = xml_attribute(&event, b"id").and_then(|id| id.parse().ok()) {
                        current = Some((id, ThreeMfObject::default()));
                    }
                }
            }
            Ok(Event::Empty(event)) => {
                let binding = event.name();
                match xml_local_name(binding.as_ref()) {
                    b"vertex" => {
                        if let Some((_, object)) = current.as_mut() {
                            let point = Vec3::new(
                                xml_number(&event, b"x").unwrap_or_default(),
                                xml_number(&event, b"y").unwrap_or_default(),
                                xml_number(&event, b"z").unwrap_or_default(),
                            );
                            if point.finite() {
                                object.mesh.vertices.push(point);
                            }
                        }
                    }
                    b"triangle" => {
                        if let Some((_, object)) = current.as_mut() {
                            if let (Some(v1), Some(v2), Some(v3)) = (
                                xml_index(&event, b"v1"),
                                xml_index(&event, b"v2"),
                                xml_index(&event, b"v3"),
                            ) {
                                object.mesh.triangles.push([v1, v2, v3]);
                            }
                        }
                    }
                    b"component" => {
                        if let Some((_, object)) = current.as_mut() {
                            if let Some(instance) = parse_3mf_instance(&event) {
                                object.components.push(instance);
                            }
                        }
                    }
                    b"item" => {
                        if let Some(instance) = parse_3mf_instance(&event) {
                            part.build.push(instance);
                        }
                    }
                    b"object" => {
                        if let Some(id) =
                            xml_attribute(&event, b"id").and_then(|id| id.parse().ok())
                        {
                            part.objects.insert(id, ThreeMfObject::default());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(event)) => {
                let binding = event.name();
                if xml_local_name(binding.as_ref()) == b"object" {
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
        object_id: xml_attribute(event, b"objectid")?.parse().ok()?,
        path: xml_attribute(event, b"path").map(|path| normalize_3mf_path(&path)),
        transform: Transform::parse(xml_attribute(event, b"transform")),
    })
}

fn emit_build_plates(
    project: &ThreeMfProject,
    selection: PlateSelection,
    build: &[ThreeMfInstance],
    output: &mut Mesh,
) -> Result<(), String> {
    let Some([width, height]) = project.plate_size else {
        return Ok(());
    };
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
    let step_x = width * 1.2;
    let step_y = height * 1.2;
    for objects in plates {
        let Some(anchor) = build.iter().find(|item| objects.contains(&item.object_id)) else {
            continue;
        };
        let translation = anchor.transform.translation();
        let tile_x = ((translation.x - width * 0.5) / step_x).round();
        let tile_y = ((translation.y - height * 0.5) / step_y).round();
        let center_x = width * 0.5 + tile_x * step_x;
        let center_y = height * 0.5 + tile_y * step_y;
        emit_build_plate(output, center_x, center_y, width, height);
    }
    Ok(())
}

fn emit_build_plate(output: &mut Mesh, center_x: f32, center_y: f32, width: f32, height: f32) {
    let surface = -0.55;
    let radius = (width.min(height) * 0.028)
        .clamp(2.0, 8.0)
        .min(width * 0.45)
        .min(height * 0.45);
    let outline = rounded_rectangle(center_x, center_y, width, height, radius, surface);
    emit_colored_fan(
        output,
        Vec3::new(center_x, center_y, surface),
        &outline,
        BUILD_PLATE_SURFACE,
    );

    let border_width = 1.7_f32.min(radius * 0.4);
    let inner = rounded_rectangle(
        center_x,
        center_y,
        width - border_width * 2.0,
        height - border_width * 2.0,
        radius - border_width,
        surface + 0.04,
    );
    emit_colored_ring(output, &outline, &inner, BUILD_PLATE_BORDER);

    let spacing = 10.0;
    let half_width = width * 0.5;
    let half_height = height * 0.5;
    let max_vertical = (half_width / spacing).floor() as i32;
    let max_horizontal = (half_height / spacing).floor() as i32;
    for step in -max_vertical..=max_vertical {
        let local_x = step as f32 * spacing;
        if local_x.abs() >= half_width - border_width {
            continue;
        }
        let major = step % 5 == 0;
        let thickness = if major { 0.62 } else { 0.34 };
        let extent =
            rounded_axis_extent(local_x, half_width, half_height, radius) - border_width * 1.35;
        emit_colored_quad(
            output,
            [
                Vec3::new(
                    center_x + local_x - thickness * 0.5,
                    center_y - extent,
                    surface + 0.07,
                ),
                Vec3::new(
                    center_x + local_x + thickness * 0.5,
                    center_y - extent,
                    surface + 0.07,
                ),
                Vec3::new(
                    center_x + local_x + thickness * 0.5,
                    center_y + extent,
                    surface + 0.07,
                ),
                Vec3::new(
                    center_x + local_x - thickness * 0.5,
                    center_y + extent,
                    surface + 0.07,
                ),
            ],
            if major {
                BUILD_PLATE_GRID_MAJOR
            } else {
                BUILD_PLATE_GRID
            },
        );
    }
    for step in -max_horizontal..=max_horizontal {
        let local_y = step as f32 * spacing;
        if local_y.abs() >= half_height - border_width {
            continue;
        }
        let major = step % 5 == 0;
        let thickness = if major { 0.62 } else { 0.34 };
        let extent =
            rounded_axis_extent(local_y, half_height, half_width, radius) - border_width * 1.35;
        emit_colored_quad(
            output,
            [
                Vec3::new(
                    center_x - extent,
                    center_y + local_y - thickness * 0.5,
                    surface + 0.07,
                ),
                Vec3::new(
                    center_x + extent,
                    center_y + local_y - thickness * 0.5,
                    surface + 0.07,
                ),
                Vec3::new(
                    center_x + extent,
                    center_y + local_y + thickness * 0.5,
                    surface + 0.07,
                ),
                Vec3::new(
                    center_x - extent,
                    center_y + local_y + thickness * 0.5,
                    surface + 0.07,
                ),
            ],
            if major {
                BUILD_PLATE_GRID_MAJOR
            } else {
                BUILD_PLATE_GRID
            },
        );
    }
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

fn emit_colored_fan(output: &mut Mesh, center: Vec3, outline: &[Vec3], color: [u8; 3]) {
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
    }
}

fn emit_colored_ring(output: &mut Mesh, outer: &[Vec3], inner: &[Vec3], color: [u8; 3]) {
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
    }
}

fn emit_colored_quad(output: &mut Mesh, vertices: [Vec3; 4], color: [u8; 3]) {
    let base = output.vertices.len();
    output.vertices.extend(vertices);
    output.vertex_colors.extend(std::iter::repeat_n(color, 4));
    output
        .triangles
        .extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
}

struct ThreeMfEmitTarget<'a> {
    stack: &'a mut HashSet<(String, u32)>,
    output: &'a mut Mesh,
}

fn emit_3mf_object(
    parts: &HashMap<String, ThreeMfPart>,
    project: &ThreeMfProject,
    part_path: &str,
    object_id: u32,
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
        color.unwrap_or([216, 200, 170]),
        object.mesh.vertices.len(),
    ));
    target.output.triangles.extend(
        object
            .mesh
            .triangles
            .iter()
            .filter(|triangle| {
                triangle
                    .iter()
                    .all(|index| *index < object.mesh.vertices.len())
            })
            .map(|triangle| [base + triangle[0], base + triangle[1], base + triangle[2]]),
    );
    for component in &object.components {
        let component_path = component.path.as_deref().unwrap_or(part_path);
        emit_3mf_object(
            parts,
            project,
            component_path,
            component.object_id,
            transform.then(component.transform),
            color,
            target,
        )?;
    }
    target.stack.remove(&key);
    Ok(())
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

fn xml_local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|value| *value == b':').next().unwrap_or(name)
}

fn xml_attribute(event: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        (xml_local_name(attribute.key.as_ref()) == name)
            .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
    })
}

fn xml_number(event: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<f32> {
    xml_attribute(event, name)?.parse().ok()
}

fn xml_index(event: &quick_xml::events::BytesStart<'_>, name: &[u8]) -> Option<usize> {
    xml_attribute(event, name)?.parse().ok()
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
    let has_colors = mesh.vertex_colors.len() == mesh.vertices.len();
    let mut bytes = Vec::with_capacity(
        12 + mesh.vertices.len() * std::mem::size_of::<[f32; 3]>()
            + index_count as usize * std::mem::size_of::<u32>()
            + if has_colors {
                mesh.vertex_colors.len() * 3
            } else {
                0
            },
    );
    bytes.extend_from_slice(if has_colors { b"VLM2" } else { b"VLM1" });
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
    if has_colors {
        for color in &mesh.vertex_colors {
            bytes.extend_from_slice(color);
        }
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
    for triangle in &mesh.triangles {
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
            .vertex_colors
            .get(a_index)
            .copied()
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
        emit_build_plate(&mut mesh, 0.0, 0.0, 100.0, 80.0);
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
    fn resolves_external_3mf_components_and_build_transforms() {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("3D/3dmodel.model", options).unwrap();
        writer.write_all(br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06"><resources><object id="2"><components><component objectid="1" p:path="/3D/Objects/object_1.model"/></components></object></resources><build><item objectid="2" transform="1 0 0 0 1 0 0 0 1 10 20 30"/></build></model>"#).unwrap();
        writer
            .start_file("3D/Objects/object_1.model", options)
            .unwrap();
        writer.write_all(br#"<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#).unwrap();
        let cursor = writer.finish().unwrap();
        let mesh = parse_3mf(cursor.get_ref()).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
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
        writer.write_all(br#"<config><object id="1"><metadata key="extruder" value="1"/></object><object id="2"><metadata key="extruder" value="2"/></object><plate><metadata key="plater_id" value="1"/><model_instance><metadata key="object_id" value="1"/></model_instance></plate><plate><metadata key="plater_id" value="2"/><model_instance><metadata key="object_id" value="2"/></model_instance></plate></config>"#).unwrap();
        let cursor = writer.finish().unwrap();
        let archive = ZipArchive::new(cursor).unwrap();
        let mesh = parse_3mf_archive(archive, PlateSelection::Index(2)).unwrap();
        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(mesh.vertices[0].x, 30.0);
        assert!(mesh.vertex_colors.iter().all(|color| *color == [0, 255, 0]));
        assert_eq!(encode_viewer_mesh(&mesh).unwrap()[..4], *b"VLM2");
    }
}
