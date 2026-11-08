#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|arg| arg == "--runtime-transition-test") {
        std::process::exit(agent_grid_lib::runtime_transition_test::run());
    }
    if std::env::args().any(|arg| arg == "--self-test") {
        std::process::exit(agent_grid_lib::self_test::run());
    }
    if std::env::args().any(|arg| arg == "--codex-smoke") {
        std::process::exit(agent_grid_lib::self_test::run_codex_smoke());
    }
    agent_grid_lib::run();
}
