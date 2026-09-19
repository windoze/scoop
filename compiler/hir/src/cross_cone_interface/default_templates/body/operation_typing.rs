use crate::{DefaultBodyOperationAuthority, DefaultBodyValidationInputV1};

use std::fmt;

use scoop_identity::{
    Effect, PersistentCallbackRegistrationId, PersistentObjectValueId, PersistentPropertyId,
    SignatureTypeKey,
};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use crate::{
    CanonicalBooleanV1, DefaultCallableDeclarationV1, DefaultCallableRefV1,
    DefaultConstructorRefV1, DefaultEnumVariantFieldRefV1, DefaultEnumVariantRefV1,
    DefaultExpressionKindV1, DefaultExpressionV1, DefaultFieldRefV1, DefaultIntegerKindV1,
    DefaultIntegerOperationV1, DefaultIteratorConformanceV1, DefaultIteratorNextV1,
    DefaultLiteralEqualityV1, DefaultMethodCalleeV1, ExportDefaultTemplateV1,
};

mod authority;
mod body;
mod expression;

pub use body::{
    DefaultAssignmentOperationV1, DefaultBindingActionOperationV1, DefaultBindingShapeOperationV1,
    DefaultBodyOperationTypingProblemV1, DefaultBodyOperationTypingSiteV1, DefaultBodyOperationV1,
    DefaultForOperationV1, DefaultPatternOperationV1, DefaultStatementOperationV1,
    ExportDefaultBodyOperationTypingValidationError,
};

#[cfg(test)]
mod tests;

/// Canonical closed core types used by default-body operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultOperationCoreTypeV1 {
    Unit,
    Boolean,
    String,
    Integer(DefaultIntegerKindV1),
    Throwable,
    ForeignCallbackState,
}

/// A recognized application of one compiler-known generic core type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefaultCoreApplicationV1 {
    Array { element: SignatureTypeKey },
    MutableArray { element: SignatureTypeKey },
    Option { element: SignatureTypeKey },
    ForeignCallback { function_type: SignatureTypeKey },
}

impl DefaultCoreApplicationV1 {
    const fn kind(&self) -> DefaultOperationExpectedTypeShapeV1 {
        match self {
            Self::Array { .. } => DefaultOperationExpectedTypeShapeV1::Array,
            Self::MutableArray { .. } => DefaultOperationExpectedTypeShapeV1::MutableArray,
            Self::Option { .. } => DefaultOperationExpectedTypeShapeV1::Option,
            Self::ForeignCallback { .. } => DefaultOperationExpectedTypeShapeV1::ForeignCallback,
        }
    }

    const fn element(&self) -> &SignatureTypeKey {
        match self {
            Self::Array { element } | Self::MutableArray { element } | Self::Option { element } => {
                element
            }
            Self::ForeignCallback { function_type } => function_type,
        }
    }
}

/// Fully applied callable signature. `captures` is the hidden ABI prefix and
/// `receiver` is separate from source value parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultCallableOperationShapeV1 {
    effect: Effect,
    receiver: Option<SignatureTypeKey>,
    captures: Vec<SignatureTypeKey>,
    parameters: Vec<SignatureTypeKey>,
    result: SignatureTypeKey,
}

impl DefaultCallableOperationShapeV1 {
    pub fn new(
        effect: Effect,
        receiver: Option<SignatureTypeKey>,
        captures: Vec<SignatureTypeKey>,
        parameters: Vec<SignatureTypeKey>,
        result: SignatureTypeKey,
    ) -> Self {
        Self {
            effect,
            receiver,
            captures,
            parameters,
            result,
        }
    }

    pub const fn effect(&self) -> Effect {
        self.effect
    }

    pub const fn receiver(&self) -> Option<&SignatureTypeKey> {
        self.receiver.as_ref()
    }

    pub fn captures(&self) -> &[SignatureTypeKey] {
        &self.captures
    }

    pub fn parameters(&self) -> &[SignatureTypeKey] {
        &self.parameters
    }

    pub const fn result(&self) -> &SignatureTypeKey {
        &self.result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultAggregateOperationShapeV1 {
    owner_type: SignatureTypeKey,
    fields: Vec<SignatureTypeKey>,
}

impl DefaultAggregateOperationShapeV1 {
    pub fn new(owner_type: SignatureTypeKey, fields: Vec<SignatureTypeKey>) -> Self {
        Self { owner_type, fields }
    }

    pub const fn owner_type(&self) -> &SignatureTypeKey {
        &self.owner_type
    }

    pub fn fields(&self) -> &[SignatureTypeKey] {
        &self.fields
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultVariantFieldOperationShapeV1 {
    owner_type: SignatureTypeKey,
    declaration_index: u32,
    value_type: SignatureTypeKey,
}

impl DefaultVariantFieldOperationShapeV1 {
    pub const fn new(
        owner_type: SignatureTypeKey,
        declaration_index: u32,
        value_type: SignatureTypeKey,
    ) -> Self {
        Self {
            owner_type,
            declaration_index,
            value_type,
        }
    }

    pub const fn owner_type(&self) -> &SignatureTypeKey {
        &self.owner_type
    }

    pub const fn declaration_index(&self) -> u32 {
        self.declaration_index
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultFieldOperationKindV1 {
    Struct,
    Class,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultFieldOperationShapeV1 {
    kind: DefaultFieldOperationKindV1,
    owner_type: SignatureTypeKey,
    declaration_index: u32,
    value_type: SignatureTypeKey,
    mutable: CanonicalBooleanV1,
}

impl DefaultFieldOperationShapeV1 {
    pub const fn new(
        kind: DefaultFieldOperationKindV1,
        owner_type: SignatureTypeKey,
        declaration_index: u32,
        value_type: SignatureTypeKey,
        mutable: CanonicalBooleanV1,
    ) -> Self {
        Self {
            kind,
            owner_type,
            declaration_index,
            value_type,
            mutable,
        }
    }

    pub const fn kind(&self) -> DefaultFieldOperationKindV1 {
        self.kind
    }

    pub const fn owner_type(&self) -> &SignatureTypeKey {
        &self.owner_type
    }

    pub const fn declaration_index(&self) -> u32 {
        self.declaration_index
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn mutable(&self) -> CanonicalBooleanV1 {
        self.mutable
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultValueOperationShapeV1 {
    value_type: SignatureTypeKey,
    mutable: CanonicalBooleanV1,
}

impl DefaultValueOperationShapeV1 {
    pub const fn new(value_type: SignatureTypeKey, mutable: CanonicalBooleanV1) -> Self {
        Self {
            value_type,
            mutable,
        }
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn mutable(&self) -> CanonicalBooleanV1 {
        self.mutable
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultCallbackOperationShapeV1 {
    closure_type: SignatureTypeKey,
    callback_type: SignatureTypeKey,
}

impl DefaultCallbackOperationShapeV1 {
    pub const fn new(closure_type: SignatureTypeKey, callback_type: SignatureTypeKey) -> Self {
        Self {
            closure_type,
            callback_type,
        }
    }

    pub const fn closure_type(&self) -> &SignatureTypeKey {
        &self.closure_type
    }

    pub const fn callback_type(&self) -> &SignatureTypeKey {
        &self.callback_type
    }
}

/// Kind-specific entity requested from the validated provider authority.
#[derive(Clone, Copy, Debug)]
pub enum DefaultOperationEntityV1<'a> {
    Callable(&'a DefaultCallableRefV1),
    MethodCallee(&'a DefaultMethodCalleeV1),
    Constructor(&'a DefaultConstructorRefV1),
    Struct(&'a SignatureTypeKey),
    Class(&'a SignatureTypeKey),
    Enum(&'a SignatureTypeKey),
    Variant(&'a DefaultEnumVariantRefV1),
    VariantField(&'a DefaultEnumVariantFieldRefV1),
    Global(PersistentPropertyId),
    Singleton(PersistentObjectValueId),
    Field(&'a DefaultFieldRefV1),
    FunctionAddress(DefaultCallableDeclarationV1),
    CallbackRegistration(PersistentCallbackRegistrationId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefaultOperationEntityShapeV1 {
    Callable(DefaultCallableOperationShapeV1),
    Constructor {
        owner_type: SignatureTypeKey,
        parameters: Vec<SignatureTypeKey>,
    },
    Aggregate(DefaultAggregateOperationShapeV1),
    VariantField(DefaultVariantFieldOperationShapeV1),
    Value(DefaultValueOperationShapeV1),
    Field(DefaultFieldOperationShapeV1),
    Type(SignatureTypeKey),
    CallbackRegistration(DefaultCallbackOperationShapeV1),
}

impl DefaultOperationEntityShapeV1 {
    const fn kind(&self) -> DefaultOperationEntityShapeKindV1 {
        match self {
            Self::Callable(_) => DefaultOperationEntityShapeKindV1::Callable,
            Self::Constructor { .. } => DefaultOperationEntityShapeKindV1::Constructor,
            Self::Aggregate(_) => DefaultOperationEntityShapeKindV1::Aggregate,
            Self::VariantField(_) => DefaultOperationEntityShapeKindV1::VariantField,
            Self::Value(_) => DefaultOperationEntityShapeKindV1::Value,
            Self::Field(_) => DefaultOperationEntityShapeKindV1::Field,
            Self::Type(_) => DefaultOperationEntityShapeKindV1::Type,
            Self::CallbackRegistration(_) => {
                DefaultOperationEntityShapeKindV1::CallbackRegistration
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultOperationEntityShapeKindV1 {
    Callable,
    Constructor,
    Aggregate,
    VariantField,
    Value,
    Field,
    Type,
    CallbackRegistration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultOperationTypeRelationV1 {
    ReferenceRetype,
    MemberReceiver,
    FunctionCoercion,
    Boxing,
    Unboxing,
    RuntimeTypeCheck,
    ReferenceIdentity,
    ConcreteGcFreeValue,
    IteratorConformance,
}

#[derive(Clone, Copy, Debug)]
pub enum DefaultOperationIntrinsicV1<'a> {
    IntegerOperation(&'a DefaultIntegerOperationV1),
    IntegerConversion {
        source: DefaultIntegerKindV1,
        target: DefaultIntegerKindV1,
        callable: &'a DefaultCallableRefV1,
    },
    DirectSuper(&'a DefaultMethodCalleeV1),
    LiteralEquality {
        equality: &'a DefaultLiteralEqualityV1,
        subject_type: &'a SignatureTypeKey,
    },
    IteratorNext {
        conformance: &'a DefaultIteratorConformanceV1,
        next: &'a DefaultIteratorNextV1,
    },
    BindingComponent {
        source_type: &'a SignatureTypeKey,
        index: std::num::NonZeroU32,
        call: &'a DefaultExpressionV1,
    },
}

/// Typed semantic facts not reconstructible from the portable expression.
pub trait DefaultOperationTypingSemanticAuthority<E> {
    fn canonical_default_operation_type(
        &mut self,
        template: &ExportDefaultTemplateV1,
        role: DefaultOperationCoreTypeV1,
    ) -> Result<SignatureTypeKey, E>;

    fn classify_default_core_application(
        &mut self,
        template: &ExportDefaultTemplateV1,
        value: &SignatureTypeKey,
    ) -> Result<Option<DefaultCoreApplicationV1>, E>;

    fn default_operation_entity_shape(
        &mut self,
        template: &ExportDefaultTemplateV1,
        entity: DefaultOperationEntityV1<'_>,
    ) -> Result<DefaultOperationEntityShapeV1, E>;

    fn default_operation_type_relation(
        &mut self,
        template: &ExportDefaultTemplateV1,
        relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,
    ) -> Result<bool, E>;

    fn validate_default_operation_intrinsic(
        &mut self,
        template: &ExportDefaultTemplateV1,
        intrinsic: DefaultOperationIntrinsicV1<'_>,
    ) -> Result<(), E>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultExpressionOperationV1 {
    StringLiteral,
    IntegerLiteral,
    BooleanLiteral,
    UnitLiteral,
    TupleLiteral,
    StructInit,
    StructConstruct,
    ClassInit,
    VariantConstruct,
    VariantTest,
    VariantPayloadProject,
    Local,
    GlobalRead,
    SingletonValue,
    Lambda,
    AnonymousFunction,
    CallableReference,
    FunctionCoercion,
    PtrFromNonZeroULong,
    PtrToULong,
    PtrCast,
    PtrLoad,
    PtrStore,
    PtrOffset,
    AddressOf,
    SizeOf,
    AlignOf,
    FunctionAddress,
    ForeignCallbackRegister,
    ForeignCallbackOperation,
    FieldAccess,
    MethodCall,
    DirectSuperMethodCall,
    Box,
    Unbox,
    IsInstance,
    Cast,
    ArrayLiteral,
    ArrayAssembly,
    Index,
    ArraySet,
    ArrayLen,
    ArrayClone,
    Call,
    LocalFunctionCall,
    CallableCall,
    PrimitiveBinary,
    PrimitiveUnary,
    IntegerOperation,
    IntegerConversion,
    Binary,
    Unary,
    SomeWrap,
    NoneLiteral,
    IsSome,
    Unwrap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultOperationValueRoleV1 {
    Result,
    Operand,
    Source,
    Target,
    Initializer,
    Subject,
    Condition,
    ReturnValue,
    Local,
    Interface,
    Element,
    Pointer,
    Offset,
    Value,
    Receiver,
    Index,
    Argument { index: usize },
    Capture { index: usize },
    Field { index: usize },
    Part { index: usize },
    Place,
    Callable,
    CheckedType,
    QueriedType,
    EmbeddedResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DefaultOperationTypingSiteV1 {
    operation: DefaultExpressionOperationV1,
    role: DefaultOperationValueRoleV1,
}

impl DefaultOperationTypingSiteV1 {
    pub const fn operation(self) -> DefaultExpressionOperationV1 {
        self.operation
    }

    pub const fn role(self) -> DefaultOperationValueRoleV1 {
        self.role
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultOperationExpectedTypeShapeV1 {
    Function,
    NativeFunctionPointer,
    RawPointer,
    Tuple,
    Array,
    MutableArray,
    Option,
    ForeignCallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultOperationTypingProblemV1 {
    EmptyTuple,
    MissingLocal,
    MissingCallableReceiver,
    UnexpectedCallableReceiver,
    UnexpectedCallableCaptures,
    SuspendNotAllowed,
    LocalDeclarationMismatch,
    FunctionCoercionRequired,
    FieldKindMismatch {
        expected: DefaultFieldOperationKindV1,
        actual: DefaultFieldOperationKindV1,
    },
    InvalidArrayAccess,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultOperationTypingValidationError<E> {
    Authority {
        site: DefaultOperationTypingSiteV1,
        error: E,
    },
    Type {
        site: DefaultOperationTypingSiteV1,
        expected: Box<SignatureTypeKey>,
        actual: Box<SignatureTypeKey>,
    },
    Arity {
        site: DefaultOperationTypingSiteV1,
        expected: usize,
        actual: usize,
    },
    TypeShape {
        site: DefaultOperationTypingSiteV1,
        expected: DefaultOperationExpectedTypeShapeV1,
        actual: Box<SignatureTypeKey>,
    },
    CoreApplication {
        site: DefaultOperationTypingSiteV1,
        expected: DefaultOperationExpectedTypeShapeV1,
        actual: Option<DefaultOperationExpectedTypeShapeV1>,
    },
    EntityShape {
        site: DefaultOperationTypingSiteV1,
        expected: DefaultOperationEntityShapeKindV1,
        actual: DefaultOperationEntityShapeKindV1,
    },
    Relation {
        site: DefaultOperationTypingSiteV1,
        relation: DefaultOperationTypeRelationV1,
        source: Box<SignatureTypeKey>,
        target: Box<SignatureTypeKey>,
    },
    Problem {
        site: DefaultOperationTypingSiteV1,
        problem: DefaultOperationTypingProblemV1,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultOperationTypingValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authority { site, error } => {
                write!(
                    formatter,
                    "default operation authority failed at {site:?}: {error}"
                )
            }
            Self::Type {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default operation type mismatch at {site:?}: expected {expected:?}, found {actual:?}"
            ),
            Self::Arity {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default operation arity mismatch at {site:?}: expected {expected}, found {actual}"
            ),
            Self::TypeShape {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default operation type at {site:?} must be {expected:?}, found {actual:?}"
            ),
            Self::CoreApplication {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default operation core application at {site:?} must be {expected:?}, found {actual:?}"
            ),
            Self::EntityShape {
                site,
                expected,
                actual,
            } => write!(
                formatter,
                "default operation authority shape at {site:?} must be {expected:?}, found {actual:?}"
            ),
            Self::Relation {
                site,
                relation,
                source,
                target,
            } => write!(
                formatter,
                "default operation relation {relation:?} does not hold at {site:?}: {source:?} -> {target:?}"
            ),
            Self::Problem { site, problem } => {
                write!(
                    formatter,
                    "invalid default operation at {site:?}: {problem:?}"
                )
            }
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default operation typing resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultOperationTypingValidationError<E>
{
}

struct Validator<'a, A, E> {
    template: DefaultBodyValidationInputV1<'a>,
    authority: &'a mut A,
    meter: &'a mut BudgetMeter,
    path: &'a WirePath,
    error: std::marker::PhantomData<fn() -> E>,
}

impl<A, E> Validator<'_, A, E>
where
    A: DefaultBodyOperationAuthority<E>,
{
    fn site(
        operation: DefaultExpressionOperationV1,
        role: DefaultOperationValueRoleV1,
    ) -> DefaultOperationTypingSiteV1 {
        DefaultOperationTypingSiteV1 { operation, role }
    }

    fn enter_node(
        &mut self,
        depth: u64,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)?;
        self.meter
            .charge_nodes(1, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)
    }

    fn child_depth(
        &mut self,
        parent: u64,
    ) -> Result<u64, ExportDefaultOperationTypingValidationError<E>> {
        let depth = parent.checked_add(1).ok_or_else(|| {
            ExportDefaultOperationTypingValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                self.path.clone(),
                None,
            ))
        })?;
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)?;
        self.meter
            .charge_edges(1, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)?;
        Ok(depth)
    }

    fn charge_work(&mut self) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.meter
            .charge_work(1, self.path)
            .map_err(ExportDefaultOperationTypingValidationError::Resource)
    }

    fn core_type(
        &mut self,
        role: DefaultOperationCoreTypeV1,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<SignatureTypeKey, ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        self.authority
            .canonical_default_operation_type(role, self.meter, self.path)
            .map_err(|error| ExportDefaultOperationTypingValidationError::Authority { site, error })
    }

    fn core_application(
        &mut self,
        value: &SignatureTypeKey,
        expected: DefaultOperationExpectedTypeShapeV1,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<DefaultCoreApplicationV1, ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        let actual = self
            .authority
            .classify_default_core_application(value, self.meter, self.path)
            .map_err(
                |error| ExportDefaultOperationTypingValidationError::Authority { site, error },
            )?;
        if actual
            .as_ref()
            .is_none_or(|actual| actual.kind() != expected)
        {
            return Err(
                ExportDefaultOperationTypingValidationError::CoreApplication {
                    site,
                    expected,
                    actual: actual.as_ref().map(DefaultCoreApplicationV1::kind),
                },
            );
        }
        Ok(actual.expect("the matching core application is present"))
    }

    fn entity_shape(
        &mut self,
        entity: DefaultOperationEntityV1<'_>,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<DefaultOperationEntityShapeV1, ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        self.authority
            .default_operation_entity_shape(entity, self.meter, self.path)
            .map_err(|error| ExportDefaultOperationTypingValidationError::Authority { site, error })
    }

    fn expect_type(
        &mut self,
        actual: &SignatureTypeKey,
        expected: &SignatureTypeKey,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        if actual == expected {
            Ok(())
        } else {
            Err(ExportDefaultOperationTypingValidationError::Type {
                site,
                expected: Box::new(expected.clone()),
                actual: Box::new(actual.clone()),
            })
        }
    }

    fn expect_result(
        &mut self,
        expression: &DefaultExpressionV1,
        operation: DefaultExpressionOperationV1,
        principal: &SignatureTypeKey,
        allow_reference_retype: bool,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        let site = Self::site(operation, DefaultOperationValueRoleV1::Result);
        let result = expression.result_type();
        if result == principal {
            return self.charge_work();
        }
        if !allow_reference_retype {
            return self.expect_type(result, principal, site);
        }
        if matches!(principal, SignatureTypeKey::Function { .. })
            && matches!(result, SignatureTypeKey::Function { .. })
        {
            return Err(ExportDefaultOperationTypingValidationError::Problem {
                site,
                problem: DefaultOperationTypingProblemV1::FunctionCoercionRequired,
            });
        }
        self.expect_relation(
            DefaultOperationTypeRelationV1::ReferenceRetype,
            principal,
            result,
            site,
        )
    }

    fn expect_relation(
        &mut self,
        relation: DefaultOperationTypeRelationV1,
        source: &SignatureTypeKey,
        target: &SignatureTypeKey,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        let valid = self
            .authority
            .default_operation_type_relation(relation, source, target, self.meter, self.path)
            .map_err(
                |error| ExportDefaultOperationTypingValidationError::Authority { site, error },
            )?;
        if valid {
            Ok(())
        } else {
            Err(ExportDefaultOperationTypingValidationError::Relation {
                site,
                relation,
                source: Box::new(source.clone()),
                target: Box::new(target.clone()),
            })
        }
    }

    fn expect_arity(
        &mut self,
        actual: usize,
        expected: usize,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        if actual == expected {
            Ok(())
        } else {
            Err(ExportDefaultOperationTypingValidationError::Arity {
                site,
                expected,
                actual,
            })
        }
    }

    fn expect_function<'type_>(
        &mut self,
        value: &'type_ SignatureTypeKey,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<
        (&'type_ [SignatureTypeKey], &'type_ SignatureTypeKey, Effect),
        ExportDefaultOperationTypingValidationError<E>,
    > {
        self.charge_work()?;
        match value {
            SignatureTypeKey::Function {
                effect,
                parameters,
                result,
            } => Ok((parameters, result, *effect)),
            _ => Err(ExportDefaultOperationTypingValidationError::TypeShape {
                site,
                expected: DefaultOperationExpectedTypeShapeV1::Function,
                actual: Box::new(value.clone()),
            }),
        }
    }

    fn expect_raw_pointer<'type_>(
        &mut self,
        value: &'type_ SignatureTypeKey,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<&'type_ SignatureTypeKey, ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        match value {
            SignatureTypeKey::RawPointer(pointee) => Ok(pointee),
            _ => Err(ExportDefaultOperationTypingValidationError::TypeShape {
                site,
                expected: DefaultOperationExpectedTypeShapeV1::RawPointer,
                actual: Box::new(value.clone()),
            }),
        }
    }

    fn expect_native_function_pointer(
        &mut self,
        value: &SignatureTypeKey,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        if matches!(value, SignatureTypeKey::NativeFunctionPointer { .. }) {
            Ok(())
        } else {
            Err(ExportDefaultOperationTypingValidationError::TypeShape {
                site,
                expected: DefaultOperationExpectedTypeShapeV1::NativeFunctionPointer,
                actual: Box::new(value.clone()),
            })
        }
    }

    fn validate_intrinsic(
        &mut self,
        intrinsic: DefaultOperationIntrinsicV1<'_>,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        self.authority
            .validate_default_operation_intrinsic(intrinsic, self.meter, self.path)
            .map_err(|error| ExportDefaultOperationTypingValidationError::Authority { site, error })
    }

    fn ensure_callable_effect(
        &mut self,
        shape: &DefaultCallableOperationShapeV1,
        site: DefaultOperationTypingSiteV1,
    ) -> Result<(), ExportDefaultOperationTypingValidationError<E>> {
        self.charge_work()?;
        if shape.effect() == Effect::Suspend && !self.template.allows_suspend().value() {
            Err(ExportDefaultOperationTypingValidationError::Problem {
                site,
                problem: DefaultOperationTypingProblemV1::SuspendNotAllowed,
            })
        } else {
            Ok(())
        }
    }
}

fn expression_operation(kind: &DefaultExpressionKindV1) -> DefaultExpressionOperationV1 {
    match kind {
        DefaultExpressionKindV1::StringLiteral { .. } => {
            DefaultExpressionOperationV1::StringLiteral
        }
        DefaultExpressionKindV1::IntegerLiteral(_) => DefaultExpressionOperationV1::IntegerLiteral,
        DefaultExpressionKindV1::BooleanLiteral(_) => DefaultExpressionOperationV1::BooleanLiteral,
        DefaultExpressionKindV1::UnitLiteral => DefaultExpressionOperationV1::UnitLiteral,
        DefaultExpressionKindV1::TupleLiteral(_) => DefaultExpressionOperationV1::TupleLiteral,
        DefaultExpressionKindV1::StructInit { .. } => DefaultExpressionOperationV1::StructInit,
        DefaultExpressionKindV1::StructConstruct { .. } => {
            DefaultExpressionOperationV1::StructConstruct
        }
        DefaultExpressionKindV1::ClassInit { .. } => DefaultExpressionOperationV1::ClassInit,
        DefaultExpressionKindV1::VariantConstruct { .. } => {
            DefaultExpressionOperationV1::VariantConstruct
        }
        DefaultExpressionKindV1::VariantTest { .. } => DefaultExpressionOperationV1::VariantTest,
        DefaultExpressionKindV1::VariantPayloadProject { .. } => {
            DefaultExpressionOperationV1::VariantPayloadProject
        }
        DefaultExpressionKindV1::Local(_) => DefaultExpressionOperationV1::Local,
        DefaultExpressionKindV1::GlobalRead(_) => DefaultExpressionOperationV1::GlobalRead,
        DefaultExpressionKindV1::SingletonValue(_) => DefaultExpressionOperationV1::SingletonValue,
        DefaultExpressionKindV1::Lambda(_) => DefaultExpressionOperationV1::Lambda,
        DefaultExpressionKindV1::AnonymousFunction(_) => {
            DefaultExpressionOperationV1::AnonymousFunction
        }
        DefaultExpressionKindV1::CallableReference(_) => {
            DefaultExpressionOperationV1::CallableReference
        }
        DefaultExpressionKindV1::FunctionCoercion { .. } => {
            DefaultExpressionOperationV1::FunctionCoercion
        }
        DefaultExpressionKindV1::PtrFromNonZeroULong(_) => {
            DefaultExpressionOperationV1::PtrFromNonZeroULong
        }
        DefaultExpressionKindV1::PtrToULong(_) => DefaultExpressionOperationV1::PtrToULong,
        DefaultExpressionKindV1::PtrCast(_) => DefaultExpressionOperationV1::PtrCast,
        DefaultExpressionKindV1::PtrLoad { .. } => DefaultExpressionOperationV1::PtrLoad,
        DefaultExpressionKindV1::PtrStore { .. } => DefaultExpressionOperationV1::PtrStore,
        DefaultExpressionKindV1::PtrOffset { .. } => DefaultExpressionOperationV1::PtrOffset,
        DefaultExpressionKindV1::AddressOf(_) => DefaultExpressionOperationV1::AddressOf,
        DefaultExpressionKindV1::SizeOf(_) => DefaultExpressionOperationV1::SizeOf,
        DefaultExpressionKindV1::AlignOf(_) => DefaultExpressionOperationV1::AlignOf,
        DefaultExpressionKindV1::FunctionAddress(_) => {
            DefaultExpressionOperationV1::FunctionAddress
        }
        DefaultExpressionKindV1::ForeignCallbackRegister { .. } => {
            DefaultExpressionOperationV1::ForeignCallbackRegister
        }
        DefaultExpressionKindV1::ForeignCallbackOperation { .. } => {
            DefaultExpressionOperationV1::ForeignCallbackOperation
        }
        DefaultExpressionKindV1::FieldAccess { .. } => DefaultExpressionOperationV1::FieldAccess,
        DefaultExpressionKindV1::MethodCall { .. } => DefaultExpressionOperationV1::MethodCall,
        DefaultExpressionKindV1::DirectSuperMethodCall { .. } => {
            DefaultExpressionOperationV1::DirectSuperMethodCall
        }
        DefaultExpressionKindV1::Box(_) => DefaultExpressionOperationV1::Box,
        DefaultExpressionKindV1::Unbox(_) => DefaultExpressionOperationV1::Unbox,
        DefaultExpressionKindV1::IsInstance { .. } => DefaultExpressionOperationV1::IsInstance,
        DefaultExpressionKindV1::Cast { .. } => DefaultExpressionOperationV1::Cast,
        DefaultExpressionKindV1::ArrayLiteral(_) => DefaultExpressionOperationV1::ArrayLiteral,
        DefaultExpressionKindV1::ArrayAssembly(_) => DefaultExpressionOperationV1::ArrayAssembly,
        DefaultExpressionKindV1::Index { .. } => DefaultExpressionOperationV1::Index,
        DefaultExpressionKindV1::ArraySet { .. } => DefaultExpressionOperationV1::ArraySet,
        DefaultExpressionKindV1::ArrayLen(_) => DefaultExpressionOperationV1::ArrayLen,
        DefaultExpressionKindV1::ArrayClone(_) => DefaultExpressionOperationV1::ArrayClone,
        DefaultExpressionKindV1::Call { .. } => DefaultExpressionOperationV1::Call,
        DefaultExpressionKindV1::LocalFunctionCall { .. } => {
            DefaultExpressionOperationV1::LocalFunctionCall
        }
        DefaultExpressionKindV1::CallableCall { .. } => DefaultExpressionOperationV1::CallableCall,
        DefaultExpressionKindV1::PrimitiveBinary { .. } => {
            DefaultExpressionOperationV1::PrimitiveBinary
        }
        DefaultExpressionKindV1::PrimitiveUnary { .. } => {
            DefaultExpressionOperationV1::PrimitiveUnary
        }
        DefaultExpressionKindV1::IntegerOperation { .. } => {
            DefaultExpressionOperationV1::IntegerOperation
        }
        DefaultExpressionKindV1::IntegerConversion { .. } => {
            DefaultExpressionOperationV1::IntegerConversion
        }
        DefaultExpressionKindV1::Binary { .. } => DefaultExpressionOperationV1::Binary,
        DefaultExpressionKindV1::Unary { .. } => DefaultExpressionOperationV1::Unary,
        DefaultExpressionKindV1::SomeWrap(_) => DefaultExpressionOperationV1::SomeWrap,
        DefaultExpressionKindV1::NoneLiteral => DefaultExpressionOperationV1::NoneLiteral,
        DefaultExpressionKindV1::IsSome(_) => DefaultExpressionOperationV1::IsSome,
        DefaultExpressionKindV1::Unwrap { .. } => DefaultExpressionOperationV1::Unwrap,
    }
}
