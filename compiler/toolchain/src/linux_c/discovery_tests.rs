use super::*;

#[test]
fn underlying_gcc_selection_uses_the_direct_driver_or_musl_wrapper_input() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().as_os_str();
    let gcc = directory.path().join("gcc");
    let selected = directory.path().join("selected-gcc");
    for file in [&gcc, &selected] {
        std::fs::write(file, "compiler locator").unwrap();
    }
    let selected = std::fs::canonicalize(selected).unwrap();
    let missing = OsStr::new("missing-gcc");
    assert_eq!(
        underlying_compiler(LinuxLibc::Gnu, &selected, path, Some(missing)).unwrap(),
        selected,
    );
    assert_eq!(
        underlying_compiler(LinuxLibc::Musl, &selected, path, None).unwrap(),
        std::fs::canonicalize(gcc).unwrap(),
    );
    assert_eq!(
        underlying_compiler(LinuxLibc::Musl, &selected, path, Some(selected.as_os_str())).unwrap(),
        selected,
    );
    assert!(underlying_compiler(LinuxLibc::Musl, &selected, path, Some(missing)).is_err());
}
