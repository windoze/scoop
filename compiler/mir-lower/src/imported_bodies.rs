//! Complete optimization templates with provider identity, never definitions.
use super::*;

mod constructors;
mod dispatch;
mod intrinsics;

#[derive(Default)]
pub(super) struct ImportedBodies {
    pub functions: HashMap<mir::ExternalCallableUseId, mir::Function>,
    pub call_sites: HashMap<mir::ExternalCallableUseId, Vec<cfg::CallSite>>,
    pub unsupported: HashSet<mir::ExternalCallableUseId>,
    pub required: HashSet<mir::ExternalCallableUseId>,
    pub dispatch: dispatch::IntrinsicDispatch,
}

pub(super) use dispatch::IntrinsicDispatch;

impl Lowerer {
    pub(super) fn lower_imported_bodies(&mut self, module: &hir::Module) -> ImportedBodies {
        let mut bodies = HashMap::new();
        let mut call_sites = HashMap::new();
        let mut unsupported = HashSet::new();
        for (id, source) in module.imported_dependency_callables.iter() {
            if source.requires_provider_entry()
                && let Some(ImportedCallableTarget {
                    entry:
                        ImportedCallableEntry::Scoop {
                            callable,
                            native_c: None,
                        },
                    ..
                }) = self.imported_dependency_callable_map.get(&id)
            {
                unsupported.insert(*callable);
            }
        }
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
            // Suspend ABI lowering needs a local coroutine identity. A provider
            // template remains available in HIR without becoming a local definition.
            if source.is_suspend {
                unsupported.insert(callable);
                continue;
            }
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
                    debug_variable: Default::default(),
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
            let lowered = cfg::lower(body, return_ty.clone(), &self.enums.defs, &self.classes);
            if self.profiling {
                call_sites.insert(callable, lowered.call_sites);
            }
            let body = lowered.body;
            bodies.insert(
                callable,
                mir::Function {
                    debug_origin: Default::default(),
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
            call_sites,
            unsupported,
            ..Default::default()
        };
        self.lower_imported_constructors(module, &mut bodies);
        self.lower_imported_intrinsics(module, &mut bodies);
        bodies
    }
}
