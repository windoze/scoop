use super::Cli;
use clap::{Command, CommandFactory};
use std::ffi::{OsStr, OsString};
use std::path::Path;

/// Insert the existing argument separator only after an explicit single-file
/// root. Clap remains responsible for option validation and every other command.
pub(crate) fn with_single_file_arguments(mut arguments: Vec<OsString>) -> Vec<OsString> {
    let mut cli = Cli::command();
    cli.build();
    let mut command = &cli;
    let mut in_run = false;
    let mut single_file = false;
    let mut index = 1;
    while let Some(argument) = arguments.get(index) {
        if argument == "--" {
            break;
        }
        if argument != "-" && argument.as_encoded_bytes().starts_with(b"-") {
            let Some(width) = option_width(command, argument) else {
                break;
            };
            index += width;
            continue;
        }
        if !in_run {
            if argument != "run" {
                break;
            }
            command = cli.find_subcommand("run").expect("run is a CLI command");
            in_run = true;
        } else if !single_file {
            if Path::new(argument).extension() != Some(OsStr::new("scoop")) {
                break;
            }
            single_file = true;
        } else {
            arguments.insert(index, OsString::from("--"));
            break;
        }
        index += 1;
    }
    arguments
}

// Read option arity from the actual command rather than duplicating its flags.
// Scoop runner options take either no value or one value. Attached values stay
// opaque, including non-UTF-8 paths, and are validated by Clap afterward.
fn option_width(command: &Command, argument: &OsStr) -> Option<usize> {
    let bytes = argument.as_encoded_bytes();
    if let Some(long) = bytes.strip_prefix(b"--") {
        let separator = long.iter().position(|byte| *byte == b'=');
        let name = std::str::from_utf8(&long[..separator.unwrap_or(long.len())]).ok()?;
        let option = command
            .get_arguments()
            .find(|option| option.get_long() == Some(name))?;
        return Some(1 + usize::from(option.get_action().takes_values() && separator.is_none()));
    }
    let short = bytes.strip_prefix(b"-")?;
    for (index, byte) in short.iter().enumerate() {
        let option = command
            .get_arguments()
            .find(|option| option.get_short() == Some(char::from(*byte)))?;
        if option.get_action().takes_values() {
            return Some(if index + 1 == short.len() { 2 } else { 1 });
        }
    }
    Some(1)
}
