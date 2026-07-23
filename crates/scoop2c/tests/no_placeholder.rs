//! 零 placeholder 守卫：扫描所有 scoop2 新 crate 的源码，禁止出现
//! 未实现宏与生产路径占位构造。
//!
//! 规则：
//! - 禁止 `todo!(` / `unimplemented!(`；
//! - 禁止 ` unreachable!(`，除非同一行或上一行带有 `// invariant:` 注释
//!   （用于可证明的局部不变量，必须写明理由）；
//! - 本测试自身所在的文件被豁免（守卫文本本身包含被禁字符串）。

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/scoop2c must live two levels below the workspace root")
        .to_path_buf()
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            // 跳过 target 等构建产物目录。
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_unimplemented_macros_or_placeholders() {
    let root = workspace_root();
    let this_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/no_placeholder.rs");
    let crates = ["scoop2_base", "scoop2_syntax", "scoop2_hir", "scoop2c"];

    let mut violations = Vec::new();
    for krate in crates {
        let crate_dir = root.join("crates").join(krate);
        let mut files = Vec::new();
        collect_rs_files(&crate_dir, &mut files);
        for file in files {
            if file == this_file {
                continue;
            }
            let text = std::fs::read_to_string(&file)
                .unwrap_or_else(|err| panic!("cannot read {}: {err}", file.display()));
            let lines: Vec<&str> = text.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed.contains("todo!(") || trimmed.contains("unimplemented!(") {
                    violations.push(format!(
                        "{}:{}: forbidden placeholder macro: {trimmed}",
                        file.display(),
                        index + 1
                    ));
                }
                if trimmed.contains("unreachable!(") {
                    let has_invariant_note = trimmed.contains("// invariant:")
                        || index > 0 && lines[index - 1].contains("// invariant:");
                    if !has_invariant_note {
                        violations.push(format!(
                            "{}:{}: unreachable! without `// invariant:` justification: {trimmed}",
                            file.display(),
                            index + 1
                        ));
                    }
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "placeholder guard violations:\n{}",
        violations.join("\n")
    );
}
