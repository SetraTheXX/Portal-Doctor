use std::fmt;

use serde::{Deserialize, Serialize};

/// The frontend component whose software version is collected separately from
/// the operating-system release in [`crate::model::environment::SystemInfo`].
pub const PORTAL_FRONTEND_COMPONENT: &str = "xdg-desktop-portal";

/// Numeric semantic version used for bounded compatibility comparisons.
///
/// Distribution revisions remain in [`PortalFrontendInfo::raw_version`] and
/// are never compared as strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemanticVersion {
    /// Construct a comparable three-component version.
    #[must_use]
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Provenance for the frontend version evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum VersionEvidenceSource {
    /// The frontend executable answered its bounded `--version` query.
    FrontendExecutable { command: String },
    /// A supported package-manager query returned the installed package
    /// version after the frontend executable was unavailable.
    DpkgQuery { package: String },
}

/// Normalized xdg-desktop-portal frontend version evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortalFrontendInfo {
    /// Always `xdg-desktop-portal`; kept explicit in the JSON contract.
    pub component: String,
    /// The version token exactly as returned by the selected source, without
    /// its transport newline or frontend command label.
    pub raw_version: String,
    /// Numeric form used by compatibility rules.
    pub normalized_version: SemanticVersion,
    /// Source and command/package provenance for the evidence.
    pub source: VersionEvidenceSource,
}

impl PortalFrontendInfo {
    /// Build evidence after a source-specific parser has established the
    /// numeric version.
    #[must_use]
    pub fn new(
        raw_version: impl Into<String>,
        normalized_version: SemanticVersion,
        source: VersionEvidenceSource,
    ) -> Self {
        Self {
            component: PORTAL_FRONTEND_COMPONENT.to_owned(),
            raw_version: raw_version.into(),
            normalized_version,
            source,
        }
    }
}

/// Parse a strict three-component upstream version with a bounded,
/// distribution-revision suffix.
///
/// Accepted examples include `1.22.0`, `1.22.0+ds-1ubuntu3` and
/// `1.23.0-1.fc42`. Pre-release, git/date and otherwise ambiguous suffixes
/// are rejected instead of being guessed.
#[must_use]
pub fn parse_semantic_version(raw: &str) -> Option<SemanticVersion> {
    let value = raw.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return None;
    }

    let core_end = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(value.len());
    let core = &value[..core_end];
    let suffix = &value[core_end..];
    if !valid_distribution_suffix(suffix) {
        return None;
    }

    let mut components = core.split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components.next()?.parse().ok()?;
    if components.next().is_some() {
        return None;
    }
    Some(SemanticVersion::new(major, minor, patch))
}

/// Parse the stable upstream frontend output (`xdg-desktop-portal X.Y.Z`) or
/// a bare version emitted by a compatible wrapper.
#[must_use]
pub fn parse_frontend_version_output(raw: &str) -> Option<(String, SemanticVersion)> {
    let line = single_line(raw)?;
    let version = line
        .strip_prefix("xdg-desktop-portal ")
        .unwrap_or(line)
        .trim();
    if version.is_empty() || version.chars().any(char::is_whitespace) {
        return None;
    }
    let normalized = parse_semantic_version(version)?;
    Some((version.to_owned(), normalized))
}

/// Parse package-manager output, which must contain exactly one version line.
#[must_use]
pub fn parse_package_version_output(raw: &str) -> Option<(String, SemanticVersion)> {
    let line = single_line(raw)?;
    let normalized = parse_semantic_version(line)?;
    Some((line.to_owned(), normalized))
}

fn single_line(raw: &str) -> Option<&str> {
    let line = raw.trim();
    (!line.is_empty() && !line.contains(['\n', '\r'])).then_some(line)
}

fn valid_distribution_suffix(suffix: &str) -> bool {
    if suffix.is_empty() {
        return true;
    }
    let lowercase = suffix.to_ascii_lowercase();
    if ["git", "dev", "snapshot"]
        .iter()
        .any(|marker| lowercase.contains(marker))
    {
        return false;
    }
    if !suffix.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.' | b'_' | b'~')
    }) {
        return false;
    }

    let starts_with_numeric_revision =
        |revision: &str| revision.as_bytes().first().is_some_and(u8::is_ascii_digit);
    suffix
        .strip_prefix('-')
        .is_some_and(starts_with_numeric_revision)
        || suffix
            .strip_prefix('+')
            .and_then(|value| value.split_once('-'))
            .is_some_and(|(_, revision)| starts_with_numeric_revision(revision))
}

#[cfg(test)]
mod tests {
    use super::{
        PORTAL_FRONTEND_COMPONENT, PortalFrontendInfo, SemanticVersion, VersionEvidenceSource,
        parse_frontend_version_output, parse_package_version_output, parse_semantic_version,
    };

    #[test]
    fn parses_and_compares_numeric_versions() {
        assert_eq!(
            parse_semantic_version("1.22.0"),
            Some(SemanticVersion::new(1, 22, 0))
        );
        assert!(SemanticVersion::new(1, 23, 0) > SemanticVersion::new(1, 22, 99));
    }

    #[test]
    fn normalizes_supported_distribution_revisions_without_losing_raw_value() {
        let (raw, normalized) = parse_package_version_output("1.22.0+ds-1ubuntu3\n").unwrap();
        assert_eq!(raw, "1.22.0+ds-1ubuntu3");
        assert_eq!(normalized, SemanticVersion::new(1, 22, 0));
        let info = PortalFrontendInfo::new(
            raw,
            normalized,
            VersionEvidenceSource::DpkgQuery {
                package: PORTAL_FRONTEND_COMPONENT.to_owned(),
            },
        );
        assert_eq!(info.component, PORTAL_FRONTEND_COMPONENT);
    }

    #[test]
    fn accepts_upstream_frontend_output_and_rejects_ambiguous_versions() {
        assert_eq!(
            parse_frontend_version_output("xdg-desktop-portal 1.22.0\n"),
            Some(("1.22.0".to_owned(), SemanticVersion::new(1, 22, 0)))
        );
        for value in [
            "1.22.0-rc1",
            "1.22.0+git20260909",
            "1.22.0+git-1",
            "1:1.22.0-1",
            "1.22",
            "1.22.0.1",
            "not-a-version",
        ] {
            assert_eq!(parse_semantic_version(value), None, "{value}");
        }
        assert_eq!(
            parse_frontend_version_output("xdg-desktop-portal 1.22.0\nextra"),
            None
        );
    }

    #[test]
    fn serializes_typed_provenance_and_version() {
        let info = PortalFrontendInfo::new(
            "1.22.0",
            SemanticVersion::new(1, 22, 0),
            VersionEvidenceSource::FrontendExecutable {
                command: "xdg-desktop-portal".to_owned(),
            },
        );
        let value = serde_json::to_value(info).unwrap();
        assert_eq!(value["component"], PORTAL_FRONTEND_COMPONENT);
        assert_eq!(value["normalized_version"]["minor"], 22);
        assert_eq!(value["source"]["kind"], "frontend_executable");
    }
}
