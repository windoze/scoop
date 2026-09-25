use super::*;
use crate::{
    CanonicalExactCallableAbiExportsV1, CanonicalExactDescriptorExportsV1,
    CanonicalExactDispatchExportsV1, CanonicalExactLayoutExportsV1, ExactLayoutBodyKindV1,
    InstanceRepresentationKindV1,
};
use scoop_identity::ScanRole;

impl ExternalShapeLinkImportV1<'_> {
    pub(crate) fn semantic_target(
        &self,
        layouts: &CanonicalExactLayoutExportsV1,
    ) -> Result<Option<crate::LayoutAbiSemanticTargetV1>, ShapeLinkError> {
        semantic_target(self.subject(), layouts)
    }
    /// Rebinds the semantic contract to the terminal section's actual tables.
    /// Storage and initialization retain the enclosing closure's support join.
    pub(crate) fn validate_semantic_against(
        &self,
        layouts: &CanonicalExactLayoutExportsV1,
        callables: &CanonicalExactCallableAbiExportsV1,
        descriptors: &CanonicalExactDescriptorExportsV1,
        dispatch: &CanonicalExactDispatchExportsV1,
    ) -> Result<(), ShapeLinkError> {
        if [
            layouts.provider(),
            callables.provider(),
            descriptors.provider(),
            dispatch.provider(),
        ]
        .into_iter()
        .any(|provider| provider != self.provider)
        {
            return Err(ShapeLinkError::Provider);
        }
        use ExternalStrongShapeSubjectV1 as Subject;

        let expected = match self.subject {
            Subject::Callable(target) => {
                let record = callables
                    .get(target)
                    .ok_or(ShapeLinkError::MissingSubject(self.subject))?;
                ShapeLinkContractV1::CallableAbi {
                    canonical_signature: record.canonical_signature(),
                    calling_convention: record.calling_convention(),
                    protocol: record.call_protocol(),
                }
            }
            Subject::Layout(id) => ShapeLinkContractV1::Layout {
                record: layouts
                    .get(id)
                    .ok_or(ShapeLinkError::MissingSubject(self.subject))?,
            },
            Subject::Scan(id) => {
                let record = layouts
                    .records()
                    .iter()
                    .find(|record| record.scan() == id)
                    .ok_or(ShapeLinkError::MissingSubject(self.subject))?;
                let (role, canonical_scan) = match record.kind() {
                    ExactLayoutBodyKindV1::Value(value) => (
                        ScanRole::InlineValue,
                        crate::shape_link::provider::contracts::storage_scan(
                            value.value().storage(),
                        ),
                    ),
                    ExactLayoutBodyKindV1::Instance(instance) => {
                        match instance.representation().kind() {
                            InstanceRepresentationKindV1::InlineArray { .. } => {
                                (ScanRole::ArrayElement, instance.shape().inline_scan())
                            }
                            _ => (ScanRole::ManagedObject, instance.shape().object_scan()),
                        }
                    }
                };
                ShapeLinkContractV1::Scan {
                    layout: record.identity().layout(),
                    role,
                    canonical_scan,
                }
            }
            Subject::TypeDescriptor(id) | Subject::TypeRegistration(id) => {
                ShapeLinkContractV1::Type {
                    descriptor_projection: descriptors
                        .get(id)
                        .ok_or(ShapeLinkError::MissingSubject(self.subject))?,
                }
            }
            Subject::DispatchTable(id) => ShapeLinkContractV1::Dispatch {
                table_projection: dispatch
                    .get(id)
                    .ok_or(ShapeLinkError::MissingSubject(self.subject))?,
            },
            Subject::StaticStorage(_)
            | Subject::StaticStorageRegistration(_)
            | Subject::InitializationCell(_)
            | Subject::InitializationDescriptor(_) => return Ok(()),
        };
        if !super::super::wire::equal_fields(self.contract(), &expected)? {
            return Err(ShapeLinkError::Contract);
        }
        Ok(())
    }
}

pub(in crate::shape_link) fn semantic_target(
    subject: ExternalStrongShapeSubjectV1,
    layouts: &CanonicalExactLayoutExportsV1,
) -> Result<Option<crate::LayoutAbiSemanticTargetV1>, ShapeLinkError> {
    use crate::{ExternalStrongShapeSubjectV1 as Subject, LayoutAbiSemanticTargetV1 as Target};

    Ok(match subject {
        Subject::Callable(owner) => Some(Target::Callable(owner)),
        Subject::Layout(id) => Some(Target::Layout(id)),
        Subject::Scan(id) => Some(Target::Layout(
            layouts
                .records()
                .iter()
                .find(|record| record.scan() == id)
                .ok_or(ShapeLinkError::MissingSubject(subject))?
                .identity()
                .layout(),
        )),
        Subject::TypeDescriptor(id) | Subject::TypeRegistration(id) => Some(Target::Descriptor(id)),
        Subject::DispatchTable(id) => Some(Target::Dispatch(id)),
        Subject::StaticStorage(_)
        | Subject::StaticStorageRegistration(_)
        | Subject::InitializationCell(_)
        | Subject::InitializationDescriptor(_) => None,
    })
}
