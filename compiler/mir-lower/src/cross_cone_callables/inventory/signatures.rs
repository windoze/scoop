//! Source-only nominal dependencies do not demand absent machine bodies.

use super::*;
use scoop_identity::SignatureTypeKey;

pub(super) struct MaterializableSignatures<'a> {
    nominals: &'a hir::CanonicalNominalInterfacesV1,
    closure: hir::NominalMaterializationClosure,
}

impl<'a> MaterializableSignatures<'a> {
    pub(super) fn new(
        public: &'a hir::CrossConeHirInterfaceSectionV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Ok(Self {
            nominals: public.nominal_interfaces(),
            closure: hir::NominalMaterializationClosure::from_declarations(
                public.nominal_interfaces(),
                public.callable_interfaces(),
                meter,
            )
            .map_err(|error| match error {
                hir::NominalMaterializationClosureError::Resource(error) => Error::Resource(error),
                other => Error::Materialization(other),
            })?,
        })
    }

    pub(super) fn owner(
        &self,
        owner: Option<hir::SourceNominalId>,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        work(
            u64::from(self.nominals.declaration_count().max(1).ilog2())
                + u64::from(self.closure.sources().len().max(1).ilog2())
                + 2,
            meter,
        )?;
        Ok(match owner {
            Some(hir::SourceNominalId::Concrete(source)) => {
                self.nominals
                    .declaration(hir::SourceNominalId::Concrete(source))
                    .is_none()
                    || self.closure.contains(source)
            }
            Some(hir::SourceNominalId::GenericTemplate(_)) => false,
            None => true,
        })
    }

    pub(super) fn all<'s>(
        &self,
        types: impl IntoIterator<Item = &'s SignatureTypeKey>,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        for ty in types {
            if !self.ty(ty, 1, meter)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn ty(
        &self,
        ty: &SignatureTypeKey,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<bool, Error> {
        meter.check_semantic_depth(depth, &WirePath::root())?;
        meter.charge_nodes(1, &WirePath::root())?;
        work(1, meter)?;
        match ty {
            SignatureTypeKey::Nominal(source) => {
                self.owner(Some(hir::SourceNominalId::Concrete(*source)), meter)
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
