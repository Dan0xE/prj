use crate::config::{Config, WriteError};
use dirs::home_dir;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigureError {
    #[error("home directory not found")]
    HomeDirNotFound,
    #[error("path is not valid UTF-8")]
    PathNotParsable,
    #[error("failed to read input")]
    EndOfInput,
    #[error("cancelled by user")]
    UserCancelled,
    #[error("failed to write config")]
    WriteError(#[from] WriteError),
}

fn ask(prompt: &str, default: &str) -> Result<String, ConfigureError> {
    let stdin = io::stdin();

    let mut raw_result = String::new();

    println!("{} [{}]", prompt, default);
    stdin
        .read_line(&mut raw_result)
        .or(Err(ConfigureError::EndOfInput))?;

    let result = raw_result.trim();

    if result.is_empty() {
        return Ok(default.to_string());
    }
    Ok(result.to_string())
}

pub fn configure() -> Result<(), ConfigureError> {
    let home_dir = home_dir().ok_or(ConfigureError::HomeDirNotFound)?;

    let existing_config = Config::autoload().ok();
    if existing_config.is_some() {
        let overwrite = ask(
            "I found an existing config file, do you want to overwrite it?",
            "N",
        )?;
        if overwrite != "y" && overwrite != "Y" {
            return Err(ConfigureError::UserCancelled);
        }
    }

    println!(include_str!("./configure_root_doc.txt"));

    let default_root = match existing_config {
        Some(ref c) => c.root.clone(),
        None => home_dir.join("src"),
    };

    let root = ask(
        "Where will your projects live?",
        default_root
            .to_str()
            .ok_or(ConfigureError::PathNotParsable)?,
    )?;

    let location = match existing_config {
        Some(ref c) => c.location.clone(),
        None => home_dir.join(".prj"),
    };

    let root = PathBuf::from(root);
    let config = Config { root, location };

    config.write_config()?;
    Ok(())
}
