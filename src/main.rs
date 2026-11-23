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

    if matches.subcommand_matches("configure").is_some() {
        commands::configure::configure().unwrap();
    } else if matches.subcommand_matches("list").is_some() {
        let config = Config::autoload().unwrap();

        if commands::cd::run(&config).is_err() {
            exit(1);
        }
    } else {
        println!("{}", app.render_usage());
        exit(1)
    }
}
