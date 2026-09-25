use super::*;
use crate::{
    CallableModalityV1, CheckedNominalSupportCallableSourceV1,
    ProtectedPropertyAccessorClosureError,
};
use scoop_identity::{CallableTemplateOrigin, PersistentPropertyAccessorId};

impl CheckedNominalSupportRuntimePropertySourceV1<'_> {
    pub fn validate_accessor_contracts(
        &self,
        getter: CheckedNominalSupportCallableSourceV1<'_>,
        setter: Option<CheckedNominalSupportCallableSourceV1<'_>>,
    ) -> Result<(), ProtectedPropertyAccessorClosureError> {
        self.validate_accessor_views(getter.into(), setter.map(Into::into))
    }

    pub(in crate::cross_cone_type_semantics) fn validate_resolved_accessor_records(
        &self,
        getter: &crate::NominalSupportCallableInterfaceV1,
        setter: Option<&crate::NominalSupportCallableInterfaceV1>,
    ) -> Result<(), ProtectedPropertyAccessorClosureError> {
        self.validate_accessor_views(getter.into(), setter.map(Into::into))
    }

    fn validate_accessor_views(
        &self,
        getter: AccessorSourceView<'_>,
        setter: Option<AccessorSourceView<'_>>,
    ) -> Result<(), ProtectedPropertyAccessorClosureError> {
        use ProtectedPropertyAccessorClosureError as Error;

        self.check_accessor(getter, self.payload.getter(), self.access.source())?;
        if !getter.payload.parameters().is_empty()
            || getter.payload.result() != self.payload.value_type()
        {
            return Err(Error::Signature);
        }
        match (self.payload.mutability(), setter) {
            (ProtectedPropertyMutabilityV1::ReadOnly, None) => Ok(()),
            (
                ProtectedPropertyMutabilityV1::ReadWrite {
                    setter: expected,
                    setter_access,
                },
                Some(setter),
            ) => {
                self.check_accessor(setter, *expected, setter_access)?;
                let parameters = setter.payload.parameters().parameters();
                if parameters.len() != 1 || parameters[0].value_type() != self.payload.value_type()
                {
                    return Err(Error::Signature);
                }
                Ok(())
            }
            _ => Err(Error::Setter),
        }
    }
    fn check_accessor(
        &self,
        accessor: AccessorSourceView<'_>,
        expected: PersistentPropertyAccessorId,
        source: &DeclarationAccessSourceV1,
    ) -> Result<(), ProtectedPropertyAccessorClosureError> {
        use ProtectedPropertyAccessorClosureError as Error;
        if accessor.declaration != CallableTemplateOrigin::Accessor(expected) {
            return Err(Error::Getter);
        }
        let actual = accessor.access;

        if accessor.payload.owner() != self.payload.owner()
            || actual.declared_visibility() != source.declared_visibility()
            || actual.lexical_owners() != source.lexical_owners()
            || actual.definition_origin().origin().source()
                != source.definition_origin().origin().source()
        {
            return Err(Error::Access);
        }
        let abstract_slot = self.payload.representation() == PropertyRepresentationV1::AbstractSlot;
        if (abstract_slot && accessor.payload.modality() != CallableModalityV1::Abstract)
            || (!abstract_slot
                && self.kind != SourceDeclarationKind::Interface
                && accessor.payload.modality() == CallableModalityV1::Abstract)
        {
            return Err(Error::Representation);
        }

        for slot in accessor.payload.slot_relations().slots() {
            if self
                .payload
                .slot_relations()
                .slots()
                .binary_search(slot)
                .is_err()
            {
                return Err(Error::Slot);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct AccessorSourceView<'a> {
    declaration: CallableTemplateOrigin,
    payload: &'a crate::NominalSourceCallablePayloadV1,
    access: &'a DeclarationAccessSourceV1,
}
impl<'a> From<CheckedNominalSupportCallableSourceV1<'a>> for AccessorSourceView<'a> {
    fn from(value: CheckedNominalSupportCallableSourceV1<'a>) -> Self {
        Self {
            declaration: value.declaration(),
            payload: value.payload(),
            access: value.declaration_access().source(),
        }
    }
}
impl<'a> From<&'a crate::NominalSupportCallableInterfaceV1> for AccessorSourceView<'a> {
    fn from(value: &'a crate::NominalSupportCallableInterfaceV1) -> Self {
        Self {
            declaration: value.declaration(),
            payload: value.payload(),
            access: value.declaration_access(),
        }
    }
}
