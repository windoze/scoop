use std::path::{Path, PathBuf};

const COMPILER_SRC_ROOTS: &[&str] = &[
    "crates/scoop/src",
    "crates/scoop_project_model/src",
    "crates/scoopc/src",
    "crates/scoopc_ast/src",
    "crates/scoopc_codegen_llvm/src",
    "crates/scoopc_cone/src",
    "crates/scoopc_effect_facts/src",
    "crates/scoopc_effect_facts_stage/src",
    "crates/scoopc_hir/src",
    "crates/scoopc_hir_facts/src",
    "crates/scoopc_ids/src",
    "crates/scoopc_lir/src",
    "crates/scoopc_lir_facts/src",
    "crates/scoopc_mir/src",
    "crates/scoopc_mir_facts/src",
    "crates/scoopc_source/src",
    "crates/scoopc_span/src",
    "crates/scoopc_types/src",
    "crates/scoopld/src",
];

const ALLOWED_IMPL_LOWERING_FQN_LINES: &[&str] = &[
    "SYNC_MUTEX_TYPE_FQN: &'static str = \"scoop.sync.Mutex\"",
    "SYNC_MUTEX_CREATE_FQN: &'static str = \"scoop.sync.mutexCreate\"",
    "SYNC_MUTEX_LOCK_FQN: &'static str = \"scoop.sync.lock\"",
    "SYNC_MUTEX_UNLOCK_FQN: &'static str = \"scoop.sync.unlock\"",
];

const ALLOWED_SYNC_MUTEX_NAMES: &[&str] = &[
    "SYNC_MUTEX_TYPE_FQN",
    "SYNC_MUTEX_CREATE_FQN",
    "SYNC_MUTEX_LOCK_FQN",
    "SYNC_MUTEX_UNLOCK_FQN",
];

const ALLOWED_SYNC_MUTEX_CONSUMER_FILES: &[&str] = &[
    "crates/scoopc_hir/src/hir/lower/util/decls.rs",
    "crates/scoopc_hir/src/hir/lower/sugar.rs",
];

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|err| {
            panic!(
                "failed to read directory entry under {}: {err}",
                path.display()
            )
        });
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "tests") {
                continue;
            }
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

fn relative_workspace_path(path: &Path) -> String {
    let root = workspace_path("");
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn line_mentions_sync_hardcoding(line: &str) -> bool {
    line.contains("scoop.sync")
        || line.contains("scoop_sync_")
        || line.contains("SYNC_MUTEX_")
        || line.contains("SCOOP_SYNC_")
        || line.contains("__scoop_sync_once_run")
        || line.contains("codegen_sysroot_sync_once_run")
        || line.contains("declare_runtime_sync_once_run")
        || line.contains("lower_sync_intrinsic")
}

fn allowed_sync_mutex_consumer_hit(relative_path: &str, line: &str) -> bool {
    if relative_path == "crates/scoopc_hir/src/hir/lower/main/impl_lowering.rs" {
        return ALLOWED_IMPL_LOWERING_FQN_LINES
            .iter()
            .any(|allowed| line.contains(allowed));
    }

    ALLOWED_SYNC_MUTEX_CONSUMER_FILES.contains(&relative_path)
        && !line.contains("scoop.sync")
        && ALLOWED_SYNC_MUTEX_NAMES
            .iter()
            .any(|allowed| line.contains(allowed))
}

#[test]
fn p4_sync_hardcoding_guard_allows_only_delegate_mutex_consumer_boundary() {
    let mut hits = Vec::new();
    for root in COMPILER_SRC_ROOTS {
        let root_path = workspace_path(root);
        assert!(
            root_path.is_dir(),
            "compiler source root should exist: {}",
            root_path.display()
        );

        let mut files = Vec::new();
        collect_rust_files(&root_path, &mut files);
        for file in files {
            let relative_path = relative_workspace_path(&file);
            let contents = std::fs::read_to_string(&file)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", file.display()));
            for (line_number, line) in contents.lines().enumerate() {
                if line_mentions_sync_hardcoding(line)
                    && !allowed_sync_mutex_consumer_hit(&relative_path, line)
                {
                    hits.push(format!(
                        "{}:{}: {}",
                        relative_path,
                        line_number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "scoop.sync implementation hardcoding should not remain in compiler sources; \
         only the P4-T04(a) delegated-property Mutex consumer boundary is allowed:\n{}",
        hits.join("\n")
    );
}
