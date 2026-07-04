//! No-mutation setup planner for host dependencies, framework toolchains, and
//! optional debug helpers required by LilyGO development workflows.
use crate::model::{SetupPlan, ToolchainPlan};
use std::path::Path;

pub(crate) fn setup_plan(framework: &str, project: Option<&Path>) -> Result<SetupPlan, String> {
    let framework = normalize_framework(framework)?;
    let mut toolchains = common_toolchains();
    match framework {
        "arduino" => toolchains.extend(arduino_toolchains()),
        "platformio" => toolchains.extend(platformio_toolchains()),
        "esp-idf" => toolchains.extend(esp_idf_toolchains()),
        "rust" => toolchains.extend(rust_toolchains()),
        other => return Err(format!("unsupported setup framework: {other}")),
    }
    Ok(SetupPlan {
        schema_version: 1,
        framework: framework.to_string(),
        project: project.map(|path| path.display().to_string()),
        status: "planned".to_string(),
        dry_run: true,
        no_mutation: true,
        host_requirements: vec![
            "rustup".to_string(),
            "cargo".to_string(),
            "node".to_string(),
            "git".to_string(),
            "python3".to_string(),
        ],
        next_commands: next_commands(framework),
        private_inputs_needed: vec![
            "USB serial port is needed only for later flash/monitor commands".to_string(),
            "Wi-Fi credentials or OTA target must stay in private local config if needed later"
                .to_string(),
        ],
        toolchains,
        writes: Vec::new(),
    })
}

fn normalize_framework(framework: &str) -> Result<&'static str, String> {
    match framework {
        "arduino" | "fw-arduino" => Ok("arduino"),
        "platformio" | "pio" | "fw-platformio" => Ok("platformio"),
        "esp-idf" | "idf" | "fw-esp-idf" => Ok("esp-idf"),
        "rust" | "esp-rs" | "fw-rust" => Ok("rust"),
        other => Err(format!("unsupported setup framework: {other}")),
    }
}

fn common_toolchains() -> Vec<ToolchainPlan> {
    vec![
        tool(
            "rustup",
            &["cli-runtime"],
            "rustup --version",
            "Install from https://rustup.rs/",
        ),
        tool(
            "cargo",
            &["cli-runtime"],
            "cargo --version",
            "Installed by rustup; required to build lilygo-skills.",
        ),
        tool(
            "node",
            &["installer"],
            "node --version",
            "Install Node.js LTS for install.js and runtime parity checks.",
        ),
        tool(
            "git",
            &["source"],
            "git --version",
            "Install Git for LilyGO, Espressif, and reference source checkouts.",
        ),
    ]
}

fn arduino_toolchains() -> Vec<ToolchainPlan> {
    vec![
        tool(
            "arduino-cli",
            &["arduino"],
            "arduino-cli version",
            "Install Arduino CLI from https://docs.arduino.cc/arduino-cli/.",
        ),
        tool(
            "arduino-esp32-core",
            &["arduino"],
            "arduino-cli core list | grep esp32:esp32",
            "Use arduino-cli core update-index and core install esp32:esp32.",
        ),
        tool(
            "lilygo-libraries",
            &["arduino"],
            "arduino-cli lib list | grep -i LilyGo",
            "Install LilyGoLib dependencies from the official LilyGO repository guidance.",
        ),
        tool(
            "espflash",
            &["flash", "serial"],
            "espflash --version",
            "Install with cargo install espflash when flash/serial evidence is needed.",
        ),
        tool(
            "serial-mcp-server",
            &["serial-debug"],
            "serial-mcp-server --help",
            "Optional serial observation loop: https://github.com/Adancurusul/serial-mcp-server.",
        ),
    ]
}

fn platformio_toolchains() -> Vec<ToolchainPlan> {
    vec![
        tool(
            "python3",
            &["platformio"],
            "python3 --version",
            "Install Python 3 before PlatformIO Core.",
        ),
        tool(
            "platformio-core",
            &["platformio"],
            "pio --version",
            "Install PlatformIO Core from https://docs.platformio.org/.",
        ),
        tool(
            "platformio-esp32-platform",
            &["platformio"],
            "pio pkg list --global | grep espressif32",
            "PlatformIO resolves espressif32 from platformio.ini or pio pkg install.",
        ),
        tool(
            "serial-mcp-server",
            &["serial-debug"],
            "serial-mcp-server --help",
            "Optional serial observation loop for pio device monitor output.",
        ),
    ]
}

fn esp_idf_toolchains() -> Vec<ToolchainPlan> {
    vec![
        tool(
            "esp-idf",
            &["esp-idf"],
            "idf.py --version",
            "Install ESP-IDF from Espressif get-started docs for ESP32-S3.",
        ),
        tool(
            "idf-tools",
            &["esp-idf"],
            "python3 $IDF_PATH/tools/idf_tools.py list",
            "Use the official install script to provision compiler, OpenOCD, and Python environment.",
        ),
        tool(
            "serial-mcp-server",
            &["serial-debug"],
            "serial-mcp-server --help",
            "Optional serial observation loop for idf.py monitor output.",
        ),
    ]
}

fn rust_toolchains() -> Vec<ToolchainPlan> {
    vec![
        tool(
            "espup",
            &["rust", "esp-rs"],
            "espup --version",
            "Install with cargo install espup and run espup install.",
        ),
        tool(
            "espflash",
            &["rust", "flash", "serial"],
            "espflash --version",
            "Install with cargo install espflash.",
        ),
        tool(
            "cargo-espflash",
            &["rust", "flash"],
            "cargo espflash --version",
            "Install with cargo install cargo-espflash when using cargo espflash.",
        ),
        tool(
            "serial-mcp-server",
            &["serial-debug"],
            "serial-mcp-server --help",
            "Optional serial observation loop for espflash monitor output.",
        ),
    ]
}

fn next_commands(framework: &str) -> Vec<String> {
    match framework {
        "arduino" => vec![
            "rustup --version".to_string(),
            "cargo run -p lilygo-skills-cli -- verify --json".to_string(),
            "node install.js --all --dry-run".to_string(),
            "arduino-cli version".to_string(),
            "arduino-cli core update-index".to_string(),
            "arduino-cli core install esp32:esp32".to_string(),
        ],
        "platformio" => vec![
            "rustup --version".to_string(),
            "cargo run -p lilygo-skills-cli -- verify --json".to_string(),
            "node install.js --all --dry-run".to_string(),
            "python3 --version".to_string(),
            "pio --version".to_string(),
            "pio run".to_string(),
        ],
        "esp-idf" => vec![
            "rustup --version".to_string(),
            "cargo run -p lilygo-skills-cli -- verify --json".to_string(),
            "node install.js --all --dry-run".to_string(),
            "idf.py --version".to_string(),
            "idf.py build".to_string(),
        ],
        "rust" => vec![
            "rustup --version".to_string(),
            "cargo run -p lilygo-skills-cli -- verify --json".to_string(),
            "node install.js --all --dry-run".to_string(),
            "espup --version".to_string(),
            "espflash --version".to_string(),
            "cargo build".to_string(),
        ],
        _ => Vec::new(),
    }
}

fn tool(id: &str, required_for: &[&str], check: &str, install_hint: &str) -> ToolchainPlan {
    ToolchainPlan {
        id: id.to_string(),
        required_for: required_for.iter().map(|value| value.to_string()).collect(),
        check: check.to_string(),
        install_hint: install_hint.to_string(),
        mutates: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_plan_toolchains() {
        for framework in ["arduino", "platformio", "esp-idf", "rust"] {
            let plan = setup_plan(framework, None).expect("setup plan");
            assert_eq!(plan.status, "planned");
            assert!(plan.dry_run);
            assert!(plan.no_mutation);
            assert!(plan.host_requirements.contains(&"rustup".to_string()));
            assert!(plan.host_requirements.contains(&"cargo".to_string()));
            assert!(plan.host_requirements.contains(&"node".to_string()));
            assert!(!plan.toolchains.is_empty());
            assert!(!plan.next_commands.is_empty());
        }
        let arduino = setup_plan("arduino", None).expect("arduino");
        let ids = arduino
            .toolchains
            .iter()
            .map(|tool| tool.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(ids.contains("arduino-cli"));
        assert!(ids.contains("arduino-esp32-core"));
    }

    #[test]
    fn setup_plan_no_mutation() {
        let plan = setup_plan("platformio", None).expect("platformio");
        assert!(plan.writes.is_empty());
        assert!(plan.toolchains.iter().all(|tool| !tool.mutates));
        assert!(
            plan.next_commands
                .iter()
                .any(|command| command == "node install.js --all --dry-run")
        );
    }
}
