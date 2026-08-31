//! Fixture runner: compiles and runs every `tests/fixtures/**/*.scoop`
//! through the full pipeline and snapshot-tests a single merged snapshot
//! per fixture (rendered diagnostics on failure; stage dumps plus the
//! program's stdout on success).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}

fn fixture_root() -> PathBuf {
    workspace_root().join("tests/fixtures")
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
        // Snapshot name and diagnostic file name are both relative to
        // `tests/fixtures` so snapshots stay portable across machines.
        let relative = fixture
            .strip_prefix(&root)
            .expect("fixture under root")
            .to_string_lossy()
            .replace('\\', "/");
        // Snapshots live next to their fixture
        // (`tests/fixtures/<group>/<name>.scoop.snap`), not in a
        // separate directory tree.
        let name = fixture
            .file_name()
            .expect("fixture file name")
            .to_string_lossy()
            .to_string();
        let snapshot_dir = fixture.parent().expect("fixture dir").to_path_buf();
        let out_dir = workspace_root()
            .join("target/fixtures-out")
            .join(relative.trim_end_matches(".scoop").replace('/', "__"));
        let source = fs::read_to_string(&fixture)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", fixture.display()));
        // Fixtures whose first line is `// EXPECT-TRAP` must compile and
        // then abort at runtime (M3: `!!` on `None`); their stderr is
        // snapshotted instead of stdout.
        let expect_trap = source
            .lines()
            .next()
            .is_some_and(|line| line.trim() == "// EXPECT-TRAP");

        let mut options = scoopc::CompileOptions::default();
        let native_source = fixture
            .parent()
            .expect("fixture directory")
            .join("native.c");
        if native_source.exists() {
            fs::create_dir_all(&out_dir).expect("create native fixture output directory");
            let native_object = out_dir.join("native.o");
            let native_archive = out_dir.join("libfixture_native.a");
            let compile = Command::new("cc")
                .arg("-std=c11")
                .arg("-c")
                .arg(&native_source)
                .arg("-o")
                .arg(&native_object)
                .output()
                .expect("run native fixture C compiler");
            assert!(
                compile.status.success(),
                "{relative}: native fixture compilation failed: {}",
                String::from_utf8_lossy(&compile.stderr)
            );
            let archive = Command::new("ar")
                .arg("rcs")
                .arg(&native_archive)
                .arg(&native_object)
                .output()
                .expect("run native fixture archiver");
            assert!(
                archive.status.success(),
                "{relative}: native fixture archive failed: {}",
                String::from_utf8_lossy(&archive.stderr)
            );
            options.library_paths.push(out_dir.clone());
        }

        let snapshot = match scoopc::compile_file_with_options(&fixture, &out_dir, &options) {
            Ok(success) => {
                let run = Command::new(&success.binary)
                    .output()
                    .unwrap_or_else(|e| panic!("cannot run {}: {e}", success.binary.display()));
                let (run_section, output) = if expect_trap {
                    assert!(
                        !run.status.success(),
                        "{relative}: expected a trap, but the binary exited with {}",
                        run.status
                    );
                    (
                        "== trap ==",
                        String::from_utf8_lossy(&run.stderr).to_string(),
                    )
                } else {
                    assert!(
                        run.status.success(),
                        "{relative}: compiled binary exited with {}",
                        run.status
                    );
                    (
                        "== run ==",
                        String::from_utf8_lossy(&run.stdout).to_string(),
                    )
                };
                format!(
                    "== ast ==\n{}\n== hir ==\n{}\n== mir ==\n{}\n== lir ==\n{}\n{run_section}\n{output}",
                    success.dumps.ast, success.dumps.hir, success.dumps.mir, success.dumps.lir,
                )
            }
            Err(diagnostics) => {
                // Diagnostics carry the index of their input file (core
                // files first, the user fixture last). Name the fixture
                // relative to `tests/fixtures` so snapshots stay portable.
                let mut inputs = scoopc::load_inputs(&fixture).unwrap_or_default();
                if let Some(user) = inputs.last_mut() {
                    user.name = relative.clone();
                    user.source = source.clone();
                }
                let rendered =
                    scoopc::render_diagnostics(&diagnostics, &inputs, &relative, &source);
                format!("== diagnostics ==\n{rendered}\n")
            }
        };
        insta::with_settings!({
            snapshot_path => &snapshot_dir,
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(name, snapshot);
        });
    }
}
