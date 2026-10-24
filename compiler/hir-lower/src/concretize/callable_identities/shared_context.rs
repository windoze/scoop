//! Caller-expanded defaults share their lexical helpers across Cones.

use super::*;

impl CallableIdentityBuilder<'_> {
    pub(super) fn shared_lexical_materialization(
        &mut self,
        excluded_function: Option<export::FunctionId>,
        root: export::LexicalDefinitionRoot,
        path: &StructuralDefinitionPath,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let parent = self.enclosing_materialization(excluded_function, root, path, arguments);
        if path.segments().iter().any(|segment| {
            segment.site_role() == scoop_identity::StructuralDefinitionSiteRole::DefaultValue
        }) {
            self.shared_body_materialization(parent)
        } else {
            parent
        }
    }

    pub(super) fn imported_shared_parent(
        &mut self,
        parent: CallableTemplateOwner,
        arguments: &[concrete::TypeId],
    ) -> CallableMaterialization {
        let parent = self.imported_parent_materialization(parent, arguments);
        self.shared_body_materialization(parent)
    }

    fn shared_body_materialization(
        &mut self,
        materialization: CallableMaterialization,
    ) -> CallableMaterialization {
        if materialization.context() != CallableMaterializationContext::NoSubstitution {
            return materialization;
        }
        let owner = CallableInstantiationOwner::NoOwner;
        let key = match materialization.template() {
            CallableTemplateOwner::Function(id) => CallableApplicationKey::for_function(id, owner),
            CallableTemplateOwner::Constructor(id) => {
                CallableApplicationKey::for_constructor(id, owner)
            }
            CallableTemplateOwner::Accessor(id) => CallableApplicationKey::for_accessor(id, owner),
            CallableTemplateOwner::VariantConstructor(id) => {
                CallableApplicationKey::for_variant_constructor(id, owner)
            }
            CallableTemplateOwner::GenericFunction(_)
            | CallableTemplateOwner::Generated(_)
            | CallableTemplateOwner::ReleaseHook(_) => {
                unreachable!("a shared nested body inherits its parent's application")
            }
        };
        let application = self.record_application(key);
        CallableMaterialization::new(
            materialization.template(),
            CallableMaterializationContext::Application(application),
        )
    }
}
