//! Request-local declaration handles for ordinary-dependency callables.

mod constructors;
pub use constructors::*;
mod applications;
pub use applications::*;
mod delegates;
pub use delegates::*;
mod closures;
pub use closures::*;

/// A dependency implementation in the consumer's type and value domains.
/// Its kind and executable nodes are shared with current declarations.
/// Its declaration remains owned by the provider, outside `Module::functions`.
#[derive(Debug, Clone)]
pub struct ImportedGenericCallableTemplate {
    pub signature: ImportedGenericCallableSignature,
    pub implementation: crate::FunctionKind,
}

impl std::ops::Deref for ImportedGenericCallableTemplate {
    type Target = ImportedGenericCallableSignature;
    fn deref(&self) -> &Self::Target {
        &self.signature
    }
}

#[derive(Debug, Clone)]
pub struct ImportedGenericCallableSignature {
    pub declaration: ImportedCallableTemplateOrigin,
    pub signature: crate::CallableSignature,
    pub type_parameters: ImportedCallableTypeParameters,
    pub no_gc_type_params: Vec<crate::TypeParamId>,
    pub gc_free_pointee_requirements: Vec<crate::RequiresGcFreePointee>,
    pub receiver: Option<crate::TypeId>,
    pub origin: crate::DefinitionOrigin,
}

impl std::ops::Deref for ImportedGenericCallableSignature {
    type Target = crate::CallableSignature;

    fn deref(&self) -> &Self::Target {
        &self.signature
    }
}

impl std::ops::DerefMut for ImportedGenericCallableSignature {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.signature
    }
}

/// A lexical body keeps its source declaration and enclosing application.
/// Inherited binders do not turn an ordinary local declaration into a generic
/// declaration with a different persistent identity.
#[derive(Debug, Clone)]
pub enum ImportedCallableTemplateOrigin {
    Generic(scoop_identity::PersistentGenericFunctionId),
    ExtensionAccessor(scoop_identity::PersistentPropertyAccessorId),
    Initialization {
        template: crate::ImportedGenericDelegateTemplateId,
        owner: scoop_identity::PersistentGeneratedCallableId,
    },
    Nominal {
        declaration: crate::DefaultCallableDeclarationV1,
        owner: crate::SourceNominalId,
        owner_parameter_count: usize,
        modifier: crate::MethodModifier,
        dispatch: crate::DeclaredMethodDispatch,
    },
    Local {
        parent: scoop_identity::CallableTemplateOwner,
        descriptor: crate::DefaultLocalFunctionV1,
        capture_bindings: Vec<crate::BindingId>,
    },
    Closure {
        parent: scoop_identity::CallableTemplateOwner,
        body: scoop_identity::PersistentGeneratedCallableId,
        capture_bindings: Vec<crate::BindingId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportedDispatchCallable {
    External(crate::ImportedDependencyCallableUseId),
    Template(crate::ImportedGenericCallableApplicationId),
}

impl ImportedCallableTemplateOrigin {
    pub fn declaration(&self) -> scoop_identity::CallableTemplateOrigin {
        match self {
            Self::Generic(id) => scoop_identity::CallableTemplateOrigin::GenericFunction(*id),
            Self::ExtensionAccessor(id) => scoop_identity::CallableTemplateOrigin::Accessor(*id),
            Self::Initialization { .. } | Self::Closure { .. } => {
                panic!("initialization helpers and closures have generated identities")
            }
            Self::Nominal { declaration, .. } => match declaration {
                crate::DefaultCallableDeclarationV1::Function(id) => {
                    scoop_identity::CallableTemplateOrigin::Function(*id)
                }
                crate::DefaultCallableDeclarationV1::GenericFunction(id) => {
                    scoop_identity::CallableTemplateOrigin::GenericFunction(*id)
                }
                crate::DefaultCallableDeclarationV1::PropertyAccessor(id) => {
                    scoop_identity::CallableTemplateOrigin::Accessor(*id)
                }
                crate::DefaultCallableDeclarationV1::Generated(_) => {
                    unreachable!("nominal source members have source identities")
                }
            },
            Self::Local { descriptor, .. } => descriptor.declaration(),
        }
    }

    pub fn body_owner(&self) -> crate::DefaultCallableDeclarationV1 {
        if let Self::Initialization { owner, .. } | Self::Closure { body: owner, .. } = self {
            return crate::DefaultCallableDeclarationV1::Generated(*owner);
        }
        match self.declaration() {
            scoop_identity::CallableTemplateOrigin::Function(id) => {
                crate::DefaultCallableDeclarationV1::Function(id)
            }
            scoop_identity::CallableTemplateOrigin::GenericFunction(id) => {
                crate::DefaultCallableDeclarationV1::GenericFunction(id)
            }
            scoop_identity::CallableTemplateOrigin::Accessor(id) => {
                crate::DefaultCallableDeclarationV1::PropertyAccessor(id)
            }
            _ => unreachable!("local descriptors only contain source function declarations"),
        }
    }
}

/// Source candidates need declared bounds for inference. Lexical calls were
/// resolved by their provider and only need their ordered substitution slots.
#[derive(Debug, Clone)]
pub enum ImportedCallableTypeParameters {
    Declared(Vec<crate::TypeParamDecl>),
    Substitution(Vec<crate::TypeParamId>),
}

impl ImportedCallableTypeParameters {
    pub fn declarations(&self) -> &[crate::TypeParamDecl] {
        match self {
            Self::Declared(parameters) => parameters,
            Self::Substitution(_) => {
                panic!("a lexical implementation does not participate in source inference")
            }
        }
    }

    pub fn ids(&self) -> Vec<crate::TypeParamId> {
        match self {
            Self::Declared(parameters) => parameters.iter().map(|parameter| parameter.id).collect(),
            Self::Substitution(parameters) => parameters.clone(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Declared(parameters) => parameters.len(),
            Self::Substitution(parameters) => parameters.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A source-selected application. Arguments may still name the caller's
/// binders; only concretization produces persistent exact applications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedGenericCallableApplication {
    pub template: crate::ImportedGenericCallableTemplateId,
    pub arguments: ImportedCallableArguments,
}

/// Export-HIR use of one callable committed through the ordinary-dependency
/// selection transaction.
///
/// The reference names the actual typed declaration. The complete output
/// owns its selected interface for subsequent stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedDependencyCallableUse {
    reference: crate::ImportedDependencyCallableRef,
    dispatch: ImportedDependencyDispatch,
    receiver: Option<crate::TypeId>,
}

/// The source-selected dispatch table and its provider-defined slot position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportedDependencyDispatch {
    Direct,
    Virtual {
        slot: u32,
    },
    Interface {
        interface: scoop_identity::PersistentTypeId,
        slot: u32,
    },
}

impl ImportedDependencyCallableUse {
    pub fn new(
        reference: crate::ImportedDependencyCallableRef,
        dispatch: ImportedDependencyDispatch,
        receiver: Option<crate::TypeId>,
    ) -> Self {
        Self {
            reference,
            dispatch,
            receiver,
        }
    }

    pub const fn reference(self) -> crate::ImportedDependencyCallableRef {
        self.reference
    }

    pub const fn dispatch(self) -> ImportedDependencyDispatch {
        self.dispatch
    }

    /// Required receiver from the selected declaration; absent for free calls.
    pub const fn receiver(self) -> Option<crate::TypeId> {
        self.receiver
    }
}
