//! Fixture runner: compiles and runs every `tests/fixtures/**/*.scoop`
//! through the full pipeline and snapshot-tests a single merged snapshot
//! per fixture (rendered diagnostics on failure; stage dumps plus the
//! program's stdout on success).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompilationOutcome {
    Success,
    Diagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureExpectation {
    Compile,
    Diagnostics,
    Trap,
}

impl FixtureExpectation {
    fn directive(self) -> &'static str {
        match self {
            Self::Compile => "// EXPECT-COMPILE",
            Self::Diagnostics => "// EXPECT-DIAGNOSTICS",
            Self::Trap => "// EXPECT-TRAP",
        }
    }

    fn outcome(self) -> CompilationOutcome {
        match self {
            Self::Compile | Self::Trap => CompilationOutcome::Success,
            Self::Diagnostics => CompilationOutcome::Diagnostics,
        }
    }
}

impl CompilationOutcome {
    fn description(self) -> &'static str {
        match self {
            Self::Success => "successful compilation",
            Self::Diagnostics => "compile-time diagnostics",
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}

fn fixture_root() -> PathBuf {
    workspace_root().join("tests/fixtures")
}

fn declared_fixture_expectation(source: &str, relative: &str) -> Option<FixtureExpectation> {
    let mut declared = None;
    for line in source.lines().map(str::trim) {
        let expectation = match line {
            "// EXPECT-COMPILE" => Some(FixtureExpectation::Compile),
            "// EXPECT-DIAGNOSTICS" => Some(FixtureExpectation::Diagnostics),
            "// EXPECT-TRAP" => Some(FixtureExpectation::Trap),
            "" if declared.is_none() => continue,
            line if line.starts_with("// EXPECT-") => {
                panic!("{relative}: unknown fixture expectation `{line}`")
            }
            _ => None,
        };
        if let Some(expectation) = expectation {
            assert!(
                declared.is_none_or(|previous| previous == expectation),
                "{relative}: fixture declares conflicting expectations"
            );
            declared = Some(expectation);
        } else {
            break;
        }
    }
    declared
}

#[test]
fn compilation_outcome_directives_are_explicit() {
    assert_eq!(
        declared_fixture_expectation("// EXPECT-COMPILE\nfun main() {}", "fixture.scoop"),
        Some(FixtureExpectation::Compile)
    );
    assert_eq!(
        declared_fixture_expectation("// EXPECT-DIAGNOSTICS\nfun main() {}", "fixture.scoop"),
        Some(FixtureExpectation::Diagnostics)
    );
    assert_eq!(
        declared_fixture_expectation("// EXPECT-TRAP\nfun main() {}", "fixture.scoop"),
        Some(FixtureExpectation::Trap)
    );
    assert_eq!(
        declared_fixture_expectation(
            "/*\n// EXPECT-DIAGNOSTICS\n*/\nfun main() {}",
            "fixture.scoop"
        ),
        None
    );
    assert_eq!(
        declared_fixture_expectation("fun main() {}", "fixture.scoop"),
        None
    );
}

#[test]
#[should_panic(expected = "fixture.scoop: fixture declares conflicting expectations")]
fn compilation_outcome_directives_cannot_conflict() {
    declared_fixture_expectation("// EXPECT-COMPILE\n// EXPECT-DIAGNOSTICS", "fixture.scoop");
}

fn existing_snapshot_outcome(fixture: &Path) -> Option<CompilationOutcome> {
    let file_name = fixture
        .file_name()
        .expect("fixture has a file name")
        .to_string_lossy();
    let snapshot = fixture.with_file_name(format!("{file_name}.snap"));
    match snapshot.try_exists() {
        Ok(true) => {}
        Ok(false) => return None,
        Err(error) => panic!("cannot inspect {}: {error}", snapshot.display()),
    }
    let existing = insta::Snapshot::from_file(&snapshot)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", snapshot.display()));
    let body = existing
        .as_text()
        .unwrap_or_else(|| panic!("existing snapshot for {} is not text", fixture.display()))
        .to_string();
    if body.starts_with("== ast ==\n") {
        Some(CompilationOutcome::Success)
    } else if body.starts_with("== diagnostics ==\n") {
        Some(CompilationOutcome::Diagnostics)
    } else {
        panic!(
            "existing snapshot for {} does not have a compilation outcome",
            fixture.display()
        )
    }
}

fn required_compilation_outcome(
    declared: Option<FixtureExpectation>,
    snapshot: Option<CompilationOutcome>,
    relative: &str,
) -> CompilationOutcome {
    declared
        .map(FixtureExpectation::outcome)
        .or(snapshot)
        .unwrap_or_else(|| {
            panic!(
                "{relative}: new fixture has no accepted snapshot; declare one of \
                 `// EXPECT-COMPILE`, `// EXPECT-DIAGNOSTICS`, or `// EXPECT-TRAP` at the file header"
            )
        })
}

fn render_fixture_diagnostics(
    diagnostics: &[scoop_ast::Diagnostic],
    compile_path: &Path,
    relative: &str,
    source: &str,
) -> String {
    let mut inputs = scoopc::load_inputs(compile_path).unwrap_or_default();
    if let Some(user) = inputs.last_mut() {
        user.name = relative.to_string();
        user.source = source.to_string();
    }
    scoopc::render_diagnostics(diagnostics, &inputs, relative, source)
}

#[test]
#[should_panic(expected = "new.scoop: new fixture has no accepted snapshot")]
fn new_fixtures_must_declare_their_expected_outcome() {
    required_compilation_outcome(None, None, "new.scoop");
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
            | "m10-coroutines/catch-finally-suspension.scoop"
            | "m11-functions/captures.scoop"
            | "m11-functions/variance.scoop"
            | "m12-extern-scoop/extern-scoop.scoop"
            | "m13-callback/managed-callback.scoop"
            | "m13-callback/foreign-continuation.scoop"
            | "m13-callback/callback-exception.scoop"
            | "m14-generics/generic-exception.scoop"
            | "m15-moving/external-exception.scoop"
            | "m15-moving/handle-pin.scoop"
            | "m15-moving/root-shapes.scoop"
            | "m17-vararg/vararg-semantics.scoop"
            | "m18-invoke/combined.scoop"
            | "m18-operators/combined.scoop"
            | "m18-safe-call/combined.scoop"
            | "m19-initialization/moving-stress.scoop"
            | "m19-initialization/parameters-and-closure.scoop"
            | "m20-generics/combined.scoop"
            | "m21-initialization/top-level-runtime.scoop"
            | "m21-companions/companions.scoop"
            | "m21-companions/failure.scoop"
            | "m21-delegated-globals/values.scoop"
            | "m21-objects/failure.scoop"
            | "m21-objects/objects.scoop"
            | "m21-top-level-static/values.scoop"
            | "m22-copy-update/suspend-moving-gc.scoop"
            | "m25-eh/combined.scoop"
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
        let declared = declared_fixture_expectation(&source, &relative);
        let expect_trap = declared == Some(FixtureExpectation::Trap);
        let snapshot_outcome = existing_snapshot_outcome(&fixture);
        let expected_outcome = required_compilation_outcome(declared, snapshot_outcome, &relative);

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

        let compilation = scoopc::compile_file_with_options(&compile_path, &out_dir, &options);
        let outcome = if compilation.is_ok() {
            CompilationOutcome::Success
        } else {
            CompilationOutcome::Diagnostics
        };
        assert_eq!(
            outcome,
            expected_outcome,
            "{relative}: {} expected {}, but compilation produced {}",
            declared.map_or("accepted snapshot", FixtureExpectation::directive),
            expected_outcome.description(),
            outcome.description(),
        );

        let snapshot = match compilation {
            Ok(success) => {
                let warning_section = if success.warnings.is_empty() {
                    String::new()
                } else {
                    let rendered = render_fixture_diagnostics(
                        &success.warnings,
                        &compile_path,
                        &relative,
                        &source,
                    );
                    format!("== warnings ==\n{rendered}\n")
                };
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
                    "== ast ==\n{}\n== hir ==\n{}\n== mir ==\n{}\n== lir ==\n{}\n{warning_section}{run_section}\n{output}",
                    success.dumps.ast, success.dumps.hir, success.dumps.mir, success.dumps.lir,
                )
            }
            Err(diagnostics) => {
                let rendered =
                    render_fixture_diagnostics(&diagnostics, &compile_path, &relative, &source);
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
