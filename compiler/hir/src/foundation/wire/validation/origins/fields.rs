use scoop_identity::{FieldIdentityKey, FieldIdentityView};

use super::{DefinitionOriginSubject, OriginExpectation, nominal_subject};

pub(super) fn expectation(key: &FieldIdentityKey) -> Option<OriginExpectation<'_>> {
    match key.view() {
        FieldIdentityView::SourceDeclared { owner, .. }
        | FieldIdentityView::SourcePropertyBacking { owner, .. }
        | FieldIdentityView::SourcePropertyDelegate { owner, .. } => {
            Some(OriginExpectation::SameSource(nominal_subject(owner)))
        }
        FieldIdentityView::Generated { key, .. } => key.object_backing_property().map(|property| {
            OriginExpectation::SameSource(DefinitionOriginSubject::Property(property))
        }),
    }
}
