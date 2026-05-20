use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const ALLOWLIST_HEADER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../runtime/c/scoop_runtime_api.h"
));

fn parse_allowlist_symbols() -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();

    for (line_no, raw_line) in ALLOWLIST_HEADER.lines().enumerate() {
        let line = raw_line.trim();
        if !line.starts_with("X(") {
            continue;
        }

        let rest = line.strip_prefix("X(").unwrap_or("");
        let close_paren_at = rest.find(')');
        let Some(close_paren_at) = close_paren_at else {
            panic!(
                "scoop_runtime_api.h: 无法解析 allowlist 符号（缺少 ')'）：line={}",
                line_no + 1
            );
        };

        let sym = &rest[..close_paren_at];
        if sym.is_empty() {
            continue;
        }

        symbols.insert(sym.to_string());
    }

    symbols
}

fn scooprt_static_lib_path() -> PathBuf {
    // build.rs 使用 cc crate 产出静态库到 OUT_DIR；该路径在“单元测试编译”阶段就固定了。
    //
    // 备注：
    // - Unix-like：通常是 `libscooprt.a`
    // - MSVC：通常是 `scooprt.lib`
    let out_dir = PathBuf::from(env!("OUT_DIR"));

    let candidates: &[&str] = if cfg!(target_env = "msvc") {
        &["scooprt.lib", "libscooprt.a"]
    } else {
        &["libscooprt.a", "scooprt.lib"]
    };

    for name in candidates {
        let path = out_dir.join(name);
        if path.exists() {
            return path;
        }
    }

    panic!(
        "未找到 scooprt 静态库：OUT_DIR={}, candidates={:?}",
        out_dir.display(),
        candidates
    );
}

fn run_nm_exports(lib_path: &Path) -> String {
    // 优先用 llvm-nm（若存在）；否则回退到系统 nm。
    // 说明：不强依赖 `--defined-only` 等 flag，以便在不同平台保持兼容；由我们自己过滤 U 符号。
    let tools = ["llvm-nm", "nm"];

    let mut last_err: Option<String> = None;
    for tool in tools {
        match Command::new(tool).arg("-g").arg(lib_path).output() {
            Ok(out) if out.status.success() => return String::from_utf8_lossy(&out.stdout).into(),
            Ok(out) => {
                last_err = Some(format!(
                    "{tool} 运行失败：status={}, stderr={}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                ));
            }
            Err(err) => {
                last_err = Some(format!("{tool} 启动失败：{err}"));
            }
        }
    }

    panic!(
        "无法运行 nm/llvm-nm 获取导出符号：lib={}, last_err={:?}",
        lib_path.display(),
        last_err
    );
}

fn parse_defined_export_symbols(nm_stdout: &str) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();

    for raw_line in nm_stdout.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        // static archive 的 object 分隔行，例如：`foo.o:`
        if line.ends_with(':') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        // 兼容两种常见格式：
        // - `00000000 T _symbol`（addr type name）
        // - `U _symbol`（type name）
        let sym_type = parts[parts.len() - 2];
        let mut name = parts[parts.len() - 1];

        if sym_type == "U" || sym_type == "u" {
            continue;
        }

        // Mach-O（macOS）下的 external symbol 会带一个额外的 '_' 前缀；
        // 这里仅剥离“平台前缀”的 1 个 '_'，保留 ABI 自身的 `__foo` 双下划线约定。
        if cfg!(target_vendor = "apple") {
            name = name.strip_prefix('_').unwrap_or(name);
        }

        if name.is_empty() {
            continue;
        }

        symbols.insert(name.to_string());
    }

    symbols
}

#[test]
fn migrated_string_cone_helpers_are_not_runtime_core_exports() {
    let allowlist = parse_allowlist_symbols();
    for symbol in [
        "scoop_string_from_byte_array",
        "scoop_string_from_char_array",
        "scoop_string_from_string_array",
        "scoop_string_to_float64",
    ] {
        assert!(
            !allowlist.contains(symbol),
            "{symbol} should belong to scoop.lang.string native code, not runtime core"
        );
    }
}

#[test]
fn runtime_exports_must_be_allowlisted() {
    let allowlist = parse_allowlist_symbols();
    assert!(
        !allowlist.is_empty(),
        "allowlist 为空：请检查 runtime/c/scoop_runtime_api.h"
    );

    let lib_path = scooprt_static_lib_path();
    let nm_stdout = run_nm_exports(&lib_path);
    let exports = parse_defined_export_symbols(&nm_stdout);

    let unknown: Vec<String> = exports.difference(&allowlist).cloned().collect();

    assert!(
        unknown.is_empty(),
        "发现未登记的 runtime 导出符号（请更新 runtime/c/scoop_runtime_api.h）：\n{}\n\nlib={}",
        unknown.join("\n"),
        lib_path.display()
    );
}
