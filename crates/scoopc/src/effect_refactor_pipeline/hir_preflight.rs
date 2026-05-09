use std::fmt;
use std::path::PathBuf;

use crate::mir::{Item as MirItem, Rvalue, StatementKind, TerminatorKind, UnwindAction};
use crate::session::{Session, SessionOptions};
use crate::source::SourceFile;

use super::{RefactorMirStageOutput, TypedHirStageOutput, load_typed_hir_stage_output_for_dump};

const HIR_COMPLETENESS_FIXTURES: &[HirCompletenessFixture] = &[
    HirCompletenessFixture {
        phase: "hir",
        name: "refactor_comptime_control_flow.scoop",
        requirements: &[],
    },
    HirCompletenessFixture {
        phase: "hir",
        name: "refactor_decl_graph.scoop",
        requirements: &[RequiredHirContract::DeclarationGraph],
    },
    HirCompletenessFixture {
        phase: "comptime",
        name: "splice_field_access_v0_basic.scoop",
        requirements: &[RequiredHirContract::DeclarationGraph],
    },
    HirCompletenessFixture {
        phase: "hir",
        name: "refactor_call_args.scoop",
        requirements: &[RequiredHirContract::CallSite],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "refactor_hir_call_contracts_surface_ok.scoop",
        requirements: &[
            RequiredHirContract::CallSite,
            RequiredHirContract::ContinuationResume,
            RequiredHirContract::Perform,
            RequiredHirContract::Handle,
        ],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "refactor_hir_class_literal_runtime_ok.scoop",
        requirements: &[RequiredHirContract::DeclarationGraph],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "reflection_runtime_fallback_v0.scoop",
        requirements: &[RequiredHirContract::CallSite],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "get_platform_runtime_ok.scoop",
        requirements: &[RequiredHirContract::CallSite],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "with_update_struct_field_ok.scoop",
        requirements: &[RequiredHirContract::WithUpdate],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "with_update_tuple_nested_path_ok.scoop",
        requirements: &[RequiredHirContract::WithUpdate],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "with_update_enum_variant_payload_ok.scoop",
        requirements: &[RequiredHirContract::WithUpdate],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "refactor_hir_assignment_places_ok.scoop",
        requirements: &[RequiredHirContract::AssignPlace],
    },
    HirCompletenessFixture {
        phase: "typecheck",
        name: "for_loop_iter_protocol_ok.scoop",
        requirements: &[RequiredHirContract::CallSite],
    },
    HirCompletenessFixture {
        phase: "hir",
        name: "refactor_top_level_init.scoop",
        requirements: &[
            RequiredHirContract::TopLevelInitRoot,
            RequiredHirContract::ExternGlobal,
        ],
    },
];

const HIR_ORIGIN_MIR_FALLBACK_REASONS: &[&str] = &[
    "ExprKind::Missing",
    "Item::Todo(comptime_if_item)",
    "StmtKind::Todo(missing_stmt)",
    "StmtKind::Todo(comptime_block)",
    "StmtKind::Todo(comptime_if)",
    "StmtKind::Todo(comptime_for)",
    "StmtKind::Todo(for_custom_iterator)",
    "comptime_if_item",
    "missing_stmt",
    "comptime_block",
    "comptime_if",
    "comptime_for",
    "for_custom_iterator",
    "array_lit",
    "spread_arg",
    "named_arg",
    "structured_concurrency_spawn_deferred",
    "structured_concurrency_join_deferred",
    "splice_field",
    "assign",
    "with_update",
    "class_lit",
    "typealias",
    "type",
    "object",
    "extension_property_no_getter",
    "missing expr",
    "val decl missing symbol id",
    "assign lhs missing local",
    "assign lhs lowering pending",
    "assign place contract missing",
    "assign place local missing",
    "assign place member receiver missing",
    "call callee lowering pending",
    "ctor call lowering pending",
    "resume lowering requires canonical callee shape",
    "dispatch callee lowering pending",
    "or_pattern_binder",
    "cross_thread_resume_outward",
    "gc_pin_intrinsic",
    "gc_handle_intrinsic",
    "unbound local ref",
];

#[derive(Clone, Copy)]
struct HirCompletenessFixture {
    phase: &'static str,
    name: &'static str,
    requirements: &'static [RequiredHirContract],
}

impl HirCompletenessFixture {
    fn label(self) -> String {
        format!("{}/{}", self.phase, self.name)
    }

    fn load(self) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(self.phase)
            .join(self.name);
        SourceFile::load(&path)
            .unwrap_or_else(|err| panic!("preflight fixture `{}` should load: {err}", self.label()))
    }
}

#[derive(Clone, Copy)]
enum RequiredHirContract {
    DeclarationGraph,
    CallSite,
    ContinuationResume,
    Perform,
    Handle,
    AssignPlace,
    WithUpdate,
    TopLevelInitRoot,
    ExternGlobal,
}

impl fmt::Display for RequiredHirContract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeclarationGraph => f.write_str("declaration graph"),
            Self::CallSite => f.write_str("call-site contract"),
            Self::ContinuationResume => f.write_str("continuation resume contract"),
            Self::Perform => f.write_str("perform contract"),
            Self::Handle => f.write_str("handle contract"),
            Self::AssignPlace => f.write_str("assignment place contract"),
            Self::WithUpdate => f.write_str("copy-update contract"),
            Self::TopLevelInitRoot => f.write_str("top-level init root"),
            Self::ExternGlobal => f.write_str("extern global contract"),
        }
    }
}

#[test]
fn refactor_hir_preflight_checks_completeness_fixtures_and_mir_smoke() {
    let session = refactor_session();
    let mut mir_smoke_count = 0usize;

    for fixture in HIR_COMPLETENESS_FIXTURES {
        let source = fixture.load();
        run_typed_hir_preflight(&session, &source, *fixture);
        mir_smoke_count += 1;
        run_direct_mir_preflight(&session, &source, *fixture);
    }

    assert_eq!(
        mir_smoke_count,
        HIR_COMPLETENESS_FIXTURES.len(),
        "all legal typed-HIR completeness fixtures must run strict MIR smoke"
    );
}

#[test]
fn refactor_mir_comptime_splice_class_literal_and_with_update_preclosure() {
    let session = refactor_session();
    let fixture = HirCompletenessFixture {
        phase: "mir_refactor",
        name: "comptime_splice_class_with_update.scoop",
        requirements: &[
            RequiredHirContract::DeclarationGraph,
            RequiredHirContract::WithUpdate,
        ],
    };
    let source = fixture.load();

    let typed_hir = run_typed_hir_preflight(&session, &source, fixture);
    let hir_dump = typed_hir.stable_dump();
    for forbidden in [
        "ComptimeBlock",
        "ComptimeIf",
        "ComptimeFor",
        "Todo(\"comptime_block\"",
        "Todo(\"comptime_if\"",
        "Todo(\"comptime_for\"",
    ] {
        assert!(
            !hir_dump.contains(forbidden),
            "comptime surface `{forbidden}` must be expanded before MIR for `{}`",
            fixture.label()
        );
    }

    let output = super::load_direct_style_mir_stage_output_for_dump(&session, &source)
        .unwrap_or_else(|err| panic!("refactor MIR preclosure fixture should pass: {err:?}"));
    assert_no_hir_origin_mir_fallbacks(&output, fixture);

    let dump = format!("{:#?}", output.file());
    assert!(
        !dump.contains("Todo"),
        "MIR-T04 fixture must not contain placeholder Todo: {dump}"
    );
    assert!(
        dump.contains("TypeMetadataLiteral"),
        "class literal policy should lower to a MIR type metadata literal: {dump}"
    );
    assert!(
        dump.contains("MemberAccess"),
        "splice field access should become concrete member access: {dump}"
    );
    assert!(
        dump.contains("StructLit") && dump.contains("MakeTuple") && dump.contains("EnumVariant"),
        "with-update should lower struct, tuple, and enum transports into concrete MIR: {dump}"
    );
}

fn refactor_session() -> Session {
    Session::with_options(SessionOptions::new()).unwrap()
}

fn run_typed_hir_preflight(
    session: &Session,
    source: &SourceFile,
    fixture: HirCompletenessFixture,
) -> TypedHirStageOutput {
    let output = load_typed_hir_stage_output_for_dump(session, source).unwrap_or_else(|err| {
        panic!(
            "typed HIR preflight should pass for `{}`: {err:?}",
            fixture.label()
        )
    });
    for requirement in fixture.requirements {
        assert_required_contract(*requirement, &output, fixture);
    }
    output
}

fn assert_required_contract(
    requirement: RequiredHirContract,
    output: &TypedHirStageOutput,
    fixture: HirCompletenessFixture,
) {
    let present = match requirement {
        RequiredHirContract::DeclarationGraph => !output.hir_file().decls.is_empty(),
        RequiredHirContract::CallSite => {
            !output.effect_contracts().call_site_contracts().is_empty()
        }
        RequiredHirContract::ContinuationResume => !output
            .effect_contracts()
            .continuation_resume_sites()
            .is_empty(),
        RequiredHirContract::Perform => !output.effect_contracts().perform_sites().is_empty(),
        RequiredHirContract::Handle => !output.effect_contracts().handle_sites().is_empty(),
        RequiredHirContract::AssignPlace => !output
            .effect_contracts()
            .assign_place_contracts()
            .is_empty(),
        RequiredHirContract::WithUpdate => {
            !output.effect_contracts().with_update_contracts().is_empty()
        }
        RequiredHirContract::TopLevelInitRoot => {
            !output.effect_contracts().top_level_init_roots().is_empty()
        }
        RequiredHirContract::ExternGlobal => !output
            .effect_contracts()
            .extern_global_contracts()
            .is_empty(),
    };

    assert!(
        present,
        "typed HIR preflight fixture `{}` did not publish required {}",
        fixture.label(),
        requirement
    );
}

fn run_direct_mir_preflight(
    session: &Session,
    source: &SourceFile,
    fixture: HirCompletenessFixture,
) {
    let output = super::load_direct_style_mir_stage_output_for_dump(session, source)
        .unwrap_or_else(|err| {
            panic!(
                "strict direct-style MIR preflight should pass for `{}`: {err:?}",
                fixture.label()
            )
        });
    assert_no_hir_origin_mir_fallbacks(&output, fixture);
}

fn assert_no_hir_origin_mir_fallbacks(
    output: &RefactorMirStageOutput,
    fixture: HirCompletenessFixture,
) {
    for item in &output.file().items {
        let fun = match item {
            MirItem::Fun(fun) => fun,
            MirItem::Todo { kind, .. } => {
                assert_not_hir_origin_mir_fallback(kind, fixture, "top-level MIR item");
                continue;
            }
            MirItem::InitializerRoot(_) | MirItem::ExternGlobal(_) | MirItem::Metadata(_) => {
                continue;
            }
        };
        let Some(body) = &fun.body else {
            continue;
        };

        for block in &body.blocks {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StatementKind::Todo(reason) => {
                        assert_not_hir_origin_mir_fallback(reason, fixture, &fun.fqn)
                    }
                    StatementKind::Assign {
                        value: Rvalue::Todo(reason),
                        ..
                    } => assert_not_hir_origin_mir_fallback(reason, fixture, &fun.fqn),
                    StatementKind::Nop
                    | StatementKind::Assign { .. }
                    | StatementKind::StoreMember { .. }
                    | StatementKind::StoreTopLevelVar { .. } => {}
                }
            }

            if let UnwindAction::Todo(reason) = &block.terminator.unwind {
                assert_not_hir_origin_mir_fallback(reason, fixture, &fun.fqn);
            }
            if let TerminatorKind::Todo(reason) = &block.terminator.kind {
                assert_not_hir_origin_mir_fallback(reason, fixture, &fun.fqn);
            }
        }
    }
}

fn assert_not_hir_origin_mir_fallback(reason: &str, fixture: HirCompletenessFixture, fqn: &str) {
    assert!(
        !HIR_ORIGIN_MIR_FALLBACK_REASONS.contains(&reason),
        "direct-style MIR preflight fixture `{}` leaked HIR-origin fallback `{}` in `{}`",
        fixture.label(),
        reason,
        fqn
    );
}
