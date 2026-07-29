//! no_placeholder 守卫：禁止 `todo!` / `unimplemented!` / `Todo` 变体 / `panic!`
//! 出现在 scoop2_mir 源码中。
//!
//! 这是 scoop2 体系的统一约束：任何 placeholder 都意味着 lowering 未覆盖某个构造，
//! 必须显式报 `scoop::mir::*` 诊断而非留 Todo。

use std::path::Path;

/// 收集 crate 源码（src/）中出现的禁用 token，返回 (file, line, token) 列表。
fn collect_placeholders() -> Vec<(String, usize, String)> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let src = Path::new(manifest_dir).join("src");
    let mut out = Vec::new();
    walk(&src, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<(String, usize, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
            continue;
        }
        if path.extension().is_some_and(|e| e == "rs") {
            scan_file(&path, out);
        }
    }
}

fn scan_file(path: &Path, out: &mut Vec<(String, usize, String)>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        // 跳过注释行（文档注释中可能出现这些词）。
        if trimmed.starts_with("//") {
            continue;
        }
        for forbidden in [
            "todo!()",
            "unimplemented!()",
            "unreachable!()",
            "Todo(",
            "panic!(",
        ] {
            if line.contains(forbidden) {
                out.push((path.display().to_string(), i + 1, forbidden.to_string()));
            }
        }
    }
}

#[test]
fn no_placeholder_in_source() {
    let hits = collect_placeholders();
    assert!(
        hits.is_empty(),
        "scoop2_mir 源码中检测到禁用的 placeholder（todo!/unimplemented!/Todo/panic!）：\n{}",
        hits.iter()
            .map(|(f, l, t)| format!("  {f}:{l}: {t}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
