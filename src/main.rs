mod commands;
mod config;
mod discovery;

use clap::Command;
use config::Config;
use std::process::exit;

fn main() {
    let mut app = Command::new("prj")
        .version("0.1.0")
        .author("Jordan West")
        .about("Manage your local git projects")
        .subcommand(Command::new("configure").about("Create a configuration"))
        .subcommand(Command::new("list").about("Select a project"));
    let matches = app.get_matches_mut();

    if let Some(_) = matches.subcommand_matches("configure") {
        commands::configure::configure().unwrap();
    } else if let Some(_) = matches.subcommand_matches("list") {
        let config = Config::autoload().unwrap();

        if let Err(_) = commands::cd::run(&config) {
            exit(1);
        }
    } else {
        println!("{}", app.render_usage());
        exit(1)
    }
}
