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

fn native_sources(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut sources = Vec::new();
    for entry in fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .map(|entry| entry.expect("read native fixture directory entry").path())
    {
        if entry.extension().is_none_or(|extension| extension != "c") {
            continue;
        }
        let stem = entry
            .file_stem()
            .expect("native source has a stem")
            .to_string_lossy()
            .into_owned();
        let library = if stem == "native" {
            Some("fixture_native")
        } else {
            stem.strip_prefix("native-")
        };
        if let Some(library) = library {
            sources.push((entry, library.to_string()));
        }
    }
    sources.sort_by(|left, right| left.1.cmp(&right.1));
    sources
}

fn runs_under_gc_stress(relative: &str) -> bool {
    matches!(
        relative,
        "m5-arrays/recursive-reference-scans.scoop"
            | "m8-exceptions/handler-exits.scoop"
            | "m8-exceptions/custom-exception.scoop"
            | "m10-coroutines/gc-across-suspension.scoop"
            | "m11-functions/captures.scoop"
            | "m11-functions/variance.scoop"
            | "m12-extern-scoop/extern-scoop.scoop"
            | "m13-callback/managed-callback.scoop"
            | "m13-callback/foreign-continuation.scoop"
            | "m14-generics/generic-exception.scoop"
            | "m15-moving/external-exception.scoop"
            | "m15-moving/handle-pin.scoop"
            | "m15-moving/root-shapes.scoop"
            | "m17-vararg/vararg-semantics.scoop"
            | "m18-invoke/combined.scoop"
            | "m18-operators/combined.scoop"
            | "m18-safe-call/combined.scoop"
    )
}

fn verify_linked_stackmap_fixups(binary: &Path, relative: &str) {
    let output = Command::new("xcrun")
        .arg("dyld_info")
        .arg("-fixups")
        .arg(binary)
        .output()
        .expect("run dyld_info for linked fixture binary");
    assert!(
        output.status.success(),
        "{relative}: dyld_info failed for {}: {}",
        binary.display(),
        String::from_utf8_lossy(&output.stderr)
    );

    let fixups = String::from_utf8_lossy(&output.stdout);
    assert!(
        fixups.lines().any(|line| {
            line.contains("__LLVM_STACKMAPS")
                && line.contains("__llvm_stackmaps")
                && line.contains("rebase")
        }),
        "{relative}: linked __llvm_stackmaps contains no dyld rebase fixup:\n{fixups}"
    );
}

#[test]
fn fixtures() {
    let workspace = workspace_root();
    std::env::set_current_dir(&workspace).expect("fixture runner enters the workspace root");
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
        let compile_path = Path::new("tests/fixtures").join(&relative);
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
        let native_sources = native_sources(fixture.parent().expect("fixture directory"));
        if !native_sources.is_empty() {
            fs::create_dir_all(&out_dir).expect("create native fixture output directory");
            for (native_source, library) in native_sources {
                let native_object = out_dir.join(format!("{library}.o"));
                let native_archive = out_dir.join(format!("lib{library}.a"));
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
                    "{relative}: native fixture library `{library}` compilation failed: {}",
                    String::from_utf8_lossy(&compile.stderr)
                );
                fs::remove_file(&native_archive).ok();
                let archive = Command::new("ar")
                    .arg("rcs")
                    .arg(&native_archive)
                    .arg(&native_object)
                    .output()
                    .expect("run native fixture archiver");
                assert!(
                    archive.status.success(),
                    "{relative}: native fixture library `{library}` archive failed: {}",
                    String::from_utf8_lossy(&archive.stderr)
                );
            }
            options.library_paths.push(out_dir.clone());
        }

        let snapshot = match scoopc::compile_file_with_options(&compile_path, &out_dir, &options) {
            Ok(success) => {
                if relative == "m15-moving/handle-pin.scoop" {
                    verify_linked_stackmap_fixups(&success.binary, &relative);
                }
                let run = Command::new(&success.binary)
                    .output()
                    .unwrap_or_else(|e| panic!("cannot run {}: {e}", success.binary.display()));
                if !expect_trap && runs_under_gc_stress(&relative) {
                    let stress_run = Command::new(&success.binary)
                        .env("SCOOP_GC_STRESS_MOVE", "1")
                        .output()
                        .unwrap_or_else(|e| {
                            panic!(
                                "cannot run {} in moving-GC stress mode: {e}",
                                success.binary.display()
                            )
                        });
                    assert!(
                        stress_run.status.success(),
                        "{relative}: moving-GC stress run exited with {}:\n{}",
                        stress_run.status,
                        String::from_utf8_lossy(&stress_run.stderr)
                    );
                    assert_eq!(
                        stress_run.stdout, run.stdout,
                        "{relative}: moving-GC stress changed observable output"
                    );
                }
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
                let mut inputs = scoopc::load_inputs(&compile_path).unwrap_or_default();
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
