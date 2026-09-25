use scoop_identity::{CallableTemplateOrigin, PersistentExactTypeId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{HirDependencyCallReasonV1, HirDependencyCallSiteV1};
use crate::{
    CallableDeclarationRecordV1, ExternalHirTargetV1, SharedTypeMetadataError,
    SharedTypeMetadataV1, SourceNominalId,
};

mod errors;

pub use errors::HirDependencyCallSignatureError;

impl HirDependencyCallSiteV1 {
    /// Joins this actual call with the provider's already checked declaration.
    /// Source access, execution roles and MIR implementation selection remain
    /// independent checks. The returned declaration has this call's complete
    /// logical signature, including every zero-sized argument.
    pub fn validate_source_signature<'a>(
        &self,
        target: ExternalHirTargetV1,
        metadata: SharedTypeMetadataV1<'a>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'a CallableDeclarationRecordV1, HirDependencyCallSignatureError> {
        use HirDependencyCallSignatureError as Error;
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        if !matches!(self.reason(), HirDependencyCallReasonV1::SourceBinding(_)) {
            return Err(Error::Reason);
        }
        let ExternalHirTargetV1::Callable(
            declaration @ (CallableTemplateOrigin::Function(_)
            | CallableTemplateOrigin::Accessor(_)
            | CallableTemplateOrigin::Constructor(_)
            | CallableTemplateOrigin::VariantConstructor(_)),
        ) = target
        else {
            return Err(Error::Target(target));
        };
        let callables = metadata.public.callable_interfaces();
        meter.charge_work(
            1 + u64::from(callables.declaration_count().max(1).ilog2()),
            path,
        )?;
        let source = callables
            .declaration(declaration)
            .ok_or(Error::Declaration(declaration))?;
        if !source.type_parameters().is_empty() {
            return Err(Error::GenericDeclaration(declaration));
        }
        let construction = matches!(
            declaration,
            CallableTemplateOrigin::Constructor(_) | CallableTemplateOrigin::VariantConstructor(_)
        );
        let receiver = match source.owner().nominal_owner() {
            Some(SourceNominalId::Concrete(_)) if construction => None,
            Some(SourceNominalId::Concrete(owner)) => {
                Some(metadata.signature_exact_type(&SignatureTypeKey::Nominal(owner), meter)?)
            }
            Some(SourceNominalId::GenericTemplate(_)) => {
                return Err(Error::GenericDeclaration(declaration));
            }
            None => source
                .receiver()
                .map(|ty| metadata.signature_exact_type(ty, meter))
                .transpose()?,
        };
        let parameters = source.parameters().parameters();
        let expected = parameters
            .len()
            .saturating_add(usize::from(receiver.is_some()));
        meter.check_table_entries(expected as u64, path)?;
        if self.arguments().len() != expected {
            return Err(Error::ArgumentCount {
                expected,
                actual: self.arguments().len(),
            });
        }
        if let Some(expected) = receiver {
            argument(0, self.arguments()[0], expected, meter, path)?;
        }
        let offset = usize::from(receiver.is_some());
        for (index, parameter) in parameters.iter().enumerate() {
            let expected = metadata.signature_exact_type(parameter.value_type(), meter)?;
            argument(
                index + offset,
                self.arguments()[index + offset],
                expected,
                meter,
                path,
            )?;
        }
        let expected = metadata.signature_exact_type(source.result(), meter)?;
        meter.charge_work(1, path)?;
        if self.result() != expected {
            return Err(Error::Result {
                expected,
                actual: self.result(),
            });
        }
        Ok(source)
    }
}

fn argument(
    index: usize,
    actual: PersistentExactTypeId,
    expected: PersistentExactTypeId,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), HirDependencyCallSignatureError> {
    meter.charge_work(1, path)?;
    if actual != expected {
        return Err(HirDependencyCallSignatureError::Argument {
            index,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
