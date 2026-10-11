use scoop_identity::{FieldIdentityKey, FieldIdentityView, GeneratedNominalKey};

use super::{DefinitionOriginSubject, OriginExpectation, nominal_subject};

pub(super) fn expectation(
    key: &FieldIdentityKey,
    generated_types: &[crate::foundation::GeneratedTypeRecord],
) -> Option<OriginExpectation<'static>> {
    match key.view() {
        FieldIdentityView::SourceDeclared { owner, .. }
        | FieldIdentityView::SourcePropertyBacking { owner, .. }
        | FieldIdentityView::SourcePropertyDelegate { owner, .. }
        | FieldIdentityView::SourceEnclosingInstance { owner }
        | FieldIdentityView::SourceCapturedEnvironment { owner }
        | FieldIdentityView::SourceInterfaceDelegate { owner, .. } => {
            Some(OriginExpectation::SameSource(nominal_subject(owner)))
        }
        FieldIdentityView::Generated { owner, key } => {
            let subject = if let Some(property) = key.object_backing_property() {
                DefinitionOriginSubject::Property(property)
            } else if key.interface_delegate().is_some() {
                let owner = generated_types.iter().find(|record| record.id() == owner)?;
                match owner.key() {
                    GeneratedNominalKey::ObjectBackingClass { object } => {
                        DefinitionOriginSubject::Type(*object)
                    }
                    GeneratedNominalKey::GenericObjectBackingClass { object } => {
                        DefinitionOriginSubject::GenericType(*object)
                    }
                    _ => return None,
                }
            } else {
                return None;
            };
            Some(OriginExpectation::SameSource(subject))
        }
    }
}
