//! Annotation targets for public bindings and their lexical containers.

use super::ImportedSemanticWorld;
use crate::{AnnotationCallableV1 as Callable, AnnotationTargetV1 as Target};
use scoop_identity::{
    BindableEntity as B, CallableTemplateOrigin as C, DefinitionOwnerAtom as Owner,
    NominalDeclarationOwner as N, SourceDeclarationKey,
};
use std::collections::BTreeMap;

impl ImportedSemanticWorld<'_> {
    /// Includes support providers reached through re-exports. These are source
    /// declaration facts already loaded at the ordinary artifact boundary.
    pub fn public_binding_annotation_scopes(&self) -> BTreeMap<B, Vec<Target>> {
        self.providers
            .iter()
            .flat_map(|provider| provider.public_bindings())
            .map(|binding| {
                let target = binding.key().target();
                (target, self.binding_annotation_scopes(target))
            })
            .collect()
    }

    fn binding_annotation_scopes(&self, binding: B) -> Vec<Target> {
        let provider = self
            .entities
            .target_provider(binding)
            .and_then(|id| self.provider(id))
            .expect("a checked public binding has its declaration provider");
        let foundation = provider.foundation().canonical_for_semantic_authority();
        let nominal_key = |owner| match owner {
            N::Concrete(id) => foundation.source_type_key(id),
            N::GenericTemplate(id) => foundation
                .generic_type_by_bytes(id.as_array())
                .map(|(_, key)| key),
        };
        let (target, key): (_, Option<&SourceDeclarationKey>) = match binding {
            B::Type(id) => (
                Target::Nominal(N::Concrete(id)),
                foundation.source_type_key(id),
            ),
            B::GenericType(id) => (
                Target::Nominal(N::GenericTemplate(id)),
                foundation
                    .generic_type_by_bytes(id.as_array())
                    .map(|(_, key)| key),
            ),
            B::Function(id) => (
                Target::Callable(Callable::Declaration(C::Function(id))),
                foundation
                    .function_by_bytes(id.as_array())
                    .map(|(_, key)| key),
            ),
            B::GenericFunction(id) => (
                Target::Callable(Callable::Declaration(C::GenericFunction(id))),
                foundation
                    .generic_function_by_bytes(id.as_array())
                    .map(|(_, key)| key),
            ),
            B::Property(id) => (
                Target::Property(id),
                foundation
                    .property_by_bytes(id.as_array())
                    .map(|(_, key)| key),
            ),
            B::ExtensionProperty(id) => (
                Target::ExtensionProperty(id),
                foundation
                    .extension_property_by_bytes(id.as_array())
                    .map(|(_, key)| key),
            ),
            B::TypeAlias(id) => (
                Target::TypeAlias(id),
                foundation
                    .type_alias_by_bytes(id.as_array())
                    .map(|(_, key)| key),
            ),
            B::Annotation(id) => (
                Target::Annotation(id),
                foundation.annotation(id).map(|record| record.key()),
            ),
            B::ObjectValue(id) => {
                let owner = self
                    .object_value(id)
                    .expect("checked object binding")
                    .owner()
                    .persistent();
                (Target::Nominal(owner), nominal_key(owner))
            }
            B::EnumVariant(id) => {
                let owner = self
                    .enum_variant(id)
                    .expect("checked variant binding")
                    .owner()
                    .persistent();
                let mut scopes = vec![Target::Variant(id), Target::Nominal(owner)];
                append_owners(
                    &mut scopes,
                    nominal_key(owner).expect("checked nominal source key"),
                );
                return scopes;
            }
        };
        let mut scopes = vec![target];
        append_owners(
            &mut scopes,
            key.expect("a public declaration retains its source key"),
        );
        scopes
    }
}

fn append_owners(scopes: &mut Vec<Target>, key: &SourceDeclarationKey) {
    scopes.extend(
        key.owners()
            .owners()
            .iter()
            .filter_map(|owner| match owner {
                Owner::Type(id) => Some(Target::Nominal(N::Concrete(*id))),
                Owner::GenericType(id) => Some(Target::Nominal(N::GenericTemplate(*id))),
                _ => None,
            }),
    );
}
