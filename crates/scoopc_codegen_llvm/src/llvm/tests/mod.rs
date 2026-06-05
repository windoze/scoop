#[cfg(test)]
mod clayout_tests {
    use super::*;

    #[test]
    fn clayout_packed_struct_has_expected_field_offsets() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_packed.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(packed: 1)
struct Packed(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s = Packed { a: a0, b: b0 }
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let data_layout = module.get_data_layout();
        let target_data = TargetData::create(data_layout.as_str().to_str().unwrap());

        let packed = context
            .get_struct_type("fixtures.clayout.Packed")
            .expect("missing llvm struct type for fixtures.clayout.Packed");
        assert!(
            packed.is_packed(),
            "expected @CLayout(packed=1) struct to be packed in LLVM"
        );
        assert_eq!(
            target_data.offset_of_element(&packed, 1).unwrap(),
            1,
            "expected second field offset to be 1 for packed struct"
        );
    }

    #[test]
    fn clayout_aligned_struct_sets_alloca_alignment() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_aligned.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(aligned: 16, packed: 1)
struct AlignedPacked(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s = AlignedPacked { a: a0, b: b0 }
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let ir = module.print_to_string().to_string();

        assert!(
            ir.lines().any(|line| {
                line.contains("@__scoop_priv0__composite_transport_desc__h")
                    && line.contains("%scoop.runtime.ScoopCompositeTransportDescriptor")
                    && line.contains("i64 16, i64 16")
            }),
            "@CLayout(aligned=16, packed=1) 应把 composite transport 物理布局发布为 size=16 / align=16\n{ir}"
        );
    }

    #[test]
    fn clayout_packed_field_load_uses_align_1() {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/clayout_packed_field_load.scoop",
            r#"
package fixtures.clayout

import scoop.core.*

@CLayout(packed: 1)
struct Packed(val a: UInt8, val b: Int64)

fun main() {
    val a0: UInt8 = 1
    val b0: Int64 = 2
    val s: Packed = Packed { a: a0, b: b0 }
    val x: Int64 = s.b
    println(0)
}
"#,
        );

        let context = Context::create();
        let module = build_minimal_main_module(&session, &source, &context).unwrap();
        let ir = module.print_to_string().to_string();

        assert!(
            ir.lines().any(|line| {
                line.contains("@__scoop_priv0__composite_transport_desc__h")
                    && line.contains("%scoop.runtime.ScoopCompositeTransportDescriptor")
                    && line.contains("i64 9, i64 1")
            }),
            "@CLayout(packed=1) 应继续把 composite transport 物理布局发布为 size=9 / align=1\n{ir}"
        );
    }
}

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::ast;
use crate::hir;
use crate::opt::OptLevel;
use crate::session::{Session, SessionOptions};
use crate::source::SourceFile;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::targets::TargetData;
use object::BinaryFormat;
use object::Object;
use object::ObjectSection;
use object::ObjectSymbol;
use object::SymbolKind;
use object::SymbolScope;

fn session_for_source(source: &SourceFile) -> Session {
    let mut deps = Vec::new();
    if source.text().contains("import scoop.runtime.test.*") {
        deps.push("scoop.runtime.test");
    }
    if source.text().contains("import scoop.thread.*") {
        deps.push("scoop.thread");
    }

    Session::with_options(SessionOptions::new().with_extra_sysroot_dependencies(deps)).unwrap()
}

// stable-id 审计只允许 identity surface 变化；行为语义必须保持等价。
const STABLE_ID_ALLOWED_SURFACE_CHANGES: &[&str] = &[
    "symbol 文本",
    "linkage",
    "dump 文本",
    "fixture expect",
    "RTTI id",
    "JSON identity 字段",
];

const STABLE_ID_FORBIDDEN_BEHAVIOR_DRIFT: &[&str] = &[
    "语义",
    "运行结果",
    "typecheck",
    "effect / continuation / GC 行为",
];

const STABLE_ID_AUDIT_SEARCH_ROOTS: &[&str] =
    &["crates/scoop/src", "crates/scoopc/src", "tests/fixtures"];

#[derive(Clone, Copy)]
enum StableIdAuditMatcher {
    Contains(&'static str),
    ContainsAll(&'static [&'static str]),
    PrefixDigitsSuffix {
        prefix: &'static str,
        suffix: &'static str,
    },
}

#[derive(Clone, Copy)]
struct StableIdAuditPattern {
    regex: &'static str,
    matcher: StableIdAuditMatcher,
}

const STABLE_ID_AUDIT_PATTERNS: &[StableIdAuditPattern] = &[
    StableIdAuditPattern {
        regex: r"TypeId\(",
        matcher: StableIdAuditMatcher::Contains("TypeId("),
    },
    StableIdAuditPattern {
        regex: r"SymbolId\(",
        matcher: StableIdAuditMatcher::Contains("SymbolId("),
    },
    StableIdAuditPattern {
        regex: r"ClosureId\(",
        matcher: StableIdAuditMatcher::Contains("ClosureId("),
    },
    StableIdAuditPattern {
        regex: r"SourceId\(",
        matcher: StableIdAuditMatcher::Contains("SourceId("),
    },
    StableIdAuditPattern {
        regex: r"ConeId\(",
        matcher: StableIdAuditMatcher::Contains("ConeId("),
    },
    StableIdAuditPattern {
        regex: r"BasicBlockId\(",
        matcher: StableIdAuditMatcher::Contains("BasicBlockId("),
    },
    StableIdAuditPattern {
        regex: r"LocalId\(",
        matcher: StableIdAuditMatcher::Contains("LocalId("),
    },
    StableIdAuditPattern {
        regex: r"SiteId\(",
        matcher: StableIdAuditMatcher::Contains("SiteId("),
    },
    StableIdAuditPattern {
        regex: r"StepSchemaId\(",
        matcher: StableIdAuditMatcher::Contains("StepSchemaId("),
    },
    StableIdAuditPattern {
        regex: r"ContinuationSchemaId\(",
        matcher: StableIdAuditMatcher::Contains("ContinuationSchemaId("),
    },
    StableIdAuditPattern {
        regex: r"CaseTag\(",
        matcher: StableIdAuditMatcher::Contains("CaseTag("),
    },
    StableIdAuditPattern {
        regex: r"ResumeInterfaceId\(",
        matcher: StableIdAuditMatcher::Contains("ResumeInterfaceId("),
    },
    StableIdAuditPattern {
        regex: r"ContinuationObjectId\(",
        matcher: StableIdAuditMatcher::Contains("ContinuationObjectId("),
    },
    StableIdAuditPattern {
        regex: r"StateId\(",
        matcher: StableIdAuditMatcher::Contains("StateId("),
    },
    StableIdAuditPattern {
        regex: r"BoundaryId\(",
        matcher: StableIdAuditMatcher::Contains("BoundaryId("),
    },
    StableIdAuditPattern {
        regex: r"FrameSlotId\(",
        matcher: StableIdAuditMatcher::Contains("FrameSlotId("),
    },
    StableIdAuditPattern {
        regex: r"module\.add_function\(.*None\)",
        matcher: StableIdAuditMatcher::ContainsAll(&["module.add_function(", "None"]),
    },
    StableIdAuditPattern {
        regex: "stable_template_symbol_suffix",
        matcher: StableIdAuditMatcher::Contains("stable_template_symbol_suffix"),
    },
    StableIdAuditPattern {
        regex: "source_path.*decl_span",
        matcher: StableIdAuditMatcher::ContainsAll(&["source_path", "decl_span"]),
    },
    StableIdAuditPattern {
        regex: r"scoop\.lambda\$[0-9]+",
        matcher: StableIdAuditMatcher::PrefixDigitsSuffix {
            prefix: "scoop.lambda$",
            suffix: "",
        },
    },
    StableIdAuditPattern {
        regex: r"scoop\.lambda_resume\$[0-9]+",
        matcher: StableIdAuditMatcher::PrefixDigitsSuffix {
            prefix: "scoop.lambda_resume$",
            suffix: "",
        },
    },
    StableIdAuditPattern {
        regex: r"scoop\.lambda_env\$[0-9]+",
        matcher: StableIdAuditMatcher::PrefixDigitsSuffix {
            prefix: "scoop.lambda_env$",
            suffix: "",
        },
    },
    StableIdAuditPattern {
        regex: r"__schema[0-9]+",
        matcher: StableIdAuditMatcher::PrefixDigitsSuffix {
            prefix: "__schema",
            suffix: "",
        },
    },
    StableIdAuditPattern {
        regex: r"__k[0-9]+",
        matcher: StableIdAuditMatcher::PrefixDigitsSuffix {
            prefix: "__k",
            suffix: "",
        },
    },
    StableIdAuditPattern {
        regex: r"t[0-9]+__",
        matcher: StableIdAuditMatcher::PrefixDigitsSuffix {
            prefix: "t",
            suffix: "__",
        },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StableIdExternalSymbolRole {
    RuntimeOrNativeImport,
    FixedExternalException,
    UserAbi,
    CompilerPrivateHelper,
}

#[derive(Debug, Default)]
struct StableIdObjectAudit {
    all_external_symbols: Vec<String>,
    runtime_or_native_imports: Vec<String>,
    fixed_external_exceptions: Vec<String>,
    user_abi_symbols: Vec<String>,
    compiler_private_helpers: Vec<String>,
}

#[derive(Debug)]
struct StableIdGrepHit {
    root: &'static str,
    path: String,
    line_number: usize,
}

#[derive(Debug)]
struct StableIdGrepAuditEntry {
    pattern: &'static str,
    hits: Vec<StableIdGrepHit>,
}

fn make_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("scoopc_{prefix}_{}_{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

impl StableIdAuditPattern {
    fn matches(self, line: &str) -> bool {
        match self.matcher {
            StableIdAuditMatcher::Contains(needle) => line.contains(needle),
            StableIdAuditMatcher::ContainsAll(needles) => {
                needles.iter().all(|needle| line.contains(needle))
            }
            StableIdAuditMatcher::PrefixDigitsSuffix { prefix, suffix } => {
                stable_id_line_contains_prefix_digits_suffix(line, prefix, suffix)
            }
        }
    }
}

impl StableIdObjectAudit {
    fn from_object(obj: &object::File<'_>) -> Self {
        let mut audit = Self::default();
        for symbol in obj.symbols() {
            let kind = symbol.kind();
            if matches!(
                kind,
                SymbolKind::Section | SymbolKind::File | SymbolKind::Label
            ) {
                continue;
            }
            let scope = symbol.scope();
            let is_external = symbol.is_undefined()
                || matches!(scope, SymbolScope::Linkage | SymbolScope::Dynamic);
            if !is_external {
                continue;
            }
            let Ok(raw_name) = symbol.name() else {
                continue;
            };
            if raw_name.is_empty() {
                continue;
            }

            let name = stable_id_normalize_object_symbol_name(raw_name, obj.format()).to_string();
            audit.all_external_symbols.push(name.clone());
            match stable_id_classify_external_symbol(&name, symbol.is_undefined()) {
                StableIdExternalSymbolRole::RuntimeOrNativeImport => {
                    audit.runtime_or_native_imports.push(name)
                }
                StableIdExternalSymbolRole::FixedExternalException => {
                    audit.fixed_external_exceptions.push(name)
                }
                StableIdExternalSymbolRole::UserAbi => audit.user_abi_symbols.push(name),
                StableIdExternalSymbolRole::CompilerPrivateHelper => {
                    audit.compiler_private_helpers.push(name)
                }
            }
        }
        stable_id_sort_and_dedup(&mut audit.all_external_symbols);
        stable_id_sort_and_dedup(&mut audit.runtime_or_native_imports);
        stable_id_sort_and_dedup(&mut audit.fixed_external_exceptions);
        stable_id_sort_and_dedup(&mut audit.user_abi_symbols);
        stable_id_sort_and_dedup(&mut audit.compiler_private_helpers);
        audit
    }

    fn summary(&self) -> String {
        format!(
            "external={:?}\nruntime/native={:?}\nfixed-external={:?}\nuser-abi={:?}\ncompiler-private={:?}",
            self.all_external_symbols,
            self.runtime_or_native_imports,
            self.fixed_external_exceptions,
            self.user_abi_symbols,
            self.compiler_private_helpers,
        )
    }
}

fn stable_id_sort_and_dedup(symbols: &mut Vec<String>) {
    symbols.sort();
    symbols.dedup();
}

fn stable_id_normalize_object_symbol_name(name: &str, format: BinaryFormat) -> &str {
    // 只有 Mach-O 会在 object 外部符号上额外加一层 `_` ABI 装饰。
    if matches!(format, BinaryFormat::MachO) {
        name.strip_prefix('_').unwrap_or(name)
    } else {
        name
    }
}

mod abi;
mod audits;
mod baseline;
mod effects;
mod frontend_rewrite;
mod late_lower;
mod named_intrinsic;

// Cross-submodule re-exports (private to `tests/`): each sub-file can pick
// up helpers defined in any other sub-file via `use super::*;`. Some of
// these globs are only consumed by sibling submodules, not by `mod.rs`
// itself, hence the `#[allow(unused_imports)]`.
#[allow(unused_imports)]
use {
    abi::*, audits::*, baseline::*, effects::*, frontend_rewrite::*, late_lower::*,
    named_intrinsic::*,
};
