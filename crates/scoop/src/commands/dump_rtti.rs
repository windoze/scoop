//! `scoop dump-rtti` facade.

use std::ffi::OsString;
use std::path::PathBuf;

use miette::Result;

pub fn run(
    input: PathBuf,
    type_name: Option<String>,
    session_options: super::FacadeSessionOptions,
) -> Result<()> {
    let mut args = vec![OsString::from("dump-rtti"), input.into_os_string()];
    if let Some(type_name) = type_name {
        args.push(OsString::from("--type"));
        args.push(OsString::from(type_name));
    }
    super::run_compiler_passthrough(args, &session_options, "dump-rtti")
}
