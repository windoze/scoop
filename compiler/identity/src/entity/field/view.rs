//! Borrowed semantic inspection of a validated field key. This API adds no
//! identity kind, persistent record, or wire representation.

use super::{
    FieldIdentityKey, FieldIdentityKeyKind, GeneratedFieldKey, GeneratedFieldKeyKind,
    SourceFieldKey, SourceFieldKeyKind,
};
use crate::{CanonicalIdentifier, NominalDeclarationOwner, PersistentPropertyId, PersistentTypeId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldIdentityView<'a> {
    SourceDeclared {
        owner: NominalDeclarationOwner,
        name: &'a CanonicalIdentifier,
    },
    SourcePropertyBacking {
        owner: NominalDeclarationOwner,
        property: PersistentPropertyId,
    },
    SourcePropertyDelegate {
        owner: NominalDeclarationOwner,
        property: PersistentPropertyId,
    },
    Generated {
        owner: PersistentTypeId,
        key: &'a GeneratedFieldKey,
    },
}

impl FieldIdentityKey {
    pub const fn view(&self) -> FieldIdentityView<'_> {
        match &self.0 {
            FieldIdentityKeyKind::Source(SourceFieldKey(SourceFieldKeyKind::Declared {
                owner,
                name,
            })) => FieldIdentityView::SourceDeclared {
                owner: *owner,
                name,
            },
            FieldIdentityKeyKind::Source(SourceFieldKey(SourceFieldKeyKind::PropertyBacking {
                owner,
                property,
            })) => FieldIdentityView::SourcePropertyBacking {
                owner: *owner,
                property: *property,
            },
            FieldIdentityKeyKind::Source(SourceFieldKey(
                SourceFieldKeyKind::PropertyDelegate { owner, property },
            )) => FieldIdentityView::SourcePropertyDelegate {
                owner: *owner,
                property: *property,
            },
            FieldIdentityKeyKind::Generated { owner, key } => {
                FieldIdentityView::Generated { owner: *owner, key }
            }
        }
    }
}

impl GeneratedFieldKey {
    pub const fn object_backing_property(&self) -> Option<PersistentPropertyId> {
        match self.0 {
            GeneratedFieldKeyKind::ObjectBackingProperty(property) => Some(property),
            GeneratedFieldKeyKind::BoxPayload
            | GeneratedFieldKeyKind::ClosureCapture(_)
            | GeneratedFieldKeyKind::CallableReferenceReceiver(_)
            | GeneratedFieldKeyKind::CoroutineFrameState
            | GeneratedFieldKeyKind::CoroutineFrameCompletion
            | GeneratedFieldKeyKind::CoroutineFrameSaved(_)
            | GeneratedFieldKeyKind::CoroutineFrameFailure
            | GeneratedFieldKeyKind::CoroutineAdapterFrame
            | GeneratedFieldKeyKind::CoroutineAdapterState
            | GeneratedFieldKeyKind::CoroutineAdapterResult
            | GeneratedFieldKeyKind::CoroutineAdapterFailure
            | GeneratedFieldKeyKind::FunctionAdapterSource => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConeIdentity, GeneratedNominalKey, PersistentExactTypeId, SourceNominalKind};

    #[test]
    fn source_view_keeps_declared_backing_and_delegate_roles_distinct() {
        let structure = source_nominal(SourceNominalKind::Struct);
        let class = source_nominal(SourceNominalKind::Class);
        let property = PersistentPropertyId(ConeIdentity::CORE.0);
        let name = CanonicalIdentifier::new("value").unwrap();
        let declared = FieldIdentityKey::source_declared(&structure, name.clone()).unwrap();
        assert_eq!(
            declared.view(),
            FieldIdentityView::SourceDeclared {
                owner: NominalDeclarationOwner::from_source_declaration(&structure).unwrap(),
                name: &name
            }
        );
        assert!(
            matches!(FieldIdentityKey::source_property_backing(&class, property).unwrap().view(), FieldIdentityView::SourcePropertyBacking { property: actual, .. } if actual == property)
        );
        assert!(
            matches!(FieldIdentityKey::source_property_delegate(&class, property).unwrap().view(), FieldIdentityView::SourcePropertyDelegate { property: actual, .. } if actual == property)
        );
    }

    #[test]
    fn generated_view_keeps_owner_and_excludes_nonobject_roles() {
        let nominal = GeneratedNominalKey::BoxedValue {
            payload: PersistentExactTypeId(ConeIdentity::CORE.0),
        };
        let key = FieldIdentityKey::box_payload(&nominal).unwrap();
        let FieldIdentityView::Generated { owner, key } = key.view() else {
            panic!("expected a generated field")
        };
        assert_eq!(
            owner,
            PersistentTypeId::from_generated_key(&nominal).unwrap()
        );
        assert_eq!(key.object_backing_property(), None);
    }

    fn source_nominal(kind: SourceNominalKind) -> crate::SourceDeclarationKey {
        let site = crate::SourceDeclarationSite::new(
            ConeIdentity::CORE,
            crate::PackagePath::root(),
            crate::DefinitionOwnerChain::top_level(),
            crate::DeclarationScope::ConeWide,
        )
        .unwrap();
        crate::SourceDeclarationKey::nominal(
            site,
            CanonicalIdentifier::new("Owner").unwrap(),
            kind,
            0,
        )
    }
}
