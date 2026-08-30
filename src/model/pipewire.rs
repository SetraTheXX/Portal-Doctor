use serde::{Deserialize, Serialize};

/// Version of the normalized `PipeWire` model embedded in snapshot schema v1.
pub const PIPEWIRE_MODEL_VERSION: u32 = 1;

/// Portal-relevant facts normalized from `pw-dump`.
///
/// The raw `PipeWire` graph is intentionally not retained. Counts describe the
/// complete graph, while `nodes` and `links` contain only video-relevant
/// topology and privacy-safe properties needed for `ScreenCast` diagnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipeWireInfo {
    pub model_version: u32,
    pub version: Option<String>,
    pub object_count: usize,
    pub node_count: usize,
    pub link_count: usize,
    pub portal_client_count: usize,
    pub screen_cast_source_count: usize,
    pub nodes: Vec<PipeWireNode>,
    pub links: Vec<PipeWireLink>,
}

/// Normalized video node properties. Names and arbitrary application metadata
/// are deliberately omitted; those values are handled by the later privacy
/// and report phases rather than entering the base snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipeWireNode {
    pub id: u64,
    pub media_class: Option<String>,
    pub state: Option<String>,
    pub is_video_source: bool,
    pub is_screen_cast_source: bool,
}

/// Normalized video link topology without the raw format/property payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipeWireLink {
    pub id: u64,
    pub output_node_id: Option<u64>,
    pub input_node_id: Option<u64>,
    pub media_type: Option<String>,
    pub state: Option<String>,
}

/// Version of the normalized `WirePlumber` model embedded in snapshot schema v1.
pub const WIREPLUMBER_MODEL_VERSION: u32 = 1;

/// Portal-relevant health facts normalized from bounded `wpctl status` output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WirePlumberInfo {
    pub model_version: u32,
    pub pipewire_version: Option<String>,
    pub wireplumber_client_count: usize,
}

#[cfg(test)]
mod tests {
    use super::{PIPEWIRE_MODEL_VERSION, PipeWireInfo, WIREPLUMBER_MODEL_VERSION, WirePlumberInfo};
    use serde_json::json;

    #[test]
    fn model_versions_and_privacy_safe_shape_serialize() {
        let pipewire = PipeWireInfo {
            model_version: PIPEWIRE_MODEL_VERSION,
            version: Some("1.6.2".to_owned()),
            object_count: 3,
            node_count: 1,
            link_count: 1,
            portal_client_count: 1,
            screen_cast_source_count: 1,
            nodes: Vec::new(),
            links: Vec::new(),
        };
        let wireplumber = WirePlumberInfo {
            model_version: WIREPLUMBER_MODEL_VERSION,
            pipewire_version: Some("1.6.2".to_owned()),
            wireplumber_client_count: 1,
        };
        let value = serde_json::to_value((pipewire, wireplumber)).unwrap();
        assert_eq!(value[0]["model_version"], json!(1));
        assert_eq!(value[1]["wireplumber_client_count"], json!(1));
    }
}
