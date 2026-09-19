use scoop_identity::SignatureTypeKey;
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::OptionalTemplateReceiverV1;
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    DefaultTemplateProviderShapeV1, MeteredDefaultTemplateTypeSubstitutionError,
    compare_default_signature_reference_targets,
};

impl OptionalTemplateReceiverV1 {
    /// Checks a receiver against a separately established callable contract.
    pub fn validate_expected_semantics_metered(
        &self,
        expected: Option<&SignatureTypeKey>,
        locals: &CanonicalTemplateLocalTableV1,
        provider: DefaultTemplateProviderShapeV1,
        mapping: &CanonicalBinderUseListV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), MeteredTemplateReceiverSemanticValidationError> {
        use MeteredTemplateReceiverSemanticValidationError as Error;
        meter.charge_work(1, path).map_err(Error::Resource)?;
        let (receiver, expected) = match (self.receiver(), expected) {
            (None, None) => return Ok(()),
            (None, Some(_)) => return Err(Error::Missing),
            (Some(_), None) => return Err(Error::Unexpected),
            (Some(receiver), Some(expected)) => (receiver, expected),
        };
        let comparisons = u64::from(u32::BITS - locals.len_u32().leading_zeros()) + 1;
        meter
            .charge_work(comparisons, path)
            .map_err(Error::Resource)?;
        let local = locals.get(receiver.local()).ok_or(Error::MissingLocal)?;
        if local.mutable() != CanonicalBooleanV1::False {
            return Err(Error::MutableLocal);
        }
        if !compare_default_signature_reference_targets(
            local.value_type(),
            receiver.value_type(),
            meter,
            path,
        )
        .map_err(Error::Resource)?
        .is_eq()
        {
            return Err(Error::LocalType);
        }
        let mapped = mapping
            .substitute_provider_type_metered(provider, receiver.value_type(), meter, path)
            .map_err(Error::Substitution)?;
        if !compare_default_signature_reference_targets(&mapped, expected, meter, path)
            .map_err(Error::Resource)?
            .is_eq()
        {
            return Err(Error::CallableType);
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum MeteredTemplateReceiverSemanticValidationError {
    Resource(WireError),
    Missing,
    Unexpected,
    MissingLocal,
    MutableLocal,
    LocalType,
    Substitution(MeteredDefaultTemplateTypeSubstitutionError),
    CallableType,
}
impl std::fmt::Display for MeteredTemplateReceiverSemanticValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Substitution(error) => error.fmt(f),
            Self::Missing => f.write_str("default template is missing its source receiver"),
            Self::Unexpected => {
                f.write_str("default template has a receiver forbidden by its source contract")
            }
            Self::MissingLocal => f.write_str("default receiver local is absent"),
            Self::MutableLocal => f.write_str("default receiver local is mutable"),
            Self::LocalType => f.write_str("default receiver type differs from its local"),
            Self::CallableType => {
                f.write_str("mapped default receiver type differs from its source owner")
            }
        }
    }
}
impl std::error::Error for MeteredTemplateReceiverSemanticValidationError {}
