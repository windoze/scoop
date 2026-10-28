//! Forward a tail suspend call's complete step and completion without a frame.

use super::*;

mod returns;

pub(super) fn rewrite(
    lowerer: &mut Lowerer,
    coroutine: mir::CoroutineFunctionId,
    sites: &[SuspendSite],
) -> bool {
    let metadata = &lowerer.coroutines.functions[coroutine];
    let function = &lowerer.functions[metadata.function];
    let completed = lowerer.coroutines.steps[metadata.step].completed();
    let mut addressed = HashSet::new();
    for (_, block) in function.body.blocks.iter() {
        mir::visit_block_exprs(block, &mut |expression| {
            if let mir::ExprKind::AddressOf { local, .. } = expression.kind {
                addressed.insert(local);
            }
        });
    }
    if function.body.blocks.iter().any(|(_, block)| {
        block.unwind.is_some()
            || block
                .statements
                .iter()
                .any(|statement| matches!(statement.kind, mir::StatementKind::Eh(_)))
    }) || !sites.iter().all(|site| {
        site.resume.result == metadata.source_return
            && site.resume.pending.0.is_empty()
            && direct_target(lowerer, &site.kind).is_some_and(|callee| {
                lowerer.functions[callee].return_ty == function.return_ty
                    && lowerer.functions[callee]
                        .params
                        .last()
                        .map(|param| &param.ty)
                        == function.params.last().map(|param| &param.ty)
            })
            && returns::unchanged(&function.body, &site.resume, completed, &addressed)
    }) {
        return false;
    }

    let function_id = metadata.function;
    let owner = metadata.source;
    let function = &mut lowerer.functions[function_id];
    let completion = function
        .params
        .last()
        .expect("suspend ABI has a completion");
    let completion = mir::Expr::local(completion.local, completion.ty.clone());
    for site in sites {
        let SuspendKind::Call(mut call) = site.kind.clone() else {
            unreachable!("only direct suspend calls were selected");
        };
        call.args.push(completion.clone());
        let result = function.body.locals.alloc(mir::Local {
            debug_origin: Default::default(),
            name: format!("$tail_step.{}", site.resume.state),
            ty: function.return_ty.clone(),
            mutable: false,
        });
        lowerer.local_values.record_generated_local(
            function_id,
            result,
            owner,
            hir::StructuralDefinitionSiteRole::SyntheticValue,
            hir::SyntheticLocalRole::Temporary,
        );
        let block = &mut function.body.blocks[site.resume.block];
        let span = block.statements[site.resume.statement].span;
        block.statements.truncate(site.resume.statement);
        block.statements.push(mir::Statement {
            debug_origin: Default::default(),
            kind: mir::StatementKind::Call(mir::CallEffect::Value {
                destination: result,
                call,
            }),
            span,
        });
        block.terminator = mir::Terminator::Return {
            value: mir::Expr::local(result, function.return_ty.clone()),
        };
    }
    lowerer.coroutines.functions[coroutine].lowering = mir::CoroutineLowering::TailForwarding;
    true
}

fn direct_target(lowerer: &Lowerer, kind: &SuspendKind) -> Option<mir::FunctionId> {
    let SuspendKind::Call(call) = kind else {
        return None;
    };
    match call.target {
        mir::CallTarget::Direct(mir::Callee::User(function)) => Some(function),
        mir::CallTarget::Direct(mir::Callee::Monomorphized(instance)) => {
            Some(lowerer.instances.meta[instance].function)
        }
        _ => None,
    }
}
