use scoop_identity::SignatureTypeKey;
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::CanonicalTemplateValueParametersV1;
use crate::{
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalTemplateLocalTableV1,
    DefaultTemplateProviderShapeV1, MeteredDefaultTemplateTypeSubstitutionError,
    compare_default_signature_reference_targets,
};

impl CanonicalTemplateValueParametersV1 {
    /// Checks exactly the preceding source parameter types without constructing
    /// a callable/source-interface adapter or copying the expected type table.
    pub fn validate_prefix_types_metered<'a>(
        &self,
        expected: impl ExactSizeIterator<Item = &'a SignatureTypeKey>,
        locals: &CanonicalTemplateLocalTableV1,
        provider: DefaultTemplateProviderShapeV1,
        mapping: &CanonicalBinderUseListV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), MeteredTemplateValueParameterSemanticValidationError> {
        self.validate_prefix_with(
            expected,
            locals,
            meter,
            path,
            |value, position, meter, path| {
                mapping
                    .substitute_provider_type_metered(provider, value, meter, path)
                    .map(std::borrow::Cow::Owned)
                    .map_err(|error| {
                        MeteredTemplateValueParameterSemanticValidationError::Substitution {
                            position,
                            error,
                        }
                    })
            },
        )
    }

    /// Compares raw source locals before inherited-owner substitution.
    pub fn validate_provider_prefix_types_metered<'a>(
        &self,
        expected: impl ExactSizeIterator<Item = &'a SignatureTypeKey>,
        locals: &CanonicalTemplateLocalTableV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), MeteredTemplateValueParameterSemanticValidationError> {
        self.validate_prefix_with(expected, locals, meter, path, |value, _, _, _| {
            Ok(std::borrow::Cow::Borrowed(value))
        })
    }

    fn validate_prefix_with<'a, 'l>(
        &self,
        expected: impl ExactSizeIterator<Item = &'a SignatureTypeKey>,
        locals: &'l CanonicalTemplateLocalTableV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
        mut map: impl FnMut(
            &'l SignatureTypeKey,
            u32,
            &mut BudgetMeter,
            &WirePath,
        ) -> Result<
            std::borrow::Cow<'l, SignatureTypeKey>,
            MeteredTemplateValueParameterSemanticValidationError,
        >,
    ) -> Result<(), MeteredTemplateValueParameterSemanticValidationError> {
        use MeteredTemplateValueParameterSemanticValidationError as Error;
        if self.parameters().len() != expected.len() {
            return Err(Error::PrefixArity {
                expected: expected.len(),
                actual: self.len_u32(),
            });
        }
        for (parameter, expected) in self.parameters().iter().zip(expected) {
            let position = parameter.position();
            let comparisons = u64::from(u32::BITS - locals.len_u32().leading_zeros()) + 1;
            meter
                .charge_work(comparisons, path)
                .map_err(Error::Resource)?;
            let local = locals
                .get(parameter.local())
                .ok_or(Error::MissingLocal { position })?;
            if local.mutable() != CanonicalBooleanV1::False {
                return Err(Error::MutableLocal { position });
            }
            let mapped = map(local.value_type(), position, meter, path)?;
            if !compare_default_signature_reference_targets(mapped.as_ref(), expected, meter, path)
                .map_err(Error::Resource)?
                .is_eq()
            {
                return Err(Error::LocalType { position });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum MeteredTemplateValueParameterSemanticValidationError {
    Resource(WireError),
    PrefixArity {
        expected: usize,
        actual: u32,
    },
    MissingLocal {
        position: u32,
    },
    MutableLocal {
        position: u32,
    },
    LocalType {
        position: u32,
    },
    Substitution {
        position: u32,
        error: MeteredDefaultTemplateTypeSubstitutionError,
    },
}
impl std::fmt::Display for MeteredTemplateValueParameterSemanticValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::PrefixArity { expected, actual } => write!(
                f,
                "default template has {actual} preceding parameters, expected {expected}"
            ),
            Self::MissingLocal { position } => {
                write!(f, "default parameter {position} local is absent")
            }
            Self::MutableLocal { position } => {
                write!(f, "default parameter {position} local is mutable")
            }
            Self::LocalType { position } => write!(
                f,
                "mapped default parameter {position} local has a different source type"
            ),
            Self::Substitution { position, error } => write!(
                f,
                "default parameter {position} substitution failed: {error}"
            ),
        }
    }
}
impl std::error::Error for MeteredTemplateValueParameterSemanticValidationError {}
