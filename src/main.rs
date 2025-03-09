use std::{
    env::current_dir,
    fs::{self, OpenOptions},
    io::{self, stdin, stdout, Write},
    str::FromStr,
};

use ansi_escape_codes::*;
use lazy_static::lazy_static;
use regex::Regex;

mod ansi_escape_codes;
mod arguments;

lazy_static! {
    static ref SEMVER_REGEX: Regex = Regex::from_str(r"(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)(?:-(?P<prerelease>(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?P<buildmetadata>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?").unwrap();
}

fn main() {
    let args = arguments::parse_arguments();

    let new_version = args.new_version.unwrap_or_else(get_user_version);

    let paths = find_files(&args.includes).unwrap();

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
    print!("{BOLD}{BLUE_FG}New version (in SemVer format):{RESET_FG}{RESET_BOLD} ");
    stdout()
        .flush()
        .expect("flush should not fail in this scenario");
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

fn find_files(includes: &[String]) -> Result<Vec<String>, io::Error> {
    let current_dir = current_dir().unwrap();
    let current_dir = current_dir.as_path().to_str().unwrap();

    let mut directories_to_check = Vec::from([current_dir.to_string()]);
    let mut file_paths = Vec::new();
    let working_directory = format!("{}/", current_dir);

    while let Some(directory) = directories_to_check.pop() {
        let directory_results = fs::read_dir(directory)?;
        for result in directory_results {
            let result = result?;
            let file_type = result.file_type()?;
            let file_path = result.path().to_str().expect("improve this").to_string();

            if file_type.is_dir() {
                directories_to_check.push(file_path);
            } else if file_type.is_file() {
                let relative_file_path = file_path
                    .strip_prefix(&working_directory)
                    .unwrap()
                    .to_string();

                if includes.is_empty() {
                    file_paths.push(relative_file_path);
                    continue;
                }

                if includes.iter().any(|i| relative_file_path.contains(i)) {
                    file_paths.push(relative_file_path);
                }
            }
        }
    }

    Ok(file_paths)
}

#[derive(Debug)]
enum FindAndReplaceVersionsError {
    UnableToSave,
}

fn find_and_replace_versions_in_files(
    paths: &[String],
    new_version: &str,
) -> Result<(), FindAndReplaceVersionsError> {
    for path in paths {
        let result = find_and_replace_versions_in_file(path, new_version)?;

        if result.should_quit {
            return Ok(());
        }
    }

    Ok(())
}

struct FindAndReplaceVersionsInFileResult {
    should_quit: bool,
}

fn find_and_replace_versions_in_file(
    path: &str,
    new_version: &str,
) -> Result<FindAndReplaceVersionsInFileResult, FindAndReplaceVersionsError> {
    let Ok(file_contents) = fs::read_to_string(path) else {
        // TODO: Only return Ok if the file is not UTF8.
        return Ok(FindAndReplaceVersionsInFileResult { should_quit: false });
    };

    let mut should_replace_all_in_file = false;

    let mut file_lines = file_contents
        .lines()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();

    for line_index in 0..file_lines.len() {
        let mut current_offset = 0;

        loop {
            let line = file_lines
                .get_mut(line_index)
                .expect("line should always be present");

            let Some(captures) = SEMVER_REGEX.captures_at(&line, current_offset) else {
                break;
            };

            let whole_match = captures
                .get(0)
                .expect("capture with index 0 is guaranteed to be non-null");

            let line_start = &line[..whole_match.start()];
            let line_end = &line[whole_match.end()..];

            let should_replace = if should_replace_all_in_file {
                true
            } else {
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
                let action = get_action(
                    path,
                    &(line_index + 1),
                    &formatted_original_line,
                    &formatted_pending_update_line,
                );
                match action {
                    Action::Replace => true,
                    Action::Ignore => false,
                    Action::Quit => {
                        return Ok(FindAndReplaceVersionsInFileResult { should_quit: true })
                    }
                    Action::ReplaceAllInFile => {
                        should_replace_all_in_file = true;
                        true
                    }
                    Action::IgnoreAllInFile => {
                        return Ok(FindAndReplaceVersionsInFileResult { should_quit: false })
                    }
                }
            };

            if !should_replace {
                current_offset = whole_match.end();
                continue;
            }

            current_offset = whole_match.start() + new_version.len();

            let pending_updated_line = format!("{}{}{}", line_start, new_version, line_end);

            *line = pending_updated_line;

            OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .map_err(|_| FindAndReplaceVersionsError::UnableToSave)?
                .write_all(file_lines.join("\n").as_bytes())
                .map_err(|_| FindAndReplaceVersionsError::UnableToSave)?;
        }
    }

    Ok(FindAndReplaceVersionsInFileResult { should_quit: false })
}

fn get_action(
    path: &str,
    line_number: &usize,
    formatted_original_line: &str,
    formatted_pending_update_line: &str,
) -> Action {
    println!("{BOLD}{}{RESET_BOLD}", path);
    println!("{YELLOW_FG}@@ line {} @@{RESET_FG}", line_number);
    println!("{RED_FG}- {}{RESET_FG}", formatted_original_line);
    println!("{GREEN_FG}+ {}{RESET_FG}", formatted_pending_update_line);

    let action = loop {
        print_user_confirmation_input_prompt();
        let confirmation_response = read_user_confirmation();
        if let Ok(confirmation_response) = confirmation_response {
            match confirmation_response {
                UserConfirmationResponse::Replace => break Action::Replace,
                UserConfirmationResponse::Ignore => break Action::Ignore,
                UserConfirmationResponse::Quit => break Action::Quit,
                UserConfirmationResponse::ReplaceAllInFile => break Action::ReplaceAllInFile,
                UserConfirmationResponse::IgnoreAllInFile => break Action::IgnoreAllInFile,
                UserConfirmationResponse::Help => {
                    print_user_confirmation_input_help();
                }
            };
        }
    };

    println!();

    action
}

fn print_user_confirmation_input_prompt() {
    print!("{BOLD}{BLUE_FG}Replace this version [y,n,q,a,d,?]?{RESET_FG}{RESET_BOLD} ");
    stdout()
        .flush()
        .expect("flush should not fail in this scenario");
}

fn print_user_confirmation_input_help() {
    print_user_confirmation_input_help_line("y - replace");
    print_user_confirmation_input_help_line("n - skip");
    print_user_confirmation_input_help_line("q - quit");
    print_user_confirmation_input_help_line("a - replace remaining in file");
    print_user_confirmation_input_help_line("d - skip remaining in file");
    print_user_confirmation_input_help_line("? - print help");
}

fn print_user_confirmation_input_help_line(text: &str) {
    println!("{BOLD}{MAGENTA_FG}{}{RESET_FG}{RESET_BOLD}", text);
}

#[derive(PartialEq)]
enum Action {
    Replace,
    Ignore,
    Quit,
    ReplaceAllInFile,
    IgnoreAllInFile,
}

#[derive(PartialEq)]
enum UserConfirmationResponse {
    Replace,
    Ignore,
    Quit,
    ReplaceAllInFile,
    IgnoreAllInFile,
    Help,
}

fn read_user_confirmation() -> Result<UserConfirmationResponse, ()> {
    let mut buffer = String::new();
    stdin().read_line(&mut buffer).expect("improve this");

    match buffer.trim().to_ascii_lowercase().as_str() {
        "y" => Ok(UserConfirmationResponse::Replace),
        "n" => Ok(UserConfirmationResponse::Ignore),
        "q" => Ok(UserConfirmationResponse::Quit),
        "a" => Ok(UserConfirmationResponse::ReplaceAllInFile),
        "d" => Ok(UserConfirmationResponse::IgnoreAllInFile),
        "?" => Ok(UserConfirmationResponse::Help),
        _ => Err(()),
    }
}
