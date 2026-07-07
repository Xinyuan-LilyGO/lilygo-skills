//! Binary entry point that delegates all behavior to the CLI command module.
mod benchmark;
#[cfg(test)]
mod benchmark_tests;
mod commands;
mod doctor;
mod facts;
mod generate;
mod goal;
mod hardware;
mod model;
mod peripheral_source;
mod playbooks;
mod preferences;
mod product_source;
mod project_context;
mod project_ledger;
mod project_skills;
mod recipes;
mod reference_catalog;
mod registry;
mod router;
mod session_context;
mod setup_plan;
mod source;
mod source_generation;
mod source_refresh;
mod templates;
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
