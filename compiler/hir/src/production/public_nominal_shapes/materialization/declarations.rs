use super::*;
use crate::CallableImplementationV1;
use scoop_identity::{Effect, SignatureTypeKey};

impl Graph {
    pub(super) fn requirement(
        &mut self,
        requirement: NominalMaterializationRequirementV1<'_>,
    ) -> Result<(), NominalMaterializationClosureError> {
        use NominalMaterializationRequirementV1 as Requirement;
        let source = requirement.owner();
        let owner = self
            .position(SourceNominalId::Concrete(source))
            .ok_or(NominalMaterializationClosureError::MissingNominal(source))?;
        match requirement {
            Requirement::Field { field, .. } => self.require(owner, field.value_type())?,
            Requirement::EnumVariantField { field, .. } => {
                self.require(owner, field.value_type())?
            }
            Requirement::Inheritance { parent, .. } => {
                // The ordinary inheritance section describes unapplied parents.
                // Generic storage does not require a parent dispatch schema.
                if matches!(parent, SignatureTypeKey::NominalApplication { .. }) {
                    self.block(owner)?;
                } else {
                    self.require(owner, parent)?;
                }
            }
            Requirement::Constructor { callable, .. } | Requirement::Slot { callable, .. } => {
                if !callable.type_parameters().is_empty()
                    || callable.effects().execution() == Effect::Suspend
                    || matches!(
                        callable.effects().implementation(),
                        CallableImplementationV1::SourceExternScoop
                            | CallableImplementationV1::SourceExternC
                    )
                {
                    self.block(owner)?;
                }
                for parameter in callable.parameters().parameters() {
                    self.require(owner, parameter.value_type())?;
                }
                self.require(owner, callable.result())?;
            }
        }
        Ok(())
    }

    fn require(&mut self, owner: usize, ty: &SignatureTypeKey) -> Result<(), WireError> {
        match ty {
            SignatureTypeKey::Nominal(source) => {
                if let Some(dependency) = self.position(SourceNominalId::Concrete(*source)) {
                    self.edge(dependency, owner)?;
                }
            }
            SignatureTypeKey::NominalApplication { arguments, .. } => {
                for argument in arguments.as_slice() {
                    self.require(owner, argument)?;
                }
            }
            SignatureTypeKey::Binder { .. } => self.block(owner)?,
            SignatureTypeKey::Tuple(elements) => {
                for element in elements.as_slice() {
                    self.require(owner, element)?;
                }
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                for parameter in parameters {
                    self.require(owner, parameter)?;
                }
                self.require(owner, result)?;
            }
            SignatureTypeKey::RawPointer(pointee) => self.require(owner, pointee)?,
        }
        Ok(())
    }
}
