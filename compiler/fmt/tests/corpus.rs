mod support;
use std::path::{Path, PathBuf};

#[test]
fn repository_sources_preserve_syntax_and_tokens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for directory in ["sysroot/src", "tests/fixtures", "examples"] {
        let path = root.join(directory);
        if path.exists() {
            collect(&path, &mut files);
        }
    }
    files.sort();
    let mut checked = 0;
    for file in files {
        let Ok(source) = String::from_utf8(std::fs::read(&file).unwrap()) else {
            continue;
        };
        let parsed = scoop_parser::parse_recovering(&source).unwrap();
        if parsed.ast().is_none() {
            continue;
        }
        assert!(
            std::panic::catch_unwind(|| support::checked(&source)).is_ok(),
            "formatter contract failed for {}",
            file.display()
        );
        checked += 1;
    }
    assert!(checked > 500, "only {checked} corpus sources found");
    eprintln!("validated {checked} parseable repository sources");
}

fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "scoop")
        {
            files.push(path);
        }
    }
}
