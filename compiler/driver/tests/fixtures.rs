//! Fixture runner: runs every `tests/fixtures/**/*.scoop` through the
//! compiler and snapshot-tests the rendered diagnostics (golden dump).

use std::fs;
use std::path::{Path, PathBuf};

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn collect_fixtures(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.expect("read dir entry").path())
    {
        if entry.is_dir() {
            collect_fixtures(&entry, out);
        } else if entry.extension().is_some_and(|ext| ext == "scoop") {
            out.push(entry);
        }
    }
}

#[test]
fn fixtures() {
    let root = fixture_root();
    let mut fixtures = Vec::new();
    collect_fixtures(&root, &mut fixtures);
    fixtures.sort();
    assert!(
        !fixtures.is_empty(),
        "no fixtures found under {}",
        root.display()
    );

    for fixture in fixtures {
        let name = fixture
            .strip_prefix(&root)
            .expect("fixture under root")
            .to_string_lossy()
            .replace('/', "__");
        let rendered = match scoopc::compile_file(&fixture) {
            Ok(()) => "OK".to_string(),
            Err(diagnostics) => diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        };
        insta::assert_snapshot!(name, rendered);
    }
}
