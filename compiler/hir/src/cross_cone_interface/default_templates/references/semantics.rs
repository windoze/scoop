use std::marker::PhantomData;

mod errors;
mod scoped_types;
pub use errors::*;

use scoop_identity::{
    CallableTemplateOrigin, OptionalSignatureType, PersistentObjectValueId, PersistentPropertyId,
    SignatureTypeKey,
};
use scoop_wire::{WireError, WirePath};

use super::{
    ExportDefaultCallableTargetV1, ExportDefaultReferenceKindV1, ExportDefaultReferenceSetV1,
    ExportDefaultReferenceV1,
};
use crate::{
    CallableInterfaceRecordV1, DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1,
    DefaultCallableRefV1, DefaultConstructorRefV1, DefaultFieldRefV1,
    DefaultTemplateProviderShapeV1, ExportDefaultCallDomainV1, ExportDefaultTemplateV1,
    ExportDefinitionSourceSemanticAuthority, ExportDefinitionSourceSemanticValidationError,
    ExportDefinitionSourceV1, NominalInterfaceShapeAuthority, PublicLookupAccessV1,
    SignatureBinderScopeError, SignatureTypeSemanticError,
};

/// Supplies already validated public-surface and lexical-identity facts for
/// direct references captured by one exported default template.
///
/// Every successful method must select the exact kind-specific target passed
/// to it. For an independently declared target, success proves a universal
/// public interface plus any required `DefaultDependency` route. For a
/// template-owned lexical target, success proves the exact template root,
/// path, and generated role. Implementations must use canonical typed
/// identities and previously validated interface data, never display names,
/// FQNs, link symbols, or an artifact-wide entity scan.
pub trait DefaultReferenceSemanticAuthority<E>:
    NominalInterfaceShapeAuthority<E>
    + ExportDefinitionSourceSemanticAuthority<E>
    + crate::DefaultLocalFunctionSignatureAuthority<E>
{
    fn validate_default_callable_reference_target(
        &mut self,
        template: &ExportDefaultTemplateV1,
        target: &ExportDefaultCallableTargetV1,
    ) -> Result<(), E>;

    fn validate_default_constructor_reference_target(
        &mut self,
        template: &ExportDefaultTemplateV1,
        target: &DefaultConstructorRefV1,
    ) -> Result<(), E>;

    fn validate_default_type_reference_target(
        &mut self,
        template: &ExportDefaultTemplateV1,
        target: &SignatureTypeKey,
    ) -> Result<(), E>;

    fn validate_default_global_reference_target(
        &mut self,
        template: &ExportDefaultTemplateV1,
        target: PersistentPropertyId,
    ) -> Result<(), E>;

    fn validate_default_singleton_reference_target(
        &mut self,
        template: &ExportDefaultTemplateV1,
        target: PersistentObjectValueId,
    ) -> Result<(), E>;

    fn validate_default_field_reference_target(
        &mut self,
        template: &ExportDefaultTemplateV1,
        target: &DefaultFieldRefV1,
    ) -> Result<(), E>;
}

impl ExportDefaultTemplateV1 {
    /// Validates the declared reference records without reconstructing the
    /// body-to-reference exact closure.
    ///
    /// The caller supplies the provider shape checked with the template contract.
    /// Local declaration signatures use their typed body attachment and binder
    /// arity; other occurrences use the provider scope. Reference coverage is
    /// checked separately.
    pub fn validate_reference_envelope_semantics<A, E>(
        &self,
        owner_interface: &CallableInterfaceRecordV1,
        provider: DefaultTemplateProviderShapeV1,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), ExportDefaultReferenceSetSemanticValidationError<E>>
    where
        A: DefaultReferenceSemanticAuthority<E>,
    {
        let expected_owner = self.key().owner();
        let actual_owner = owner_interface.declaration();
        if actual_owner != expected_owner {
            return Err(
                ExportDefaultReferenceSetSemanticValidationError::OwnerInterface {
                    expected: expected_owner,
                    actual: actual_owner,
                },
            );
        }

        ReferenceSetValidator {
            template: self,
            expected_call_domain: call_domain(owner_interface.access()),
            scope: provider.signature_scope(),
            authority,

            path,
            error: PhantomData,
        }
        .validate(self.references())
    }
}

struct ReferenceSetValidator<'a, A, E> {
    template: &'a ExportDefaultTemplateV1,
    expected_call_domain: ExportDefaultCallDomainV1,
    scope: crate::SignatureBinderScopeV1,
    authority: &'a mut A,

    path: &'a WirePath,
    error: PhantomData<fn() -> E>,
}

impl<A, E> ReferenceSetValidator<'_, A, E>
where
    A: DefaultReferenceSemanticAuthority<E>,
{
    fn validate(
        &mut self,
        references: &ExportDefaultReferenceSetV1,
    ) -> Result<(), ExportDefaultReferenceSetSemanticValidationError<E>> {
        for (index, reference) in references.callables().iter().enumerate() {
            self.validate_record(
                ExportDefaultReferenceKindV1::Callable,
                index,
                reference,
                |validator, target, origin| validator.validate_callable(target, origin),
            )?;
        }
        for (index, reference) in references.constructors().iter().enumerate() {
            self.validate_record(
                ExportDefaultReferenceKindV1::Constructor,
                index,
                reference,
                |validator, target, origin| validator.validate_constructor(target, origin),
            )?;
        }
        for (index, reference) in references.types().iter().enumerate() {
            self.validate_record(
                ExportDefaultReferenceKindV1::Type,
                index,
                reference,
                |validator, target, origin| validator.validate_type_target(target, origin),
            )?;
        }
        for (index, reference) in references.globals().iter().enumerate() {
            self.validate_record(
                ExportDefaultReferenceKindV1::Global,
                index,
                reference,
                |validator, target, _| {
                    validator
                        .authority
                        .validate_default_global_reference_target(validator.template, *target)
                        .map_err(ExportDefaultReferenceValidationError::Target)
                },
            )?;
        }
        for (index, reference) in references.singleton_values().iter().enumerate() {
            self.validate_record(
                ExportDefaultReferenceKindV1::Singleton,
                index,
                reference,
                |validator, target, _| {
                    validator
                        .authority
                        .validate_default_singleton_reference_target(validator.template, *target)
                        .map_err(ExportDefaultReferenceValidationError::Target)
                },
            )?;
        }
        for (index, reference) in references.fields().iter().enumerate() {
            self.validate_record(
                ExportDefaultReferenceKindV1::Field,
                index,
                reference,
                |validator, target, origin| validator.validate_field(target, origin),
            )?;
        }
        Ok(())
    }

    fn validate_record<T>(
        &mut self,
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        reference: &ExportDefaultReferenceV1<T>,
        validate_target: impl FnOnce(
            &mut Self,
            &T,
            &ExportDefinitionSourceV1,
        ) -> Result<(), ExportDefaultReferenceValidationError<E>>,
    ) -> Result<(), ExportDefaultReferenceSetSemanticValidationError<E>> {
        reference
            .witness()
            .validate_public_access(self.template.key().owner(), self.expected_call_domain)
            .map_err(|error| self.record_error(kind, index, error.into()))?;

        reference
            .definition_origin()
            .validate_semantics(self.authority)
            .map_err(|error| {
                self.record_error(
                    kind,
                    index,
                    ExportDefaultReferenceValidationError::DefinitionOrigin(error),
                )
            })?;
        validate_target(self, reference.target(), reference.definition_origin())
            .map_err(|error| self.record_error(kind, index, error))
    }

    fn validate_callable(
        &mut self,
        target: &ExportDefaultCallableTargetV1,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceValidationError<E>> {
        match target {
            ExportDefaultCallableTargetV1::Callable(callable) => {
                self.validate_callable_ref(callable, origin)
            }
            ExportDefaultCallableTargetV1::Bound(callable) => {
                self.validate_bound_callable_ref(callable, origin)
            }
            ExportDefaultCallableTargetV1::DerivedEquality { owner_type } => self.validate_type(
                owner_type,
                ExportDefaultReferenceTargetTypeSiteV1::DerivedEqualityOwner,
                origin,
            ),
            ExportDefaultCallableTargetV1::LocalFunction { .. }
            | ExportDefaultCallableTargetV1::Lambda { .. }
            | ExportDefaultCallableTargetV1::AnonymousFunction { .. }
            | ExportDefaultCallableTargetV1::CallableReference { .. }
            | ExportDefaultCallableTargetV1::FunctionAddress { .. } => Ok(()),
        }?;
        self.authority
            .validate_default_callable_reference_target(self.template, target)
            .map_err(ExportDefaultReferenceValidationError::Target)
    }

    fn validate_callable_ref(
        &mut self,
        callable: &DefaultCallableRefV1,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceValidationError<E>> {
        if let OptionalSignatureType::Present(owner) = callable.owner() {
            self.validate_type(
                owner,
                ExportDefaultReferenceTargetTypeSiteV1::CallableOwner,
                origin,
            )?;
        }
        for (index, argument) in callable.type_arguments().iter().enumerate() {
            self.validate_type(
                argument,
                ExportDefaultReferenceTargetTypeSiteV1::CallableTypeArgument { index },
                origin,
            )?;
        }
        Ok(())
    }

    fn validate_bound_callable_ref(
        &mut self,
        callable: &DefaultBoundCallableRefV1,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceValidationError<E>> {
        let receiver = callable.receiver_parameter();
        self.scope
            .validate(&SignatureTypeKey::Binder {
                depth: receiver.depth(),
                index: receiver.index(),
            })
            .map_err(|error| ExportDefaultReferenceValidationError::Binder {
                site: ExportDefaultReferenceTargetTypeSiteV1::BoundCallableReceiverParameter,
                definition_origin: Box::new(origin.clone()),
                error,
            })?;
        match callable.source() {
            DefaultBoundCallableSourceV1::Class {
                bound,
                callable: source,
            } => {
                self.validate_type(
                    bound,
                    ExportDefaultReferenceTargetTypeSiteV1::BoundCallableBound,
                    origin,
                )?;
                self.validate_callable_ref(source, origin)?;
            }
            DefaultBoundCallableSourceV1::Interface { bound, .. } => {
                self.validate_type(
                    bound,
                    ExportDefaultReferenceTargetTypeSiteV1::BoundCallableBound,
                    origin,
                )?;
            }
        }
        self.validate_type(
            callable.instantiated_signature(),
            ExportDefaultReferenceTargetTypeSiteV1::BoundCallableInstantiatedSignature,
            origin,
        )
    }

    fn validate_constructor(
        &mut self,
        target: &DefaultConstructorRefV1,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceValidationError<E>> {
        self.validate_type(
            target.owner_type(),
            ExportDefaultReferenceTargetTypeSiteV1::ConstructorOwner,
            origin,
        )?;
        self.authority
            .validate_default_constructor_reference_target(self.template, target)
            .map_err(ExportDefaultReferenceValidationError::Target)
    }

    fn validate_type_target(
        &mut self,
        target: &SignatureTypeKey,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceValidationError<E>> {
        if let SignatureTypeKey::Binder { depth, index } = target {
            return Err(ExportDefaultReferenceValidationError::BinderTypeTarget {
                depth: *depth,
                index: *index,
            });
        }
        scoped_types::validate(self, target, origin)?;
        self.authority
            .validate_default_type_reference_target(self.template, target)
            .map_err(ExportDefaultReferenceValidationError::Target)
    }

    fn validate_field(
        &mut self,
        target: &DefaultFieldRefV1,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceValidationError<E>> {
        match target {
            DefaultFieldRefV1::Struct { owner_type, .. }
            | DefaultFieldRefV1::Class { owner_type, .. } => self.validate_type(
                owner_type,
                ExportDefaultReferenceTargetTypeSiteV1::FieldOwner,
                origin,
            ),
            DefaultFieldRefV1::Tuple { .. } => Ok(()),
        }?;
        self.authority
            .validate_default_field_reference_target(self.template, target)
            .map_err(ExportDefaultReferenceValidationError::Target)
    }

    fn validate_type(
        &mut self,
        signature: &SignatureTypeKey,
        site: ExportDefaultReferenceTargetTypeSiteV1,
        origin: &ExportDefinitionSourceV1,
    ) -> Result<(), ExportDefaultReferenceValidationError<E>> {
        match self
            .scope
            .validate_signature_semantics(signature, self.authority)
        {
            Err(SignatureTypeSemanticError::Allocation(error)) => {
                Err(ExportDefaultReferenceValidationError::Resource(error))
            }
            Ok(()) => Ok(()),
            Err(error) => Err(ExportDefaultReferenceValidationError::Type {
                site,
                definition_origin: Box::new(origin.clone()),
                error: Box::new(error),
            }),
        }
    }

    fn record_error(
        &self,
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        error: ExportDefaultReferenceValidationError<E>,
    ) -> ExportDefaultReferenceSetSemanticValidationError<E> {
        ExportDefaultReferenceSetSemanticValidationError::Record {
            kind,
            index,
            error: Box::new(error),
        }
    }
}

const fn call_domain(access: PublicLookupAccessV1) -> ExportDefaultCallDomainV1 {
    match access {
        PublicLookupAccessV1::DirectOnly => ExportDefaultCallDomainV1::DirectPublic,
        PublicLookupAccessV1::PublicSlot => ExportDefaultCallDomainV1::DirectAndPublicSlot,
    }
}

#[cfg(test)]
mod tests;
