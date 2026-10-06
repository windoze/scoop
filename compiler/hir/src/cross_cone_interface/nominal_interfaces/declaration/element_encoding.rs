use super::*;
use scoop_identity::{DecodedSignatureTypeKey, SignatureTypeKey};

pub(super) mod wire;

/// The checked element obligation and ordinary dispatch choices of a core container.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalElementEncodingV1 {
    element: SignatureTypeKey,
    interface: SignatureTypeKey,
    selections: CanonicalNominalDispatchSelectionsV1,
}

impl NominalElementEncodingV1 {
    pub(in super::super) fn validate(
        &self,
        kind: PublicNominalKindV1,
        parameters: usize,
    ) -> Result<(), NominalInterfaceRecordBuildError> {
        if !matches!(kind, PublicNominalKindV1::Class | PublicNominalKindV1::Enum)
            || parameters != 1
            || self.element != (SignatureTypeKey::Binder { depth: 0, index: 0 })
            || !matches!(self.interface, SignatureTypeKey::Nominal(_))
            || self.selections.records().len() != 1
            || !matches!(self.selections.records()[0].role(), crate::NominalDispatchSelectionRoleV1::Interface { interface } if *interface == self.interface)
        {
            return Err(NominalInterfaceRecordBuildError::ElementEncoding);
        }
        Ok(())
    }

    pub const fn new(
        element: SignatureTypeKey,
        interface: SignatureTypeKey,
        selections: CanonicalNominalDispatchSelectionsV1,
    ) -> Self {
        Self {
            element,
            interface,
            selections,
        }
    }

    pub const fn element(&self) -> &SignatureTypeKey {
        &self.element
    }

    pub const fn interface(&self) -> &SignatureTypeKey {
        &self.interface
    }

    pub const fn selections(&self) -> &CanonicalNominalDispatchSelectionsV1 {
        &self.selections
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalElementEncodingV1 {
    element: DecodedSignatureTypeKey,
    interface: DecodedSignatureTypeKey,
    selections: DecodedCanonicalNominalDispatchSelectionsV1,
}

impl DecodedNominalElementEncodingV1 {
    pub fn resolve<R: NominalInterfaceRecordResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalElementEncodingV1, NominalDeclarationDetailsResolutionError<E>> {
        use NominalDeclarationDetailsResolutionError as Error;
        Ok(NominalElementEncodingV1::new(
            self.element.resolve(resolver).map_err(Error::Reference)?,
            self.interface.resolve(resolver).map_err(Error::Reference)?,
            self.selections
                .resolve(resolver)
                .map_err(Error::DispatchSelections)?,
        ))
    }
}
