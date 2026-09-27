//! Request-local HIR handles for executable ordinary-dependency callables.

/// A dependency body normalized into the consumer's type and value domains.
/// Its declaration remains owned by the provider, outside `Module::functions`.
#[derive(Debug, Clone)]
pub struct ImportedGenericCallableTemplate {
    pub signature: ImportedGenericCallableSignature,
    pub body: crate::Body,
}

impl std::ops::Deref for ImportedGenericCallableTemplate {
    type Target = ImportedGenericCallableSignature;
    fn deref(&self) -> &Self::Target {
        &self.signature
    }
}

#[derive(Debug, Clone)]
pub struct ImportedGenericCallableSignature {
    pub declaration: scoop_identity::PersistentGenericFunctionId,
    pub name: String,
    pub type_parameters: Vec<crate::TypeParamDecl>,
    pub no_gc_type_params: Vec<crate::TypeParamId>,
    pub gc_free_pointee_requirements: Vec<crate::RequiresGcFreePointee>,
    pub parameters: Vec<crate::Param>,
    pub return_type: crate::TypeId,
    pub effects: crate::CallableSourceEffectsV1,
    pub receiver: Option<crate::TypeId>,
    pub origin: crate::DefinitionOrigin,
    pub span: crate::Span,
}

/// A source-selected application. Arguments may still name the caller's
/// binders; only concretization produces persistent exact applications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedGenericCallableApplication {
    pub template: crate::ImportedGenericCallableTemplateId,
    pub arguments: crate::NonEmptyVec<crate::TypeId>,
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
    ) -> Self {
        Self {
            reference,
            dispatch,
        }
    }

    pub const fn reference(self) -> crate::ImportedDependencyCallableRef {
        self.reference
    }

    pub const fn dispatch(self) -> ImportedDependencyDispatch {
        self.dispatch
    }
}
