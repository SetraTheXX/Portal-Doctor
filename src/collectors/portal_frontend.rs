use std::process::{Command, Stdio};

use crate::collectors::timeouts::{BoundedOutput, SHORT_METADATA, output_bounded_with_limit};
use crate::model::portal_frontend::{
    PORTAL_FRONTEND_COMPONENT, PortalFrontendInfo, VersionEvidenceSource,
    parse_frontend_version_output, parse_package_version_output,
};
use crate::model::section::Section;
use crate::model::status::CollectorState;

/// Keep version evidence small even when a broken wrapper writes endlessly.
const MAX_VERSION_OUTPUT_BYTES: usize = 4 * 1024;
const FRONTEND_EXECUTABLE_CANDIDATES: &[&str] = &[
    "xdg-desktop-portal",
    "/usr/libexec/xdg-desktop-portal",
    "/usr/lib/xdg-desktop-portal",
];

#[derive(Clone, Copy)]
enum VersionCommand {
    FrontendExecutable,
    DpkgQuery,
}

/// Collect reliable frontend software-version evidence.
///
/// The frontend's bounded `--version` output is preferred. The normal PATH is
/// searched first, followed by standard installed executable locations; on
/// Debian/Ubuntu-style systems the supported `dpkg-query` package metadata is
/// the fallback. No shell or operating-system release field is used.
pub fn collect() -> Section<PortalFrontendInfo> {
    collect_with_candidates(FRONTEND_EXECUTABLE_CANDIDATES, "dpkg-query")
}

fn collect_with_candidates(
    frontend_programs: &[&str],
    package_program: &str,
) -> Section<PortalFrontendInfo> {
    for program in frontend_programs {
        let frontend = collect_command(program, VersionCommand::FrontendExecutable);
        if frontend.status != CollectorState::Unsupported {
            return frontend;
        }
    }
    collect_command(package_program, VersionCommand::DpkgQuery)
}

fn collect_command(program: &str, kind: VersionCommand) -> Section<PortalFrontendInfo> {
    let mut command = Command::new(program);
    command.stderr(Stdio::null());
    match kind {
        VersionCommand::FrontendExecutable => {
            command.arg("--version");
        }
        VersionCommand::DpkgQuery => {
            command.args(["-W", r"-f=${Version}\n", PORTAL_FRONTEND_COMPONENT]);
        }
    }

    let output = match output_bounded_with_limit(SHORT_METADATA, MAX_VERSION_OUTPUT_BYTES, command)
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Section::unsupported(match kind {
                VersionCommand::FrontendExecutable => "frontend version executable is unavailable",
                VersionCommand::DpkgQuery => "supported package metadata command is unavailable",
            });
        }
        Err(_) => return Section::unavailable("frontend version command could not be started"),
    };

    let output = match output {
        BoundedOutput::TimedOut => return Section::timed_out("frontend version query timed out"),
        BoundedOutput::OutputLimitExceeded => {
            return Section::unavailable("frontend version output exceeded the bounded limit");
        }
        BoundedOutput::Completed(output) => output,
    };
    if !output.status.success() {
        return Section::unavailable("frontend version query exited unsuccessfully");
    }

    let Ok(raw_output) = String::from_utf8(output.stdout) else {
        return Section::parse_error("frontend version output was not UTF-8");
    };
    let parsed = match kind {
        VersionCommand::FrontendExecutable => parse_frontend_version_output(&raw_output),
        VersionCommand::DpkgQuery => parse_package_version_output(&raw_output),
    };
    let Some((raw_version, normalized_version)) = parsed else {
        return Section::parse_error("frontend version output was not comparable");
    };
    let source = match kind {
        VersionCommand::FrontendExecutable => VersionEvidenceSource::FrontendExecutable {
            command: program.to_owned(),
        },
        VersionCommand::DpkgQuery => VersionEvidenceSource::DpkgQuery {
            package: PORTAL_FRONTEND_COMPONENT.to_owned(),
        },
    };
    Section::available(PortalFrontendInfo::new(
        raw_version,
        normalized_version,
        source,
    ))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use crate::model::portal_frontend::{SemanticVersion, VersionEvidenceSource};
    use crate::model::status::CollectorState;

    use super::{VersionCommand, collect, collect_command, collect_with_candidates};

    const GUARD: &str = "isolated-portal-version";

    fn mode_file() -> PathBuf {
        PathBuf::from(env::var("PORTALDOCTOR_VERSION_FAKE_MODE_FILE").unwrap())
    }

    fn write_mode(mode: &str) {
        fs::write(mode_file(), format!("{mode}\n")).unwrap();
    }

    #[test]
    #[ignore = "runs through the guarded fake executable/package-query wrapper"]
    fn isolated_version_collector_matrix() {
        assert_eq!(
            env::var("PORTALDOCTOR_VERSION_FAKE_GUARD").as_deref(),
            Ok(GUARD)
        );
        let fake_bin = PathBuf::from(env::var("PORTALDOCTOR_VERSION_FAKE_BIN").unwrap());
        let fake_script = PathBuf::from(env::var("PORTALDOCTOR_VERSION_FAKE_SCRIPT").unwrap());
        let frontend = fake_bin.join("xdg-desktop-portal");

        for (mode, expected) in [
            ("exact", SemanticVersion::new(1, 22, 0)),
            ("revision", SemanticVersion::new(1, 22, 0)),
            ("newer", SemanticVersion::new(1, 23, 0)),
        ] {
            write_mode(mode);
            let section = collect_with_candidates(&["xdg-desktop-portal"], "dpkg-query");
            assert_eq!(section.status, CollectorState::Available, "{mode}");
            let info = section.value.unwrap();
            assert_eq!(info.normalized_version, expected, "{mode}");
            assert!(matches!(
                info.source,
                VersionEvidenceSource::DpkgQuery { .. }
            ));
        }

        for (mode, expected_status) in [
            ("malformed", CollectorState::ParseError),
            ("nonzero", CollectorState::Unavailable),
            ("timeout", CollectorState::TimedOut),
            ("oversized", CollectorState::Unavailable),
            ("unexpected", CollectorState::ParseError),
        ] {
            write_mode(mode);
            let section = collect_with_candidates(&["xdg-desktop-portal"], "dpkg-query");
            assert_eq!(section.status, expected_status, "{mode}");
            assert!(section.value.is_none(), "{mode} must fail closed");
        }

        let missing = collect_command(
            "portaldoctor-version-command-does-not-exist",
            VersionCommand::DpkgQuery,
        );
        assert_eq!(missing.status, CollectorState::Unsupported);
        assert!(missing.value.is_none());

        fs::copy(fake_script, &frontend).unwrap();
        let mut permissions = fs::metadata(&frontend).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&frontend, permissions).unwrap();
        write_mode("frontend-exact");
        let section = collect();
        assert_eq!(section.status, CollectorState::Available);
        let info = section.value.unwrap();
        assert_eq!(info.raw_version, "1.22.0");
        assert_eq!(info.normalized_version, SemanticVersion::new(1, 22, 0));
        assert!(matches!(
            info.source,
            VersionEvidenceSource::FrontendExecutable { .. }
        ));

        let log = fs::read_to_string(env::var("PORTALDOCTOR_VERSION_FAKE_LOG").unwrap()).unwrap();
        assert!(log.contains("dpkg-query mode=exact"));
        assert!(log.contains("xdg-desktop-portal mode=frontend-exact"));
        assert!(!log.contains("dpkg-query mode=frontend-exact"));
        assert!(!log.contains("unexpected-argv"));
        fs::remove_file(frontend).unwrap();
    }
}
