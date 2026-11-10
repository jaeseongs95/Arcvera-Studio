// Deliberately console + argc1: libtest flags must never leak into NSIS /ARGS.
fn main() {
    #[cfg(windows)]
    if let Err(error) = arcvera_studio_lib::run_updater_actual_probe() {
        eprintln!("updater_actual_probe: {error}");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("updater_actual_probe requires Windows");
        std::process::exit(1);
    }
}
