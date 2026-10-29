//! Primary value constructors retain their exact field assembly across Cones.
use super::*;

impl Lowerer {
    pub(super) fn lower_imported_constructors(
        &mut self,
        module: &hir::Module,
        bodies: &mut ImportedBodies,
    ) {
        for (source, declaration) in module.imported_dependency_callables.iter() {
            let Some(ImportedCallableTarget {
                entry:
                    ImportedCallableEntry::Scoop {
                        callable,
                        native_c: None,
                    },
                lowering_role: mir::MirCallableLoweringRoleV1::PrimaryValueConstructor { owner },
                signature,
                ..
            }) = self.imported_dependency_callable_map.get(&source).cloned()
            else {
                continue;
            };
            let scoop_identity::CallableTemplateOrigin::Constructor(constructor) =
                declaration.reference().declaration()
            else {
                unreachable!("selected primary value constructors retain their declaration");
            };
            let materialization = scoop_identity::CallableMaterialization::new(
                scoop_identity::CallableTemplateOwner::Constructor(constructor),
                scoop_identity::CallableMaterializationContext::NoSubstitution,
            );
            let gc_effect = self.external_callables[callable].gc_effect();
            let source_type = |exact| {
                module
                    .exact_type_identities
                    .type_for_identity(exact)
                    .expect("selected constructor signatures retain concrete HIR types")
            };
            let mut lowerer = self.body_lowerer(
                module,
                crate::local_values::BodyOwner::Imported(callable),
                materialization,
                mir::ImmortalObjectOwner::Callable(materialization),
            );
            let return_ty = lowerer.lower_type(source_type(owner));
            let mir::Type::Struct(struct_id) = return_ty else {
                unreachable!("primary value constructor results have a struct representation");
            };
            let params = signature
                .parameters()
                .iter()
                .enumerate()
                .map(|(index, &exact)| {
                    let ty = lowerer.lower_type(source_type(exact));
                    let name = format!("arg{index}");
                    let local = lowerer.locals.alloc(mir::Local {
                        debug_variable: Default::default(),
                        name: name.clone(),
                        ty: ty.clone(),
                        mutable: false,
                    });
                    mir::Param { name, ty, local }
                })
                .collect::<Vec<_>>();
            let args = params
                .iter()
                .map(|param| smir::Expr::local(param.local, param.ty.clone()))
                .collect();
            let body = cfg::lower(
                smir::Body {
                    locals: lowerer.locals,
                    statements: vec![smir::Statement {
                        debug_origin: Default::default(),
                        kind: smir::StatementKind::Return {
                            value: smir::Expr::new(
                                return_ty.clone(),
                                smir::ExprKind::StructInit { struct_id, args },
                            ),
                        },
                        span: hir::Span::new(0, 0),
                    }],
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
                    debug_origin: Default::default(),
                    name: "imported primary value constructor".into(),
                    gc_effect,
                    params,
                    return_ty,
                    body,
                },
            );
        }
    }
}
