use std::path::PathBuf;

use crate::model::environment::SearchRoots;

/// Candidate `portals.conf` paths in upstream precedence order: for every
/// desktop name (lowercased like upstream) probe every config root followed
/// by every data root for the desktop-specific file, then fall back to the
/// generic `portals.conf` across the same config-then-data root sequence.
#[must_use]
pub fn portal_config_candidates(roots: &SearchRoots, desktops: &[String]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for desktop in desktops {
        let name = format!("{}-portals.conf", desktop.to_ascii_lowercase());
        for root in &roots.config_roots {
            candidates.push(PathBuf::from(root).join("xdg-desktop-portal").join(&name));
        }
        for root in &roots.data_roots {
            candidates.push(PathBuf::from(root).join("xdg-desktop-portal").join(&name));
        }
    }
    for root in &roots.config_roots {
        candidates.push(
            PathBuf::from(root)
                .join("xdg-desktop-portal")
                .join("portals.conf"),
        );
    }
    for root in &roots.data_roots {
        candidates.push(
            PathBuf::from(root)
                .join("xdg-desktop-portal")
                .join("portals.conf"),
        );
    }
    candidates
}

/// `.portal` descriptor search roots: the `portals` subdirectory of every
/// effective `XDG` data root, in precedence order.
#[must_use]
pub fn portal_descriptor_roots(roots: &SearchRoots) -> Vec<PathBuf> {
    roots
        .data_roots
        .iter()
        .map(|root| {
            PathBuf::from(root)
                .join("xdg-desktop-portal")
                .join("portals")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{portal_config_candidates, portal_descriptor_roots};
    use crate::model::environment::SearchRoots;

    fn roots() -> SearchRoots {
        SearchRoots {
            config_roots: vec!["/home/tester/.config".to_owned(), "/cfg/global".to_owned()],
            data_roots: vec![
                "/home/tester/.local/share".to_owned(),
                "/usr/local/share".to_owned(),
                "/usr/share".to_owned(),
            ],
        }
    }

    #[test]
    fn config_candidates_order_desktops_then_generic_across_roots() {
        let desktops = vec!["ubuntu".to_owned(), "GNOME".to_owned()];
        let candidates = portal_config_candidates(&roots(), &desktops);
        let paths: Vec<String> = candidates
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            [
                // Desktop-specific: every config root, then every data root.
                "/home/tester/.config/xdg-desktop-portal/ubuntu-portals.conf",
                "/cfg/global/xdg-desktop-portal/ubuntu-portals.conf",
                "/home/tester/.local/share/xdg-desktop-portal/ubuntu-portals.conf",
                "/usr/local/share/xdg-desktop-portal/ubuntu-portals.conf",
                "/usr/share/xdg-desktop-portal/ubuntu-portals.conf",
                // Desktop names are lowercased like upstream.
                "/home/tester/.config/xdg-desktop-portal/gnome-portals.conf",
                "/cfg/global/xdg-desktop-portal/gnome-portals.conf",
                "/home/tester/.local/share/xdg-desktop-portal/gnome-portals.conf",
                "/usr/local/share/xdg-desktop-portal/gnome-portals.conf",
                "/usr/share/xdg-desktop-portal/gnome-portals.conf",
                // Generic: every config root, then every data root.
                "/home/tester/.config/xdg-desktop-portal/portals.conf",
                "/cfg/global/xdg-desktop-portal/portals.conf",
                "/home/tester/.local/share/xdg-desktop-portal/portals.conf",
                "/usr/local/share/xdg-desktop-portal/portals.conf",
                "/usr/share/xdg-desktop-portal/portals.conf",
            ]
        );
    }

    #[test]
    fn descriptor_roots_follow_data_root_precedence() {
        let roots = portal_descriptor_roots(&roots());
        let paths: Vec<String> = roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            [
                "/home/tester/.local/share/xdg-desktop-portal/portals",
                "/usr/local/share/xdg-desktop-portal/portals",
                "/usr/share/xdg-desktop-portal/portals",
            ]
        );
    }
}
