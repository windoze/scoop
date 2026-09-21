use crate::{DefaultBodyNestedAuthority, DefaultBodyValidationInputV1};

use std::collections::HashSet;

use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey, StructuralDefinitionPath};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::{
    DefaultAnonymousFunctionV1, DefaultCallableBodyTypeArgumentsV1, DefaultCallableReferenceV1,
    DefaultCaptureV1, DefaultLambdaV1, DefaultLocalFunctionV1, ExportDefaultTemplateV1,
};

#[cfg(test)]
mod tests;

mod authority;
mod body;
mod contracts;
mod errors;
pub use contracts::{
    DefaultNestedCallableAbiShapeV1, DefaultNestedCallableAuthorityQueryV1,
    DefaultNestedCallableBodyArgumentsV1, DefaultNestedCallableBodyShapeV1,
    DefaultNestedCallableIdentityShapeV1, DefaultNestedCallableIdentityV1,
    DefaultNestedCallableKindV1, DefaultNestedCallableLocalUseV1,
    DefaultNestedCallableProvenanceV1, DefaultNestedCallableSemanticAuthority,
};
mod local_scopes;
mod site;
pub use errors::DefaultNestedCallableAbiValidationError;
pub use site::DefaultNestedCallableSiteV1;

impl DefaultLocalFunctionV1 {
    pub fn validate_nested_callable_abi_semantics<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        Validator::new(
            DefaultBodyValidationInputV1::from(template),
            &mut authority::PublicAuthority {
                template,
                authority,
            },
            meter,
            path,
        )
        .validate_local_function(self, 1)
    }
}

impl DefaultLambdaV1 {
    pub fn validate_nested_callable_abi_semantics<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        Validator::new(
            DefaultBodyValidationInputV1::from(template),
            &mut authority::PublicAuthority {
                template,
                authority,
            },
            meter,
            path,
        )
        .validate_lambda(self, 1)
    }
}

impl DefaultAnonymousFunctionV1 {
    pub fn validate_nested_callable_abi_semantics<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        Validator::new(
            DefaultBodyValidationInputV1::from(template),
            &mut authority::PublicAuthority {
                template,
                authority,
            },
            meter,
            path,
        )
        .validate_anonymous(self, 1)
    }
}

impl DefaultCallableReferenceV1 {
    pub fn validate_nested_callable_abi_semantics<A, E>(
        &self,
        template: &ExportDefaultTemplateV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>>
    where
        A: DefaultNestedCallableSemanticAuthority<E>,
    {
        Validator::new(
            DefaultBodyValidationInputV1::from(template),
            &mut authority::PublicAuthority {
                template,
                authority,
            },
            meter,
            path,
        )
        .validate_callable_reference(self, 1)
    }
}

pub(super) struct Validator<'a, A, E> {
    template: DefaultBodyValidationInputV1<'a>,
    authority: &'a mut A,
    meter: &'a mut BudgetMeter,
    path: &'a WirePath,
    local_scopes: Vec<HashSet<CallableTemplateOrigin>>,
    next_site: DefaultNestedCallableSiteV1,
    error: std::marker::PhantomData<fn() -> E>,
}

impl<'a, A, E> Validator<'a, A, E>
where
    A: DefaultBodyNestedAuthority<E>,
{
    pub(super) fn new(
        template: DefaultBodyValidationInputV1<'a>,
        authority: &'a mut A,
        meter: &'a mut BudgetMeter,
        path: &'a WirePath,
    ) -> Self {
        Self {
            template,
            authority,
            meter,
            path,
            local_scopes: Vec::new(),
            next_site: DefaultNestedCallableSiteV1::Standalone,
            error: std::marker::PhantomData,
        }
    }

    pub(super) fn validate_local_function(
        &mut self,
        function: &DefaultLocalFunctionV1,
        depth: u64,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.validate_descriptor(
            DefaultNestedCallableIdentityV1::LocalFunction(function.declaration()),
            function.definition_path(),
            function.function_type(),
            function.captures(),
            function.owner_type_parameter_count(),
            DefaultNestedCallableBodyArgumentsV1::Absent,
            depth,
        )
    }

    pub(super) fn validate_lambda(
        &mut self,
        lambda: &DefaultLambdaV1,
        depth: u64,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.validate_descriptor(
            DefaultNestedCallableIdentityV1::Lambda(lambda.body()),
            lambda.definition_path(),
            lambda.function_type(),
            lambda.captures(),
            lambda.owner_type_parameter_count(),
            body_arguments(lambda.body_type_arguments()),
            depth,
        )
    }

    pub(super) fn validate_anonymous(
        &mut self,
        function: &DefaultAnonymousFunctionV1,
        depth: u64,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.validate_descriptor(
            DefaultNestedCallableIdentityV1::AnonymousFunction(function.body()),
            function.definition_path(),
            function.function_type(),
            function.captures(),
            function.owner_type_parameter_count(),
            body_arguments(function.body_type_arguments()),
            depth,
        )
    }

    pub(super) fn validate_callable_reference(
        &mut self,
        reference: &DefaultCallableReferenceV1,
        depth: u64,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.validate_descriptor(
            DefaultNestedCallableIdentityV1::CallableReference(reference.invoke()),
            reference.definition_path(),
            reference.function_type(),
            reference.captures(),
            reference.owner_type_parameter_count(),
            DefaultNestedCallableBodyArgumentsV1::Absent,
            depth,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_descriptor(
        &mut self,
        identity: DefaultNestedCallableIdentityV1,
        definition_path: &StructuralDefinitionPath,
        function_type: &SignatureTypeKey,
        captures: &[DefaultCaptureV1],
        owner_type_parameter_count: u32,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
        depth: u64,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        let kind = identity.kind();
        self.enter_node(depth)?;
        self.charge_work()?;
        let site = self
            .next_site
            .advance(self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        let identity_shape = self
            .authority
            .default_nested_callable_identity_shape(identity, site, self.meter, self.path)
            .map_err(|error| DefaultNestedCallableAbiValidationError::Authority {
                kind,
                query: DefaultNestedCallableAuthorityQueryV1::Identity,
                error,
            })?;

        self.charge_work()?;
        if definition_path != identity_shape.definition_path() {
            return Err(DefaultNestedCallableAbiValidationError::DefinitionPath {
                kind,
                expected: identity_shape.definition_path().clone(),
                actual: definition_path.clone(),
            });
        }
        if identity_shape.provenance() == DefaultNestedCallableProvenanceV1::TemplateLexical {
            self.charge_work()?;
            if !is_strict_descendant(definition_path, self.template.definition_path()) {
                return Err(
                    DefaultNestedCallableAbiValidationError::DefinitionPathNotDescendant {
                        kind,
                        template: self.template.definition_path().clone(),
                        actual: definition_path.clone(),
                    },
                );
            }
        }

        self.charge_work()?;
        if owner_type_parameter_count != identity_shape.owner_type_parameter_count() {
            return Err(
                DefaultNestedCallableAbiValidationError::OwnerTypeParameterCount {
                    kind,
                    expected: identity_shape.owner_type_parameter_count(),
                    actual: owner_type_parameter_count,
                },
            );
        }

        self.charge_work()?;
        let actual_body = body_arguments.shape();
        if actual_body != identity_shape.body() {
            return Err(DefaultNestedCallableAbiValidationError::BodyArguments {
                kind,
                expected: identity_shape.body(),
                actual: actual_body,
            });
        }

        self.charge_work()?;
        let abi = self
            .authority
            .default_nested_callable_abi_shape(
                identity,
                site,
                body_arguments,
                self.meter,
                self.path,
            )
            .map_err(|error| DefaultNestedCallableAbiValidationError::Authority {
                kind,
                query: DefaultNestedCallableAuthorityQueryV1::Abi,
                error,
            })?;

        self.charge_work()?;
        if function_type != abi.function_type() {
            return Err(DefaultNestedCallableAbiValidationError::FunctionType {
                kind,
                expected: Box::new(abi.function_type().clone()),
                actual: Box::new(function_type.clone()),
            });
        }

        self.charge_work()?;
        if captures.len() != abi.capture_types().len() {
            return Err(DefaultNestedCallableAbiValidationError::CaptureArity {
                kind,
                expected: abi.capture_types().len(),
                actual: captures.len(),
            });
        }
        for (index, (capture, expected)) in captures.iter().zip(abi.capture_types()).enumerate() {
            self.enter_leaf()?;
            if capture.value_type() != expected {
                return Err(DefaultNestedCallableAbiValidationError::CaptureType {
                    kind,
                    index,
                    expected: Box::new(expected.clone()),
                    actual: Box::new(capture.value_type().clone()),
                });
            }
        }
        Ok(())
    }

    pub(super) fn enter_node(
        &mut self,
        depth: u64,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.meter
            .charge_nodes(1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.charge_work()
    }

    fn enter_leaf(&mut self) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.meter
            .charge_edges(1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.meter
            .charge_nodes(1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.charge_work()
    }

    pub(super) fn child_depth(
        &mut self,
        parent: u64,
    ) -> Result<u64, DefaultNestedCallableAbiValidationError<E>> {
        let depth = parent.checked_add(1).ok_or_else(|| {
            DefaultNestedCallableAbiValidationError::Resource(WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                self.path.clone(),
                None,
            ))
        })?;
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.meter
            .charge_edges(1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        Ok(depth)
    }

    fn charge_work(&mut self) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.meter
            .charge_work(1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)
    }
}

fn body_arguments(
    arguments: &DefaultCallableBodyTypeArgumentsV1,
) -> DefaultNestedCallableBodyArgumentsV1<'_> {
    if let Some(arguments) = arguments.explicit_arguments() {
        DefaultNestedCallableBodyArgumentsV1::Explicit(arguments)
    } else {
        DefaultNestedCallableBodyArgumentsV1::Lexical
    }
}

fn is_strict_descendant(
    path: &StructuralDefinitionPath,
    ancestor: &StructuralDefinitionPath,
) -> bool {
    path.segments().len() > ancestor.segments().len()
        && path.segments().starts_with(ancestor.segments())
}
