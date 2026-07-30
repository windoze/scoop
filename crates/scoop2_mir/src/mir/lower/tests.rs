//! lowering 单元测试。

#![cfg(test)]

use scoop2_base::Interner;
use scoop2_hir::ty::TypeStore;

/// dump 一个空 Module 应产出稳定的 `Module { ... }` 文本。
#[test]
fn dumps_empty_module_stably() {
    let interner = Interner::new();
    let module = crate::mir::Module {
        items: Vec::new(),
        types: TypeStore::new(),
    };
    let text = crate::mir::dump::dump_module(&module, &interner);
    assert!(text.starts_with("Module {"));
    // 空 items：dump 为 `items: [\n    ],`（无 FunDecl 行）。
    assert!(text.contains("items: ["));
    assert!(!text.contains("FunDecl"));
}

/// CFG 验证：空 Body（仅入口块）应通过 verify_cfg。
#[test]
fn verify_minimal_body() {
    use crate::mir::verify::verify_body;
    use crate::mir::{BasicBlock, Body, Terminator, TerminatorKind};
    let mut body = Body::new();
    // push 入口块。
    let _ = body.push_block(BasicBlock::new(Terminator {
        span: scoop2_base::Span::default(),
        kind: TerminatorKind::Return { value: None },
    }));
    let mut errors = Vec::new();
    verify_body(&body, &mut errors);
    assert!(errors.is_empty(), "最小 body 不应有验证错误: {:?}", errors);
}

/// CFG 验证：悬空后继应被检测到。
#[test]
fn verify_detects_dangling_successor() {
    use crate::diagnostics::VerifyError;
    use crate::mir::verify::verify_body;
    use crate::mir::{BasicBlock, BasicBlockId, Body, Terminator, TerminatorKind};
    let mut body = Body::new();
    // push 入口块，Goto 一个不存在的基本块。
    let _ = body.push_block(BasicBlock::new(Terminator {
        span: scoop2_base::Span::default(),
        kind: TerminatorKind::Goto {
            target: BasicBlockId(99),
        },
    }));
    let mut errors = Vec::new();
    verify_body(&body, &mut errors);
    assert!(
        errors.iter().any(
            |e| matches!(e, VerifyError { code, .. } if *code == crate::diagnostics::VERIFY_CFG)
        ),
        "悬空后继应报 verify_cfg: {:?}",
        errors
    );
}

/// production 语义验证：空 callee_fqn 的 Direct 调用应报 verify_semantic。
#[test]
fn verify_semantic_detects_empty_callee() {
    use crate::diagnostics::{VERIFY_SEMANTIC, VerifyError};
    use crate::mir::verify::verify_semantic;
    use crate::mir::{
        BasicBlock, Body, CallArg, CallKind, FunDecl, LocalDecl, LocalId, LocalSource, Operand,
        Rvalue, Statement, StatementKind, Terminator, TerminatorKind,
    };
    use scoop2_hir::ty::TypeStore;
    let mut store = TypeStore::new();
    let unit_ty = store.unit();
    let int_ty = store.int();
    // 构造一个 body：bb0 有 `tmp = Call(Direct { callee_fqn: "" }, ...)`。
    let mut body = Body::new();
    let tmp = LocalId(0);
    body.locals.push(LocalDecl {
        span: scoop2_base::Span::default(),
        name: None,
        ty: int_ty,
        source: LocalSource::Temp,
        mutable: false,
    });
    let _ = body.push_block(BasicBlock {
        stmts: vec![Statement {
            span: scoop2_base::Span::default(),
            kind: StatementKind::Assign {
                target: tmp,
                value: Rvalue::Call {
                    site_id: None,
                    kind: CallKind::Direct {
                        callee_fqn: String::new(),
                        type_args: Vec::new(),
                        is_intrinsic: false,
                        stable_template_key: None,
                        stable_instance_key: None,
                        generic_type_args: Vec::new(),
                        generic_eff_args: Vec::new(),
                    },
                    args: Vec::<CallArg>::new(),
                    transport: crate::mir::CallTransportMetadata::plain_no_outward(
                        int_ty,
                        crate::mir::MirTransportKind::Scalar,
                    ),
                },
            },
        }],
        terminator: Terminator {
            span: scoop2_base::Span::default(),
            kind: TerminatorKind::Return { value: None },
        },
    });
    let fd = FunDecl {
        span: scoop2_base::Span::default(),
        fqn: "test".to_string(),
        name: "test".to_string(),
        ty: unit_ty,
        params: Vec::new(),
        return_ty: unit_ty,
        effect_row: scoop2_hir::ty::EffectRow::pure(),
        type_params: Vec::new(),
        body: None,
        file: scoop2_base::FileId(0),
        stable_template_key: None,
        effect_abi: None,
        instance_symbol: None,
        intrinsic_name: None,
    };
    let mut errors = Vec::new();
    let kf: std::collections::HashSet<String> = std::collections::HashSet::new();
    let kt = kf.clone();
    let aks = kf.clone();
    verify_semantic(&fd, &body, &kf, &kt, &aks, &mut errors);
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, VerifyError { code, .. } if *code == VERIFY_SEMANTIC)),
        "空 callee_fqn 的 Direct 调用应报 verify_semantic: {:?}",
        errors
    );
}
