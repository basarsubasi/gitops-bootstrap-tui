use std::process::Command;

mod actions;
mod config;
mod explorer;
mod generate;
mod git;
mod helm;
mod home;
mod summary;
mod ui;
mod wizard;

fn check_cli_deps() -> Result<(), String> {
    if Command::new("git").arg("--version").output().is_err() {
        return Err("Required dependency 'git' is not installed or not in PATH.".to_string());
    }
    if Command::new("helm").arg("version").output().is_err() {
        return Err("Required dependency 'helm' is not installed or not in PATH.".to_string());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = check_cli_deps() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
    let cfg = config::AppConfig::load()?;
    ui::run_app(cfg)?;

    Ok(())
}
