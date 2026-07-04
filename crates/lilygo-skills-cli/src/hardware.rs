//! Read-only hardware profile verification for matching attached evidence to
//! known LilyGO board capabilities without claiming firmware behavior.
use crate::model::{HardwareProfile, HardwareVerifyReport};
use crate::source::load_board_index;
use std::fs;
use std::path::Path;

pub fn verify_hardware_profile(root: &Path, profile_path: &Path) -> HardwareVerifyReport {
    let profile_display = profile_path.display().to_string();
    let profile = match fs::read_to_string(profile_path)
        .map_err(|error| format!("failed to read profile: {error}"))
        .and_then(|data| {
            serde_json::from_str::<HardwareProfile>(&data)
                .map_err(|error| format!("invalid profile JSON: {error}"))
        }) {
        Ok(profile) => profile,
        Err(error) => {
            return HardwareVerifyReport {
                status: "FAIL".to_string(),
                verification_level: "V0".to_string(),
                profile: profile_display,
                board: String::new(),
                framework: String::new(),
                capabilities: Vec::new(),
                boundaries: Vec::new(),
                errors: vec![error],
            };
        }
    };

    let mut errors = Vec::new();
    let mut boundaries = Vec::new();
    let board_supported = load_board_index(root)
        .map(|index| {
            index
                .boards
                .iter()
                .any(|board| board.supported && board.id == profile.board)
        })
        .unwrap_or(false);
    if !board_supported {
        errors.push(format!("unknown or unsupported board {}", profile.board));
    }
    if !matches!(
        profile.framework.as_str(),
        "arduino" | "esp-idf" | "rust" | "platformio"
    ) {
        errors.push(format!("unsupported framework {}", profile.framework));
    }
    if profile.capabilities.is_empty() {
        boundaries.push("hardware-verification boundary: no capabilities declared".to_string());
    }
    if !matches!(profile.verification_level.as_str(), "V1" | "V4" | "V5") {
        errors.push(format!(
            "unsupported verification level {}",
            profile.verification_level
        ));
    }
    if profile.verification_level == "V5" && profile.port.is_none() {
        boundaries.push("hardware-verification boundary: V5 requires an attached port".to_string());
    }
    if let Some(port) = profile.port.as_deref() {
        if port.trim().is_empty() {
            boundaries.push("serial boundary: configured port is empty".to_string());
        } else if !Path::new(port).exists() {
            boundaries.push(format!(
                "serial boundary: configured port is not present on this machine: {port}"
            ));
        }
    }
    if profile.port.is_none()
        && profile
            .capabilities
            .iter()
            .any(|capability| matches!(capability.as_str(), "serial" | "flash" | "ota"))
    {
        boundaries.push(
            "serial boundary: serial, flash, and OTA evidence require a discovered port"
                .to_string(),
        );
    }
    if profile
        .capabilities
        .iter()
        .any(|capability| capability == "lvgl")
        && profile.verification_level == "V4"
        && profile.simulator.is_none()
    {
        boundaries.push(
            "simulator boundary: V4 LVGL evidence requires simulator or page-data path".to_string(),
        );
    }

    let status = if !errors.is_empty() {
        "FAIL"
    } else if boundaries.is_empty() {
        "PASS"
    } else {
        "BOUNDARY"
    };

    HardwareVerifyReport {
        status: status.to_string(),
        verification_level: profile.verification_level,
        profile: profile_display,
        board: profile.board,
        framework: profile.framework,
        capabilities: profile.capabilities,
        boundaries,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn hardware_profile_contract() {
        let path = std::env::temp_dir().join("lilygo-skills-profile-boundary.json");
        let mut file = fs::File::create(&path).expect("profile temp file");
        writeln!(
            file,
            r#"{{"board":"board-t-display-s3","framework":"esp-idf","capabilities":["lvgl"],"verification_level":"V5"}}"#
        )
        .expect("write profile");
        let root = root();
        let report = verify_hardware_profile(root.as_path(), &path);
        assert_eq!(report.status, "BOUNDARY");
        assert!(
            report
                .boundaries
                .iter()
                .any(|boundary| boundary.contains("hardware-verification boundary"))
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn hardware_boundary_fixtures() {
        let path = std::env::temp_dir().join("lilygo-skills-profile-ota-boundary.json");
        let mut file = fs::File::create(&path).expect("profile temp file");
        writeln!(
            file,
            r#"{{"board":"board-t-watch","framework":"arduino","capabilities":["ota","serial"],"verification_level":"V1"}}"#
        )
        .expect("write profile");
        let root = root();
        let report = verify_hardware_profile(root.as_path(), &path);
        assert_eq!(report.status, "BOUNDARY");
        assert!(
            report
                .boundaries
                .iter()
                .any(|boundary| boundary.contains("serial boundary"))
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn serial_evidence_adapter_requires_present_port() {
        let path = std::env::temp_dir().join("lilygo-skills-profile-serial-boundary.json");
        let mut file = fs::File::create(&path).expect("profile temp file");
        writeln!(
            file,
            r#"{{"board":"board-t-display-s3","framework":"esp-idf","port":"/dev/tty.usbmodem-test","capabilities":["serial","flash"],"verification_level":"V5"}}"#
        )
        .expect("write profile");
        let root = root();
        let report = verify_hardware_profile(root.as_path(), &path);
        assert_eq!(report.status, "BOUNDARY", "{report:?}");
        assert!(
            report
                .boundaries
                .iter()
                .any(|boundary| boundary.contains("configured port is not present"))
        );
        let _ = fs::remove_file(path);
    }
}
