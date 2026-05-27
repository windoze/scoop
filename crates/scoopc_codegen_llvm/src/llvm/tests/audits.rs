//! Audit-style tests: stable-id audits, function-declaration helpers, materialized closures, external-symbol audits.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn stable_id_object_symbol_normalization_only_strips_macho_abi_prefix() {
    assert_eq!(
        stable_id_normalize_object_symbol_name(
            "___scoop_abi0_fun__sample_helper__hdeadbeef",
            BinaryFormat::MachO,
        ),
        "__scoop_abi0_fun__sample_helper__hdeadbeef"
    );
    assert_eq!(
        stable_id_normalize_object_symbol_name(
            "__scoop_abi0_fun__sample_helper__hdeadbeef",
            BinaryFormat::Elf,
        ),
        "__scoop_abi0_fun__sample_helper__hdeadbeef"
    );
}

pub(super) fn stable_id_classify_external_symbol(
    name: &str,
    is_undefined: bool,
) -> StableIdExternalSymbolRole {
    if is_undefined || stable_id_symbol_looks_like_runtime_or_native_import(name) {
        StableIdExternalSymbolRole::RuntimeOrNativeImport
    } else if name == "main" {
        StableIdExternalSymbolRole::FixedExternalException
    } else if name.starts_with("__scoop_abi0_") {
        StableIdExternalSymbolRole::UserAbi
    } else if stable_id_symbol_looks_like_compiler_private_helper(name) {
        StableIdExternalSymbolRole::CompilerPrivateHelper
    } else {
        StableIdExternalSymbolRole::UserAbi
    }
}

pub(super) fn stable_id_symbol_looks_like_runtime_or_native_import(name: &str) -> bool {
    name.starts_with("scoop_")
        || name.starts_with("llvm.")
        || matches!(name, "malloc" | "free" | "exit")
}

pub(super) fn stable_id_symbol_looks_like_compiler_private_helper(name: &str) -> bool {
    (name.starts_with("__scoop_") && !name.starts_with("__scoop_abi0_"))
        || name.starts_with("scoop.lambda$")
        || name.starts_with("scoop.lambda_resume$")
        || name.starts_with("scoop.lambda_env$")
        || name.contains(".$lambda")
}

pub(super) fn stable_id_symbol_looks_like_closure_family(name: &str) -> bool {
    name.contains("lambda") || name.contains("closure")
}

pub(super) fn stable_id_symbol_looks_like_plain_adapter_family(name: &str) -> bool {
    name.contains("plain_adapter")
}

pub(super) fn stable_id_symbol_looks_like_closure_step_adapter_family(name: &str) -> bool {
    name.contains("closure_step_adapter")
}

pub(super) fn stable_id_symbol_looks_like_closure_dynamic_entry_family(name: &str) -> bool {
    name.contains("closure_dynamic_entry")
}

pub(super) fn stable_id_symbol_looks_like_effect_helper_family(name: &str) -> bool {
    name.contains("lowered_") || name.contains("__outcome") || name.contains("__k")
}

pub(super) fn stable_id_symbol_looks_like_hidden_init_family(name: &str) -> bool {
    name.contains("object_init")
        || name.contains("top_level_init")
        || name.contains("top_level_val_init")
}

pub(super) fn stable_id_symbol_mentions_fqn(name: &str, fqn: &str) -> bool {
    name.contains(fqn) || name.contains(&fqn.replace('.', "_"))
}

pub(super) fn stable_id_symbol_is_exported_abi_fun(name: &str) -> bool {
    name.starts_with("__scoop_abi0_fun__") && stable_id_symbol_has_hash128_suffix(name)
}

pub(super) fn stable_id_symbol_has_hash128_suffix(name: &str) -> bool {
    let Some((_, hash)) = name.rsplit_once("__h") else {
        return false;
    };
    hash.len() == 32 && hash.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(super) fn stable_id_symbol_has_private_role(name: &str, role: &str) -> bool {
    name.starts_with(&format!("__scoop_priv0__{role}__h"))
        && stable_id_symbol_has_hash128_suffix(name)
}

pub(super) fn stable_id_type_name_has_hashed_family(name: &str, family: &str) -> bool {
    name.starts_with(&format!("scoop.lowered.{family}__h"))
        && stable_id_symbol_has_hash128_suffix(name)
}

pub(super) fn stable_id_symbol_looks_like_private_closure_body(name: &str) -> bool {
    stable_id_symbol_has_private_role(name, "closure_body")
}

pub(super) fn stable_id_ir_contains_hidden_init_call(ir: &str) -> bool {
    ir.lines().any(|line| {
        line.contains(" call ")
            && (line.contains("__scoop_priv0__object_init__h")
                || line.contains("__scoop_priv0__hidden_object_init_bridge__h")
                || line.contains("__scoop_priv0__top_level_val_init__h")
                || line.contains("__scoop_priv0__hidden_top_level_init_bridge__h"))
    })
}

pub(super) fn stable_id_emit_object_audit(
    prefix: &str,
    source: &SourceFile,
) -> StableIdObjectAudit {
    let dir = make_temp_dir(prefix);
    let output = dir.join("main.o");
    let session = Session::new().unwrap();
    emit_minimal_main_obj_to_file(&session, source, &output).unwrap();

    let bytes = std::fs::read(&output).unwrap();
    let obj = object::File::parse(&*bytes).expect("failed to parse object file");
    let audit = StableIdObjectAudit::from_object(&obj);

    std::fs::remove_dir_all(&dir).unwrap();
    audit
}

pub(super) fn maybe_function_ir_matching<F>(ir: &str, predicate: F) -> Option<&str>
where
    F: Fn(&str, &str) -> bool,
{
    for chunk in ir.split("\ndefine ").skip(1) {
        let end = chunk.find("\n}").expect("expected end of function body") + 2;
        let function = &chunk[..end];
        let header = function.lines().next().expect("expected function header");
        if predicate(header, function) {
            return Some(function);
        }
    }
    None
}

pub(super) fn function_ir_matching<'ir, F>(
    ir: &'ir str,
    description: &str,
    predicate: F,
) -> &'ir str
where
    F: Fn(&str, &str) -> bool,
{
    maybe_function_ir_matching(ir, predicate).unwrap_or_else(|| {
        let headers = ir
            .split("\ndefine ")
            .skip(1)
            .filter_map(|chunk| chunk.lines().next())
            .collect::<Vec<_>>()
            .join("\n");
        panic!("expected function matching {description}; available function headers:\n{headers}")
    })
}

pub(super) fn maybe_function_ir_for_symbol<'ir>(
    ir: &'ir str,
    symbol_name: &str,
) -> Option<&'ir str> {
    maybe_function_ir_matching(ir, |_, function| {
        llvm_function_symbol_name(function) == symbol_name
    })
}

pub(super) fn llvm_function_header_uses_internal_or_private_linkage(header: &str) -> bool {
    header.starts_with("internal ") || header.starts_with("private ")
}

pub(super) fn llvm_declaration_line_matching<'ir, F>(
    ir: &'ir str,
    description: &str,
    predicate: F,
) -> &'ir str
where
    F: Fn(&str) -> bool,
{
    ir.lines()
        .find(|line| line.starts_with("declare ") && predicate(line))
        .unwrap_or_else(|| panic!("expected declaration matching {description}\n{ir}"))
}

pub(super) fn llvm_attribute_group_for_declaration<'ir>(
    ir: &'ir str,
    decl_line: &str,
) -> Option<&'ir str> {
    let (_, group) = decl_line.rsplit_once(" #")?;
    let group = group.trim();
    ir.lines()
        .find(|line| line.starts_with(&format!("attributes #{group} =")))
}

pub(super) fn assert_ir_contains_internal_or_private_helper_family(
    ir: &str,
    description: &str,
    family_matches: fn(&str) -> bool,
) {
    let function_ir = function_ir_matching(ir, description, |header, function| {
        llvm_function_header_uses_internal_or_private_linkage(header)
            && family_matches(llvm_function_symbol_name(function))
    });
    let header = function_ir
        .lines()
        .next()
        .expect("expected helper function header");
    assert!(
        llvm_function_header_uses_internal_or_private_linkage(header),
        "{description} 应使用 internal/private linkage，实际 header:\n{header}"
    );
}

pub(super) fn stable_id_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(super) fn stable_id_collect_audit_files(
    root_name: &'static str,
    path: &Path,
    files: &mut Vec<(&'static str, PathBuf)>,
) {
    let entries = std::fs::read_dir(path).unwrap_or_else(|err| {
        panic!("failed to read stable-id audit root {root_name} at {path:?}: {err}")
    });
    for entry in entries {
        let entry = entry
            .unwrap_or_else(|err| panic!("failed to read directory entry under {path:?}: {err}"));
        let entry_path = entry.path();
        if entry_path.is_dir() {
            stable_id_collect_audit_files(root_name, &entry_path, files);
            continue;
        }
        if entry_path.is_file() {
            files.push((root_name, entry_path));
        }
    }
}

pub(super) fn stable_id_run_grep_audit() -> Vec<StableIdGrepAuditEntry> {
    let repo_root = stable_id_repo_root();
    let mut files = Vec::new();
    for root in STABLE_ID_AUDIT_SEARCH_ROOTS {
        let path = repo_root.join(root);
        assert!(path.is_dir(), "stable-id audit root should exist: {path:?}");
        stable_id_collect_audit_files(root, &path, &mut files);
    }

    let mut entries = Vec::with_capacity(STABLE_ID_AUDIT_PATTERNS.len());
    for pattern in STABLE_ID_AUDIT_PATTERNS {
        let mut hits = Vec::new();
        for (root_name, path) in &files {
            let Ok(contents) = std::fs::read_to_string(path) else {
                continue;
            };
            for (line_number, line) in contents.lines().enumerate() {
                if pattern.matches(line) {
                    hits.push(StableIdGrepHit {
                        root: root_name,
                        path: stable_id_relative_repo_path(path),
                        line_number: line_number + 1,
                    });
                }
            }
        }
        entries.push(StableIdGrepAuditEntry {
            pattern: pattern.regex,
            hits,
        });
    }
    entries
}

pub(super) fn stable_id_relative_repo_path(path: &Path) -> String {
    let repo_root = stable_id_repo_root();
    path.strip_prefix(&repo_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub(super) fn llvm_raw_add_function_none_callsites() -> Vec<String> {
    let llvm_root = stable_id_repo_root().join("crates/scoopc_codegen_llvm/src/llvm");
    let mut files = Vec::new();
    stable_id_collect_audit_files(
        "crates/scoopc_codegen_llvm/src/llvm",
        &llvm_root,
        &mut files,
    );

    let mut hits = Vec::new();
    for (_, path) in files {
        if path.components().any(|c| c.as_os_str() == "tests") {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        let lines = contents.lines().collect::<Vec<_>>();
        for start in 0..lines.len() {
            if !lines[start].contains("module.add_function(") {
                continue;
            }
            let mut snippet = lines[start].trim().to_string();
            let mut cursor = start + 1;
            while !snippet.contains(");") && cursor < lines.len() && cursor <= start + 8 {
                snippet.push(' ');
                snippet.push_str(lines[cursor].trim());
                cursor += 1;
            }
            if snippet.contains("None") {
                hits.push(format!(
                    "{}:{}: {}",
                    stable_id_relative_repo_path(&path),
                    start + 1,
                    snippet
                ));
            }
        }
    }
    hits
}

pub(super) fn llvm_legacy_closure_private_naming_hits() -> Vec<String> {
    let llvm_codegen_root =
        stable_id_repo_root().join("crates/scoopc_codegen_llvm/src/llvm/codegen");
    let mut files = Vec::new();
    stable_id_collect_audit_files(
        "crates/scoopc_codegen_llvm/src/llvm/codegen",
        &llvm_codegen_root,
        &mut files,
    );
    let legacy_needles = [
        "scoop.lambda$",
        "scoop.lambda_resume$",
        "scoop.lambda_env$",
        "scoop.mir.lambda_env$",
        "__scoop_type_desc_mir_closure_env__",
        "direct_hir_closure_carrier_alias",
        "is_direct_hir_closure_carrier_alias",
    ];

    let mut hits = Vec::new();
    for (_, path) in files {
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (line_number, line) in contents.lines().enumerate() {
            if legacy_needles.iter().any(|needle| line.contains(needle)) {
                hits.push(format!(
                    "{}:{}: {}",
                    stable_id_relative_repo_path(&path),
                    line_number + 1,
                    line.trim()
                ));
            }
        }
    }
    hits
}

pub(super) fn stable_id_line_contains_prefix_digits_suffix(
    line: &str,
    prefix: &str,
    suffix: &str,
) -> bool {
    let bytes = line.as_bytes();
    let prefix_bytes = prefix.as_bytes();
    let suffix_bytes = suffix.as_bytes();
    if prefix_bytes.is_empty() || bytes.len() < prefix_bytes.len() {
        return false;
    }

    for start in 0..=bytes.len() - prefix_bytes.len() {
        if !bytes[start..].starts_with(prefix_bytes) {
            continue;
        }
        let mut cursor = start + prefix_bytes.len();
        let digit_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() {
            cursor += 1;
        }
        if cursor == digit_start {
            continue;
        }
        if suffix_bytes.is_empty() || bytes[cursor..].starts_with(suffix_bytes) {
            return true;
        }
    }
    false
}

#[test]
pub(super) fn stable_id_audit_contract_keeps_surface_boundary_explicit() {
    for allowed in [
        "symbol 文本",
        "linkage",
        "dump 文本",
        "fixture expect",
        "RTTI id",
        "JSON identity 字段",
    ] {
        assert!(
            STABLE_ID_ALLOWED_SURFACE_CHANGES.contains(&allowed),
            "stable-id 审计必须显式记住允许变化的 surface: {allowed}"
        );
    }
    for forbidden in [
        "语义",
        "运行结果",
        "typecheck",
        "effect / continuation / GC 行为",
    ] {
        assert!(
            STABLE_ID_FORBIDDEN_BEHAVIOR_DRIFT.contains(&forbidden),
            "stable-id 审计必须显式记住禁止漂移的行为: {forbidden}"
        );
    }
}

#[test]
pub(super) fn stable_id_audit_external_symbol_classifier_distinguishes_roles() {
    assert_eq!(
        stable_id_classify_external_symbol("scoop_runtime_init", true),
        StableIdExternalSymbolRole::RuntimeOrNativeImport
    );
    assert_eq!(
        stable_id_classify_external_symbol("main", false),
        StableIdExternalSymbolRole::FixedExternalException
    );
    assert_eq!(
        stable_id_classify_external_symbol("a.helper", false),
        StableIdExternalSymbolRole::UserAbi
    );
    let stable_user_abi = "__scoop_abi0_fun__a_helper__h0123456789abcdef0123456789abcdef";
    assert!(stable_id_symbol_has_hash128_suffix(stable_user_abi));
    assert_eq!(
        stable_id_classify_external_symbol(stable_user_abi, false),
        StableIdExternalSymbolRole::UserAbi
    );
    for helper in [
        "__scoop_priv0__hidden_object_init_bridge__h0123456789abcdef0123456789abcdef",
        "__scoop_priv0__top_level_val_init__h0123456789abcdef0123456789abcdef",
        "__scoop_priv0__object_instance__h0123456789abcdef0123456789abcdef",
        "scoop.lambda$0",
        "scoop.lambda_resume$0",
        "scoop.lambda_env$0",
        "__scoop_priv0__closure_body__h0123456789abcdef0123456789abcdef",
    ] {
        if helper.starts_with("__scoop_priv0__") {
            assert!(stable_id_symbol_has_hash128_suffix(helper));
        }
        assert_eq!(
            stable_id_classify_external_symbol(helper, false),
            StableIdExternalSymbolRole::CompilerPrivateHelper,
            "expected compiler-private helper classification for {helper}"
        );
    }
}

#[test]
pub(super) fn stable_id_audit_grep_inventory_scans_repo_roots() {
    let report = stable_id_run_grep_audit();
    assert_eq!(
        report.len(),
        STABLE_ID_AUDIT_PATTERNS.len(),
        "stable-id grep 审计应覆盖 STABLE_ID.md §11 的全部固定 pattern"
    );
    println!("stable-id grep audit summary:");
    for entry in &report {
        let sample_hits = entry
            .hits
            .iter()
            .take(3)
            .map(|hit| format!("{}:{} ({})", hit.path, hit.line_number, hit.root))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "  {} -> {} hit(s){}",
            entry.pattern,
            entry.hits.len(),
            if sample_hits.is_empty() {
                String::new()
            } else {
                format!(": {sample_hits}")
            }
        );
    }
}

#[test]
pub(super) fn function_declaration_helpers_emit_explicit_linkage() {
    let context = Context::create();
    let module = context.create_module("function_declaration_helpers");
    let fn_ty = context.void_type().fn_type(&[], false);

    let _ = crate::llvm::codegen::declare_exported_abi_function(&module, "user_export", fn_ty);
    let _ = crate::llvm::codegen::declare_runtime_or_native_import_function(
        &module,
        "runtime_import",
        fn_ty,
    );
    let _ = crate::llvm::codegen::declare_compiler_private_helper_function(
        &module,
        "__helper_internal",
        fn_ty,
        Linkage::Internal,
    );
    let _ = crate::llvm::codegen::declare_compiler_private_helper_function(
        &module,
        "__helper_private",
        fn_ty,
        Linkage::Private,
    );

    let ir = module.print_to_string().to_string();
    assert!(
        ir.contains("declare void @user_export()"),
        "exported ABI declaration 应显式保留 external surface\n{ir}"
    );
    assert!(
        ir.contains("declare void @runtime_import()"),
        "runtime/native import declaration 应显式保留 external surface\n{ir}"
    );
    assert!(
        ir.contains("declare internal void @__helper_internal()"),
        "compiler-private helper 应能显式切到 internal linkage\n{ir}"
    );
    assert!(
        ir.contains("declare private void @__helper_private()"),
        "compiler-private helper 应能显式切到 private linkage\n{ir}"
    );
}

#[test]
pub(super) fn stable_id_private_closure_symbol_helpers_use_hash128_namespaces() {
    let owner = crate::stable_id::StableDefKey::new(
        crate::stable_id::StableConeKey::new("demo", "0.1.0"),
        crate::stable_id::StableDefNamespace::Fun,
        "demo.main",
        "top_level_fun",
        None,
    );
    let closure = crate::stable_id::StableClosureKey::new(&owner, "$lambda0.$lambda1");

    let body = crate::llvm::codegen::private_closure_body_fn_name(&closure);
    let resume = crate::llvm::codegen::private_closure_resume_fn_name(&closure);
    let env = crate::llvm::codegen::private_closure_env_type_name(&closure);
    let env_desc = crate::llvm::codegen::private_closure_env_type_desc_name(&closure);

    assert!(stable_id_symbol_has_private_role(&body, "closure_body"));
    assert!(stable_id_symbol_has_private_role(&resume, "closure_resume"));
    assert!(stable_id_symbol_has_private_role(&env, "closure_env"));
    assert!(stable_id_symbol_has_private_role(
        &env_desc,
        "closure_env_type_desc"
    ));
}

#[test]
pub(super) fn function_declaration_inventory_eliminates_raw_add_function_none_callsites() {
    let hits = llvm_raw_add_function_none_callsites();
    assert!(
        hits.is_empty(),
        "LLVM declaration path 不应再直接留下 raw `module.add_function(..., None)`：\n{}",
        hits.join("\n")
    );
}

#[test]
pub(super) fn stable_id_source_inventory_removes_legacy_closure_private_naming_from_codegen() {
    let hits = llvm_legacy_closure_private_naming_hits();
    assert!(
        hits.is_empty(),
        "closure private naming 生产代码不应再残留 legacy ClosureId/alias 路径：\n{}",
        hits.join("\n")
    );
}

#[test]
pub(super) fn materialized_mir_closure_private_symbols_use_stable_hash_namespaces() {
    let source = SourceFile::new_virtual(
        "<mem>/stable_id_materialized_closure.scoop",
        r#"
package a

fun make(x: Int): () -> Int {
    return { x }
}

fun main(): Int {
    return make(1)()
}
"#,
    );
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    for legacy in [
        "scoop.mir.lambda_env$",
        "__scoop_type_desc_mir_closure_env__",
    ] {
        assert!(
            !ir.contains(legacy),
            "materialized MIR closure env naming 不应再回到 legacy spelling `{legacy}`\n{ir}"
        );
    }

    assert_ir_contains_internal_or_private_helper_family(
        &ir,
        "materialized MIR closure body",
        stable_id_symbol_looks_like_private_closure_body,
    );

    assert!(
        ir.contains("__scoop_priv0__closure_env__h")
            && ir.contains("__scoop_priv0__closure_env_type_desc__h"),
        "materialized MIR closure env/type-desc 应使用 stable private symbol 家族\n{ir}"
    );
}

#[test]
pub(super) fn external_symbol_audit_top_level_and_materialized_generic_smoke() {
    let source = SourceFile::new_virtual(
        "<mem>/stable_id_audit_generic.scoop",
        r#"
package a

fun <T> id(x: T): T {
    return x
}

fun helper(): Int {
    return id<Int>(1)
}

fun main(): Int {
    return helper()
}
"#,
    );
    let audit = stable_id_emit_object_audit("stable_id_audit_generic", &source);
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        !audit.all_external_symbols.is_empty(),
        "stable-id object audit 应能看见 external symbol 集\n{}",
        audit.summary()
    );
    assert!(
        audit
            .user_abi_symbols
            .iter()
            .any(|name| stable_id_symbol_mentions_fqn(name, "a.helper")),
        "source-level top-level function 现在应以 user ABI symbol 进入 external symbol 集\n{}",
        audit.summary()
    );
    assert!(
        audit
            .user_abi_symbols
            .iter()
            .any(|name| stable_id_symbol_mentions_fqn(name, "a.id")),
        "materialized generic callable 现在应以 user ABI symbol 进入 external symbol 集\n{}",
        audit.summary()
    );
    assert!(
        audit
            .fixed_external_exceptions
            .iter()
            .any(|name| name == "main"),
        "宿主固定入口 main 仍应保留 external surface\n{}",
        audit.summary()
    );

    let helper_ir = function_ir_matching(&ir, "source-level helper impl", |header, function| {
        !llvm_function_header_uses_internal_or_private_linkage(header)
            && stable_id_symbol_is_exported_abi_fun(llvm_function_symbol_name(function))
            && stable_id_symbol_mentions_fqn(llvm_function_symbol_name(function), "a.helper")
    });
    assert!(
        stable_id_symbol_is_exported_abi_fun(llvm_function_symbol_name(helper_ir)),
        "source-level helper implementation 应使用 AbiMangler fun namespace"
    );

    let generic_ir = function_ir_matching(&ir, "materialized generic impl", |header, function| {
        !llvm_function_header_uses_internal_or_private_linkage(header)
            && stable_id_symbol_is_exported_abi_fun(llvm_function_symbol_name(function))
            && stable_id_symbol_mentions_fqn(llvm_function_symbol_name(function), "a.id")
    });
    assert!(
        stable_id_symbol_is_exported_abi_fun(llvm_function_symbol_name(generic_ir)),
        "materialized generic implementation 应使用 AbiMangler fun namespace"
    );
}

#[test]
pub(super) fn external_symbol_audit_closure_effect_and_hidden_init_helpers_smoke() {
    let source = SourceFile::new_virtual(
        "<mem>/stable_id_audit_helpers.scoop",
        r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

object Boot {
    val ready: Int = 7
}

val Seed: Int = 9

fun entry(): Int / (Ask) {
    val captured = Boot.ready + Seed
    val thunk: () -> Int / (Ask) = {
        Ask.ask(captured)
    }
    return thunk()
}

fun main(): Int {
    return handle {
        entry()
    } on {
        Ask.ask(seed) -> seed
    }
}
"#,
    );
    let audit = stable_id_emit_object_audit("stable_id_audit_helpers", &source);
    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        !audit
            .all_external_symbols
            .iter()
            .any(|name| stable_id_symbol_looks_like_closure_family(name)),
        "closure body/resume/env 不应再进入 external symbol 集\n{}",
        audit.summary()
    );
    assert!(
        !audit
            .all_external_symbols
            .iter()
            .any(|name| stable_id_symbol_looks_like_effect_helper_family(name)),
        "effect helper shell / continuation outcome helper 不应再进入 external symbol 集\n{}",
        audit.summary()
    );
    assert!(
        !audit
            .all_external_symbols
            .iter()
            .any(|name| stable_id_symbol_looks_like_hidden_init_family(name)),
        "object init bridge/object init function/top-level init bridge 不应再进入 external symbol 集\n{}",
        audit.summary()
    );
    assert!(
        !audit
            .all_external_symbols
            .iter()
            .any(|name| stable_id_symbol_mentions_fqn(name, "a.entry")),
        "仅模块内使用的 source-level entry 不应泄漏到 external symbol 集\n{}",
        audit.summary()
    );
    assert!(
        audit
            .fixed_external_exceptions
            .iter()
            .any(|name| name == "main"),
        "宿主固定入口 main 仍应保留 external surface\n{}",
        audit.summary()
    );

    let entry_ir = function_ir_matching(&ir, "source-level entry impl", |header, function| {
        llvm_function_header_uses_internal_or_private_linkage(header)
            && stable_id_symbol_has_private_role(
                llvm_function_symbol_name(function),
                "lowered_direct_invoke",
            )
    });
    let entry_step_ty = llvm_function_return_named_struct_type(entry_ir)
        .expect("expected hashed Step return type for source-level entry impl");
    assert!(
        llvm_function_header_uses_internal_or_private_linkage(
            entry_ir.lines().next().expect("expected entry header"),
        ) && stable_id_type_name_has_hashed_family(entry_step_ty, "Step"),
        "source-level entry implementation 应使用 internal/private linkage + hashed Step family:\n{entry_ir}"
    );

    for legacy in [
        "scoop.lambda$",
        "scoop.lambda_resume$",
        "scoop.lambda_env$",
        "scoop.mir.lambda_env$",
        "__scoop_type_desc_mir_closure_env__",
    ] {
        assert!(
            !ir.contains(legacy),
            "closure private naming 不应再回到 legacy ClosureId spelling `{legacy}`\n{ir}"
        );
    }

    assert!(
        ir.contains("__scoop_priv0__closure_env__h")
            && ir.contains("__scoop_priv0__closure_env_type_desc__h"),
        "closure env type/type-desc 应使用 stable private symbol 家族\n{ir}"
    );

    assert_ir_contains_internal_or_private_helper_family(
        &ir,
        "closure helper",
        stable_id_symbol_looks_like_closure_family,
    );
    assert_ir_contains_internal_or_private_helper_family(
        &ir,
        "effect helper",
        stable_id_symbol_looks_like_effect_helper_family,
    );
    assert_ir_contains_internal_or_private_helper_family(
        &ir,
        "hidden init helper",
        stable_id_symbol_looks_like_hidden_init_family,
    );
}
