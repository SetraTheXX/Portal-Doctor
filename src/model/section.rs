use serde::Serialize;

use crate::model::status::CollectorState;

/// A non-fatal collection problem attached to a snapshot section
/// (architecture §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CollectionNote {
    pub message: String,
}

/// Snapshot section carrying the collection status next to optional data
/// (architecture §6). This keeps "not supported" and "failed unexpectedly"
/// distinct conditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section<T> {
    pub status: CollectorState,
    pub value: Option<T>,
    pub errors: Vec<CollectionNote>,
}

impl<T> Section<T> {
    /// Section whose data was collected successfully.
    #[must_use]
    pub fn available(value: T) -> Self {
        Self {
            status: CollectorState::Available,
            value: Some(value),
            errors: Vec::new(),
        }
    }

    /// Section for a collector that is not applicable on this system.
    #[must_use]
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::with_error(CollectorState::Unsupported, message)
    }

    /// Section for a collector that could not obtain its data.
    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::with_error(CollectorState::Unavailable, message)
    }

    /// Section for a collector that exceeded its bounded timeout.
    #[must_use]
    pub fn timed_out(message: impl Into<String>) -> Self {
        Self::with_error(CollectorState::TimedOut, message)
    }

    /// Section for a collection blocked by permissions.
    #[must_use]
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::with_error(CollectorState::PermissionDenied, message)
    }

    /// Section for successfully running a collector whose payload was invalid.
    #[must_use]
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::with_error(CollectorState::ParseError, message)
    }

    /// Append a note to an existing section.
    pub fn push_note(&mut self, message: impl Into<String>) {
        self.errors.push(CollectionNote {
            message: message.into(),
        });
    }

    fn with_error(status: CollectorState, message: impl Into<String>) -> Self {
        Self {
            status,
            value: None,
            errors: vec![CollectionNote {
                message: message.into(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CollectionNote, Section};
    use crate::model::status::CollectorState;
    use serde_json::json;

    #[test]
    fn available_section_serializes_status_and_value() {
        let section = Section::available("value".to_owned());
        assert_eq!(section.status, CollectorState::Available);
        let value = serde_json::to_value(&section).unwrap();
        assert_eq!(
            value,
            json!({ "status": "available", "value": "value", "errors": [] })
        );
    }

    #[test]
    fn failed_sections_carry_notes_instead_of_values() {
        let section: Section<String> = Section::timed_out("collector hung");
        assert_eq!(section.status, CollectorState::TimedOut);
        assert!(section.value.is_none());
        assert_eq!(
            section.errors,
            vec![CollectionNote {
                message: "collector hung".to_owned()
            }]
        );
    }
}
