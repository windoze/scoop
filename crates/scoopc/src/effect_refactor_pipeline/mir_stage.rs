use std::collections::BTreeMap;

use crate::mir::{
    File as MirFile, FunDecl as MirFunDecl, Item as MirItem, LoweredMir, MaterializedMir,
    MirLowerError, MirLoweringFacts, lower_hir_file_for_dump_with_facts,
};
use crate::ty::TypeStore;

use super::{TypedHirEffectContracts, TypedHirStageOutput};

/// refactor direct-style MIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P3/P4 及后续阶段直接消费：
/// - `lowered_mir` 仍是 direct-style MIR，而不是 late-lowered `Step` IR；
/// - 当前所有 effect-sensitive site 继续通过 MIR 节点上的 `SiteId` 锚定；
/// - `effect_contracts` 保留这次 lowering 消费过的 P2 typed HIR handoff，便于测试/审计；
///   canonical 的 site-level contract 现已下沉到 MIR 节点 metadata；P4 可以把它用于审计，
///   但不得把它当成重新解释 `Call / Perform / Resume / Handle` 语义的 source of truth；
/// - `callable_body_indices` 与可选的 `materialized_mir` 把 P4 会消费的 canonical MIR
///   handoff 显式挂在 stage 输出上，而不是继续藏在 `LoweredHir` 私有字段或 dump helper 里。
/// - P4 的 authoritative 输入是这份 stage 输出上的 callable body 身份、可选
///   `materialized_mir` 快照，以及 MIR 节点上的 `SiteId` / metadata；P4 不得回看 P2 原始
///   HIR side tables 重新猜测 site contract。
/// - 本 stage 仍未提供 `StepSchema`、`ContinuationSchema` 或 `MaterializedEffectFacts`；这些属于
///   P4/P5 的职责，而不是 P3 dump / stage 输出应提前伪造的内容。
#[derive(Debug)]
pub struct RefactorMirStageOutput {
    lowered_mir: LoweredMir,
    effect_contracts: TypedHirEffectContracts,
    callable_body_indices: BTreeMap<String, usize>,
    materialized_mir: Option<MaterializedMir>,
}

impl RefactorMirStageOutput {
    pub(crate) fn new(
        lowered_mir: LoweredMir,
        effect_contracts: TypedHirEffectContracts,
        materialized_mir: Option<MaterializedMir>,
    ) -> Self {
        Self {
            callable_body_indices: collect_callable_body_indices(&lowered_mir.file),
            lowered_mir,
            effect_contracts,
            materialized_mir,
        }
    }

    pub fn file(&self) -> &MirFile {
        &self.lowered_mir.file
    }

    pub fn types(&self) -> &TypeStore {
        &self.lowered_mir.types
    }

    pub fn effect_contracts(&self) -> &TypedHirEffectContracts {
        &self.effect_contracts
    }

    /// 返回当前 stage 输出上显式挂住的 canonical materialized MIR 快照（若存在）。
    pub fn materialized_mir(&self) -> Option<&MaterializedMir> {
        self.materialized_mir.as_ref()
    }

    pub(crate) fn materialized_mir_mut(&mut self) -> Option<&mut MaterializedMir> {
        self.materialized_mir.as_mut()
    }

    pub(crate) fn with_materialized_mir(mut self, materialized_mir: MaterializedMir) -> Self {
        self.materialized_mir = Some(materialized_mir);
        self
    }

    /// 以稳定顺序枚举当前 direct-style MIR 中可查询的 callable body 身份。
    pub fn callable_body_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.callable_body_indices.keys().map(String::as_str)
    }

    /// 按 callable body 身份查询 canonical direct-style MIR body。
    pub fn callable_body(&self, fqn: &str) -> Option<&MirFunDecl> {
        let item_index = *self.callable_body_indices.get(fqn)?;
        match self.file().items.get(item_index)? {
            MirItem::Fun(fun) if fun.body.is_some() => Some(fun),
            _ => None,
        }
    }

    /// refactor `dump-mir` / `mir_refactor` fixtures / 定向单测共用的稳定文本 surface。
    ///
    /// P3-T04 起，这个 formatter 就是 refactor direct-style MIR 的 snapshot/golden 基线：
    /// - 必须稳定暴露 direct-style MIR body / CFG；
    /// - 必须保留 `SiteId`、cleanup/finally target，以及 `Call / Perform / Resume / Handle`
    ///   的关键 metadata；
    /// - 不能在 CLI、fixture runner、或单测之间各自拼接不同文本。
    pub fn stable_dump(&self) -> String {
        format!("{:#?}\n", self.file())
    }

    pub fn into_lowered_mir(self) -> LoweredMir {
        self.lowered_mir
    }
}

fn collect_callable_body_indices(file: &MirFile) -> BTreeMap<String, usize> {
    let mut indices = BTreeMap::new();
    for (item_index, item) in file.items.iter().enumerate() {
        let MirItem::Fun(fun) = item else {
            continue;
        };
        if fun.body.is_none() {
            continue;
        }
        indices.entry(fun.fqn.clone()).or_insert(item_index);
    }
    indices
}

fn validate_refactor_bodies(file: &MirFile) -> Result<(), MirLowerError> {
    for item in &file.items {
        let MirItem::Fun(fun) = item else {
            continue;
        };
        let Some(body) = &fun.body else {
            continue;
        };
        body.validate_refactor_direct_style().map_err(|error| {
            MirLowerError::InvalidRefactorMir {
                fqn: fun.fqn.clone(),
                error,
            }
        })?;
    }
    Ok(())
}

pub(crate) fn run(
    typed_hir_output: TypedHirStageOutput,
) -> Result<RefactorMirStageOutput, MirLowerError> {
    let facts = MirLoweringFacts::from_refactor_typed_handoff(
        typed_hir_output.lowered_hir(),
        typed_hir_output.effect_contracts(),
    );
    let effect_contracts = typed_hir_output.effect_contracts().clone();
    let mut lowered_hir = typed_hir_output.into_lowered_hir();
    let builtins = lowered_hir.types.intern_builtins();
    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    validate_refactor_bodies(&file)?;
    let types = std::mem::replace(&mut lowered_hir.types, TypeStore::new());
    let materialized_mir = lowered_hir.into_materialized_mir();

    Ok(RefactorMirStageOutput::new(
        LoweredMir { file, types },
        effect_contracts,
        materialized_mir,
    ))
}

#[cfg(test)]
mod tests {
    use super::RefactorMirStageOutput;
    use crate::mir::{
        CallKind, HandlerArmKind, Operand, Rvalue, StatementKind, TerminatorKind, UnwindAction,
    };
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::ty::TypeStore;
    use std::path::PathBuf;

    use super::super::TypedHirEffectContracts;

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    fn run_fixture(phase: &str, name: &str) -> RefactorMirStageOutput {
        let session = refactor_session();
        let source = load_fixture(phase, name);
        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap();
        super::run(typed_hir_output).expect("fixture 应可通过 refactor MIR stage")
    }

    fn callable_body<'a>(output: &'a RefactorMirStageOutput, fqn: &str) -> &'a crate::mir::Body {
        output
            .callable_body(fqn)
            .and_then(|fun| fun.body.as_ref())
            .unwrap_or_else(|| panic!("应找到 callable body: {fqn}"))
    }

    fn validated_callable_body<'a>(
        output: &'a RefactorMirStageOutput,
        fqn: &str,
    ) -> &'a crate::mir::Body {
        let body = callable_body(output, fqn);
        body.validate_refactor_direct_style()
            .unwrap_or_else(|err| panic!("refactor MIR body `{fqn}` 应通过验证器: {err}"));
        body
    }

    fn unit_operand_is_visible_in_body(
        output: &RefactorMirStageOutput,
        body: &crate::mir::Body,
        operand: &Operand,
    ) -> bool {
        match operand {
            Operand::Const(crate::mir::ConstValue::Unit) => true,
            Operand::Local(local) => {
                output
                    .types()
                    .display(body.locals[local.as_u32() as usize].ty)
                    .to_string()
                    == "Unit"
            }
            Operand::Const(_) => false,
        }
    }

    #[test]
    fn refactor_direct_mir_stage_output_is_constructible() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        );

        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap();
        let output = super::run(typed_hir_output).unwrap();

        assert_eq!(output.file().items.len(), 2);
        assert!(output.callable_body("sample.helper").is_some());
        assert!(output.callable_body("sample.main").is_some());
        assert_eq!(output.effect_contracts().function_effects().len(), 2);
        assert!(output.stable_dump().contains("FunDecl"));
    }

    #[test]
    fn refactor_direct_mir_stage_keeps_callable_body_query_surface_stable() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let output = RefactorMirStageOutput::new(
            crate::mir::LoweredMir {
                file: crate::mir::File {
                    items: vec![crate::mir::Item::Fun(crate::mir::FunDecl {
                        span: crate::span::Span::new(0, 1),
                        fqn: "sample.main".to_string(),
                        name: "main".to_string(),
                        ty: builtins.unit,
                        params: Vec::new(),
                        return_ty: builtins.unit,
                        body: Some(crate::mir::Body::new_empty()),
                    })],
                },
                types,
            },
            TypedHirEffectContracts::default(),
            None,
        );

        assert_eq!(
            output.callable_body_fqns().collect::<Vec<_>>(),
            vec!["sample.main"]
        );
        assert!(output.callable_body("sample.main").is_some());
    }

    #[test]
    fn refactor_mir_lowering_contract_keeps_direct_dispatch_and_resume_sites_explicit() {
        let direct_output = run_fixture("mir", "direct_and_fun_value_call.scoop");
        let main_body = callable_body(&direct_output, "a.main");
        let main_calls = main_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value: Rvalue::Call { kind, .. },
                    ..
                } => Some(kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            main_calls.as_slice(),
            [CallKind::Direct { callee_fqn }, CallKind::Direct { callee_fqn: callee_fqn_2 }]
                if callee_fqn == "a.id" && callee_fqn_2 == "a.apply"
        ));

        let apply_body = callable_body(&direct_output, "a.apply");
        assert!(
            apply_body
                .blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| {
                    matches!(
                        &stmt.kind,
                        StatementKind::Assign {
                            value: Rvalue::Call {
                                kind: CallKind::FunValue { .. },
                                ..
                            },
                            ..
                        }
                    )
                })
        );

        let dispatch_output = run_fixture("mir", "dispatch_and_resume_call.scoop");
        let virtual_body = callable_body(&dispatch_output, "fixtures.mir.callVirtual");
        assert!(
            virtual_body
                .blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| {
                    matches!(
                        &stmt.kind,
                        StatementKind::Assign {
                            value: Rvalue::Call {
                                kind: CallKind::Virtual { .. },
                                ..
                            },
                            ..
                        }
                    )
                })
        );
        let interface_body = callable_body(&dispatch_output, "fixtures.mir.callInterface");
        assert!(
            interface_body
                .blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| {
                    matches!(
                        &stmt.kind,
                        StatementKind::Assign {
                            value: Rvalue::Call {
                                kind: CallKind::Interface { .. },
                                ..
                            },
                            ..
                        }
                    )
                })
        );

        let resume_once_body = callable_body(&dispatch_output, "fixtures.mir.resumeOnce");
        let resume_once = resume_once_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Resume { resume, .. },
                            args,
                            ..
                        },
                    ..
                } => Some((resume, args)),
                _ => None,
            })
            .expect("resumeOnce 应 lower 成显式 Resume call");
        assert_eq!(resume_once.1.len(), 1);
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_once.0.resume_ty)
                .to_string(),
            "Int"
        );
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_once.0.answer_ty)
                .to_string(),
            "Unit"
        );
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_once.0.return_ty)
                .to_string(),
            "Unit"
        );
        assert!(resume_once.0.out_effects.is_pure());
        assert!(!resume_once.0.suspends_outward);
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_once.0.runtime_error_effect_ty.unwrap())
                .to_string(),
            "scoop.core.Raise<scoop.core.RuntimeError>"
        );

        let resume_boom_body = callable_body(&dispatch_output, "fixtures.mir.resumeBoom");
        let resume_boom = resume_boom_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Resume { resume, .. },
                            ..
                        },
                    ..
                } => Some(resume),
                _ => None,
            })
            .expect("resumeBoom 应 lower 成显式 Resume call");
        assert!(resume_boom.suspends_outward);
        assert_eq!(resume_boom.out_effects.terms.len(), 1);
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_boom.out_effects.terms[0])
                .to_string(),
            "fixtures.mir.Boom"
        );
        assert!(
            !dispatch_output
                .stable_dump()
                .contains("resume callee lowering pending")
        );
    }

    #[test]
    fn refactor_mir_lowering_contract_records_perform_and_handle_metadata() {
        let output = run_fixture("mir", "handle_perform.scoop");
        let body = callable_body(&output, "a.main");
        let entry = &body.blocks[body.start.as_u32() as usize].terminator.kind;
        let (handle_metadata, arms) = match entry {
            TerminatorKind::Handle { metadata, arms, .. } => (metadata, arms),
            other => panic!("expected handle terminator, got {other:?}"),
        };
        assert_eq!(
            output
                .types()
                .display(handle_metadata.result_ty)
                .to_string(),
            "Int"
        );
        assert_eq!(
            output
                .types()
                .display(handle_metadata.body_result_ty)
                .to_string(),
            "Int"
        );
        assert!(handle_metadata.finally_result_ty.is_none());
        assert_eq!(arms.len(), 1);
        let arm = &arms[0];
        assert_eq!(arm.op_fqn, "scoop.core.Raise.raise");
        assert_eq!(arm.kind, HandlerArmKind::NonResuming);
        assert_eq!(
            output.types().display(arm.handled_effect_ty).to_string(),
            "scoop.core.Raise<Int>"
        );
        assert_eq!(arm.payload_component_tys.len(), 1);
        assert_eq!(
            output
                .types()
                .display(arm.payload_component_tys[0])
                .to_string(),
            "Int"
        );
        assert_eq!(output.types().display(arm.body_ty).to_string(), "Int");

        let (perform_metadata, perform_args) = body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator.kind {
                TerminatorKind::Perform { metadata, args, .. } => Some((metadata, args)),
                _ => None,
            })
            .expect("handle_perform 应包含显式 Perform terminator");
        assert_eq!(
            output
                .types()
                .display(perform_metadata.effect_ty)
                .to_string(),
            "scoop.core.Raise<Int>"
        );
        assert_eq!(perform_metadata.arg_mapping, vec![0]);
        assert_eq!(perform_metadata.payload_component_tys.len(), 1);
        assert_eq!(
            output
                .types()
                .display(perform_metadata.payload_component_tys[0])
                .to_string(),
            "Int"
        );
        assert_eq!(perform_args.len(), 1);
        assert_eq!(perform_args[0].source_arg_index, 0);
    }

    #[test]
    fn refactor_mir_lowering_contract_canonicalizes_resume_unit_sugar() {
        let output = run_fixture("mir_refactor", "continuation_resume_unit_sugar.scoop");
        let body = callable_body(&output, "fixtures.mir_refactor.resumeUnit");

        let resume_calls = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Resume { resume, .. },
                            args,
                            ..
                        },
                    ..
                } => Some((resume, args)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(resume_calls.len(), 2);
        for (resume, args) in resume_calls {
            assert_eq!(args.len(), 1);
            assert_eq!(output.types().display(resume.resume_ty).to_string(), "Unit");
            assert_eq!(output.types().display(resume.answer_ty).to_string(), "Unit");
            assert_eq!(output.types().display(resume.return_ty).to_string(), "Unit");
            assert!(unit_operand_is_visible_in_body(
                &output,
                body,
                &args[0].value
            ));
        }

        let direct_unit_calls = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                    ..
                } if callee_fqn == "fixtures.mir_refactor.takesUnit" => Some(args),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(direct_unit_calls.len(), 2);
        for args in direct_unit_calls {
            assert_eq!(args.len(), 1);
            assert!(unit_operand_is_visible_in_body(
                &output,
                body,
                &args[0].value
            ));
        }
        assert!(
            !output
                .stable_dump()
                .contains("resume callee lowering pending")
        );
    }

    #[test]
    fn refactor_mir_cfg_existing_control_flow_samples_validate() {
        let while_output = run_fixture("mir", "while_break_continue.scoop");
        validated_callable_body(&while_output, "a.main");

        let if_when_output = run_fixture("mir", "if_when.scoop");
        validated_callable_body(&if_when_output, "a.main");
    }

    #[test]
    fn refactor_mir_cfg_handle_finally_boundary_is_explicit() {
        let output = run_fixture("mir_refactor", "handle_finally_boundary.scoop");

        let completes = validated_callable_body(&output, "fixtures.mir_refactor.body_completes");
        let completes_entry = &completes.blocks[completes.start.as_u32() as usize]
            .terminator
            .kind;
        let (body_target, arm_targets, finally_target, exit_target) = match completes_entry {
            TerminatorKind::Handle {
                has_finally,
                body_target,
                arm_targets,
                finally_target,
                exit_target,
                ..
            } => {
                assert!(*has_finally, "body_completes 应保留 finally boundary");
                (
                    *body_target,
                    arm_targets.clone(),
                    finally_target.expect("body_completes 应显式指向 finally cleanup block"),
                    *exit_target,
                )
            }
            other => panic!("body_completes 入口应为 Handle terminator，而不是 {other:?}"),
        };
        assert!(completes.blocks[finally_target.as_u32() as usize].is_cleanup);
        assert_eq!(arm_targets.len(), 1);
        assert!(matches!(
            completes.blocks[body_target.as_u32() as usize].terminator.kind,
            TerminatorKind::Goto { target } if target == finally_target
        ));
        assert!(matches!(
            completes.blocks[arm_targets[0].as_u32() as usize].terminator.kind,
            TerminatorKind::Goto { target } if target == finally_target
        ));
        assert!(matches!(
            completes.blocks[finally_target.as_u32() as usize].terminator.kind,
            TerminatorKind::Goto { target } if target == exit_target
        ));

        let raised = validated_callable_body(&output, "fixtures.mir_refactor.handled_raise");
        let raised_entry = &raised.blocks[raised.start.as_u32() as usize]
            .terminator
            .kind;
        let raised_finally = match raised_entry {
            TerminatorKind::Handle {
                has_finally,
                finally_target,
                ..
            } => {
                assert!(*has_finally, "handled_raise 应保留 finally boundary");
                finally_target.expect("handled_raise 应显式指向 finally cleanup block")
            }
            other => panic!("handled_raise 入口应为 Handle terminator，而不是 {other:?}"),
        };
        assert!(raised.blocks[raised_finally.as_u32() as usize].is_cleanup);
        let perform = raised
            .blocks
            .iter()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Perform { .. }))
            .expect("handled_raise 应包含显式 Perform terminator");
        assert!(matches!(
            perform.terminator.unwind,
            UnwindAction::Cleanup { target } if raised.blocks[target.as_u32() as usize].is_cleanup
        ));
    }

    #[test]
    fn refactor_mir_cfg_effect_boundary_inside_expr_context_uses_explicit_blocks() {
        let output = run_fixture("mir_refactor", "effect_boundary_inside_expr_context.scoop");
        let body = validated_callable_body(&output, "fixtures.mir_refactor.main");

        let handle_count = body
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.terminator.kind,
                    TerminatorKind::Handle {
                        has_finally: true,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(
            handle_count, 4,
            "local init / call arg / if 条件 / return expr 中的 boundary 都应显式落成独立 Handle block"
        );
        assert!(
            body.blocks.iter().filter(|block| block.is_cleanup).count() >= 4,
            "每个带 finally 的 boundary 都应生成 cleanup block"
        );
        assert!(body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value:
                            Rvalue::Call {
                                kind: CallKind::Direct { callee_fqn },
                                ..
                            },
                        ..
                    } if callee_fqn == "fixtures.mir_refactor.box_int"
                )
            })
        }));
        assert!(
            body.blocks
                .iter()
                .any(|block| { matches!(block.terminator.kind, TerminatorKind::CondBr { .. }) })
        );
    }

    #[test]
    fn refactor_mir_cfg_escape_continuation_finally_materializes_continuation_local() {
        let output = run_fixture(
            "run-pass",
            "effect_handle_return_from_function_finally.scoop",
        );
        let body = validated_callable_body(&output, "returnThroughFinally");
        let entry = &body.blocks[body.start.as_u32() as usize].terminator.kind;
        let arm = match entry {
            TerminatorKind::Handle { arms, .. } => arms
                .first()
                .expect("escape continuation fixture 应包含唯一的 handler arm"),
            other => panic!("returnThroughFinally 入口应为 Handle terminator，而不是 {other:?}"),
        };
        assert_eq!(arm.kind, HandlerArmKind::EscapeContinuation);
        assert!(
            arm.continuation_local.is_some(),
            "escape continuation arm 应显式 materialize continuation binder local"
        );
        assert!(
            !output.stable_dump().contains("unbound local ref"),
            "escape continuation arm 不应再回退成未绑定局部占位"
        );
    }
}
