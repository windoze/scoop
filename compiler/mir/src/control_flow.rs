use super::*;

mod calls;
mod visit;

pub use calls::*;
pub use visit::*;

#[derive(Debug)]
pub struct Function {
    /// Whether this body participates in managed GC instrumentation.
    pub gc_effect: GcEffect,
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: Type,
    pub body: Body,
}

#[derive(Debug)]
pub struct ExternFunction {
    pub source_contract: SourceNativeExternalContractRecord,
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub abi: ExternAbi,
    pub calling_convention: CallingConvention,
    pub gc_effect: GcEffect,
    pub params: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
    Scoop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcEffect {
    Managed,
    NoGc,
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    /// Parameters are (immutable) locals.
    pub local: LocalId,
}

#[derive(Debug)]
pub struct Body {
    pub locals: Arena<Local>,
    pub blocks: Arena<BasicBlock>,
    pub entry: BlockId,
    /// Normalized loop headers that require a managed safepoint poll when
    /// entered. The typed target is body-local and survives CFG rewrites that
    /// add alternate coroutine resume entries.
    pub loop_header_polls: Vec<LoopHeaderPollTarget>,
}

/// A body-local, explicitly marked loop header. Consumers must not rediscover
/// this fact from block names, dominance, or back edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoopHeaderPollTarget {
    header: BlockId,
}

impl LoopHeaderPollTarget {
    pub fn new(header: BlockId) -> Self {
        Self { header }
    }

    pub fn header(self) -> BlockId {
        self.header
    }
}

/// Body-local destination reached after one generated cleanup body has
/// completed normally.  It is intentionally incompatible with source loop,
/// resume-entry, and exception targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoroutineCleanupFallthroughTarget {
    block: BlockId,
}

impl CoroutineCleanupFallthroughTarget {
    pub const fn new(block: BlockId) -> Self {
        Self { block }
    }

    pub const fn block(self) -> BlockId {
        self.block
    }
}

/// Body-local destination of a pending `break` after its cleanup suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoroutineLoopExitTarget {
    block: BlockId,
}

impl CoroutineLoopExitTarget {
    pub const fn new(block: BlockId) -> Self {
        Self { block }
    }

    pub const fn block(self) -> BlockId {
        self.block
    }
}

/// Body-local destination of a pending `continue` after its cleanup suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoroutineLoopHeaderTarget {
    block: BlockId,
}

impl CoroutineLoopHeaderTarget {
    pub const fn new(block: BlockId) -> Self {
        Self { block }
    }

    pub const fn block(self) -> BlockId {
        self.block
    }
}

/// Body-local managed-exception successor.  Propagation out of the function
/// is represented by `None`, rather than by a forged block id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoroutineUnwindTarget {
    block: BlockId,
}

impl CoroutineUnwindTarget {
    pub const fn new(block: BlockId) -> Self {
        Self { block }
    }

    pub const fn block(self) -> BlockId {
        self.block
    }
}

/// Body-local entry selected by the coroutine driver's state dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoroutineResumeEntryTarget {
    block: BlockId,
}

impl CoroutineResumeEntryTarget {
    pub const fn new(block: BlockId) -> Self {
        Self { block }
    }

    pub const fn block(self) -> BlockId {
        self.block
    }
}

impl Body {
    /// A valid body for signature-only function shells. It contains one
    /// unreachable entry block so downstream code never handles a missing
    /// entry or an empty block arena.
    pub fn unreachable(locals: Arena<Local>) -> Self {
        let mut blocks = Arena::new();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            statements: Vec::new(),
            terminator: Terminator::Unreachable,
            unwind: None,
        });
        Self {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct BasicBlock {
    pub name: String,
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
    /// Explicit exceptional successor for every potentially throwing user,
    /// virtual, or interface call evaluated in this block.
    pub unwind: Option<BlockId>,
}

#[derive(Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: SourceSpan,
}

#[derive(Debug)]
pub enum StatementKind {
    Expr(Expr),
    Call(CallEffect),
    /// Runs on the normal edge after the complete outer initializer call.
    PublishReleaseReady {
        class: ClassId,
        receiver: Expr,
    },
    ValDecl {
        local: LocalId,
        init: Expr,
    },
    Assign {
        local: LocalId,
        value: Expr,
    },
    GlobalAssign {
        global: GlobalId,
        value: Expr,
    },
    ArraySet {
        array_type: ClassId,
        array: Expr,
        index: Expr,
        value: Expr,
    },
    FieldSet {
        object: Expr,
        index: u32,
        value: Expr,
    },
    /// Release-store an aligned 64-bit synthetic state field. This is a
    /// distinct MIR operation so coroutine synchronization cannot be lost by
    /// reconstructing atomic intent from field names downstream.
    AtomicFieldStore {
        kind: MachineScalarKind,
        object: Expr,
        index: u32,
        value: Expr,
    },
    Eh(EhStatement),
}

#[derive(Debug)]
pub enum CallEffect {
    /// A call whose source result type is `Unit`.
    Unit(Call),
    /// A value-producing call. The destination local carries the complete,
    /// non-optional result type.
    Value { destination: LocalId, call: Call },
}

#[derive(Debug)]
pub enum EhStatement {
    /// Capture the active native exception into function-local EH slots.
    LandingPad {
        cleanup: bool,
    },
    /// Begin the catch represented by the captured exception.
    BeginCatch,
    EndCatch,
}

#[derive(Debug)]
pub enum Terminator {
    Goto(BlockId),
    Branch {
        cond: Expr,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return {
        value: Option<Expr>,
    },
    /// Throw a managed exception. `unwind` is the same edge recorded on
    /// the owning block and is repeated here because this terminator itself
    /// initiates unwinding rather than containing a call expression.
    Throw {
        exception: Expr,
        unwind: Option<BlockId>,
    },
    /// Rethrow the currently active native catch.
    Rethrow {
        unwind: Option<BlockId>,
    },
    /// Continue native unwinding with the exception record captured by the
    /// nearest landing/cleanup pad.
    Resume,
    /// A native fatal diagnostic, not a managed String value.
    Trap {
        message: String,
    },
    Unreachable,
}

/// A MIR expression whose semantic result type is complete by construction.
/// Consumers must use `ty` directly; reconstructing it from the expression
/// shape, surrounding local or expected context is forbidden.
#[derive(Debug, Clone)]
pub struct Expr {
    pub ty: Type,
    pub kind: ExprKind,
}

impl Expr {
    pub fn new(ty: Type, kind: ExprKind) -> Self {
        Self { ty, kind }
    }

    pub fn local(local: LocalId, ty: Type) -> Self {
        Self::new(ty, ExprKind::Local(local))
    }

    pub fn integer(value: MirIntegerConstant) -> Self {
        Self::new(Type::Integer(value.kind()), ExprKind::IntegerLiteral(value))
    }

    pub fn integer_unary(operation: IntegerUnaryOperation, operand: Self) -> Self {
        assert_eq!(
            operand.ty,
            Type::Integer(operation.kind()),
            "integer unary operand has the operation's exact kind"
        );
        Self::new(
            Type::Integer(operation.kind()),
            ExprKind::IntegerUnary {
                operation,
                operand: Box::new(operand),
            },
        )
    }

    pub fn integer_binary(operation: IntegerBinaryOperation, lhs: Self, rhs: Self) -> Self {
        let ty = Type::Integer(operation.kind());
        assert_eq!(
            lhs.ty, ty,
            "integer binary lhs has the operation's exact kind"
        );
        assert_eq!(
            rhs.ty, ty,
            "integer binary rhs has the operation's exact kind"
        );
        Self::new(
            ty,
            ExprKind::IntegerBinary {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
    }

    pub fn safe_integer_div_rem(
        operation: SafeIntegerDivRemOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        let ty = Type::Integer(operation.kind());
        assert_eq!(
            lhs.ty, ty,
            "safe integer div/rem lhs has the operation's exact kind"
        );
        assert_eq!(
            rhs.ty, ty,
            "safe integer div/rem rhs has the operation's exact kind"
        );
        Self::new(
            ty,
            ExprKind::SafeIntegerDivRem {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
    }

    pub fn integer_compare(operation: IntegerComparisonOperation, lhs: Self, rhs: Self) -> Self {
        let operand_ty = Type::Integer(operation.operand_kind());
        assert_eq!(
            lhs.ty, operand_ty,
            "integer comparison lhs has the operation's exact kind"
        );
        assert_eq!(
            rhs.ty, operand_ty,
            "integer comparison rhs has the operation's exact kind"
        );
        Self::new(
            Type::Boolean,
            ExprKind::IntegerCompare {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
    }

    pub fn integer_compare_to(operation: IntegerCompareToOperation, lhs: Self, rhs: Self) -> Self {
        let operand_ty = Type::Integer(operation.operand_kind());
        assert_eq!(
            lhs.ty, operand_ty,
            "integer compareTo lhs has the operation's exact kind"
        );
        assert_eq!(
            rhs.ty, operand_ty,
            "integer compareTo rhs has the operation's exact kind"
        );
        Self::new(
            Type::Integer(operation.result_kind()),
            ExprKind::IntegerCompareTo {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
    }

    pub fn integer_shift(operation: IntegerShiftOperation, value: Self, count: Self) -> Self {
        assert_eq!(
            value.ty,
            Type::Integer(operation.value_kind()),
            "integer shift value has the operation's exact kind"
        );
        assert_eq!(
            count.ty,
            Type::Integer(operation.count_kind()),
            "integer shift count is normalized to the value's exact width"
        );
        Self::new(
            Type::Integer(operation.value_kind()),
            ExprKind::IntegerShift {
                operation,
                value: Box::new(value),
                count: Box::new(count),
            },
        )
    }

    pub fn integer_conversion(conversion: IntegerConversion, operand: Self) -> Self {
        assert_eq!(
            operand.ty,
            Type::Integer(conversion.source_kind()),
            "integer conversion operand has the exact source kind"
        );
        Self::new(
            Type::Integer(conversion.target_kind()),
            ExprKind::IntegerConversion {
                conversion,
                operand: Box::new(operand),
            },
        )
    }

    /// Preserve the validated nonzero source proof while converting the raw
    /// `ULong` carrier into a data pointer. There is deliberately no MIR
    /// operation for converting an arbitrary (possibly zero) integer to a
    /// pointer.
    pub fn ptr_from_non_zero_ulong(operand: Self, pointee: Type) -> Self {
        assert_eq!(
            operand.ty,
            Type::Integer(IntegerKind::UNSIGNED_64),
            "nonzero pointer conversion consumes the ULong carrier"
        );
        Self::new(
            Type::Ptr(Box::new(pointee.clone())),
            ExprKind::PtrFromNonZeroULong {
                operand: Box::new(operand),
                pointee: Box::new(pointee),
            },
        )
    }

    pub fn machine_scalar(value: MachineScalarValue) -> Self {
        Self::new(
            Type::MachineScalar(value.kind()),
            ExprKind::MachineScalarLiteral(value),
        )
    }

    pub fn machine_eq(lhs: Self, rhs: MachineScalarValue) -> Self {
        let kind = rhs.kind();
        assert_eq!(
            lhs.ty,
            Type::MachineScalar(kind),
            "machine scalar equality operands have the same semantic kind"
        );
        Self::new(
            Type::Boolean,
            ExprKind::Binary {
                op: BinOp::MachineEq(kind),
                lhs: Box::new(lhs),
                rhs: Box::new(Self::machine_scalar(rhs)),
            },
        )
    }

    pub fn bool(value: bool) -> Self {
        Self::new(Type::Boolean, ExprKind::BoolLiteral(value))
    }

    pub fn unit() -> Self {
        Self::new(Type::Unit, ExprKind::UnitLiteral)
    }

    pub fn caught_exception() -> Self {
        Self::new(Type::Any, ExprKind::CaughtException)
    }

    pub fn enum_tag(operand: Expr) -> Self {
        Self::new(
            Type::MachineScalar(MachineScalarKind::EnumTag),
            ExprKind::EnumTag(Box::new(operand)),
        )
    }

    /// Test one checked semantic variant without exposing its physical tag or
    /// niche representation to MIR.
    pub fn variant_test(
        enums: &Arena<EnumDef>,
        operand: Expr,
        variant: MirVariantRef,
    ) -> Result<Self, MirVariantExprError> {
        variant
            .definition(enums)
            .map_err(MirVariantExprError::InvalidVariant)?;
        validate_variant_operand(enums, &operand, variant)?;
        Ok(Self::new(
            Type::Boolean,
            ExprKind::VariantTest {
                operand: Box::new(operand),
                variant,
            },
        ))
    }

    /// Project one checked payload field. The result type comes only from the
    /// field definition that produced `field`.
    pub fn variant_payload_project(
        enums: &Arena<EnumDef>,
        operand: Expr,
        field: MirVariantFieldRef,
    ) -> Result<Self, MirVariantExprError> {
        let ty = field
            .definition(enums)
            .map_err(MirVariantExprError::InvalidField)?
            .ty
            .clone();
        validate_variant_operand(enums, &operand, field.variant())?;
        Ok(Self::new(
            ty,
            ExprKind::VariantPayloadProject {
                operand: Box::new(operand),
                field,
            },
        ))
    }
}

fn validate_variant_operand(
    enums: &Arena<EnumDef>,
    operand: &Expr,
    variant: MirVariantRef,
) -> Result<(), MirVariantExprError> {
    let Type::Enum(actual, actual_arguments) = &operand.ty else {
        return Err(MirVariantExprError::OperandIsNotEnum {
            actual: operand.ty.clone(),
        });
    };
    if *actual != variant.enum_id() {
        return Err(MirVariantExprError::OperandEnumMismatch {
            expected: variant.enum_id(),
            actual: *actual,
        });
    }
    let Type::Enum(_, expected_arguments) = variant
        .enum_type(enums)
        .map_err(MirVariantExprError::InvalidVariant)?
    else {
        unreachable!("a checked MIR variant always produces an enum type")
    };
    if actual_arguments != &expected_arguments {
        return Err(MirVariantExprError::OperandTypeArgumentsMismatch {
            enum_id: variant.enum_id(),
            expected: expected_arguments,
            actual: actual_arguments.clone(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirVariantExprError {
    InvalidVariant(MirVariantRefError),
    InvalidField(MirVariantFieldRefError),
    OperandIsNotEnum {
        actual: Type,
    },
    OperandEnumMismatch {
        expected: EnumId,
        actual: EnumId,
    },
    OperandTypeArgumentsMismatch {
        enum_id: EnumId,
        expected: Vec<Type>,
        actual: Vec<Type>,
    },
}

impl std::fmt::Display for MirVariantExprError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidVariant(error) => error.fmt(formatter),
            Self::InvalidField(error) => error.fmt(formatter),
            Self::OperandIsNotEnum { actual } => {
                write!(
                    formatter,
                    "variant operation requires an enum operand, got {actual:?}"
                )
            }
            Self::OperandEnumMismatch { expected, actual } => write!(
                formatter,
                "variant operation expects MIR enum {}, got MIR enum {}",
                expected.into_raw().into_u32(),
                actual.into_raw().into_u32()
            ),
            Self::OperandTypeArgumentsMismatch {
                enum_id,
                expected,
                actual,
            } => write!(
                formatter,
                "variant operation expects MIR enum {} arguments {expected:?}, got {actual:?}",
                enum_id.into_raw().into_u32()
            ),
        }
    }
}

impl std::error::Error for MirVariantExprError {}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Context(ContextOperation<Expr>),
    StringConst(StringConstId),
    IntegerLiteral(MirIntegerConstant),
    MachineScalarLiteral(MachineScalarValue),
    BoolLiteral(bool),
    CharLiteral(char),
    FloatLiteral(scoop_identity::FloatConstant),
    FloatUnary {
        kind: FloatKind,
        operation: FloatUnaryOperator,
        operand: Box<Expr>,
    },
    FloatBinary {
        kind: FloatKind,
        operation: FloatBinaryOperator,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    FloatConversion {
        conversion: MirFloatConversion,
        operand: Box<Expr>,
    },
    CharCode(Box<Expr>),
    CharFromCodeUnchecked(Box<Expr>),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        struct_id: StructId,
        args: Vec<Expr>,
    },
    /// Compiler-only raw reconstruction in declaration field order.
    StructConstruct {
        struct_id: StructId,
        fields: Vec<Expr>,
    },
    /// Class instantiation; mir-lower generates a constructor
    /// function per class and this becomes a plain call to it.
    /// Allocate one exact class object with a zeroed complete payload.
    /// Field initialization is performed by subsequent typed initializer calls.
    ClassAlloc {
        class_id: ClassId,
    },
    ClosureAlloc {
        class: ClosureClassId,
        /// Initializers remain in language evaluation order. Each entry
        /// names its independently identity-ordered physical field.
        captures: Vec<ClosureCaptureInit>,
    },
    /// Read one inline capture field from a concrete closure object.
    ClosureCapture {
        closure: Box<Expr>,
        class: ClosureClassId,
        index: u32,
    },
    Local(LocalId),
    GlobalRead(GlobalId),
    /// Address of the image descriptor for one typed exactly-once unit.
    InitializationUnitAddress(InitializationUnitId),
    /// Integer-to-pointer conversion with a validation proof that the source
    /// `ULong` value is nonzero.
    PtrFromNonZeroULong {
        operand: Box<Expr>,
        pointee: Box<Type>,
    },
    PtrToULong(Box<Expr>),
    PtrCast {
        operand: Box<Expr>,
        pointee: Box<Type>,
    },
    PtrLoad {
        pointer: Box<Expr>,
        pointee: Box<Type>,
        offset: Option<Box<Expr>>,
    },
    PtrStore {
        pointer: Box<Expr>,
        pointee: Box<Type>,
        offset: Option<Box<Expr>>,
        value: Box<Expr>,
    },
    PtrOffset {
        pointer: Box<Expr>,
        pointee: Box<Type>,
        offset: Box<Expr>,
        subtract: bool,
    },
    AddressOf {
        local: LocalId,
        pointee: Box<Type>,
    },
    GlobalAddress {
        global: GlobalId,
        pointee: Box<Type>,
    },
    SizeOf(Box<Type>),
    AlignOf(Box<Type>),
    FunctionAddress {
        callback: CallbackBridgeId,
    },
    ForeignCallbackRegister {
        bridge: ForeignCallbackBridgeId,
        closure: Box<Expr>,
    },
    ForeignCallbackOperation {
        operation: ForeignCallbackOperation,
        callback: Box<Expr>,
    },
    /// The managed exception pointer produced by the active `BeginCatch`.
    /// It is only valid in blocks dominated by that statement.
    CaughtException,
    /// Zero-cost reference retyping (for example an `Any` local narrowed by
    /// a class smart cast). The explicit result type keeps downstream field
    /// and dispatch reconstruction independent of the local's declared type.
    Retype {
        operand: Box<Expr>,
        ty: Box<Type>,
    },
    /// Field or element access; `index` is 0-based for both structs
    /// and tuples.
    FieldAccess {
        receiver: Box<Expr>,
        index: u32,
    },
    ReleaseFieldLoad {
        class: ClassId,
        index: u32,
    },
    /// Acquire-load an aligned 64-bit synthetic state field.
    AtomicFieldLoad {
        kind: MachineScalarKind,
        object: Box<Expr>,
        index: u32,
    },
    /// Compare-exchange an aligned 64-bit synthetic state field. The returned
    /// value is the observed old word; success is acq_rel and failure acquire.
    AtomicFieldCompareExchange {
        kind: MachineScalarKind,
        object: Box<Expr>,
        index: u32,
        expected: Box<Expr>,
        replacement: Box<Expr>,
    },
    /// Box a value type into `Any` / an interface (spec 4.4.4).
    Box(Box<Expr>),
    /// Unbox a reference back to a value type.
    Unbox(Box<Expr>),
    /// `expr is T` (result `Boolean`). The checked type is in
    /// `check_ty`.
    IsInstance {
        operand: Box<Expr>,
        check_ty: Box<Type>,
    },
    /// `as` (traps on failure) or `as?` (`optional`, result
    /// `Option<T>`); the target type comes from context.
    Cast {
        operand: Box<Expr>,
        optional: bool,
    },
    /// `[e1, ...]`; `array_type` is the exact fully specialized intrinsic
    /// class application selected by HIR.
    /// Zeroed internal storage, reached only after the managed negative-size guard.
    /// The generated loop publishes the array after every element is initialized.
    ArrayAllocate {
        array_type: ClassId,
        count: Box<Expr>,
    },
    ArrayLiteral {
        array_type: ClassId,
        elements: Vec<Expr>,
    },
    /// Fresh array formed from individual elements and copied source arrays.
    ArrayAssembly {
        array_type: ClassId,
        parts: Vec<ArrayAssemblyPart>,
    },
    /// Subscript read; result is the element type.
    ArrayGet {
        array_type: ClassId,
        array: Box<Expr>,
        index: Box<Expr>,
    },
    /// `array.size`; result is canonical `Long`.
    ArrayLen {
        array_type: ClassId,
        operand: Box<Expr>,
    },
    /// Array-kind conversion (constructor or method form): memcpy snapshot.
    /// Both source and target identities are explicit and complete.
    ArrayClone {
        source_type: ClassId,
        target_type: ClassId,
        operand: Box<Expr>,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    IntegerUnary {
        operation: IntegerUnaryOperation,
        operand: Box<Expr>,
    },
    IntegerBinary {
        operation: IntegerBinaryOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    SafeIntegerDivRem {
        operation: SafeIntegerDivRemOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    IntegerCompare {
        operation: IntegerComparisonOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    IntegerCompareTo {
        operation: IntegerCompareToOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    IntegerShift {
        operation: IntegerShiftOperation,
        value: Box<Expr>,
        count: Box<Expr>,
    },
    IntegerConversion {
        conversion: IntegerConversion,
        operand: Box<Expr>,
    },
    /// Variant construction. The enclosing `Expr::ty` is the instantiated
    /// enum type; `fields` are the variant's values in declaration order.
    VariantConstruct {
        variant: MirVariantRef,
        fields: Vec<Expr>,
    },
    /// Read the variant tag of an enum value (internal enum-tag scalar).
    EnumTag(Box<Expr>),
    /// Read field `index` of variant `variant` from an enum value.
    /// Only evaluated on a path where the tag is known to match.
    EnumField {
        operand: Box<Expr>,
        variant: u32,
        index: u32,
    },
    /// Representation-independent test of one checked semantic variant.
    VariantTest {
        operand: Box<Expr>,
        variant: MirVariantRef,
    },
    /// Representation-independent projection of one checked payload field.
    VariantPayloadProject {
        operand: Box<Expr>,
        field: MirVariantFieldRef,
    },
}

#[derive(Debug, Clone)]
pub struct ClosureCaptureInit {
    field: u32,
    value: Expr,
}

impl ClosureCaptureInit {
    pub const fn new(field: u32, value: Expr) -> Self {
        Self { field, value }
    }

    pub const fn field(&self) -> u32 {
        self.field
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }

    pub const fn value_mut(&mut self) -> &mut Expr {
        &mut self.value
    }
}

#[derive(Debug, Clone)]
pub enum ArrayAssemblyPart {
    Element(Expr),
    CopyArray(Expr),
}

/// Primitive operations only: aggregate equality has been expanded by
/// mir-lower (docs/milestone2/DESIGN.md 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// Equality within one compiler-owned scalar domain. The kind is carried
    /// explicitly so lower stages never infer it from a physical integer.
    MachineEq(MachineScalarKind),
    /// Identity comparison of two reference-represented source values.
    RefEq,
    RefNe,
    BoolEq,
    BoolNe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    BoolNot,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_integer_operations_fix_result_and_operand_contracts() {
        let kind = IntegerKind::UNSIGNED_8;
        let lhs = Expr::integer(MirIntegerConstant::Unsigned8(2));
        let rhs = Expr::integer(MirIntegerConstant::Unsigned8(3));

        let binary = Expr::integer_binary(
            IntegerBinaryOperation::new(kind, IntegerBinaryOperator::Add),
            lhs.clone(),
            rhs.clone(),
        );
        assert_eq!(binary.ty, Type::Integer(kind));

        let comparison = Expr::integer_compare(
            IntegerComparisonOperation::new(kind, IntegerComparisonOperator::LessThan),
            lhs.clone(),
            rhs.clone(),
        );
        assert_eq!(comparison.ty, Type::Boolean);

        let compare_to = Expr::integer_compare_to(IntegerCompareToOperation::new(kind), lhs, rhs);
        assert_eq!(compare_to.ty, Type::Integer(IntegerKind::SIGNED_64));
    }

    #[test]
    fn shifts_store_value_width_normalized_counts_and_signed_only_ushr() {
        let signed_8_ushr =
            IntegerShiftOperation::new(IntegerKind::SIGNED_8, IntegerShiftOperator::UnsignedRight)
                .expect("signed integers provide ushr");
        assert_eq!(signed_8_ushr.value_kind(), IntegerKind::SIGNED_8);
        assert_eq!(signed_8_ushr.count_kind(), IntegerKind::SIGNED_8);
        assert_eq!(
            IntegerShiftOperation::new(
                IntegerKind::UNSIGNED_8,
                IntegerShiftOperator::UnsignedRight,
            ),
            None
        );

        let shift = Expr::integer_shift(
            signed_8_ushr,
            Expr::integer(MirIntegerConstant::Signed8(0xff)),
            Expr::integer(MirIntegerConstant::Signed8(1)),
        );
        assert_eq!(shift.ty, Type::Integer(IntegerKind::SIGNED_8));
    }

    #[test]
    fn safe_div_rem_has_a_dedicated_structural_proof_node() {
        let kind = IntegerKind::SIGNED_16;
        let operation = SafeIntegerDivRemOperation::new(kind, SafeIntegerDivRemOperator::Remainder);
        let expression = Expr::safe_integer_div_rem(
            operation,
            Expr::integer(MirIntegerConstant::Signed16(7)),
            Expr::integer(MirIntegerConstant::Signed16(3)),
        );

        assert_eq!(expression.ty, Type::Integer(kind));
        assert!(matches!(
            expression.kind,
            ExprKind::SafeIntegerDivRem {
                operation: found,
                ..
            } if found == operation
        ));
    }

    #[test]
    fn integer_conversion_carries_both_exact_kinds() {
        let conversion = IntegerConversion::new(IntegerKind::SIGNED_16, IntegerKind::UNSIGNED_64);
        let expression = Expr::integer_conversion(
            conversion,
            Expr::integer(MirIntegerConstant::Signed16(0xffff)),
        );
        assert_eq!(conversion.source_kind(), IntegerKind::SIGNED_16);
        assert_eq!(conversion.target_kind(), IntegerKind::UNSIGNED_64);
        assert_eq!(expression.ty, Type::Integer(IntegerKind::UNSIGNED_64));
    }

    #[test]
    fn nonzero_ulong_pointer_conversion_has_a_dedicated_typed_node() {
        let expression = Expr::ptr_from_non_zero_ulong(
            Expr::integer(MirIntegerConstant::Unsigned64(1)),
            Type::Integer(IntegerKind::SIGNED_32),
        );
        assert_eq!(
            expression.ty,
            Type::Ptr(Box::new(Type::Integer(IntegerKind::SIGNED_32)))
        );
        assert!(matches!(
            expression.kind,
            ExprKind::PtrFromNonZeroULong { operand, .. }
                if operand.ty == Type::Integer(IntegerKind::UNSIGNED_64)
        ));
    }

    #[test]
    #[should_panic(expected = "nonzero pointer conversion consumes the ULong carrier")]
    fn nonzero_ulong_pointer_conversion_rejects_other_carriers() {
        let _ = Expr::ptr_from_non_zero_ulong(
            Expr::integer(MirIntegerConstant::Signed64(1)),
            Type::Integer(IntegerKind::SIGNED_32),
        );
    }

    #[test]
    #[should_panic(expected = "integer shift count is normalized to the value's exact width")]
    fn integer_shift_rejects_non_normalized_count_kind() {
        let operation =
            IntegerShiftOperation::new(IntegerKind::SIGNED_8, IntegerShiftOperator::Left)
                .expect("left shift is valid for every integer kind");
        let _ = Expr::integer_shift(
            operation,
            Expr::integer(MirIntegerConstant::Signed8(1)),
            Expr::integer(MirIntegerConstant::Signed64(1)),
        );
    }
}
