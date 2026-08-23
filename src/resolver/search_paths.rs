use std::path::PathBuf;

use crate::model::environment::SearchRoots;

/// Candidate `portals.conf` paths in upstream precedence order: for every
/// desktop name (as listed in `XDG_CURRENT_DESKTOP`) probe every config root,
/// then fall back to the generic `portals.conf` across the same roots.
#[must_use]
pub fn portal_config_candidates(roots: &SearchRoots, desktops: &[String]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for desktop in desktops {
        for root in &roots.config_roots {
            candidates.push(
                PathBuf::from(root)
                    .join("xdg-desktop-portal")
                    .join(format!("{desktop}-portals.conf")),
            );
        }
    }
    for root in &roots.config_roots {
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
    fn config_candidates_order_desktops_then_generic() {
        let desktops = vec!["ubuntu".to_owned(), "GNOME".to_owned()];
        let candidates = portal_config_candidates(&roots(), &desktops);
        let paths: Vec<String> = candidates
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            [
                "/home/tester/.config/xdg-desktop-portal/ubuntu-portals.conf",
                "/cfg/global/xdg-desktop-portal/ubuntu-portals.conf",
                "/home/tester/.config/xdg-desktop-portal/GNOME-portals.conf",
                "/cfg/global/xdg-desktop-portal/GNOME-portals.conf",
                "/home/tester/.config/xdg-desktop-portal/portals.conf",
                "/cfg/global/xdg-desktop-portal/portals.conf",
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
