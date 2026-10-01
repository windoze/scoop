use super::*;

pub(super) fn check_mir_snapshot(name: &str, module: &mir::Module) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mir-stage")
        .join(format!("{name}.mir.snap"));
    let actual = dump(module);
    if std::env::var_os("SCOOP_UPDATE_MIR_STAGE_SNAPSHOTS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
