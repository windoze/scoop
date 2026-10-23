//! Complete optimization templates with provider identity, never definitions.
use super::*;

mod dispatch;
mod intrinsics;

#[derive(Default)]
pub(super) struct ImportedBodies {
    pub functions: HashMap<mir::ExternalCallableUseId, mir::Function>,
    pub required: HashSet<mir::ExternalCallableUseId>,
    pub dispatch: dispatch::IntrinsicDispatch,
}

pub(super) use dispatch::IntrinsicDispatch;

impl Lowerer {
    pub(super) fn lower_imported_bodies(&mut self, module: &hir::Module) -> ImportedBodies {
        let mut bodies = HashMap::new();
        for (_, source) in module.imported_inline_bodies.iter() {
            let Some(ImportedCallableTarget {
                entry:
                    ImportedCallableEntry::Scoop {
                        callable,
                        native_c: None,
                    },
                ..
            }) = self.imported_dependency_callable_map.get(&source.callable)
            else {
                continue;
            };
            let callable = *callable;
            let gc_effect = self.external_callables[callable].gc_effect();
            let mut lowerer = self.body_lowerer(
                module,
                crate::local_values::BodyOwner::Imported(callable),
                source.materialization,
                mir::ImmortalObjectOwner::Callable(source.materialization),
            );
            for (id, local) in source.body.locals.iter() {
                let ty = lowerer.lower_type(local.ty);
                let local = lowerer.locals.alloc(mir::Local {
                    name: local.name.clone(),
                    ty,
                    mutable: local.mutable,
                });
                lowerer.local_map.insert(id, local);
            }
            let params = source
                .params
                .iter()
                .map(|param| mir::Param {
                    name: param.name.clone(),
                    ty: lowerer.lower_type(param.ty),
                    local: lowerer.local_map[&param.local],
                })
                .collect();
            let return_ty = lowerer.lower_type(source.return_ty);
            let statements = lowerer.lower_statements(&source.body.statements);
            let body = smir::Body {
                locals: lowerer.locals,
                statements,
                coroutine_eh: None,
            };
            let body = cfg::lower(body, return_ty.clone(), &self.enums.defs, &self.classes).body;
            bodies.insert(
                callable,
                mir::Function {
                    name: "imported inline body".into(),
                    gc_effect,
                    params,
                    return_ty,
                    body,
                },
            );
        }
        let mut bodies = ImportedBodies {
            functions: bodies,
            ..Default::default()
        };
        self.lower_imported_intrinsics(module, &mut bodies);
        bodies
    }
}
