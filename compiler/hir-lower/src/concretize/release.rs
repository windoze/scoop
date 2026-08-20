use super::*;

pub(super) struct PendingReleaseHook {
    origin: export::DefinitionOrigin,
    owner: concrete::ClassId,
    body: concrete::Body,
}

impl Concretizer<'_> {
    pub(super) fn materialize_release_hook(
        &mut self,
        class: concrete::ClassId,
        origin: export::SourceNominalId,
        arguments: &[concrete::TypeId],
    ) {
        let policy = self.source.class_definition(origin).release_policy.clone();
        let hook = match policy {
            export::ReleasePolicy::None => return,
            export::ReleasePolicy::SynchronousGcFree {
                hook: export::ExportReleaseHookRef::Imported { .. },
            } => {
                self.classes[class].release_policy = concrete::ReleasePolicy::SynchronousGcFree {
                    hook: concrete::ReleaseHookTarget::External { owner: class },
                };
                return;
            }
            export::ReleasePolicy::SynchronousGcFree {
                hook: export::ExportReleaseHookRef::Template(hook),
            } => hook,
        };
        let index = self.release_hook_slots.len();
        let id = concrete::ReleaseHookId::from_raw(
            u32::try_from(index)
                .expect("release hook ids fit in u32")
                .into(),
        );
        self.release_hook_slots.push(None);
        self.classes[class].release_policy = concrete::ReleasePolicy::SynchronousGcFree {
            hook: concrete::ReleaseHookTarget::Local(id),
        };
        let source = &self.source.release_hooks[hook];
        let origin = source.origin;
        let (body, _) = self.lower_body(&source.body, arguments);
        self.release_hook_slots[index] = Some(PendingReleaseHook {
            origin,
            owner: class,
            body,
        });
    }
}

pub(super) fn finish_release_hooks(
    slots: Vec<Option<PendingReleaseHook>>,
    classes: &Arena<concrete::ClassDef>,
    exact_types: &concrete::ExactTypeIdentities,
) -> Arena<concrete::ReleaseHook> {
    slots
        .into_iter()
        .map(|slot| {
            let hook = slot.expect("every requested release body is completely lowered");
            let owner = exact_types[classes[hook.owner].canonical_type].id();
            concrete::ReleaseHook {
                origin: hook.origin,
                owner: hook.owner,
                materialization: concrete::CallableMaterialization::new(
                    concrete::CallableTemplateOwner::ReleaseHook(owner),
                    concrete::CallableMaterializationContext::NoSubstitution,
                ),
                body: hook.body,
            }
        })
        .collect()
}
