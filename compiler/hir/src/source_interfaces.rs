//! Export-side source-call protocol for defaults, named arguments and varargs.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFileMetadata {
    pub provider: IntrinsicProviderId,
    pub identity: scoop_identity::SourceIdentity,
    pub name: String,
    pub source: String,
    /// Canonical metadata copied from a decoded dependency artifact.
    /// Current-source entries keep this absent and derive the record from
    /// `source`; imported entries need not retain the provider's source text.
    pub canonical_record: Option<SourceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceContext {
    source: scoop_identity::SourceIdentity,
    subject: SourceContextSubject,
}

impl SourceContext {
    pub const fn new(
        source: scoop_identity::SourceIdentity,
        subject: SourceContextSubject,
    ) -> Self {
        Self { source, subject }
    }

    pub const fn source(&self) -> &scoop_identity::SourceIdentity {
        &self.source
    }

    pub const fn subject(&self) -> &SourceContextSubject {
        &self.subject
    }
}

/// Typed semantic subject used to derive one persistent source context.
/// Display names are intentionally absent: consumers recover them from the
/// referenced declaration rather than trusting a second textual identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceContextSubject {
    File,
    /// A context whose persistent key was authenticated by a dependency HIR
    /// foundation. It cannot be reconstructed from current-module arena ids.
    Imported(scoop_identity::SourceContextKey),
    Nominal(SourceContextNominal),
    Function(FunctionId),
    Constructor(SourceContextConstructor),
    Property(PropertyId),
    LexicalCallable {
        root: LexicalDefinitionRoot,
        path: scoop_identity::StructuralDefinitionPath,
        role: scoop_identity::LexicalCallableRole,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceContextNominal {
    Struct(StructId),
    Enum(EnumId),
    Class(ClassId),
    Interface(InterfaceId),
    Object(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceContextConstructor {
    Struct(StructConstructorId),
    Class(ClassConstructorId),
}

impl Module {
    /// Reconstruct source-facing context labels from typed declarations.
    /// These labels never participate in context equality or persistence.
    pub fn source_context_names(&self, context: SourceContextId) -> (String, String) {
        match self.source_contexts[context].subject() {
            SourceContextSubject::File => (String::new(), String::new()),
            SourceContextSubject::Imported(_) => (String::new(), String::new()),
            SourceContextSubject::Nominal(owner) => {
                (String::new(), self.source_context_nominal_name(*owner))
            }
            SourceContextSubject::Function(function) => {
                let declaration = &self.functions[*function];
                let function_name = match &self.function_identities[*function] {
                    HirFunctionIdentity::Source(identity) => match identity.declaration().name() {
                        scoop_identity::DeclarationName::Named(name) => name.as_str().to_string(),
                        scoop_identity::DeclarationName::Constructor => "<init>".to_string(),
                    },
                    HirFunctionIdentity::PropertyAccessor(accessor) => {
                        let property = match accessor {
                            HirPropertyAccessorFunction::Getter(getter) => {
                                self.property_accessor_identities[*getter].property()
                            }
                            HirPropertyAccessorFunction::Setter(setter) => {
                                self.property_accessor_identities[*setter].property()
                            }
                        };
                        self.properties[property].name.clone()
                    }
                    HirFunctionIdentity::LexicalGenerated(_)
                    | HirFunctionIdentity::Initialization { .. }
                    | HirFunctionIdentity::DerivedEquality(_) => declaration.name.clone(),
                };
                let type_name = declaration.method.map_or_else(String::new, |method| {
                    self.source_context_type_name(method.owner)
                });
                (function_name, type_name)
            }
            SourceContextSubject::Constructor(owner) => match owner {
                SourceContextConstructor::Struct(constructor) => {
                    let owner = self.struct_constructors[*constructor].owner;
                    ("<init>".to_string(), self.structs[owner].name.clone())
                }
                SourceContextConstructor::Class(constructor) => {
                    let owner = self.class_constructors[*constructor].owner;
                    let name = self
                        .objects
                        .iter()
                        .find_map(|(_, object)| {
                            (object.backing_class == owner).then(|| object.name.clone())
                        })
                        .unwrap_or_else(|| self.classes[owner].name.clone());
                    ("<init>".to_string(), name)
                }
            },
            SourceContextSubject::Property(property) => {
                let property = &self.properties[*property];
                let type_name = match property.owner {
                    PropertyOwner::TopLevel | PropertyOwner::Extension(_) => String::new(),
                    PropertyOwner::Class(owner) => self.classes[owner].name.clone(),
                    PropertyOwner::Struct(owner) => self.structs[owner].name.clone(),
                    PropertyOwner::Enum(owner) => self.enums[owner].name.clone(),
                    PropertyOwner::Interface(owner) => self.interfaces[owner].name.clone(),
                    PropertyOwner::Object(owner) => self.objects[owner].name.clone(),
                };
                (property.name.clone(), type_name)
            }
            SourceContextSubject::LexicalCallable { root, role, .. } => {
                let function_name = match role {
                    scoop_identity::LexicalCallableRole::LambdaBody => "<lambda>",
                    scoop_identity::LexicalCallableRole::AnonymousFunctionBody => "<anonymous>",
                };
                (
                    function_name.to_string(),
                    self.source_context_root_type_name(*root),
                )
            }
        }
    }

    fn source_context_nominal_name(&self, owner: SourceContextNominal) -> String {
        match owner {
            SourceContextNominal::Struct(owner) => self.structs[owner].name.clone(),
            SourceContextNominal::Enum(owner) => self.enums[owner].name.clone(),
            SourceContextNominal::Class(owner) => self.classes[owner].name.clone(),
            SourceContextNominal::Interface(owner) => self.interfaces[owner].name.clone(),
            SourceContextNominal::Object(owner) => self.objects[owner].name.clone(),
        }
    }

    fn source_context_root_type_name(&self, root: LexicalDefinitionRoot) -> String {
        match root {
            LexicalDefinitionRoot::Function(function) => self.functions[function]
                .method
                .map_or_else(String::new, |method| {
                    self.source_context_type_name(method.owner)
                }),
            LexicalDefinitionRoot::ClassConstructor(constructor) => {
                let owner = self.class_constructors[constructor].owner;
                self.objects
                    .iter()
                    .find_map(|(_, object)| {
                        (object.backing_class == owner).then(|| object.name.clone())
                    })
                    .unwrap_or_else(|| self.classes[owner].name.clone())
            }
            LexicalDefinitionRoot::StructConstructor(constructor) => self.structs
                [self.struct_constructors[constructor].owner]
                .name
                .clone(),
            LexicalDefinitionRoot::VariantConstructor(variant) => {
                self.enums[variant.enumeration()].name.clone()
            }
        }
    }

    fn source_context_type_name(&self, ty: TypeId) -> String {
        match self.types[ty] {
            Type::Struct(application) => self
                .struct_name(self.struct_applications[application].template)
                .to_owned(),
            Type::Class(application) => {
                let template = self.class_applications[application].template;
                let Some(class) = self.nominal_identities.class_id(template) else {
                    return self.class_name(template).to_owned();
                };
                self.objects
                    .iter()
                    .find_map(|(_, object)| {
                        (object.backing_class == class).then(|| object.name.clone())
                    })
                    .unwrap_or_else(|| self.classes[class].name.clone())
            }
            Type::Interface(application) => self
                .interface_name(self.interface_applications[application].template)
                .to_owned(),
            Type::Enum(application) => self
                .enum_name(self.enum_applications[application].template)
                .to_owned(),
            Type::Unit
            | Type::Integer(_)
            | Type::Boolean
            | Type::String
            | Type::Any
            | Type::Tuple(_)
            | Type::Function(_)
            | Type::Ptr(_)
            | Type::FunPtr(_)
            | Type::Param(_) => String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefinitionOrigin {
    pub provider: IntrinsicProviderId,
    pub file: u32,
    pub span: Span,
    pub context: SourceContextId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationOrigin {
    pub provider: IntrinsicProviderId,
    pub file: u32,
    pub span: Span,
    pub context: SourceContextId,
}

impl From<DefinitionOrigin> for EvaluationOrigin {
    fn from(origin: DefinitionOrigin) -> Self {
        Self {
            provider: origin.provider,
            file: origin.file,
            span: origin.span,
            context: origin.context,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcreteExpressionOrigin {
    pub definition: DefinitionOrigin,
    pub evaluation: EvaluationOrigin,
}

/// Export HIR contains both declaration-bound expression bodies and default
/// instances embedded in ordinary bodies. A template node has definition
/// provenance only; the concrete product closes the first branch by using its
/// own definition as the evaluation source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionOrigin {
    Definition(DefinitionOrigin),
    Instantiated(ConcreteExpressionOrigin),
}

impl ExpressionOrigin {
    pub const fn definition(self) -> DefinitionOrigin {
        match self {
            Self::Definition(definition)
            | Self::Instantiated(ConcreteExpressionOrigin { definition, .. }) => definition,
        }
    }

    pub fn concrete(self) -> ConcreteExpressionOrigin {
        match self {
            Self::Definition(definition) => ConcreteExpressionOrigin {
                definition,
                evaluation: definition.into(),
            },
            Self::Instantiated(origin) => origin,
        }
    }

    pub const fn instantiate(self, evaluation: EvaluationOrigin) -> Self {
        Self::Instantiated(ConcreteExpressionOrigin {
            definition: self.definition(),
            evaluation,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExportValueParameter {
    pub name: String,
    pub calling: ExportParameterCalling,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportParameterCalling {
    Required {
        value_type: TypeId,
    },
    Default {
        value_type: TypeId,
        source: ExportDefaultSourceId,
    },
    Vararg {
        parameter_type: ExportVarargParameterTypeId,
        omission: ExportVarargOmission,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportVarargOmission {
    EmptyArray,
    Default(ExportDefaultSourceId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportVarargParameterType {
    pub element_type: TypeId,
    pub array_type: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportParameterOwner {
    Function(FunctionId),
    StructConstructor(StructConstructorId),
    ClassConstructor(ClassConstructorId),
    VariantConstructor(EnumVariantRef),
}

#[derive(Debug, Clone)]
pub struct ExportParameterInterface {
    pub owner: ExportParameterOwner,
    pub parameters: Vec<ExportValueParameter>,
}

/// A default is a declaration-bound typed body. Parameter references are
/// locals in this template arena and are related to declaration positions by
/// `value_parameters`; callers never resolve their source names again.
#[derive(Debug, Clone)]
pub struct ExportDefaultExpr {
    /// Typed root shared by the complete lexical path of this template.
    pub definition_root: LexicalDefinitionRoot,
    /// Stable declaration-local path of this source default. Instantiating the
    /// template preserves this path and never assigns a call-site ordinal.
    pub definition_path: scoop_identity::StructuralDefinitionPath,
    pub expression: DefaultExpression,
    /// Whether this declaration-bound region may contain suspend calls.
    /// The reader boundary checks this against every source-parameter owner
    /// that references the template.
    pub allows_suspend: bool,
    /// Direct declaration-bound dependencies of the typed template. Each
    /// category has its own identity domain. Target access domains are cached
    /// for inherited defaults whose callable domain changes.
    pub references: ExportDefaultReferences,
}

impl std::ops::Deref for ExportDefaultExpr {
    type Target = DefaultExpression;
    fn deref(&self) -> &Self::Target {
        &self.expression
    }
}

impl std::ops::DerefMut for ExportDefaultExpr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.expression
    }
}

/// A typed expression region shares ordinary body nodes and local storage.
/// Declaration metadata is separate from the body instantiated at each use.
#[derive(Debug, Clone)]
pub struct DefaultExpression {
    pub body: Body,
    pub value: Expr,
    pub result_type: TypeId,
    pub type_parameters: Vec<TypeParamId>,
    pub receiver: Option<ExportDefaultReceiver>,
    pub value_parameters: Vec<ExportDefaultValueParameter>,
    pub origin: DefinitionOrigin,
}

impl std::ops::Deref for DefaultExpression {
    type Target = Body;
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExportDefaultReferences {
    pub callables: Vec<ExportDefaultCallableRef>,
    pub constructors: Vec<ExportDefaultConstructorRef>,
    pub types: Vec<ExportDefaultTypeRef>,
    pub globals: Vec<ExportDefaultGlobalRef>,
    pub singleton_values: Vec<ExportDefaultSingletonValueRef>,
    pub fields: Vec<ExportDefaultFieldRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallDomain {
    pub direct: EffectiveLookupDomain,
    pub slot: Option<SlotContractDomain>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultCallableRef {
    pub target: ExportDefaultCallableTarget,
    pub target_domain: AccessDomain,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDefaultCallableTarget {
    Callable(Callable),
    /// An actual imported function or accessor declaration.
    ImportedDependency(ImportedDependencyCallableUseId),
    ImportedGeneric(ImportedGenericCallableApplicationId),
    Bound(BoundCallableRefId),
    DerivedEquality(DerivedEqualityApplicationId),
    LocalFunction(LocalFunctionId),
    Lambda(LambdaId),
    AnonymousFunction(AnonymousFunctionId),
    CallableReference(CallableReferenceId),
    FunctionAddress(CallableTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultConstructorRef {
    pub target: ExportDefaultConstructorTarget,
    pub target_domain: AccessDomain,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDefaultConstructorTarget {
    Imported {
        declaration: scoop_identity::PersistentConstructorId,
        owner_type: TypeId,
    },
    Struct(StructConstructorApplicationId),
    Class(ClassConstructorApplicationId),
    Variant(EnumVariantApplication),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultTypeRef {
    pub target: ExportDefaultTypeTarget,
    pub target_domain: AccessDomain,
    pub origin: DefinitionOrigin,
}

/// Retains the binder scope of a referenced local declaration signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDefaultTypeTarget {
    Type(TypeId),
    LocalFunctionSignature(LocalFunctionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultGlobalRef {
    pub target: ExportDefaultGlobalTarget,
    pub target_domain: AccessDomain,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDefaultGlobalTarget {
    Local(GlobalId),
    Dependency(scoop_identity::PersistentPropertyId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultSingletonValueRef {
    pub target: SingletonValueTarget,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultFieldRef {
    pub target: FieldRef,
    pub target_domain: AccessDomain,
    pub origin: DefinitionOrigin,
}

/// A complete default source, either declared here or inherited from a dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportDefaultSource {
    Declared {
        expression: ExportDefaultExprId,
        /// Ordered arguments in the consuming declaration's type scope.
        type_arguments: Vec<TypeId>,
    },
    Imported {
        template: std::sync::Arc<ExportDefaultTemplateV1>,
        /// Ordered provider arguments in the consuming declaration's type scope.
        type_arguments: Vec<TypeId>,
    },
}

impl ExportDefaultSource {
    pub fn type_arguments(&self) -> &[TypeId] {
        match self {
            Self::Declared { type_arguments, .. } | Self::Imported { type_arguments, .. } => {
                type_arguments
            }
        }
    }

    pub fn declared(&self) -> Option<(ExportDefaultExprId, &[TypeId])> {
        match self {
            Self::Declared {
                expression,
                type_arguments,
            } => Some((*expression, type_arguments)),
            Self::Imported { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDefaultReceiver {
    pub local: LocalId,
    pub ty: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDefaultValueParameter {
    pub position: u32,
    pub local: LocalId,
}
