use crate::{DefaultBodyNestedAuthority, DefaultBodyValidationInputV1};

use std::collections::HashSet;
use std::fmt;

use scoop_identity::{
    CallableTemplateOrigin, PersistentGeneratedCallableId, SignatureTypeKey,
    StructuralDefinitionPath,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use crate::{
    DefaultAnonymousFunctionV1, DefaultCallableBodyTypeArgumentsV1, DefaultCallableReferenceV1,
    DefaultCaptureV1, DefaultLambdaV1, DefaultLocalFunctionV1, ExportDefaultTemplateV1,
};

#[cfg(test)]
mod tests;

mod authority;
mod body;

/// The kind-preserving persistent identity of one nested callable descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableIdentityV1 {
    LocalFunction(CallableTemplateOrigin),
    Lambda(PersistentGeneratedCallableId),
    AnonymousFunction(PersistentGeneratedCallableId),
    CallableReference(PersistentGeneratedCallableId),
}

impl DefaultNestedCallableIdentityV1 {
    pub const fn kind(self) -> DefaultNestedCallableKindV1 {
        match self {
            Self::LocalFunction(_) => DefaultNestedCallableKindV1::LocalFunction,
            Self::Lambda(_) => DefaultNestedCallableKindV1::Lambda,
            Self::AnonymousFunction(_) => DefaultNestedCallableKindV1::AnonymousFunction,
            Self::CallableReference(_) => DefaultNestedCallableKindV1::CallableReference,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableKindV1 {
    LocalFunction,
    Lambda,
    AnonymousFunction,
    CallableReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableLocalUseV1 {
    DirectCall,
    CallableReference,
}

/// Why a nested callable identity may occur in the current default template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableProvenanceV1 {
    /// The entity was declared lexically inside this exact default template.
    TemplateLexical,
    /// The entity retained its terminal-provider identity while another
    /// validated default was expanded into this template.
    DefaultDependency,
}

/// The body-binder relation required by a nested callable identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableBodyShapeV1 {
    Absent,
    Lexical,
    Explicit { arity: u32 },
}

/// A borrowed, kind-complete projection of the descriptor's body arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableBodyArgumentsV1<'a> {
    Absent,
    Lexical,
    Explicit(&'a [SignatureTypeKey]),
}

impl DefaultNestedCallableBodyArgumentsV1<'_> {
    fn shape(self) -> DefaultNestedCallableBodyShapeV1 {
        match self {
            Self::Absent => DefaultNestedCallableBodyShapeV1::Absent,
            Self::Lexical => DefaultNestedCallableBodyShapeV1::Lexical,
            Self::Explicit(arguments) => DefaultNestedCallableBodyShapeV1::Explicit {
                arity: u32::try_from(arguments.len())
                    .expect("default callable body argument count is bounded by its constructor"),
            },
        }
    }
}

/// Identity-derived facts that do not depend on a body type substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultNestedCallableIdentityShapeV1 {
    provenance: DefaultNestedCallableProvenanceV1,
    definition_path: StructuralDefinitionPath,
    owner_type_parameter_count: u32,
    body: DefaultNestedCallableBodyShapeV1,
}

impl DefaultNestedCallableIdentityShapeV1 {
    pub const fn new(
        provenance: DefaultNestedCallableProvenanceV1,
        definition_path: StructuralDefinitionPath,
        owner_type_parameter_count: u32,
        body: DefaultNestedCallableBodyShapeV1,
    ) -> Self {
        Self {
            provenance,
            definition_path,
            owner_type_parameter_count,
            body,
        }
    }

    pub const fn provenance(&self) -> DefaultNestedCallableProvenanceV1 {
        self.provenance
    }

    pub const fn definition_path(&self) -> &StructuralDefinitionPath {
        &self.definition_path
    }

    pub const fn owner_type_parameter_count(&self) -> u32 {
        self.owner_type_parameter_count
    }

    pub const fn body(&self) -> DefaultNestedCallableBodyShapeV1 {
        self.body
    }
}

/// Provider ABI after applying the descriptor's validated body substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultNestedCallableAbiShapeV1 {
    function_type: SignatureTypeKey,
    capture_types: Vec<SignatureTypeKey>,
}

impl DefaultNestedCallableAbiShapeV1 {
    pub const fn new(
        function_type: SignatureTypeKey,
        capture_types: Vec<SignatureTypeKey>,
    ) -> Self {
        Self {
            function_type,
            capture_types,
        }
    }

    pub const fn function_type(&self) -> &SignatureTypeKey {
        &self.function_type
    }

    pub fn capture_types(&self) -> &[SignatureTypeKey] {
        &self.capture_types
    }
}

/// Supplies typed nested-body facts from validated provider HIR and its
/// default-dependency closure.
///
/// The identity query must verify the exact declaration/generated role and
/// distinguish direct lexical ownership from a hygienically expanded default
/// dependency. The ABI query is called only after the body-argument shape has
/// matched; it must apply those arguments in declaration order and return the
/// result in the current template's provider binder scope.
pub trait DefaultNestedCallableSemanticAuthority<E> {
    fn default_nested_callable_identity_shape(
        &mut self,
        template: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, E>;

    fn default_nested_callable_abi_shape(
        &mut self,
        template: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
    ) -> Result<DefaultNestedCallableAbiShapeV1, E>;
}

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
    local_declarations: HashSet<CallableTemplateOrigin>,
    local_uses: Vec<(CallableTemplateOrigin, DefaultNestedCallableLocalUseV1)>,
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
            local_declarations: HashSet::new(),
            local_uses: Vec::new(),
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
        let identity_shape = self
            .authority
            .default_nested_callable_identity_shape(identity, self.meter, self.path)
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
            .default_nested_callable_abi_shape(identity, body_arguments, self.meter, self.path)
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

    pub(super) fn record_local_declaration(
        &mut self,
        declaration: CallableTemplateOrigin,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.charge_work()?;
        if self.local_declarations.contains(&declaration) {
            return Err(
                DefaultNestedCallableAbiValidationError::DuplicateLocalFunction { declaration },
            );
        }
        self.meter
            .try_reserve_set_slots(&mut self.local_declarations, 1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.local_declarations.insert(declaration);
        Ok(())
    }

    pub(super) fn record_local_use(
        &mut self,
        declaration: CallableTemplateOrigin,
        site: DefaultNestedCallableLocalUseV1,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.charge_work()?;
        self.meter
            .try_reserve_collection_slots(&mut self.local_uses, 1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.local_uses.push((declaration, site));
        Ok(())
    }

    pub(super) fn validate_local_uses(
        &mut self,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        for &(declaration, site) in &self.local_uses {
            self.meter
                .charge_work(1, self.path)
                .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
            if !self.local_declarations.contains(&declaration) {
                return Err(
                    DefaultNestedCallableAbiValidationError::MissingLocalFunction {
                        declaration,
                        site,
                    },
                );
            }
        }
        Ok(())
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableAuthorityQueryV1 {
    Identity,
    Abi,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableAbiValidationError<E> {
    Authority {
        kind: DefaultNestedCallableKindV1,
        query: DefaultNestedCallableAuthorityQueryV1,
        error: E,
    },
    DefinitionPath {
        kind: DefaultNestedCallableKindV1,
        expected: StructuralDefinitionPath,
        actual: StructuralDefinitionPath,
    },
    DefinitionPathNotDescendant {
        kind: DefaultNestedCallableKindV1,
        template: StructuralDefinitionPath,
        actual: StructuralDefinitionPath,
    },
    OwnerTypeParameterCount {
        kind: DefaultNestedCallableKindV1,
        expected: u32,
        actual: u32,
    },
    BodyArguments {
        kind: DefaultNestedCallableKindV1,
        expected: DefaultNestedCallableBodyShapeV1,
        actual: DefaultNestedCallableBodyShapeV1,
    },
    FunctionType {
        kind: DefaultNestedCallableKindV1,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    CaptureArity {
        kind: DefaultNestedCallableKindV1,
        expected: usize,
        actual: usize,
    },
    CaptureType {
        kind: DefaultNestedCallableKindV1,
        index: usize,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    DuplicateLocalFunction {
        declaration: CallableTemplateOrigin,
    },
    MissingLocalFunction {
        declaration: CallableTemplateOrigin,
        site: DefaultNestedCallableLocalUseV1,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for DefaultNestedCallableAbiValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authority { kind, query, error } => write!(
                formatter,
                "default nested {kind:?} {query:?} authority failed: {error}"
            ),
            Self::DefinitionPath {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} definition path mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::DefinitionPathNotDescendant {
                kind,
                template,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} path {actual:?} is not a strict descendant of template path {template:?}"
            ),
            Self::OwnerTypeParameterCount {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} owner type-parameter count mismatch: expected {expected}, found {actual}"
            ),
            Self::BodyArguments {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} body arguments mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::FunctionType {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} function type mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::CaptureArity {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} capture count mismatch: expected {expected}, found {actual}"
            ),
            Self::CaptureType {
                kind,
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "default nested {kind:?} capture {index} type mismatch: expected {expected:?}, found {actual:?}"
            ),
            Self::DuplicateLocalFunction { declaration } => write!(
                formatter,
                "default body declares local function {declaration:?} more than once"
            ),
            Self::MissingLocalFunction { declaration, site } => write!(
                formatter,
                "default body {site:?} refers to undeclared local function {declaration:?}"
            ),
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default nested callable ABI resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultNestedCallableAbiValidationError<E>
{
}
