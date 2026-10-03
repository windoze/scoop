use super::*;
use scoop_identity::{FieldIdentityView, GeneratedNominalKey, PersistentTypeId};

pub(super) fn validate_type<A: NominalSourceShapeSemanticAuthority<E>, E>(
    ty: &scoop_identity::SignatureTypeKey,
    kind: PublicNominalKindV1,
    scope: &SignatureBinderScopeV1,
    authority: &mut A,
) -> Result<(), SignatureTypeSemanticError<E>> {
    struct Storage<'a, A>(&'a mut A);
    impl<A: NominalSourceShapeSemanticAuthority<E>, E> NominalInterfaceShapeAuthority<E>
        for Storage<'_, A>
    {
        fn concrete_nominal_shape(
            &mut self,
            id: PersistentTypeId,
        ) -> Result<crate::PublicNominalShapeV1, E> {
            self.0.storage_nominal_shape(SourceNominalId::Concrete(id))
        }
        fn generic_nominal_shape(
            &mut self,
            id: scoop_identity::PersistentGenericTypeId,
        ) -> Result<crate::PublicNominalShapeV1, E> {
            self.0
                .storage_nominal_shape(SourceNominalId::GenericTemplate(id))
        }
    }
    match kind {
        PublicNominalKindV1::Class | PublicNominalKindV1::Object => {
            scope.validate_signature_semantics(ty, &mut Storage(authority))
        }
        _ => scope.validate_signature_semantics(ty, authority),
    }
}

pub(super) fn validate_owner<E>(
    key: &FieldIdentityKey,
    kind: PublicNominalKindV1,
    declaration: SourceNominalId,
) -> Result<(), NominalSourceFieldSemanticError<E>> {
    let valid_role = match (kind, key.view()) {
        (PublicNominalKindV1::Struct, FieldIdentityView::SourceDeclared { .. })
        | (PublicNominalKindV1::Class, FieldIdentityView::SourcePropertyBacking { .. })
        | (PublicNominalKindV1::Class, FieldIdentityView::SourcePropertyDelegate { .. }) => true,
        (PublicNominalKindV1::Object, FieldIdentityView::Generated { owner, key }) => {
            let SourceNominalId::Concrete(object) = declaration else {
                return Err(NominalSourceFieldSemanticError::FieldRole);
            };
            let expected =
                PersistentTypeId::from_generated_key(&GeneratedNominalKey::ObjectBackingClass {
                    object,
                });
            return if expected.ok() == Some(owner) && key.object_backing_property().is_some() {
                Ok(())
            } else {
                Err(NominalSourceFieldSemanticError::FieldRole)
            };
        }
        _ => false,
    };
    if !valid_role {
        return Err(NominalSourceFieldSemanticError::FieldRole);
    }
    let actual = key.source_owner();
    if actual != Some(declaration) {
        return Err(NominalSourceFieldSemanticError::Owner {
            expected: declaration,
            actual,
        });
    }
    Ok(())
}
