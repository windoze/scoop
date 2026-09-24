use super::*;
use crate::CallableImplementationV1;
use scoop_identity::{Effect, SignatureTypeKey};

impl Graph {
    pub(super) fn requirement(
        &mut self,
        requirement: NominalMaterializationRequirementV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<(), NominalMaterializationClosureError> {
        use NominalMaterializationRequirementV1 as Requirement;
        let source = requirement.owner();
        let owner = self
            .position(SourceNominalId::Concrete(source), meter)?
            .ok_or(NominalMaterializationClosureError::MissingNominal(source))?;
        match requirement {
            Requirement::Field { field, .. } => {
                self.require(owner, field.value_type(), 1, meter)?
            }
            Requirement::EnumVariantField { field, .. } => {
                self.require(owner, field.value_type(), 1, meter)?
            }
            Requirement::Inheritance { parent, .. } => self.require(owner, parent, 1, meter)?,
            Requirement::Constructor { callable, .. } | Requirement::Slot { callable, .. } => {
                if !callable.type_parameters().is_empty()
                    || callable.effects().execution() == Effect::Suspend
                    || matches!(
                        callable.effects().implementation(),
                        CallableImplementationV1::SourceExternScoop
                            | CallableImplementationV1::SourceExternC
                    )
                {
                    self.block(owner, meter)?;
                }
                for parameter in callable.parameters().parameters() {
                    self.require(owner, parameter.value_type(), 1, meter)?;
                }
                self.require(owner, callable.result(), 1, meter)?;
            }
        }
        Ok(())
    }

    fn require(
        &mut self,
        owner: usize,
        ty: &SignatureTypeKey,
        depth: u64,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        let path = WirePath::root();
        meter.check_semantic_depth(depth, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        match ty {
            SignatureTypeKey::Nominal(source) => {
                if let Some(dependency) =
                    self.position(SourceNominalId::Concrete(*source), meter)?
                {
                    self.edge(dependency, owner, meter)?;
                }
            }
            SignatureTypeKey::NominalApplication { .. } | SignatureTypeKey::Binder { .. } => {
                self.block(owner, meter)?;
            }
            SignatureTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    self.require(owner, element, depth + 1, meter)?;
                }
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    self.require(owner, parameter, depth + 1, meter)?;
                }
                self.require(owner, result, depth + 1, meter)?;
            }
            SignatureTypeKey::RawPointer(pointee) => {
                self.require(owner, pointee, depth + 1, meter)?
            }
        }
        Ok(())
    }
}
