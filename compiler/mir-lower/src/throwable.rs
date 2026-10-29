//! First-throw snapshots are ordinary managed calls before native unwinding.

use scoop_hir::concrete as hir;
use scoop_identity::{StructuralDefinitionSiteRole, SyntheticLocalRole};
use scoop_mir as mir;

use crate::local_values::{LocalValueRegistry, callable_owner};

pub(super) fn prepare(module: &mut mir::Module, source: &hir::Module) {
    let exact = source.exact_type_identities[source.throwable_trace_bytes].id();
    let mir::Type::Class(trace_bytes) = *module
        .meta
        .source_exact_types
        .get_by_identity(exact)
        .expect("the actual Throwable byte-array type crosses HIR to MIR")
        .ty()
    else {
        unreachable!("snapshot storage is the core Array<UInt8> class")
    };
    let mut registry = LocalValueRegistry::from_existing(&module.meta.local_values);
    let owners = module
        .functions
        .iter()
        .map(|(id, _)| (id, callable_owner(module, id)))
        .collect::<Vec<_>>();
    for (id, owner) in owners {
        let body = &mut module.functions[id].body;
        for (_, block) in body.blocks.iter_mut() {
            let mir::Terminator::Throw { exception, .. } = &mut block.terminator else {
                continue;
            };
            let local = body.locals.alloc(mir::Local {
                name: "$prepared.throwable".into(),
                ty: exception.ty.clone(),
                mutable: false,
                debug_variable: Default::default(),
            });
            registry.record_generated_local(
                id,
                local,
                owner.expect("every emitted throwing function has a callable identity"),
                StructuralDefinitionSiteRole::SyntheticValue,
                SyntheticLocalRole::Temporary,
            );
            let replacement = mir::Expr::local(local, exception.ty.clone());
            let argument = std::mem::replace(exception, replacement);
            block.statements.push(mir::Statement {
                span: mir::SourceSpan::new(0, 0)
                    .expect("a generated operation has an empty source span"),
                debug_origin: block.terminator_origin.clone(),
                kind: mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: local,
                    call: mir::Call {
                        target: mir::CallTarget::Direct(mir::Callee::Runtime(
                            mir::RuntimeFn::PrepareThrowable { trace_bytes },
                        )),
                        args: vec![argument],
                    },
                }),
            });
        }
    }
    module.meta.local_values = registry.finish();
}
