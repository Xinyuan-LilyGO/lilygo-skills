//! Generated board skill renderer for compact source-backed board summaries
//! used by install and generated-cache workflows.
use crate::model::BoardRecord;
use crate::source::load_board_index;
use crate::templates::render_template;
use std::path::Path;
use std::process::Command;

const BOARD_TEMPLATE: &str = include_str!("../../../templates/skills/board.md");

pub(crate) fn board_skill_files(root: &Path) -> Result<Vec<(String, String)>, String> {
    let board_index = load_board_index(root)?;
    Ok(board_index
        .boards
        .iter()
        .filter(|board| board.supported)
        .map(|board| (board.id.clone(), render_board_skill(board)))
        .collect())
}

pub(crate) fn install_runtime(root: &Path, home: Option<&Path>) -> Result<Vec<String>, String> {
    let mut command = Command::new("node");
    command.arg("install.js").arg("--all");
    if let Some(home) = home {
        command.arg("--home").arg(home);
    }
    let output = command
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to run install.js: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid install.js JSON: {error}"))?;
    if value.get("status").and_then(|status| status.as_str()) != Some("PASS") {
        return Err(format!("install.js failed: {value}"));
    }
    Ok(value
        .get("writes")
        .and_then(|writes| writes.as_array())
        .map(|writes| {
            writes
                .iter()
                .filter_map(|write| write.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

fn render_board_skill(board: &BoardRecord) -> String {
    let title = if board.product {
        "Product Board"
    } else if board.id.starts_with("series-") {
        "Series"
    } else {
        "Board"
    };
    let family = board
        .family_id
        .as_ref()
        .map(|family| format!("- Family fallback: `{family}`.\n"))
        .unwrap_or_default();
    let source_urls = render_source_urls(board);
    let matrix = render_peripheral_matrix(board);
    let demos = render_demo_refs(board);
    let warnings = if board.warnings.is_empty() {
        String::new()
    } else {
        board
            .warnings
            .iter()
            .map(|warning| format!("- Boundary: {warning}\n"))
            .collect()
    };
    render_template(
        BOARD_TEMPLATE,
        &[
            ("title", title.to_string()),
            ("display_name", board.display_name.clone()),
            ("mcu", board.mcu.clone()),
            ("frameworks", inline_list(&board.frameworks)),
            ("peripherals", inline_list(&board.peripherals)),
            ("family", family),
            ("repo_url", board.repo_url.clone()),
            ("wiki_url", board.wiki_url.clone()),
            ("source_status", board.source_status.clone()),
            ("source_urls", source_urls),
            ("peripheral_matrix", matrix),
            ("demo_refs", demos),
            ("warnings", warnings),
        ],
    )
}

fn render_source_urls(board: &BoardRecord) -> String {
    if board.source_urls.is_empty() {
        return String::new();
    }
    let body = board
        .source_urls
        .iter()
        .map(|source| format!("- `{}` [{}]: {}\n", source.kind, source.status, source.url))
        .collect::<String>();
    format!("\n## Source Pointers\n\n{body}")
}

fn render_peripheral_matrix(board: &BoardRecord) -> String {
    if board.peripheral_matrix.is_empty() {
        return String::new();
    }
    let body = board
        .peripheral_matrix
        .iter()
        .map(|entry| {
            format!(
                "- `{}`: {} / `{}` / bus `{}` / driver `{}` / evidence `{}` / source {}\n",
                entry.category,
                entry.name,
                entry.chip,
                entry.bus,
                entry.driver,
                entry.evidence_level,
                entry.source_url
            )
        })
        .collect::<String>();
    format!("\n## Peripheral Matrix\n\n{body}")
}

fn render_demo_refs(board: &BoardRecord) -> String {
    if board.demo_refs.is_empty() {
        return String::new();
    }
    let body = board
        .demo_refs
        .iter()
        .map(|demo| {
            format!(
                "- `{}` `{}`: `{}` [{} stale={}] {}\n",
                demo.framework,
                demo.target,
                demo.path,
                demo.evidence_level,
                demo.stale,
                demo.source_url
            )
        })
        .collect::<String>();
    format!("\n## Demo References\n\n{body}")
}

fn inline_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_board_skill_contract_marker() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let files = board_skill_files(root.as_path()).expect("board skills");
        let (_, content) = files
            .into_iter()
            .find(|(id, _)| id == "board-t-watch-ultra")
            .expect("t-watch board skill");
        assert!(content.contains("Generation Contract: templates/skills/board.md"));
        assert!(!content.contains("{{"));
    }
}
