use super::*;
use clap::Parser;

#[derive(Parser)]
struct Build {
    #[command(flatten)]
    options: BuildOptions,
}

#[test]
fn persisted_options_replay_paths_and_explicit_values_through_the_normal_parser() {
    let cwd = tempfile::tempdir().unwrap();
    let options = Build::try_parse_from([
        "build",
        "app/Cone.toml",
        "--release",
        "--export-ir",
        "full",
        "--debug-info",
        "line-tables",
        "--target-dir",
        "build files",
        "--generator",
        "codec=tools/my generator",
        "--jobs",
        "3",
        "--scoopc",
        "bin/scoopc",
        "--cc",
        "clang",
        "--cxx",
        "tools/clang++",
        "--link-mode",
        "static",
        "--enable-stack-trace=false",
        "--library-path",
        "native libraries",
        "--lockfile",
        "locks/Cone.lock",
        "--package-cache",
        "packages",
        "--offline",
        "--package",
        "example:app",
        "--system-library-root",
        "libs",
        "--message-format",
        "json",
    ])
    .unwrap()
    .options
    .normalized(cwd.path())
    .unwrap();
    assert_eq!(options.native.cc, Some("clang".into()));
    assert_eq!(options.native.cxx, Some(cwd.path().join("tools/clang++")));
    let value = serde_json::to_value(&options).unwrap();
    let saved: BuildOptions = serde_json::from_value(value.clone()).unwrap();
    let replayed =
        Build::try_parse_from(std::iter::once(OsString::from("build")).chain(saved.arguments()))
            .unwrap()
            .options;
    assert_eq!(serde_json::to_value(replayed).unwrap(), value);
    assert_eq!(saved.native.enable_stack_trace, Some(false));
}

#[test]
fn argument_carriers_keep_empty_and_native_arguments_distinct() {
    let mut arguments = vec![OsString::new(), "two words".into(), "汉字 %".into()];
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        arguments.push(OsString::from_vec(vec![b'a', 0xff]));
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        arguments.push(OsString::from_wide(&[b'a' as u16, 0xd800]));
    }
    let values = arguments
        .iter()
        .map(|value| CommandArgument::from_os(value).unwrap())
        .collect::<Vec<_>>();
    let decoded: Vec<CommandArgument> =
        serde_json::from_slice(&serde_json::to_vec(&values).unwrap()).unwrap();
    assert_eq!(
        decoded
            .iter()
            .map(|value| value.to_os().unwrap())
            .collect::<Vec<_>>(),
        arguments
    );
}
