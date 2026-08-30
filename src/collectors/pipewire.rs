use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Map, Value};

use crate::collectors::timeouts::{BoundedOutput, NORMAL_RUNTIME_QUERY, output_bounded_with_limit};
use crate::model::pipewire::{
    PIPEWIRE_MODEL_VERSION, PipeWireInfo, PipeWireLink, PipeWireNode, WIREPLUMBER_MODEL_VERSION,
    WirePlumberInfo,
};
use crate::model::section::Section;

/// Keep the raw `PipeWire` response bounded before JSON parsing. Normal desktop
/// graphs are much smaller; this protects the CLI from pathological output.
const PIPEWIRE_OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
const PW_DUMP_COMMAND: &str = "pw-dump";
const WPCTL_COMMAND: &str = "wpctl";

/// Collect normalized `PipeWire` and `WirePlumber` state using bounded external
/// commands. The raw graph and human-oriented `wpctl` output never enter the
/// snapshot.
pub fn collect() -> (Section<PipeWireInfo>, Section<WirePlumberInfo>) {
    collect_with_timeout(
        OsStr::new(PW_DUMP_COMMAND),
        OsStr::new(WPCTL_COMMAND),
        NORMAL_RUNTIME_QUERY,
    )
}

fn collect_with_timeout(
    pw_dump_command: &OsStr,
    wpctl_command: &OsStr,
    timeout: Duration,
) -> (Section<PipeWireInfo>, Section<WirePlumberInfo>) {
    (
        collect_pw_dump(pw_dump_command, timeout),
        collect_wireplumber(wpctl_command, timeout),
    )
}

fn collect_pw_dump(program: &OsStr, timeout: Duration) -> Section<PipeWireInfo> {
    let mut command = Command::new(program);
    command.arg("--no-colors").stderr(Stdio::piped());
    match output_bounded_with_limit(timeout, PIPEWIRE_OUTPUT_LIMIT, command) {
        Err(err) => spawn_failure("pw-dump", &err),
        Ok(BoundedOutput::TimedOut) => {
            Section::timed_out(format!("pw-dump did not finish within {timeout:?}"))
        }
        Ok(BoundedOutput::OutputLimitExceeded) => Section::unavailable(format!(
            "pw-dump output exceeded the {} MiB safety limit",
            PIPEWIRE_OUTPUT_LIMIT / (1024 * 1024)
        )),
        Ok(BoundedOutput::Completed(output)) if !output.status.success() => {
            command_failure("pw-dump", &output)
        }
        Ok(BoundedOutput::Completed(output)) => match parse_pw_dump(&output.stdout) {
            Ok(info) => Section::available(info),
            Err(message) => Section::parse_error(format!("pw-dump JSON: {message}")),
        },
    }
}

fn collect_wireplumber(program: &OsStr, timeout: Duration) -> Section<WirePlumberInfo> {
    let mut command = Command::new(program);
    command.arg("status").stderr(Stdio::piped());
    match output_bounded_with_limit(timeout, PIPEWIRE_OUTPUT_LIMIT, command) {
        Err(err) => spawn_failure("wpctl", &err),
        Ok(BoundedOutput::TimedOut) => {
            Section::timed_out(format!("wpctl status did not finish within {timeout:?}"))
        }
        Ok(BoundedOutput::OutputLimitExceeded) => Section::unavailable(format!(
            "wpctl status output exceeded the {} MiB safety limit",
            PIPEWIRE_OUTPUT_LIMIT / (1024 * 1024)
        )),
        Ok(BoundedOutput::Completed(output)) if !output.status.success() => {
            command_failure("wpctl status", &output)
        }
        Ok(BoundedOutput::Completed(output)) => match String::from_utf8(output.stdout) {
            Ok(text) => match parse_wpctl_status(&text) {
                Ok(info) => Section::available(info),
                Err(message) => Section::parse_error(format!("wpctl status: {message}")),
            },
            Err(_) => Section::parse_error("wpctl status returned non-UTF-8 output"),
        },
    }
}

fn spawn_failure<T>(label: &str, err: &std::io::Error) -> Section<T> {
    match err.kind() {
        ErrorKind::NotFound => Section::unsupported(format!("{label} is not installed")),
        ErrorKind::PermissionDenied => {
            Section::permission_denied(format!("cannot execute {label}: permission denied"))
        }
        _ => Section::unavailable(format!("cannot execute {label}: {}", err.kind())),
    }
}

fn command_failure<T>(label: &str, output: &std::process::Output) -> Section<T> {
    let permission_denied = output.status.code() == Some(126)
        || String::from_utf8_lossy(&output.stderr)
            .to_ascii_lowercase()
            .contains("permission denied");
    if permission_denied {
        return Section::permission_denied(format!("{label} was denied permission to run"));
    }
    if output.status.code() == Some(127) {
        return Section::unsupported(format!("{label} command is unavailable"));
    }
    Section::unavailable(format!("{label} exited with {}", output.status))
}

/// Parse the JSON array emitted by `pw-dump` into a privacy-safe summary.
#[must_use = "the parse result contains the normalized graph summary"]
fn parse_pw_dump(bytes: &[u8]) -> Result<PipeWireInfo, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|err| err.to_string())?;
    let objects = value
        .as_array()
        .ok_or_else(|| "top-level value is not an object array".to_owned())?;

    let mut graph = GraphAccumulator::default();
    for object in objects {
        graph.consume(object);
    }
    Ok(graph.finish(objects.len()))
}

#[derive(Default)]
struct GraphAccumulator {
    version: Option<String>,
    node_count: usize,
    link_count: usize,
    portal_client_count: usize,
    nodes: Vec<PipeWireNode>,
    relevant_node_ids: BTreeSet<u64>,
    raw_links: Vec<RawLink>,
}

impl GraphAccumulator {
    fn consume(&mut self, object: &Value) {
        let Some(object_type) = object.get("type").and_then(Value::as_str) else {
            return;
        };
        let info = object.get("info").and_then(Value::as_object);
        let props = info
            .and_then(|value| value.get("props"))
            .and_then(Value::as_object);
        let id = object.get("id").and_then(Value::as_u64);

        match object_type {
            "PipeWire:Interface:Core" => self.capture_version(info),
            "PipeWire:Interface:Node" => self.capture_node(id, info, props),
            "PipeWire:Interface:Link" => self.capture_link(id, info, props),
            "PipeWire:Interface:Client"
                if props
                    .and_then(|value| value.get("pipewire.access.portal.is_portal"))
                    .is_some_and(is_true) =>
            {
                self.portal_client_count += 1;
            }
            _ => {}
        }
    }

    fn capture_version(&mut self, info: Option<&Map<String, Value>>) {
        if self.version.is_none() {
            self.version = info
                .and_then(|value| value.get("version"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
        }
    }

    fn capture_node(
        &mut self,
        id: Option<u64>,
        info: Option<&Map<String, Value>>,
        props: Option<&Map<String, Value>>,
    ) {
        self.node_count += 1;
        let Some(id) = id else {
            return;
        };
        let media_class = string_property(props, "media.class");
        if !is_video_class(media_class.as_deref()) {
            return;
        }
        let media_name = string_property(props, "media.name");
        let node_name = string_property(props, "node.name");
        self.relevant_node_ids.insert(id);
        self.nodes.push(PipeWireNode {
            id,
            is_video_source: media_class.as_deref().is_some_and(is_video_source_class),
            is_screen_cast_source: is_screen_cast_name(media_name.as_deref())
                || is_screen_cast_name(node_name.as_deref()),
            state: info.and_then(|value| string_property(Some(value), "state")),
            media_class,
        });
    }

    fn capture_link(
        &mut self,
        id: Option<u64>,
        info: Option<&Map<String, Value>>,
        props: Option<&Map<String, Value>>,
    ) {
        self.link_count += 1;
        if let Some(id) = id {
            self.raw_links.push(RawLink::from_info(id, info, props));
        }
    }

    fn finish(self, object_count: usize) -> PipeWireInfo {
        let Self {
            version,
            node_count,
            link_count,
            portal_client_count,
            mut nodes,
            relevant_node_ids,
            raw_links,
        } = self;
        nodes.sort_by_key(|node| node.id);
        let mut links: Vec<_> = raw_links
            .into_iter()
            .filter(|link| {
                link.media_type.as_deref() == Some("video")
                    || link
                        .output_node_id
                        .is_some_and(|id| relevant_node_ids.contains(&id))
                    || link
                        .input_node_id
                        .is_some_and(|id| relevant_node_ids.contains(&id))
            })
            .map(RawLink::into_model)
            .collect();
        links.sort_by_key(|link| link.id);

        PipeWireInfo {
            model_version: PIPEWIRE_MODEL_VERSION,
            version,
            object_count,
            node_count,
            link_count,
            portal_client_count,
            screen_cast_source_count: nodes
                .iter()
                .filter(|node| node.is_screen_cast_source)
                .count(),
            nodes,
            links,
        }
    }
}

#[derive(Debug)]
struct RawLink {
    id: u64,
    output_node_id: Option<u64>,
    input_node_id: Option<u64>,
    media_type: Option<String>,
    state: Option<String>,
}

impl RawLink {
    fn from_info(
        id: u64,
        info: Option<&Map<String, Value>>,
        props: Option<&Map<String, Value>>,
    ) -> Self {
        Self {
            id,
            output_node_id: u64_property(info, "output-node-id")
                .or_else(|| u64_property(props, "link.output.node")),
            input_node_id: u64_property(info, "input-node-id")
                .or_else(|| u64_property(props, "link.input.node")),
            media_type: info
                .and_then(|value| value.get("format"))
                .and_then(Value::as_object)
                .and_then(|format| string_property(Some(format), "mediaType"))
                .or_else(|| string_property(props, "media.type"))
                .map(|value| value.to_ascii_lowercase()),
            state: info.and_then(|value| string_property(Some(value), "state")),
        }
    }

    fn into_model(self) -> PipeWireLink {
        PipeWireLink {
            id: self.id,
            output_node_id: self.output_node_id,
            input_node_id: self.input_node_id,
            media_type: self.media_type,
            state: self.state,
        }
    }
}

/// Parse the stable status header from `wpctl status` and count only the
/// `WirePlumber` client marker; arbitrary client names and host data are not
/// retained.
#[must_use = "the parse result contains normalized WirePlumber facts"]
fn parse_wpctl_status(text: &str) -> Result<WirePlumberInfo, String> {
    let header = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("PipeWire "))
        .ok_or_else(|| "missing PipeWire status header".to_owned())?;
    let pipewire_version = header
        .split_once('[')
        .and_then(|(_, rest)| rest.split_once(',').map(|(version, _)| version.trim()))
        .filter(|version| !version.is_empty())
        .map(str::to_owned);
    let wireplumber_client_count = text
        .lines()
        .filter(|line| line.trim_start().contains(". WirePlumber"))
        .count();
    Ok(WirePlumberInfo {
        model_version: WIREPLUMBER_MODEL_VERSION,
        pipewire_version,
        wireplumber_client_count,
    })
}

fn string_property(props: Option<&Map<String, Value>>, key: &str) -> Option<String> {
    props
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn u64_property(props: Option<&Map<String, Value>>, key: &str) -> Option<u64> {
    props
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
}

fn is_true(value: &Value) -> bool {
    value.as_bool().unwrap_or_else(|| {
        value
            .as_str()
            .is_some_and(|text| text.eq_ignore_ascii_case("true"))
    })
}

fn is_video_class(media_class: Option<&str>) -> bool {
    media_class.is_some_and(|class| {
        let class = class.to_ascii_lowercase();
        class.starts_with("video/") || class.ends_with("/video")
    })
}

fn is_video_source_class(media_class: &str) -> bool {
    let class = media_class.to_ascii_lowercase();
    class == "video/source" || class == "stream/output/video"
}

fn is_screen_cast_name(name: Option<&str>) -> bool {
    name.is_some_and(|name| {
        let name = name.to_ascii_lowercase();
        name.contains("screen-cast") || name.contains("screencast")
    })
}

#[cfg(test)]
mod tests {
    use super::{collect_with_timeout, parse_pw_dump, parse_wpctl_status};
    use crate::model::status::CollectorState;
    use std::ffi::OsStr;
    use std::time::Duration;

    #[cfg(unix)]
    struct TemporaryScript(std::path::PathBuf);

    #[cfg(unix)]
    impl TemporaryScript {
        fn new(body: &str) -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after the Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "portaldoctor-pipewire-test-{}-{unique}.sh",
                std::process::id()
            ));
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }

        fn set_mode(&self, mode: u32) {
            std::fs::set_permissions(&self.0, std::os::unix::fs::PermissionsExt::from_mode(mode))
                .unwrap();
        }
    }

    #[cfg(unix)]
    impl Drop for TemporaryScript {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).ok();
        }
    }

    #[test]
    fn normalizes_video_graph_without_raw_properties() {
        let json = br#"
        [
          {"id":0,"type":"PipeWire:Interface:Core","info":{"version":"1.6.2","props":{"host-name":"private-host"}}},
          {"id":10,"type":"PipeWire:Interface:Node","info":{"state":"running","props":{"media.class":"Video/Source","node.name":"camera-private"}}},
          {"id":11,"type":"PipeWire:Interface:Node","info":{"state":"running","props":{"media.class":"Stream/Output/Video","media.name":"meta-screen-cast-src","node.name":"desktop-private"}}},
          {"id":12,"type":"PipeWire:Interface:Node","info":{"state":"idle","props":{"media.class":"Audio/Sink","node.name":"audio-private"}}},
          {"id":20,"type":"PipeWire:Interface:Link","info":{"state":"active","output-node-id":11,"input-node-id":10,"format":{"mediaType":"video"}}},
          {"id":21,"type":"PipeWire:Interface:Link","info":{"state":"active","output-node-id":12,"input-node-id":12,"format":{"mediaType":"audio"}}},
          {"id":30,"type":"PipeWire:Interface:Client","info":{"props":{"pipewire.access.portal.is_portal":true}}}
        ]
        "#;
        let info = parse_pw_dump(json).unwrap();
        assert_eq!(info.version.as_deref(), Some("1.6.2"));
        assert_eq!(info.object_count, 7);
        assert_eq!(info.node_count, 3);
        assert_eq!(info.link_count, 2);
        assert_eq!(info.portal_client_count, 1);
        assert_eq!(info.screen_cast_source_count, 1);
        assert_eq!(info.nodes.len(), 2);
        assert_eq!(info.links.len(), 1);
        let serialized = serde_json::to_string(&info).unwrap();
        assert!(!serialized.contains("private-host"));
        assert!(!serialized.contains("camera-private"));
        assert!(!serialized.contains("desktop-private"));
    }

    #[test]
    fn rejects_non_array_and_invalid_json() {
        assert!(parse_pw_dump(br"{}").is_err());
        assert!(parse_pw_dump(br"not-json").is_err());
    }

    #[test]
    fn parses_wireplumber_header_without_private_identity() {
        let info = parse_wpctl_status(
            "PipeWire 'pipewire-0' [1.6.2, tuncay@private-host, cookie:123]\n \\
             33. WirePlumber [1.6.2, private-host]\n \\
             40. WirePlumber [export]\n",
        )
        .unwrap();
        assert_eq!(info.pipewire_version.as_deref(), Some("1.6.2"));
        assert_eq!(info.wireplumber_client_count, 2);
        let serialized = serde_json::to_string(&info).unwrap();
        assert!(!serialized.contains("private-host"));
        assert!(!serialized.contains("tuncay"));
    }

    #[test]
    fn missing_commands_are_classified_as_unsupported() {
        let (pipewire, wireplumber) = collect_with_timeout(
            OsStr::new("/definitely/missing/portaldoctor-pw-dump"),
            OsStr::new("/definitely/missing/portaldoctor-wpctl"),
            Duration::from_millis(200),
        );
        assert_eq!(pipewire.status, CollectorState::Unsupported);
        assert_eq!(wireplumber.status, CollectorState::Unsupported);
    }

    #[test]
    #[cfg(unix)]
    fn preserves_permission_and_parse_failures() {
        let denied = TemporaryScript::new("exit 0");
        denied.set_mode(0o644);
        let (pipewire, wireplumber) = collect_with_timeout(
            denied.path().as_os_str(),
            denied.path().as_os_str(),
            Duration::from_millis(200),
        );
        assert_eq!(pipewire.status, CollectorState::PermissionDenied);
        assert_eq!(wireplumber.status, CollectorState::PermissionDenied);

        let (pipewire, wireplumber) = collect_with_timeout(
            OsStr::new("/bin/echo"),
            OsStr::new("/bin/echo"),
            Duration::from_millis(200),
        );
        assert_eq!(pipewire.status, CollectorState::ParseError);
        assert_eq!(wireplumber.status, CollectorState::ParseError);
    }

    #[test]
    #[cfg(unix)]
    fn collects_normalized_sections_from_bounded_commands() {
        let command = TemporaryScript::new(
            r#"
case "$1" in
  --no-colors)
    printf '%s\n' '[{"id":0,"type":"PipeWire:Interface:Core","info":{"version":"1.6.2"}},{"id":10,"type":"PipeWire:Interface:Node","info":{"state":"running","props":{"media.class":"Stream/Output/Video","node.name":"private-desktop"}}},{"id":30,"type":"PipeWire:Interface:Client","info":{"props":{"pipewire.access.portal.is_portal":true}}}]'
    ;;
  status)
    printf '%s\n' "PipeWire 'pipewire-0' [1.6.2, private-host]"
    printf '%s\n' '  33. WirePlumber [1.6.2, private-host]'
    ;;
  *) exit 2 ;;
esac
"#,
        );
        let (pipewire, wireplumber) = collect_with_timeout(
            command.path().as_os_str(),
            command.path().as_os_str(),
            Duration::from_secs(1),
        );

        assert_eq!(pipewire.status, CollectorState::Available);
        let pipewire_info = pipewire.value.unwrap();
        assert_eq!(pipewire_info.object_count, 3);
        assert_eq!(pipewire_info.node_count, 1);
        assert_eq!(pipewire_info.portal_client_count, 1);
        assert_eq!(pipewire_info.screen_cast_source_count, 0);

        assert_eq!(wireplumber.status, CollectorState::Available);
        let wireplumber_info = wireplumber.value.unwrap();
        assert_eq!(wireplumber_info.pipewire_version.as_deref(), Some("1.6.2"));
        assert_eq!(wireplumber_info.wireplumber_client_count, 1);
    }

    #[test]
    #[cfg(unix)]
    fn classifies_broken_commands_as_timed_out_without_hanging() {
        let command = TemporaryScript::new("sleep 30");
        let (pipewire, wireplumber) = collect_with_timeout(
            command.path().as_os_str(),
            command.path().as_os_str(),
            Duration::from_millis(120),
        );
        assert_eq!(pipewire.status, CollectorState::TimedOut);
        assert_eq!(wireplumber.status, CollectorState::TimedOut);
    }
}
