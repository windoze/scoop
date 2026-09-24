use super::*;
use crate::{
    CallableImplementationV1, CanonicalNominalInterfacesV1, NominalMaterializationClosure,
    SourceNominalId,
};
use scoop_identity::{Effect, SignatureTypeKey};

pub(super) struct MaterializableSignatures<'a> {
    nominals: &'a CanonicalNominalInterfacesV1,
    closure: NominalMaterializationClosure,
}

impl<'a> MaterializableSignatures<'a> {
    pub(super) fn new(
        public: &'a CrossConeHirInterfaceSectionV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Ok(Self {
            nominals: public.nominal_interfaces(),
            closure: NominalMaterializationClosure::from_declarations(
                public.nominal_interfaces(),
                public.callable_interfaces(),
                meter,
            )
            .map_err(|error| match error {
                crate::NominalMaterializationClosureError::Resource(error) => {
                    Error::Resource(error)
                }
                error => Error::Materialization(error),
            })?,
        })
    }

    pub(super) fn callable(
        &self,
        source: &CallableDeclarationRecordV1,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        meter.charge_work(
            scoop_wire::encoded_length(source).map_err(|error| Error::Key(error.to_string()))?,
            &WirePath::root(),
        )?;
        if source.effects().implementation() != CallableImplementationV1::Scoop
            || source.effects().execution() != Effect::Ordinary
            || !source.type_parameters().is_empty()
            || !self.owner(source.owner().nominal_owner(), meter)?
        {
            return Ok(false);
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
            if !self.ty(ty, 1, meter)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn owner(
        &self,
        owner: Option<SourceNominalId>,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        lookup(self.nominals.declaration_count(), meter)?;
        lookup(self.closure.sources().len(), meter)?;
        Ok(match owner {
            Some(SourceNominalId::Concrete(source)) => {
                self.nominals
                    .declaration(SourceNominalId::Concrete(source))
                    .is_none()
                    || self.closure.contains(source)
            }
            Some(SourceNominalId::GenericTemplate(_)) => false,
            None => true,
        })
    }

    fn ty(
        &self,
        ty: &SignatureTypeKey,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        meter.check_semantic_depth(depth, &WirePath::root())?;
        meter.charge_nodes(1, &WirePath::root())?;
        meter.charge_work(1, &WirePath::root())?;
        match ty {
            SignatureTypeKey::Nominal(source) => {
                self.owner(Some(SourceNominalId::Concrete(*source)), meter)
            }
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                Ok(false)
            }
            SignatureTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    if !self.ty(element, depth + 1, meter)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    if !self.ty(parameter, depth + 1, meter)? {
                        return Ok(false);
                    }
                }
                self.ty(result, depth + 1, meter)
            }
            SignatureTypeKey::RawPointer(pointee) => self.ty(pointee, depth + 1, meter),
        }
    }
}
