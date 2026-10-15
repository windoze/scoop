use super::*;

pub(super) fn check_mir_snapshot(name: &str, module: &mir::Module, functions: &[&str]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mir-stage")
        .join(format!("{name}.mir.snap"));
    let mut actual = String::new();
    let mut selected = false;
    let mut found = Vec::new();
    for line in dump(module).lines() {
        if line.starts_with("  ") && !line.starts_with("    ") {
            selected = functions.iter().any(|name| {
                if line.starts_with(&format!("  fun {name} @")) {
                    found.push(*name);
                    true
                } else {
                    false
                }
            });
        }
        if selected {
            actual.push_str(line);
            actual.push('\n');
        }
    }
    assert_eq!(
        found.len(),
        functions.len(),
        "missing selected MIR function"
    );
    if std::env::var_os("SCOOP_UPDATE_MIR_STAGE_SNAPSHOTS").is_some() {
        std::fs::write(&path, &actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
