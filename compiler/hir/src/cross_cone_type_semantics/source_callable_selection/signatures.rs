use super::*;
use crate::{
    CallableImplementationV1, CanonicalNominalInterfacesV1, NominalMaterializationClosure,
    SourceNominalId,
};
use scoop_identity::SignatureTypeKey;

pub(super) struct MaterializableSignatures<'a> {
    nominals: &'a CanonicalNominalInterfacesV1,
    closure: NominalMaterializationClosure,
}

impl<'a> MaterializableSignatures<'a> {
    pub(super) fn new(public: &'a CrossConeHirInterfaceSectionV1) -> Result<Self, Error> {
        Ok(Self {
            nominals: public.nominal_interfaces(),
            closure: NominalMaterializationClosure::from_declarations(
                public.nominal_interfaces(),
                public.callable_interfaces(),
            )
            .map_err(|error| match error {
                crate::NominalMaterializationClosureError::Resource(error) => {
                    Error::Resource(error)
                }
                error => Error::Materialization(error),
            })?,
        })
    }

    pub(super) fn callable(&self, source: &CallableDeclarationRecordV1) -> bool {
        if source.effects().implementation() != CallableImplementationV1::Scoop
            || !source.type_parameters().is_empty()
            || !self.owner(source.owner().nominal_owner())
        {
            return false;
        }
        for ty in source
            .receiver()
            .into_iter()
            .chain(
                source
                    .parameters()
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.value_type()),
            )
            .chain(std::iter::once(source.result()))
        {
            if !self.ty(ty) {
                return false;
            }
        }
        true
    }

    pub(super) fn owner(&self, owner: Option<SourceNominalId>) -> bool {
        match owner {
            Some(SourceNominalId::Concrete(source)) => {
                self.nominals
                    .declaration(SourceNominalId::Concrete(source))
                    .is_none()
                    || self.closure.contains(source)
            }
            Some(SourceNominalId::GenericTemplate(_)) => false,
            None => true,
        }
    }

    fn ty(&self, ty: &SignatureTypeKey) -> bool {
        match ty {
            SignatureTypeKey::Nominal(source) => {
                self.owner(Some(SourceNominalId::Concrete(*source)))
            }
            SignatureTypeKey::NominalApplication { arguments, .. } => arguments
                .as_slice()
                .iter()
                .all(|argument| self.ty(argument)),
            SignatureTypeKey::Binder { .. } => false,
            SignatureTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    if !self.ty(element) {
                        return false;
                    }
                }
                true
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    if !self.ty(parameter) {
                        return false;
                    }
                }
                self.ty(result)
            }
            SignatureTypeKey::RawPointer(pointee) => self.ty(pointee),
        }
    }
}
