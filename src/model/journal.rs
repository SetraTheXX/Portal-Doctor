use serde::{Deserialize, Serialize};

/// Version of the normalized journal model embedded in snapshot schema v1.
pub const JOURNAL_MODEL_VERSION: u32 = 1;

/// Safe result of matching stable journal patterns. The collector does not
/// turn unknown text into a diagnosis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalMatchState {
    Matched,
    NoRelevantEvidence,
    InsufficientEvidence,
}

impl JournalMatchState {
    /// Stable label used by terminal renderers and documentation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::NoRelevantEvidence => "no relevant evidence",
            Self::InsufficientEvidence => "insufficient evidence",
        }
    }
}

/// Stable, portal-relevant categories recognized from journal messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JournalClassification {
    #[serde(rename = "portal")]
    Portal,
    #[serde(rename = "pipewire")]
    PipeWire,
    #[serde(rename = "wireplumber")]
    WirePlumber,
    #[serde(rename = "screencast")]
    ScreenCast,
}

impl JournalClassification {
    /// Stable label used by terminal renderers.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Portal => "portal failure",
            Self::PipeWire => "PipeWire failure",
            Self::WirePlumber => "WirePlumber failure",
            Self::ScreenCast => "ScreenCast failure",
        }
    }
}

/// Normalized current-boot/user-session journal evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalInfo {
    pub model_version: u32,
    pub window_minutes: u64,
    pub max_entries: usize,
    pub scanned_entry_count: usize,
    pub ignored_entry_count: usize,
    pub match_state: JournalMatchState,
    pub entries: Vec<JournalEntry>,
}

impl JournalInfo {
    /// Whether a stable classification is represented in the sanitized list.
    #[must_use]
    pub fn has_classification(&self, classification: JournalClassification) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.classification == classification)
    }
}

/// A sanitized journal excerpt safe to expose through the normalized snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub unit: String,
    pub priority: u8,
    pub classification: JournalClassification,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::{
        JOURNAL_MODEL_VERSION, JournalClassification, JournalEntry, JournalInfo, JournalMatchState,
    };
    use serde_json::json;

    #[test]
    fn model_serializes_state_and_classification() {
        let info = JournalInfo {
            model_version: JOURNAL_MODEL_VERSION,
            window_minutes: 30,
            max_entries: 80,
            scanned_entry_count: 1,
            ignored_entry_count: 0,
            match_state: JournalMatchState::Matched,
            entries: vec![JournalEntry {
                unit: "pipewire.service".to_owned(),
                priority: 3,
                classification: JournalClassification::PipeWire,
                message: "PipeWire failed: <path>".to_owned(),
            }],
        };
        let value = serde_json::to_value(&info).unwrap();
        assert_eq!(value["model_version"], json!(1));
        assert_eq!(value["match_state"], json!("matched"));
        assert_eq!(value["entries"][0]["classification"], json!("pipewire"));
        assert!(info.has_classification(JournalClassification::PipeWire));
    }

    #[test]
    fn no_evidence_state_has_a_stable_label() {
        assert_eq!(
            JournalMatchState::InsufficientEvidence.as_str(),
            "insufficient evidence"
        );
    }
}
