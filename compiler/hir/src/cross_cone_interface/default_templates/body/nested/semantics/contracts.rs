use crate::ExportDefaultTemplateV1;
use scoop_identity::{
    CallableTemplateOrigin, PersistentGeneratedCallableId, SignatureTypeKey,
    StructuralDefinitionPath,
};

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
    pub(super) fn shape(self) -> DefaultNestedCallableBodyShapeV1 {
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
        site: crate::DefaultNestedCallableSiteV1,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, E>;

    fn default_nested_callable_abi_shape(
        &mut self,
        template: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        site: crate::DefaultNestedCallableSiteV1,
        body_arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
    ) -> Result<DefaultNestedCallableAbiShapeV1, E>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultNestedCallableAuthorityQueryV1 {
    Identity,
    Abi,
}
