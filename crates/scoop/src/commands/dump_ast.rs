//! `scoop dump-ast` facade.

use std::path::PathBuf;

use miette::Result;

pub fn run(input: PathBuf, session_options: super::FacadeSessionOptions) -> Result<()> {
    super::run_compiler_passthrough(
        ["dump-ast".into(), input.into_os_string()],
        &session_options,
        "dump-ast",
    )
}
