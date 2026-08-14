//! M2 回归门：凡 typecheck 通过的 mir2 fixture，其全部函数树必须 **gap-free**
//!（C9：词汇表封闭；出现 gap 即上游决议泄漏或构造覆盖回退）。

use std::path::PathBuf;

use scoop2_archive::pipeline::{build_program, typecheck_program};

#[test]
fn gap_survey_mir2() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mir2");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "scoop"))
        .map(|e| e.path())
        .collect();
    files.sort();
    let mut failures: Vec<(String, usize, String, String)> = Vec::new();
    let mut ok_files = 0usize;
    let mut skip_files = 0usize;
    for f in &files {
        let Ok(source) = scoop2_base::SourceFile::load(f) else {
            continue;
        };
        let mut program = build_program(&source);
        let Ok(hir) = typecheck_program(&mut program, None) else {
            skip_files += 1;
            continue;
        };
        let mut file_ok = true;
        for tf in &hir.files {
            for tree in &tf.trees {
                for (span, what) in &tree.gaps {
                    file_ok = false;
                    let snippet: String = source
                        .text()
                        .bytes()
                        .skip(span.start.saturating_sub(6))
                        .take(20)
                        .map(|b| b as char)
                        .collect();
                    failures.push((
                        f.file_name().unwrap().to_string_lossy().into_owned(),
                        span.start,
                        what.clone(),
                        snippet,
                    ));
                }
            }
        }
        if file_ok {
            ok_files += 1;
        }
    }
    eprintln!(
        "=== mir2 gap survey: {} files, {} gap-free, {} skipped(typecheck失败，既有) ===",
        files.len(),
        ok_files,
        skip_files
    );
    assert!(
        failures.is_empty(),
        "gap-free 回归失败（{} 处）：{:#?}",
        failures.len(),
        failures
    );
}
