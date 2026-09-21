//! Match canonical type records to typed occurrences, preserving every scope.
use super::*;
use crate::{
    DefaultBodyReferenceAttachmentV1, DefaultBodyReferenceMetadataV1,
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1, DefaultBodyReferenceVisitorV1,
    DefaultExpressionV1, SignatureBinderScopeV1, compare_default_signature_reference_targets,
};

pub(super) fn validate<A: DefaultReferenceSemanticAuthority<E>, E>(
    validator: &mut ReferenceSetValidator<'_, A, E>,
    target: &SignatureTypeKey,
    origin: &ExportDefinitionSourceV1,
) -> Result<(), ExportDefaultReferenceValidationError<E>> {
    let mut visitor = ScopedType {
        target,
        origin,
        provider: &validator.scope,
        authority: validator.authority,
        matched: false,
        error: PhantomData,
    };
    validator.template.body().visit_direct_references(
        validator.template.locals(),
        validator.template.definition_origin(),
        &mut visitor,
        validator.meter,
        validator.path,
    )?;
    // A standalone record still has to be valid. The exact-closure pass rejects
    // records with no occurrence; it cannot lend a local frame to another record.
    if !visitor.matched {
        validator.validate_type(
            target,
            ExportDefaultReferenceTargetTypeSiteV1::TypeTarget,
            origin,
        )?;
    }
    Ok(())
}

struct ScopedType<'a, A, E> {
    target: &'a SignatureTypeKey,
    origin: &'a ExportDefinitionSourceV1,
    provider: &'a SignatureBinderScopeV1,
    authority: &'a mut A,
    matched: bool,
    error: PhantomData<fn() -> E>,
}

impl<'body, A: DefaultReferenceSemanticAuthority<E>, E> DefaultBodyReferenceVisitorV1<'body>
    for ScopedType<'_, A, E>
{
    type Error = ExportDefaultReferenceValidationError<E>;

    fn expression(
        &mut self,
        _: u32,
        _: &'body DefaultExpressionV1,
        _: &mut BudgetMeter,
        _: &WirePath,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Self::Error> {
        let DefaultBodyReferenceTargetV1::Type(target) = occurrence.target else {
            return Ok(());
        };
        if !compare_default_signature_reference_targets(self.target, target, meter, path)?.is_eq() {
            return Ok(());
        }
        let source_length = |origin: &ExportDefinitionSourceV1| {
            origin.origin().source().logical_path().as_str().len() as u64
        };
        meter.charge_work(
            source_length(self.origin)
                .saturating_add(source_length(occurrence.definition_origin))
                .saturating_add(256),
            path,
        )?;
        if self.origin != occurrence.definition_origin {
            return Ok(());
        }
        self.matched = true;
        let local_scope;
        let scope = match occurrence.attachment {
            DefaultBodyReferenceAttachmentV1::Metadata(
                DefaultBodyReferenceMetadataV1::LocalFunction(function),
            ) => {
                let arity = self
                    .authority
                    .default_local_function_own_binder_arity(function.declaration(), meter, path)
                    .map_err(Self::Error::Target)?;
                local_scope = self.provider.with_inner_frame(arity, meter, path)?;
                &local_scope
            }
            _ => self.provider,
        };
        match scope.validate_signature_semantics_metered(target, self.authority, meter, path) {
            Ok(()) => Ok(()),
            Err(MeteredSignatureTypeSemanticError::Semantic(error)) => Err(Self::Error::Type {
                site: ExportDefaultReferenceTargetTypeSiteV1::TypeTarget,
                definition_origin: Box::new(self.origin.clone()),
                error: Box::new(error),
            }),
            Err(MeteredSignatureTypeSemanticError::Resource(error)) => {
                Err(Self::Error::Resource(error))
            }
        }
    }
}
