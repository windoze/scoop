use super::*;
use crate::{NominalSupportPropertyPayloadV1, ProtectedPropertyMutabilityV1, SourceNominalId};
use scoop_identity::CallableTemplateOrigin;
use std::collections::BTreeSet;

impl ProtectedNestedSourceInterfaceV1 {
    pub(super) fn validate_reference_closure(
        &self,
        owner: SourceNominalId,
    ) -> Result<(), NestedSourceBuildError> {
        use NestedSupportDeclarationV1 as Key;
        let mut expected = BTreeSet::new();
        let mut add = |key| {
            if !expected.insert(scoop_wire::encode(&key).map_err(NestedSourceBuildError::Encoding)?)
            {
                return Err(NestedSourceBuildError::ReferenceClosure);
            }
            Ok(())
        };
        for constructor in self.constructors.values() {
            add(Key::Constructor(*constructor))?;
        }
        for member in self.members.values() {
            add(match member {
                NestedSourceMemberRefV1::Function(id) => {
                    Key::Callable(CallableTemplateOrigin::Function(*id))
                }
                NestedSourceMemberRefV1::GenericFunction(id) => {
                    Key::Callable(CallableTemplateOrigin::GenericFunction(*id))
                }
                NestedSourceMemberRefV1::Property(id) => Key::Property(*id),
            })?;
        }
        for child in self.children.values() {
            add(Key::NestedNominal(*child))?;
        }
        if let NominalSourceShapeV1::Enum(shape) = &self.source_shape {
            for variant in shape.variants() {
                add(Key::Callable(CallableTemplateOrigin::VariantConstructor(
                    variant.variant(),
                )))?;
            }
        }
        for record in self.source_support.records() {
            if record.declaration_access().lexical_owners().last().copied() != Some(owner) {
                return Err(NestedSourceBuildError::Owner);
            }
            if let NestedSourceSupportV1::Property(record) = record {
                if let NominalSupportPropertyPayloadV1::Runtime { interface } = record.payload() {
                    add(Key::Callable(CallableTemplateOrigin::Accessor(
                        interface.getter(),
                    )))?;
                    if let ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } =
                        interface.mutability()
                    {
                        add(Key::Callable(CallableTemplateOrigin::Accessor(*setter)))?;
                    }
                }
            }
        }
        for record in self.source_support.records() {
            if !expected.remove(
                &scoop_wire::encode(&record.declaration())
                    .map_err(NestedSourceBuildError::Encoding)?,
            ) {
                return Err(NestedSourceBuildError::ReferenceClosure);
            }
        }
        if !expected.is_empty() {
            return Err(NestedSourceBuildError::ReferenceClosure);
        }
        Ok(())
    }
}
