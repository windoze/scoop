//! External primitive entries use exactly the local intrinsic lowering.
use super::*;
use crate::closures::primitive::PrimitiveReference;

impl Lowerer {
    pub(super) fn lower_imported_intrinsics(
        &mut self,
        module: &hir::Module,
        bodies: &mut ImportedBodies,
    ) {
        for (source, _) in module.imported_dependency_callables.iter() {
            let Some(&operation) = module.imported_intrinsics.get(&source) else {
                continue;
            };
            let Some(target) = self.imported_dependency_callable_map.get(&source).cloned() else {
                continue;
            };
            let callable = target.scoop_entry();
            let scoop_identity::CallableTemplateOrigin::Function(owner) = module
                .imported_dependency_callables[source]
                .reference()
                .declaration()
            else {
                unreachable!("primitive members are source functions");
            };
            let materialization = scoop_identity::CallableMaterialization::new(
                scoop_identity::CallableTemplateOwner::Function(owner),
                scoop_identity::CallableMaterializationContext::NoSubstitution,
            );
            let gc_effect = self.external_callables[callable].gc_effect();
            let source_type = |exact| {
                module
                    .exact_type_identities
                    .type_for_identity(exact)
                    .expect("selected signatures retain concrete HIR types")
            };
            let mut lowerer = self.body_lowerer(
                module,
                crate::local_values::BodyOwner::Imported(callable),
                materialization,
                mir::ImmortalObjectOwner::Callable(materialization),
            );
            let params = target
                .signature
                .receiver()
                .into_option()
                .into_iter()
                .chain(target.signature.parameters().iter().copied())
                .enumerate()
                .map(|(index, exact)| {
                    let ty = lowerer.lower_type(source_type(exact));
                    let name = format!("arg{index}");
                    let local = lowerer.locals.alloc(mir::Local {
                        name: name.clone(),
                        ty: ty.clone(),
                        mutable: false,
                    });
                    mir::Param { name, ty, local }
                })
                .collect::<Vec<_>>();
            let return_ty = lowerer.lower_type(source_type(target.signature.result()));
            let arg =
                |index: usize| smir::Expr::local(params[index].local, params[index].ty.clone());
            let span = hir::Span::new(0, 0);
            let value = match PrimitiveReference::classify(operation) {
                PrimitiveReference::Unary(op) => {
                    lowerer.unary_reference_value(op, arg(0), &return_ty)
                }
                PrimitiveReference::Binary(op) => {
                    lowerer.binary_reference_value(op, arg(0), arg(1), &return_ty, span)
                }
            };
            let mut statements = lowerer
                .prelude
                .into_iter()
                .map(|kind| smir::Statement { kind, span })
                .collect::<Vec<_>>();
            statements.push(smir::Statement {
                kind: smir::StatementKind::Return { value },
                span,
            });
            let body = cfg::lower(
                smir::Body {
                    locals: lowerer.locals,
                    statements,
                    coroutine_eh: None,
                },
                return_ty.clone(),
                &self.enums.defs,
                &self.classes,
            )
            .body;
            bodies.functions.insert(
                callable,
                mir::Function {
                    name: "imported intrinsic".into(),
                    gc_effect,
                    params,
                    return_ty,
                    body,
                },
            );
            bodies.required.insert(callable);
        }
    }
}
