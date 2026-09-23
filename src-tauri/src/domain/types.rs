use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRoot {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub status: String,
    pub last_scan_at: Option<String>,
    pub model_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub root_id: String,
    pub parent_id: Option<String>,
    pub relative_path: String,
    pub name: String,
    pub model_count: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions_mm: Option<[f64; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triangle_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filament_grams: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_area_mm2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume_mm3: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub three_mf: Option<ThreeMfMetadata>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeMfMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slicer_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub printer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nozzle_diameter_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_height_mm: Option<f64>,
    #[serde(default)]
    pub plates: Vec<ThreeMfPlate>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreeMfPlate {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub object_count: u64,
    #[serde(default)]
    pub object_names: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bed_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filament_grams: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub print_time_seconds: Option<u64>,
    #[serde(default)]
    pub material_names: Vec<String>,
    pub thumbnail: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub filename: String,
    pub extension: String,
    pub relative_path: String,
    pub byte_size: i64,
    pub modified_at: String,
    pub parse_status: String,
    pub metadata: AssetMetadata,
    pub missing: bool,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummary {
    pub id: String,
    pub display_name: String,
    pub folder_id: String,
    pub folder_name: String,
    pub relative_folder: String,
    pub primary_asset_id: Option<String>,
    pub primary_extension: String,
    pub favorite: bool,
    pub added_at: String,
    pub modified_at: String,
    pub last_opened_at: Option<String>,
    pub missing: bool,
    pub asset_count: i64,
    pub bundle_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions_mm: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDetail {
    #[serde(flatten)]
    pub summary: ModelSummary,
    pub notes: String,
    pub root_id: String,
    pub root_name: String,
    pub root_path: String,
    pub assets: Vec<Asset>,
    pub collection_ids: Vec<String>,
    pub tags: Vec<Tag>,
    pub estimate: Option<CostEstimate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_source: Option<WebSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSourcePreview {
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_id: Option<String>,
    pub canonical_url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filament_grams: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSource {
    pub id: String,
    #[serde(flatten)]
    pub preview: WebSourcePreview,
    pub status: String,
    pub root_id: Option<String>,
    pub relative_path: Option<String>,
    pub model_id: Option<String>,
    pub fetched_at: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelQuery {
    pub search: Option<String>,
    pub folder_id: Option<String>,
    pub collection_id: Option<String>,
    pub favorite: Option<bool>,
    pub recent: Option<bool>,
    pub format: Option<String>,
    pub availability: Option<String>,
    pub tag_id: Option<String>,
    pub date_field: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub duplicates: Option<bool>,
    pub duplicate_kind: Option<String>,
    pub library_id: Option<String>,
    pub parse_status: Option<String>,
    pub has_web_source: Option<bool>,
    pub min_asset_count: Option<i64>,
    pub max_asset_count: Option<i64>,
    pub sort: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub color: String,
    pub model_count: i64,
    pub created_at: String,
    pub smart: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<SmartCollectionRule>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionInput {
    pub name: String,
    #[serde(default = "default_symbol")]
    pub symbol: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub smart: bool,
    pub rule: Option<SmartCollectionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartCollectionRule {
    #[serde(default = "default_rule_version")]
    pub version: u32,
    #[serde(default = "default_match_mode")]
    pub match_mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<QueryRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub availability: Option<String>,
}

impl Default for SmartCollectionRule {
    fn default() -> Self {
        Self {
            version: default_rule_version(),
            match_mode: default_match_mode(),
            rules: Vec::new(),
            tag_id: None,
            format: None,
            availability: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryRule {
    pub field: String,
    pub operator: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    pub id: String,
    pub name: String,
    pub query: ModelQuery,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearchInput {
    pub id: Option<String>,
    pub name: String,
    pub query: ModelQuery,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub model_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInput {
    pub id: Option<String>,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedModel {
    pub id: String,
    pub display_name: String,
    pub relative_folder: String,
    pub primary_extension: String,
    pub modified_at: String,
    pub relationship: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: String,
    pub match_key: String,
    pub match_kind: String,
    pub confidence: f64,
    pub model_count: i64,
    pub byte_size: i64,
    pub models: Vec<ModelSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeProjectsInput {
    pub keeper_id: String,
    pub project_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitProjectInput {
    pub project_id: String,
    pub asset_ids: Vec<String>,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCleanupInput {
    pub match_key: String,
    pub keeper_id: String,
    pub duplicate_ids: Vec<String>,
    #[serde(default)]
    pub move_to_trash: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateStats {
    pub groups: i64,
    pub models: i64,
    pub redundant_copies: i64,
}

fn default_symbol() -> String {
    "box".to_string()
}
fn default_color() -> String {
    "#ff5a36".to_string()
}
fn default_rule_version() -> u32 {
    1
}
fn default_match_mode() -> String {
    "all".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    pub id: String,
    pub name: String,
    pub material_type: String,
    pub color_name: Option<String>,
    pub color_hex: Option<String>,
    pub spool_price_minor: i64,
    pub currency: String,
    pub spool_weight_g: f64,
    pub density: Option<f64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialInput {
    pub id: Option<String>,
    pub name: String,
    pub material_type: String,
    pub color_name: Option<String>,
    pub color_hex: Option<String>,
    pub spool_price_minor: i64,
    pub currency: String,
    pub spool_weight_g: f64,
    pub density: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostEstimate {
    pub id: String,
    pub model_id: String,
    pub asset_id: Option<String>,
    pub material_id: Option<String>,
    pub plastic_g: f64,
    pub quantity: i64,
    pub source: String,
    pub cost_per_piece_minor: Option<f64>,
    pub batch_cost_minor: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostEstimateInput {
    pub model_id: String,
    pub asset_id: Option<String>,
    pub material_id: Option<String>,
    pub plastic_g: f64,
    pub quantity: i64,
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostCalculationInput {
    pub plastic_g: f64,
    pub quantity: i64,
    pub spool_price_minor: f64,
    pub spool_weight_g: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostCalculation {
    pub cost_per_piece_minor: f64,
    pub batch_cost_minor: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPayload {
    pub extension: String,
    pub bytes: Vec<u8>,
    pub source_asset_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    pub root_id: String,
    pub state: String,
    pub discovered: u64,
    pub processed: u64,
    pub errors: u64,
    pub current_path: Option<String>,
    pub message: Option<String>,
}

impl ScanStatus {
    pub fn idle(root_id: impl Into<String>) -> Self {
        Self {
            root_id: root_id.into(),
            state: "idle".into(),
            discovered: 0,
            processed: 0,
            errors: 0,
            current_path: None,
            message: None,
        }
    }
}
