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
///   canonical 的 site-level contract 现已下沉到 MIR 节点 metadata，本 stage 不应再要求下游回到
///   P2 内部缓存重新拼装语义；
/// - `callable_body_indices` 与可选的 `materialized_mir` 把 P4 会消费的 canonical MIR
///   handoff 显式挂在 stage 输出上，而不是继续藏在 `LoweredHir` 私有字段或 dump helper 里。
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

    /// 当前先保持 `dump-mir` 的稳定 surface 只打印 MIR `File` Debug。
    ///
    /// refactor 专属 snapshot / golden 会在后续 P3 任务中单独冻结，不在这里提前改变 CLI 文本。
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
    use crate::mir::{CallKind, HandlerArmKind, Operand, Rvalue, StatementKind, TerminatorKind};
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
}
