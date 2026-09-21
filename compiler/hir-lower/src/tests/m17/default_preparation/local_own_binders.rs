use super::*;

#[test]
fn local_own_binders_survive_default_expansion_and_mir_lowering() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-type-source-defaults");
    let source = [
        "local-own-binders",
        "local-own-binder-combinations",
        "local-own-binder-calls",
    ]
    .map(|name| std::fs::read_to_string(root.join(format!("{name}.scoop"))).unwrap())
    .join("\n");
    let output = lower_source(&source).unwrap();
    let mir = scoop_mir_lower::lower(&output.local).unwrap();
    for (stage, dump) in [
        ("hir", hir::dump(&output.export)),
        ("mir", scoop_mir::dump(&mir)),
    ] {
        let actual = selected(&dump);
        let path = root.join(format!("local-own-binders.{stage}.snap"));
        if std::env::var_os("SCOOP_UPDATE_DEFAULT_BINDER_SNAPSHOTS").is_some() {
            std::fs::write(&path, &actual).unwrap();
        }
        assert_eq!(actual, std::fs::read_to_string(path).unwrap());
    }
}
fn selected(dump: &str) -> String {
    let mut keep = false;
    let mut result = String::new();
    for line in dump.lines() {
        if line.starts_with("  ") && !line.starts_with("   ") {
            keep = [
                "LocalGenericHost",
                "GenericLocalHost",
                "genericLocalDependency",
                "fun main",
                "$local.",
            ]
            .iter()
            .any(|name| line.contains(name));
        }
        if keep {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}
