use scoop_identity::{CallableTemplateOrigin, PersistentExactTypeId, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{HirDependencyCallReasonV1, HirDependencyCallSiteV1};
use crate::{ExternalHirTargetV1, SharedTypeMetadataError, SharedTypeMetadataV1, SourceNominalId};

impl HirDependencyCallSiteV1 {
    /// Joins this actual call with the provider's already checked declaration.
    /// Source access, execution roles and MIR implementation selection remain
    /// independent checks; this method establishes the full logical signature.
    pub fn validate_source_signature(
        &self,
        target: ExternalHirTargetV1,
        metadata: SharedTypeMetadataV1<'_>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), HirDependencyCallSignatureError> {
        use HirDependencyCallSignatureError as Error;
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        if !matches!(self.reason(), HirDependencyCallReasonV1::SourceBinding(_)) {
            return Err(Error::Reason);
        }
        let ExternalHirTargetV1::Callable(
            declaration @ (CallableTemplateOrigin::Function(_)
            | CallableTemplateOrigin::Accessor(_)),
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
        let receiver = match source.owner().nominal_owner() {
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
        Ok(())
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

#[derive(Debug)]
pub enum HirDependencyCallSignatureError {
    Resource(WireError),
    Type(Box<SharedTypeMetadataError>),
    Reason,
    Target(ExternalHirTargetV1),
    Declaration(CallableTemplateOrigin),
    GenericDeclaration(CallableTemplateOrigin),
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    Argument {
        index: usize,
        expected: PersistentExactTypeId,
        actual: PersistentExactTypeId,
    },
    Result {
        expected: PersistentExactTypeId,
        actual: PersistentExactTypeId,
    },
}

impl From<WireError> for HirDependencyCallSignatureError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl From<SharedTypeMetadataError> for HirDependencyCallSignatureError {
    fn from(error: SharedTypeMetadataError) -> Self {
        Self::Type(Box::new(error))
    }
}

impl std::fmt::Display for HirDependencyCallSignatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Type(error) => error.fmt(f),
            Self::Reason => {
                f.write_str("source signature check requires an actual source-binding call")
            }
            Self::Target(target) => write!(
                f,
                "source call target {target:?} is not a function or accessor"
            ),
            Self::Declaration(target) => write!(
                f,
                "source call target {target:?} has no provider declaration"
            ),
            Self::GenericDeclaration(target) => write!(
                f,
                "source call target {target:?} has unresolved type parameters"
            ),
            Self::ArgumentCount { expected, actual } => write!(
                f,
                "source call has {actual} logical arguments, expected {expected}"
            ),
            Self::Argument {
                index,
                expected,
                actual,
            } => write!(
                f,
                "source call argument {index} has exact type {actual}, expected {expected}"
            ),
            Self::Result { expected, actual } => write!(
                f,
                "source call has result exact type {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for HirDependencyCallSignatureError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Type(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
