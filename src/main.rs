extern crate ffmpeg_next as ffmpeg;

use std::path::Path;

use chrono::Datelike;

use indicatif::{ProgressBar, ProgressStyle};
use log::LevelFilter;
use log::{info, warn};

use crate::pserror::error::*;
use config::configurator::{get_config, Config};
use photo::Photo;

mod config;
mod discovery;
mod photo;
mod pserror;
mod zipfiles;

mod error_messages {
    pub const BOTH_MUST_BE_PROVIDED: &str = "Both --src and --dest must be provided";
}

#[derive(PartialEq, Eq, Debug)]
enum Action {
    HELP,
    CONVERT(Config),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = get_config(Option::None);
    if config.is_err() {
        config::configurator::print_help();
        return Err(config.err().unwrap());
    }

    let config = config.unwrap();
    info!("Starting conversion for config {:?}", config);

    ffmpeg::init().unwrap();
    convert_files(&config);

    return Ok(());
}

fn convert_files(config: &Config) {
    if config.logfile.is_some() {
        let logfile = config.logfile.as_ref().unwrap();
        match simple_logging::log_to_file(logfile, LevelFilter::Info) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Couldn't enable logging: {:?}", e);
                std::process::exit(1);
            }
        }
    }

    if config.source.ends_with(".zip") {
        zipfiles::process_zip_file(&config.source, &config);
    } else {
        let file_list = discovery::discovery::list_all_files(&config.source);
        let mut photo_list = discovery::discovery::process_raw_files(&file_list);
        info!("Produced a list of {} files", photo_list.len());
        update_new_path(&config.destination, &mut photo_list);
        info!("Updated a list of {} files", file_list.len());
        let bar = ProgressBar::new(file_list.len() as u64);

        bar.set_message("Moving/copying files ... ");
        bar.set_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:80.green/red} {pos:>7}/{len:7} {msg}")
                .progress_chars("??????"),
        );
        for photo in photo_list {
            bar.inc(1);
            match move_photo(&photo, !config.copy, config.dry_run) {
                Ok(_) => {
                    info!(
                        "Moved photo {} -> {}",
                        photo.path().as_ref().unwrap(),
                        photo.new_path().as_ref().unwrap()
                    );
                }
                Err(err) => {
                    warn!("Failed to move photo {:?}: {}", photo.path(), err);
                }
            }
        }
        bar.finish();
    }
}

fn update_new_path(dest_dir: &String, photos: &mut Vec<Photo>) {
    for photo in photos {
        update_photo_new_path(dest_dir, photo, Option::None)
    }
}

fn update_photo_new_path(dest_dir: &String, photo: &mut Photo, original_name: Option<&str>) {
    let existing_path = Path::new(photo.path().as_ref().unwrap());
    match existing_path.file_name() {
        None => {
            info!(
                "Path doesn't appear to have a valid file name: {}",
                photo
                    .path()
                    .as_ref()
                    .unwrap_or(&"BAD_FILE_NAME".to_string())
            )
        }
        Some(file_name) => {
            let new_name = match original_name {
                None => file_name.to_str().unwrap(),
                Some(original_name) => original_name,
            };

            // photo must have valid date at this point.
            let date = photo.date().unwrap();
            let path = format!(
                "{}/{}/{:02}/{:02}/{}",
                dest_dir,
                date.year(),
                date.month(),
                date.day(),
                new_name // should be safe (why?)
            );

            photo.set_new_path(path);
        }
    }
}

fn move_photo(photo: &Photo, move_file: bool, dry_run: bool) -> Result<(), PsError> {
    let new_path = photo.new_path().as_ref().unwrap();

    let full_path = Path::new(new_path);
    let dir = match full_path.parent() {
        None => {
            return Err(PsError::new(
                PsErrorKind::IoError,
                format!("No parent directory for {}", new_path),
            ));
        }
        Some(dir) => dir,
    };

    if !dir.exists() {
        match std::fs::create_dir_all(dir) {
            Err(err) => {
                return Err(err.into());
            }
            _ => {}
        }
    }

    if dry_run {
        info!("Dry-run, not really copying/moving {:?}", photo.path());
        return Ok(());
    }

    // If photo doesn't have path() at this point, it's a fatal mistake.
    let original_path = photo.path().as_ref().unwrap();

    if move_file {
        match std::fs::rename(original_path, &new_path) {
            Ok(_) => {}
            Err(err) => {
                info!("Failed to move file: {}", err);
            }
        }
    } else {
        match std::fs::copy(original_path, &new_path) {
            Ok(_) => {}
            Err(err) => {
                info!("Failed to copy {} -> {}: {}", original_path, &new_path, err);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
