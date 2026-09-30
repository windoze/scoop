use super::*;

#[derive(Debug, Clone)]
pub struct ConstructorParameter {
    pub id: ConstructorParamId,
    pub binding: BindingId,
    pub definition: DefinitionOrigin,
    pub name: String,
    pub ty: TypeId,
}

#[derive(Debug, Clone)]
pub struct ClassConstructor {
    pub owner: ClassId,
    /// Whether this callable is a source constructor or the unique generated
    /// zero-argument adapter for another source constructor.
    pub identity_kind: ClassConstructorIdentityKind,
    pub access: DeclarationAccess,
    pub safety: Safety,
    /// Owner parameters required to be GC-free by this callable and its callees.
    pub no_gc_type_params: Vec<TypeParamId>,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: ClassConstructorKind,
    pub span: Span,
    pub origin: DefinitionOrigin,
    /// The executable context for operations synthesized into this body.
    pub evaluation_context: SourceContextId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassConstructorIdentityKind {
    Source,
    ZeroArgumentAdapter { source: ClassConstructorId },
}

#[derive(Debug, Clone)]
pub enum ClassConstructorKind {
    Primary {
        base: BaseInitialization,
        primary_stores: Vec<PrimaryFieldStore>,
        common_initialization: Vec<ClassInitializationStep>,
    },
    Secondary {
        delegation: ClassSecondaryDelegation,
        body: Body,
    },
}

#[derive(Debug, Clone)]
pub struct PrimaryFieldStore {
    pub field: ClassFieldId,
    pub parameter: ConstructorParamId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassInitializationStep {
    StoredProperty {
        field: ClassFieldId,
        initializer: ConstructorExpression,
        span: Span,
    },
    DelegatedProperty {
        storage: DelegateStorageId,
        field: ClassFieldId,
        initializer: ConstructorExpression,
        span: Span,
    },
    InitBlock {
        body: Body,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ConstructorExpression {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub enum BaseInitialization {
    Root,
    Super {
        target: BaseInitializerTarget,
        arguments: ConstructorArguments,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseInitializerTarget {
    Local(ClassConstructorApplicationId),
    ImportedTemplate(ImportedConstructorApplicationId),
    Imported {
        owner: TypeId,
        callable: ImportedDependencyCallableUseId,
    },
}

#[derive(Debug, Clone)]
pub enum ClassSecondaryDelegation {
    This {
        target: ClassConstructorApplicationId,
        arguments: ConstructorArguments,
    },
    Terminal {
        base: BaseInitialization,
        common_initialization: Vec<ClassInitializationStep>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassConstructorApplication {
    pub constructor: ClassConstructorId,
    pub owner: ClassApplicationId,
}

#[derive(Debug, Clone)]
pub struct StructConstructor {
    pub owner: StructId,
    pub access: DeclarationAccess,
    pub safety: Safety,
    /// Owner parameters required to be GC-free by this callable and its callees.
    pub no_gc_type_params: Vec<TypeParamId>,
    pub parameters: Vec<ConstructorParameter>,
    pub kind: StructConstructorKind,
    pub span: Span,
    pub origin: DefinitionOrigin,
}

impl StructConstructor {
    pub fn source_gc_effect(&self) -> GcEffect {
        match self.kind {
            StructConstructorKind::Primary => GcEffect::Managed,
            StructConstructorKind::Secondary { gc_effect, .. } => gc_effect,
        }
    }
}

#[derive(Debug, Clone)]
pub enum StructConstructorKind {
    Primary,
    Secondary {
        gc_effect: GcEffect,
        delegation: StructConstructorDelegation,
        body: Body,
    },
}

#[derive(Debug, Clone)]
pub struct StructConstructorDelegation {
    pub target: StructConstructorApplicationId,
    pub arguments: ConstructorArguments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructConstructorApplication {
    pub constructor: StructConstructorId,
    pub owner: StructApplicationId,
}
