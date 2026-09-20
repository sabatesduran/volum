use crate::domain::{AssetMetadata, PreviewPayload, ThreeMfMetadata, ThreeMfPlate};
use quick_xml::{
    events::{BytesStart, Event},
    Reader,
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
use zip::ZipArchive;

const MAX_PREVIEW_BYTES: u64 = 100 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_EMBEDDED_THUMBNAIL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_MODEL_XML_BYTES: u64 = 128 * 1024 * 1024;

pub fn parse_metadata(path: &Path, extension: &str) -> AssetMetadata {
    match extension {
        "stl" => parse_stl(path).unwrap_or_else(error_metadata),
        "obj" => parse_obj(path).unwrap_or_else(error_metadata),
        "3mf" => parse_3mf(path).unwrap_or_else(error_metadata),
        "zip" => inspect_zip(path).unwrap_or_else(error_metadata),
        "step" | "stp" => AssetMetadata {
            warning: Some(
                "STEP preview is indexed; tessellation support is not bundled yet.".into(),
            ),
            ..Default::default()
        },
        _ => AssetMetadata::default(),
    }
}

fn error_metadata(error: String) -> AssetMetadata {
    AssetMetadata {
        warning: Some(error),
        ..Default::default()
    }
}

pub fn partial_fingerprint(path: &Path, size: u64) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    let chunk = 64 * 1024;
    let mut buffer = vec![0_u8; chunk.min(size as usize)];
    if !buffer.is_empty() {
        file.read_exact(&mut buffer)
            .map_err(|error| error.to_string())?;
        hasher.update(&buffer);
    }
    if size > chunk as u64 {
        file.seek(SeekFrom::End(-(chunk as i64)))
            .map_err(|error| error.to_string())?;
        let mut tail = vec![0_u8; chunk];
        file.read_exact(&mut tail)
            .map_err(|error| error.to_string())?;
        hasher.update(&tail);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn full_hash(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher).map_err(|error| error.to_string())?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn parse_stl(path: &Path) -> Result<AssetMetadata, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let mut header = [0_u8; 84];
    file.read_exact(&mut header)
        .map_err(|error| error.to_string())?;
    let triangles = u32::from_le_bytes(header[80..84].try_into().unwrap()) as u64;
    let expected = 84_u64.saturating_add(triangles.saturating_mul(50));
    if triangles > 0 && expected <= size && expected.saturating_add(2) >= size {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        let mut facet = [0_u8; 50];
        for _ in 0..triangles {
            file.read_exact(&mut facet)
                .map_err(|error| error.to_string())?;
            for vertex in 0..3 {
                for axis in 0..3 {
                    let offset = 12 + vertex * 12 + axis * 4;
                    let value =
                        f32::from_le_bytes(facet[offset..offset + 4].try_into().unwrap()) as f64;
                    if value.is_finite() {
                        min[axis] = min[axis].min(value);
                        max[axis] = max[axis].max(value);
                    }
                }
            }
        }
        return Ok(AssetMetadata {
            dimensions_mm: dimensions(min, max),
            triangle_count: Some(triangles),
            ..Default::default()
        });
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| error.to_string())?;
    let mut source = String::new();
    file.take(MAX_PREVIEW_BYTES)
        .read_to_string(&mut source)
        .map_err(|error| error.to_string())?;
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut vertices = 0_u64;
    for line in source.lines() {
        let values: Vec<_> = line.split_whitespace().collect();
        if values.first().copied() == Some("vertex") && values.len() >= 4 {
            for axis in 0..3 {
                if let Ok(value) = values[axis + 1].parse::<f64>() {
                    min[axis] = min[axis].min(value);
                    max[axis] = max[axis].max(value);
                }
            }
            vertices += 1;
        }
    }
    if vertices == 0 {
        return Err("No STL facets found".into());
    }
    Ok(AssetMetadata {
        dimensions_mm: dimensions(min, max),
        triangle_count: Some(vertices / 3),
        ..Default::default()
    })
}

fn parse_obj(path: &Path) -> Result<AssetMetadata, String> {
    let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut faces = 0_u64;
    let mut materials = Vec::new();
    for line in source.lines() {
        let values: Vec<_> = line.split_whitespace().collect();
        match values.first().copied() {
            Some("v") if values.len() >= 4 => {
                for axis in 0..3 {
                    if let Ok(value) = values[axis + 1].parse::<f64>() {
                        min[axis] = min[axis].min(value);
                        max[axis] = max[axis].max(value);
                    }
                }
            }
            Some("f") => faces += values.len().saturating_sub(3) as u64 + 1,
            Some("usemtl") if values.len() > 1 => {
                let name = values[1..].join(" ");
                if !materials.contains(&name) {
                    materials.push(name);
                }
            }
            _ => {}
        }
    }
    Ok(AssetMetadata {
        dimensions_mm: dimensions(min, max),
        triangle_count: Some(faces),
        material_names: materials,
        ..Default::default()
    })
}

fn parse_3mf(path: &Path) -> Result<AssetMetadata, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    validate_archive(&mut archive)?;
    let mut model_xml = String::new();
    let mut warning = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if entry.name().to_ascii_lowercase().ends_with(".model") {
            if entry.size() > MAX_MODEL_XML_BYTES {
                warning =
                    Some("3MF model document is too large for indexed geometry metadata".into());
            } else {
                entry
                    .take(MAX_MODEL_XML_BYTES + 1)
                    .read_to_string(&mut model_xml)
                    .map_err(|error| error.to_string())?;
            }
            break;
        }
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut object_count = 0_u64;
    let mut triangle_count = 0_u64;
    if model_xml.is_empty() && warning.is_none() {
        warning = Some("3MF has no model document".into());
    } else if !model_xml.is_empty() {
        let mut reader = Reader::from_str(&model_xml);
        reader.config_mut().trim_text(true);
        loop {
            match reader.read_event() {
                Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                    let name = event.name();
                    if name.as_ref().ends_with(b"object") {
                        object_count += 1;
                    }
                    if name.as_ref().ends_with(b"triangle") {
                        triangle_count += 1;
                    }
                    if name.as_ref().ends_with(b"vertex") {
                        for attribute in event.attributes().flatten() {
                            let axis = match attribute.key.as_ref().last().copied() {
                                Some(b'x') => Some(0),
                                Some(b'y') => Some(1),
                                Some(b'z') => Some(2),
                                _ => None,
                            };
                            if let Some(axis) = axis {
                                if let Ok(text) = std::str::from_utf8(attribute.value.as_ref()) {
                                    if let Ok(value) = text.parse::<f64>() {
                                        min[axis] = min[axis].min(value);
                                        max[axis] = max[axis].max(value);
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(error) => {
                    warning = Some(format!("Invalid 3MF XML: {error}"));
                    break;
                }
                _ => {}
            }
        }
    }
    let filament_grams = extract_3mf_filament_grams(&mut archive);
    let three_mf = extract_3mf_details(&mut archive);
    Ok(AssetMetadata {
        dimensions_mm: dimensions(min, max),
        triangle_count: Some(triangle_count),
        object_count: Some(object_count),
        filament_grams,
        three_mf,
        warning,
        ..Default::default()
    })
}

fn extract_3mf_details<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Option<ThreeMfMetadata> {
    let mut project_settings = None;
    let mut slice_info = None;
    let mut model_settings = None;
    let mut plates = BTreeMap::<u32, ThreeMfPlate>::new();
    for index in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        let name = entry.name().replace('\\', "/");
        let lower = name.to_ascii_lowercase();
        if lower.ends_with("metadata/project_settings.config") && entry.size() <= 10 * 1024 * 1024 {
            let mut source = String::new();
            if entry.read_to_string(&mut source).is_ok() {
                project_settings = Some(source);
            }
        } else if lower.ends_with("metadata/slice_info.config") && entry.size() <= 10 * 1024 * 1024
        {
            let mut source = String::new();
            if entry.read_to_string(&mut source).is_ok() {
                slice_info = Some(source);
            }
        } else if lower.ends_with("metadata/model_settings.config")
            && entry.size() <= 10 * 1024 * 1024
        {
            let mut source = String::new();
            if entry.read_to_string(&mut source).is_ok() {
                model_settings = Some(source);
            }
        } else if let Some(plate_index) = plate_file_index(&lower, ".json") {
            if entry.size() <= 2 * 1024 * 1024 {
                let mut source = String::new();
                if entry.read_to_string(&mut source).is_ok() {
                    merge_plate_json(
                        plates
                            .entry(plate_index)
                            .or_insert_with(|| empty_plate(plate_index)),
                        &source,
                    );
                }
            }
        } else if let Some(plate_index) = plate_file_index(&lower, ".png") {
            plates
                .entry(plate_index)
                .or_insert_with(|| empty_plate(plate_index))
                .thumbnail = true;
        }
    }
    let mut details = ThreeMfMetadata::default();
    if let Some(source) = project_settings {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&source) {
            details.printer = json_text(&value, "printer_model")
                .or_else(|| json_text(&value, "printer_settings_id"));
            details.print_profile = json_text(&value, "print_settings_id");
            details.nozzle_diameter_mm = json_number(&value, "nozzle_diameter");
            details.layer_height_mm = json_number(&value, "layer_height");
            details.slicer_version = json_text(&value, "version");
            details.slicer = Some("Bambu Studio / OrcaSlicer".into());
        }
    }
    if let Some(source) = model_settings {
        merge_model_settings(&source, &mut plates);
    }
    if let Some(source) = slice_info {
        merge_slice_info(&source, &mut details, &mut plates);
    }
    details.plates = plates.into_values().collect();
    (details.slicer.is_some()
        || details.printer.is_some()
        || details.print_profile.is_some()
        || !details.plates.is_empty())
    .then_some(details)
}

fn empty_plate(index: u32) -> ThreeMfPlate {
    ThreeMfPlate {
        index,
        ..Default::default()
    }
}

fn plate_file_index(name: &str, suffix: &str) -> Option<u32> {
    let filename = name.rsplit('/').next()?;
    let rest = filename.strip_prefix("plate_")?.strip_suffix(suffix)?;
    if rest.contains('_') {
        return None;
    }
    rest.parse().ok()
}

fn merge_plate_json(plate: &mut ThreeMfPlate, source: &str) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return;
    };
    plate.bed_type = json_text(&value, "bed_type").map(|value| humanize_setting(&value));
    if let Some(objects) = value.get("bbox_objects").and_then(|value| value.as_array()) {
        plate.object_count = objects.len() as u64;
        plate.object_names = objects
            .iter()
            .filter_map(|object| json_text(object, "name"))
            .take(40)
            .collect();
    }
}

fn merge_model_settings(source: &str, plates: &mut BTreeMap<u32, ThreeMfPlate>) {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(true);
    let mut object_names = BTreeMap::<u32, String>::new();
    let mut current_object = None;
    let mut in_part = false;
    let mut current_plate: Option<(ThreeMfPlate, Vec<u32>)> = None;
    let mut in_model_instance = false;
    let mut next_plate_index = 1_u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => match local_xml_name(&event).as_str() {
                "object" => {
                    current_object = xml_attributes(&event)
                        .get("id")
                        .and_then(|value| value.parse().ok())
                }
                "part" => in_part = true,
                "plate" => {
                    current_plate = Some((empty_plate(next_plate_index), Vec::new()));
                    next_plate_index += 1;
                }
                "model_instance" => in_model_instance = true,
                "metadata" => apply_model_setting_metadata(
                    &event,
                    current_object,
                    in_part,
                    in_model_instance,
                    &mut object_names,
                    current_plate.as_mut(),
                ),
                _ => {}
            },
            Ok(Event::Empty(event)) => {
                if local_xml_name(&event) == "metadata" {
                    apply_model_setting_metadata(
                        &event,
                        current_object,
                        in_part,
                        in_model_instance,
                        &mut object_names,
                        current_plate.as_mut(),
                    );
                }
            }
            Ok(Event::End(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref())
                    .rsplit(':')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                match name.as_str() {
                    "part" => in_part = false,
                    "object" => current_object = None,
                    "model_instance" => in_model_instance = false,
                    "plate" => {
                        if let Some((mut parsed, object_ids)) = current_plate.take() {
                            parsed.object_count = object_ids.len() as u64;
                            parsed.object_names = object_ids
                                .into_iter()
                                .filter_map(|id| object_names.get(&id).cloned())
                                .take(40)
                                .collect();
                            let stored = plates
                                .entry(parsed.index)
                                .or_insert_with(|| empty_plate(parsed.index));
                            merge_plate(stored, parsed);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

fn apply_model_setting_metadata(
    event: &BytesStart<'_>,
    current_object: Option<u32>,
    in_part: bool,
    in_model_instance: bool,
    object_names: &mut BTreeMap<u32, String>,
    current_plate: Option<&mut (ThreeMfPlate, Vec<u32>)>,
) {
    let attributes = xml_attributes(event);
    let key = attributes
        .get("key")
        .map(String::as_str)
        .unwrap_or_default();
    let value = attributes.get("value").cloned().unwrap_or_default();
    if !in_part && key == "name" && !value.trim().is_empty() {
        if let Some(object_id) = current_object {
            object_names.insert(object_id, value.clone());
        }
    }
    let Some((plate, object_ids)) = current_plate else {
        return;
    };
    match key {
        "plater_id" => {
            if let Ok(index) = value.parse() {
                plate.index = index;
            }
        }
        "plater_name" | "plate_name" if !value.trim().is_empty() => plate.name = Some(value),
        "thumbnail_file" => plate.thumbnail = !value.is_empty(),
        "object_id" if in_model_instance => {
            if let Ok(object_id) = value.parse() {
                object_ids.push(object_id);
            }
        }
        _ => {}
    }
}

fn merge_slice_info(
    source: &str,
    details: &mut ThreeMfMetadata,
    plates: &mut BTreeMap<u32, ThreeMfPlate>,
) {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(true);
    let mut in_header = false;
    let mut current: Option<ThreeMfPlate> = None;
    let mut next_index = 1_u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let name = local_xml_name(&event);
                if name == "header" {
                    in_header = true;
                }
                if name == "plate" {
                    current = Some(empty_plate(next_index));
                    next_index += 1;
                }
                apply_slice_element(&event, in_header, details, current.as_mut());
            }
            Ok(Event::Empty(event)) => {
                apply_slice_element(&event, in_header, details, current.as_mut())
            }
            Ok(Event::End(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref())
                    .rsplit(':')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                if name == "header" {
                    in_header = false;
                }
                if name == "plate" {
                    if let Some(parsed) = current.take() {
                        let stored = plates
                            .entry(parsed.index)
                            .or_insert_with(|| empty_plate(parsed.index));
                        merge_plate(stored, parsed);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
}

fn apply_slice_element(
    event: &BytesStart<'_>,
    in_header: bool,
    details: &mut ThreeMfMetadata,
    plate: Option<&mut ThreeMfPlate>,
) {
    let name = local_xml_name(event);
    let attributes = xml_attributes(event);
    if in_header && name == "header_item" {
        if attributes
            .get("key")
            .is_some_and(|key| key == "X-BBL-Client-Version")
        {
            details.slicer_version = attributes.get("value").cloned();
        }
        if attributes
            .get("key")
            .is_some_and(|key| key == "X-BBL-Client-Type")
            && details.slicer.is_none()
        {
            details.slicer = Some("Bambu Studio / OrcaSlicer".into());
        }
    }
    let Some(plate) = plate else { return };
    if name == "metadata" {
        let key = attributes
            .get("key")
            .map(String::as_str)
            .unwrap_or_default();
        let value = attributes.get("value").cloned().unwrap_or_default();
        match key {
            "plater_id" => {
                if let Ok(index) = value.parse() {
                    plate.index = index;
                }
            }
            "plater_name" | "plate_name" if !value.trim().is_empty() => plate.name = Some(value),
            "prediction" => {
                plate.print_time_seconds = value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .map(|value| value.round() as u64)
            }
            "thumbnail_file" => plate.thumbnail = !value.is_empty(),
            _ => {}
        }
    } else if name == "filament" {
        if let Some(value) = attributes
            .get("used_g")
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            plate.filament_grams = Some(plate.filament_grams.unwrap_or_default() + value);
        }
        if let Some(material) = attributes
            .get("type")
            .filter(|value| !value.trim().is_empty())
        {
            if !plate.material_names.contains(material) {
                plate.material_names.push(material.clone());
            }
        }
    } else if name == "object" {
        plate.object_count += 1;
        if let Some(object_name) = attributes
            .get("name")
            .filter(|value| !value.trim().is_empty())
        {
            if plate.object_names.len() < 40 {
                plate.object_names.push(object_name.clone());
            }
        }
    }
}

fn merge_plate(stored: &mut ThreeMfPlate, parsed: ThreeMfPlate) {
    if parsed.name.is_some() {
        stored.name = parsed.name;
    }
    if parsed.object_count > 0 {
        stored.object_count = parsed.object_count;
    }
    if !parsed.object_names.is_empty() {
        stored.object_names = parsed.object_names;
    }
    if parsed.bed_type.is_some() {
        stored.bed_type = parsed.bed_type;
    }
    if parsed.filament_grams.is_some() {
        stored.filament_grams = parsed.filament_grams;
    }
    if parsed.print_time_seconds.is_some() {
        stored.print_time_seconds = parsed.print_time_seconds;
    }
    if !parsed.material_names.is_empty() {
        stored.material_names = parsed.material_names;
    }
    stored.thumbnail |= parsed.thumbnail;
}

fn local_xml_name(event: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(event.name().as_ref())
        .rsplit(':')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn xml_attributes(event: &BytesStart<'_>) -> std::collections::HashMap<String, String> {
    event
        .attributes()
        .flatten()
        .filter_map(|attribute| {
            let key = String::from_utf8_lossy(attribute.key.as_ref())
                .rsplit(':')
                .next()?
                .to_string();
            let value = std::str::from_utf8(attribute.value.as_ref())
                .ok()?
                .to_string();
            Some((key, value))
        })
        .collect()
}

fn json_text(value: &serde_json::Value, key: &str) -> Option<String> {
    let value = value.get(key)?;
    let text = value
        .as_str()
        .or_else(|| value.as_array()?.first()?.as_str())?
        .trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn json_number(value: &serde_json::Value, key: &str) -> Option<f64> {
    let value = value.get(key)?;
    let value = value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .or_else(|| {
            let first = value.as_array()?.first()?;
            first.as_f64().or_else(|| first.as_str()?.parse().ok())
        })?;
    value.is_finite().then_some(value)
}

fn humanize_setting(value: &str) -> String {
    value
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_3mf_filament_grams<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;
    for index in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        let name = entry.name().to_ascii_lowercase();
        if !name.ends_with("slice_info.config") || entry.size() > 5 * 1024 * 1024 {
            continue;
        }
        let mut source = String::new();
        if entry.read_to_string(&mut source).is_err() {
            continue;
        }
        if let Some(value) = filament_grams_from_slice_info(&source) {
            total += value;
            found = true;
        }
    }
    found.then_some(total)
}

fn filament_grams_from_slice_info(source: &str) -> Option<f64> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(true);
    let mut total = 0.0;
    let mut found = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event))
                if local_xml_name(&event) == "filament" =>
            {
                if let Some(value) = xml_attributes(&event)
                    .get("used_g")
                    .and_then(|value| value.trim().parse::<f64>().ok())
                    .filter(|value| value.is_finite() && *value >= 0.0)
                {
                    total += value;
                    found = true;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }
    found.then_some(total)
}

pub fn embedded_3mf_thumbnail(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("Archive contains too many entries".into());
    }
    let mut candidate: Option<(usize, u8)> = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().to_ascii_lowercase();
        if entry.size() > MAX_EMBEDDED_THUMBNAIL_BYTES || !name.ends_with(".png") {
            continue;
        }
        let score = if name.ends_with("3d/thumbnail.png") {
            100
        } else if name.ends_with("thumbnail.png") {
            95
        } else if name.ends_with("metadata/plate_1.png") || name == "plate_1.png" {
            90
        } else if name.contains("/plate_")
            && !name.contains("_small")
            && !name.contains("_no_light")
        {
            70
        } else {
            0
        };
        if score > candidate.map(|(_, current)| current).unwrap_or_default() {
            candidate = Some((index, score));
        }
    }
    let Some((index, score)) = candidate else {
        return Ok(None);
    };
    if score == 0 {
        return Ok(None);
    }
    let entry = archive.by_index(index).map_err(|error| error.to_string())?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .take(MAX_EMBEDDED_THUMBNAIL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 <= MAX_EMBEDDED_THUMBNAIL_BYTES
        && bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10])
    {
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}

pub fn embedded_3mf_plate_thumbnail(
    path: &Path,
    plate_index: u32,
) -> Result<Option<Vec<u8>>, String> {
    if plate_index == 0 {
        return Err("Plate numbers start at 1".into());
    }
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("Archive contains too many entries".into());
    }
    let expected = format!("plate_{plate_index}.png");
    let mut candidate = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let filename = entry
            .name()
            .replace('\\', "/")
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if (filename == expected || (plate_index == 1 && filename == "thumbnail.png"))
            && entry.size() <= MAX_EMBEDDED_THUMBNAIL_BYTES
        {
            candidate = Some(index);
            if filename == expected {
                break;
            }
        }
    }
    let Some(index) = candidate else {
        return Ok(None);
    };
    let entry = archive.by_index(index).map_err(|error| error.to_string())?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .take(MAX_EMBEDDED_THUMBNAIL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 <= MAX_EMBEDDED_THUMBNAIL_BYTES
        && bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10])
    {
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}

fn inspect_zip(path: &Path) -> Result<AssetMetadata, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
    validate_archive(&mut archive)?;
    let supported = (0..archive.len())
        .filter_map(|index| {
            archive
                .by_index(index)
                .ok()
                .map(|entry| entry.name().to_ascii_lowercase())
        })
        .filter(|name| {
            [".stl", ".3mf", ".obj", ".step", ".stp"]
                .iter()
                .any(|extension| name.ends_with(extension))
        })
        .count();
    Ok(AssetMetadata {
        object_count: Some(supported as u64),
        warning: if supported == 0 {
            Some("Archive contains no supported model files".into())
        } else {
            None
        },
        ..Default::default()
    })
}

fn validate_archive<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<(), String> {
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

pub fn preview_payload(
    path: &Path,
    extension: &str,
    source_asset_id: String,
) -> Result<PreviewPayload, String> {
    if extension == "zip" {
        let file = File::open(path).map_err(|error| error.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|error| error.to_string())?;
        validate_archive(&mut archive)?;
        let mut candidate = None;
        for preferred in ["3mf", "stl", "obj"] {
            for index in 0..archive.len() {
                let entry = archive.by_index(index).map_err(|error| error.to_string())?;
                if entry
                    .name()
                    .to_ascii_lowercase()
                    .ends_with(&format!(".{preferred}"))
                    && entry.size() <= MAX_PREVIEW_BYTES
                {
                    candidate = Some((index, preferred));
                    break;
                }
            }
            if candidate.is_some() {
                break;
            }
        }
        let (index, format) =
            candidate.ok_or_else(|| "Archive has no previewable model".to_string())?;
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        return Ok(PreviewPayload {
            extension: format.into(),
            bytes,
            source_asset_id,
        });
    }
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_PREVIEW_BYTES {
        return Err("This file is too large for an interactive preview".into());
    }
    if !matches!(extension, "stl" | "obj" | "3mf") {
        return Err(format!(
            "Interactive {} preview is not available",
            extension.to_ascii_uppercase()
        ));
    }
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(PreviewPayload {
        extension: extension.into(),
        bytes,
        source_asset_id,
    })
}

fn dimensions(min: [f64; 3], max: [f64; 3]) -> Option<[f64; 3]> {
    if min.iter().all(|value| value.is_finite()) && max.iter().all(|value| value.is_finite()) {
        Some([max[0] - min[0], max[1] - min[1], max[2] - min[2]])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn rejects_parent_archive_paths() {
        let cursor = std::io::Cursor::new(Vec::<u8>::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file("../escape.stl", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut writer, b"solid x\nendsolid").unwrap();
        let cursor = writer.finish().unwrap();
        let mut archive = ZipArchive::new(cursor).unwrap();
        assert!(validate_archive(&mut archive).is_err());
    }

    #[test]
    fn reads_binary_stl_dimensions() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&[0_u8; 80]).unwrap();
        file.write_all(&1_u32.to_le_bytes()).unwrap();
        file.write_all(&[0_u8; 12]).unwrap();
        for value in [0_f32, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 20.0, 5.0] {
            file.write_all(&value.to_le_bytes()).unwrap();
        }
        file.write_all(&0_u16.to_le_bytes()).unwrap();
        let metadata = parse_stl(file.path()).unwrap();
        assert_eq!(metadata.triangle_count, Some(1));
        assert_eq!(metadata.dimensions_mm, Some([10.0, 20.0, 5.0]));
    }

    #[test]
    fn reads_slicer_plate_as_embedded_thumbnail() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        {
            let mut writer = zip::ZipWriter::new(&mut file);
            writer
                .start_file(
                    "Metadata/plate_1.png",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer
                .write_all(&[137, 80, 78, 71, 13, 10, 26, 10, 1, 2, 3])
                .unwrap();
            writer.finish().unwrap();
        }
        assert!(embedded_3mf_thumbnail(file.path()).unwrap().is_some());
    }

    #[test]
    fn only_uses_explicit_slicer_filament_weights() {
        let cursor = std::io::Cursor::new(Vec::<u8>::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file(
                "Metadata/slice_info.config",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer
            .write_all(br#"<config><filament used_g="1.25"/><s:filament xmlns:s="urn:test" used_g='2.5'></s:filament><metadata filament_spool_weight="1000"/></config>"#)
            .unwrap();
        let cursor = writer.finish().unwrap();
        let mut archive = ZipArchive::new(cursor).unwrap();
        assert_eq!(extract_3mf_filament_grams(&mut archive), Some(3.75));
    }

    #[test]
    fn reads_plate_membership_from_bambu_model_settings() {
        let mut plates = BTreeMap::new();
        merge_model_settings(
            r#"<config><object id="7"><metadata key="name" value="Portrait"/><part id="1"><metadata key="name" value="Layer"/></part></object><plate><metadata key="plater_id" value="2"/><metadata key="plater_name" value="Front"/><model_instance><metadata key="object_id" value="7"/></model_instance></plate></config>"#,
            &mut plates,
        );
        let plate = plates.get(&2).unwrap();
        assert_eq!(plate.name.as_deref(), Some("Front"));
        assert_eq!(plate.object_count, 1);
        assert_eq!(plate.object_names, vec!["Portrait"]);
    }

    #[test]
    fn reads_3mf_plates_and_print_profile() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        {
            let mut writer = zip::ZipWriter::new(&mut file);
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("3D/3dmodel.model", options).unwrap();
            writer.write_all(br#"<model><resources><object id="1"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="10" y="0" z="0"/><vertex x="0" y="5" z="2"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#).unwrap();
            writer
                .start_file("Metadata/project_settings.config", options)
                .unwrap();
            writer.write_all(br#"{"printer_model":"Bambu Lab A1","print_settings_id":"0.20mm Standard","nozzle_diameter":["0.4"],"layer_height":"0.2","version":"2.3"}"#).unwrap();
            writer.start_file("Metadata/plate_1.json", options).unwrap();
            writer
                .write_all(br#"{"bed_type":"textured_plate","bbox_objects":[{"name":"Bracket"}]}"#)
                .unwrap();
            writer
                .start_file("Metadata/slice_info.config", options)
                .unwrap();
            writer.write_all(br#"<config><header><header_item key="X-BBL-Client-Version" value="2.3"/></header><plate><metadata key="plater_id" value="1"/><metadata key="plater_name" value="Main"/><metadata key="prediction" value="3661"/><metadata key="thumbnail_file" value="Metadata/plate_1.png"/><filament type="PLA" used_g="12.5"/></plate></config>"#).unwrap();
            writer.finish().unwrap();
        }
        let metadata = parse_3mf(file.path()).unwrap();
        let details = metadata.three_mf.unwrap();
        assert_eq!(metadata.filament_grams, Some(12.5));
        assert_eq!(details.printer.as_deref(), Some("Bambu Lab A1"));
        assert_eq!(details.nozzle_diameter_mm, Some(0.4));
        assert_eq!(details.plates.len(), 1);
        assert_eq!(details.plates[0].name.as_deref(), Some("Main"));
        assert_eq!(details.plates[0].object_names, vec!["Bracket"]);
        assert_eq!(details.plates[0].print_time_seconds, Some(3661));
        assert_eq!(details.plates[0].filament_grams, Some(12.5));
    }
}
