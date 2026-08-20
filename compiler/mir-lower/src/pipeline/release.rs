use super::*;

impl Lowerer {
    pub(super) fn lower_release_hooks(&mut self, module: &hir::Module) -> Arena<mir::ReleaseHook> {
        let mut hooks = Arena::new();
        let mut hook_map = HashMap::new();
        for (source_id, hook) in module.release_hooks.iter() {
            let id = mir::ReleaseHookId::from_raw(
                u32::try_from(hooks.len())
                    .expect("release hook ids fit in u32")
                    .into(),
            );
            let body = self
                .body_lowerer(
                    module,
                    id,
                    hook.materialization,
                    mir::ImmortalObjectOwner::Callable(hook.materialization),
                )
                .lower_release_hook(source_id);
            let lowered = cfg::lower(body, mir::Type::Unit, &self.enums.defs, &self.classes);
            self.local_values
                .record_generated(id, hook.materialization, &lowered.generated_values);
            let owner = self.class_map[&hook.owner];
            let allocated = hooks.alloc(mir::ReleaseHook {
                owner,
                materialization: hook.materialization,
                code: mir::Function {
                    gc_effect: mir::GcEffect::NoGc,
                    name: format!("{}::release", self.classes[owner].name),
                    params: Vec::new(),
                    return_ty: mir::Type::Unit,
                    body: lowered.body,
                },
            });
            assert_eq!(allocated, id);
            hook_map.insert(source_id, id);
        }
        for (source, declaration) in module.classes.iter() {
            let class = self.class_map[&source];
            self.classes[class].release_policy = match declaration.release_policy {
                hir::ReleasePolicy::None => mir::ReleasePolicy::None,
                hir::ReleasePolicy::SynchronousGcFree { hook } => {
                    mir::ReleasePolicy::SynchronousGcFree {
                        hook: match hook {
                            hir::ReleaseHookTarget::Local(hook) => {
                                mir::ReleaseHookTarget::Local(hook_map[&hook])
                            }
                            hir::ReleaseHookTarget::External { owner } => {
                                mir::ReleaseHookTarget::External {
                                    owner: self.class_map[&owner],
                                }
                            }
                        },
                    }
                }
            };
        }
        hooks
    }
}
