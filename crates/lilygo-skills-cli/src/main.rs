//! Binary entry point that delegates all behavior to the CLI command module.
mod board_sniff;
mod capsule;
mod commands;
mod doctor;
mod facts;
mod hardware;
mod model;
mod playbooks;
mod preferences;
mod project_context;
mod project_skills;
mod recipes;
mod reference_catalog;
mod registry;
mod router;
mod session_context;
mod setup_plan;
mod source;
mod source_packs;
mod text_match;

use std::process::ExitCode;

fn main() -> ExitCode {
    match commands::run(std::env::args().skip(1), std::io::stdin()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
