use std::{
    env::current_dir,
    fs::{self, OpenOptions},
    io::{self, stdin, Write},
    str::FromStr,
};

use lazy_static::lazy_static;
use regex::Regex;
use terminal_colors::*;

mod terminal_colors;

lazy_static! {
    static ref SEMVER_REGEX: Regex = Regex::from_str(r"(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)(?:-(?P<prerelease>(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?P<buildmetadata>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?").unwrap();
}

fn main() {
    let new_version = get_user_version();

    let paths = find_files(current_dir().unwrap().to_str().unwrap()).unwrap();

    find_and_replace_versions_in_files(&paths, &new_version).unwrap();
}

fn get_user_version() -> String {
    print_user_version_input_prompt();

    loop {
        if let Ok(version) = read_user_version() {
            break version;
        }
        print_user_version_input_prompt();
    }
}

fn print_user_version_input_prompt() {
    println!("{BOLD}{BLUE_FG}New version (in SemVer format):{RESET_FG}{RESET_BOLD}");
}

fn read_user_version() -> Result<String, ()> {
    let mut buffer = String::new();
    stdin().read_line(&mut buffer).expect("improve this");
    buffer = buffer.trim().to_string();

    if !SEMVER_REGEX.is_match(&buffer) {
        return Err(());
    }

    Ok(buffer)
}

fn find_files(path: &str) -> Result<Vec<String>, io::Error> {
    let mut directories_to_check = Vec::from([path.to_string()]);
    let mut file_paths = Vec::new();
    let working_directory = format!("{}/", current_dir()?.to_str().unwrap());

    while let Some(directory) = directories_to_check.pop() {
        let directory_results = fs::read_dir(directory)?;
        for result in directory_results {
            let result = result?;
            let file_type = result.file_type()?;
            let file_path = result.path().to_str().expect("improve this").to_string();

            if file_type.is_dir() {
                directories_to_check.push(file_path);
            } else if file_type.is_file() {
                file_paths.push(
                    file_path
                        .strip_prefix(&working_directory)
                        .unwrap()
                        .to_string(),
                );
            }
        }
    }

    Ok(file_paths)
}

fn find_and_replace_versions_in_files(
    paths: &[String],
    new_version: &str,
) -> Result<(), io::Error> {
    for path in paths {
        find_and_replace_versions_in_file(path, new_version)?;
    }

    Ok(())
}

fn find_and_replace_versions_in_file(path: &str, new_version: &str) -> Result<(), io::Error> {
    let Ok(file_contents) = fs::read_to_string(path) else {
        // TODO: Only return Ok if the file is not UTF8.
        return Ok(());
    };
    let mut has_update = false;

    let mut file_lines = file_contents
        .lines()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();

    for (line_index, line) in file_lines.iter_mut().enumerate() {
        let mut current_offset = 0;

        while let Some(captures) = SEMVER_REGEX.captures_at(line, current_offset) {
            let whole_match = captures
                .get(0)
                .expect("capture with index 0 is guaranteed to be non-null");

            let line_start = &line[..whole_match.start()];
            let line_end = &line[whole_match.end()..];

            let formatted_original_line = format!(
                "{}{BOLD}{}{RESET_BOLD}{}",
                line_start,
                whole_match.as_str(),
                line_end
            );

            let formatted_pending_update_line = format!(
                "{}{BOLD}{}{RESET_BOLD}{}",
                line_start, new_version, line_end
            );

            println!("{BOLD}{}{RESET_BOLD}", path);
            println!("{YELLOW_FG}@@ line {} @@{RESET_FG}", line_index + 1);
            println!("{RED_FG}- {}{RESET_FG}", formatted_original_line);
            println!("{GREEN_FG}+ {}{RESET_FG}", formatted_pending_update_line);
            print_user_confirmation_input_prompt();

            let user_confirmation = loop {
                let confirmation = read_user_confirmation();
                if let Ok(confirmation) = confirmation {
                    break confirmation;
                }
                print_user_confirmation_input_prompt();
            };

            println!();

            if user_confirmation == UserConfirmationResponse::Ignore {
                current_offset = whole_match.end();
                continue;
            }

            current_offset = whole_match.start() + new_version.len();

            let pending_updated_line = format!("{}{}{}", line_start, new_version, line_end);

            *line = pending_updated_line;
            has_update = true;
        }
    }

    if has_update {
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)?
            .write_all(file_lines.join("\n").as_bytes())?;
    }

    Ok(())
}

fn print_user_confirmation_input_prompt() {
    println!("{BOLD}{BLUE_FG}Replace this version [y, n]?{RESET_FG}{RESET_BOLD}");
}

#[derive(PartialEq)]
enum UserConfirmationResponse {
    Replace,
    Ignore,
}

fn read_user_confirmation() -> Result<UserConfirmationResponse, ()> {
    let mut buffer = String::new();
    stdin().read_line(&mut buffer).expect("improve this");

    match buffer.trim().to_ascii_lowercase().as_str() {
        "y" => Ok(UserConfirmationResponse::Replace),
        "n" => Ok(UserConfirmationResponse::Ignore),
        _ => Err(()),
    }
}
