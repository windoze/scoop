//! `scoop dump-stackmaps` facade.

use std::ffi::OsString;
use std::path::PathBuf;

use miette::Result;

pub fn run(input: PathBuf, verify_roots: bool, dump_records: bool) -> Result<()> {
    let mut args = vec![OsString::from("dump-stackmaps")];
    if verify_roots {
        args.push(OsString::from("--verify-roots"));
    }
    if dump_records {
        args.push(OsString::from("--dump-records"));
    }
    args.push(input.into_os_string());
    super::run_compiler_passthrough(args, &super::FacadeSessionOptions::new(), "dump-stackmaps")
}
