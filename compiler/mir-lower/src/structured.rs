//! Private construction IR used before MIR CFG and call normalization.

use la_arena::Arena;
use scoop_hir::concrete::Span;
use scoop_mir as mir;

/// Function-local identity of one structured loop after concrete-HIR ids have
/// been explicitly remapped into this construction IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LoopId(u32);

impl LoopId {
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

#[derive(Debug)]
pub(crate) struct Body {
    pub(crate) locals: Arena<mir::Local>,
    pub(crate) statements: Vec<Statement>,
    /// Present only when this concrete body actually contains a typed suspend
    /// call. Such a body must eliminate native catch state before any source
    /// catch/finally code can become a coroutine suspension context.
    pub(crate) coroutine_eh: Option<CoroutineEhMode>,
}

#[derive(Debug, Clone)]
pub(crate) struct CoroutineEhMode {
    /// The exact local-concrete `Throwable` class type. Managed exception
    /// locals and pending throws never erase this payload to `Any`.
    pub(crate) throwable: mir::Type,
}

#[derive(Debug)]
pub(crate) struct Statement {
    pub(crate) kind: StatementKind,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) struct Try {
    pub(crate) body: Vec<Statement>,
    pub(crate) catches: Vec<CatchClause>,
    pub(crate) finally_body: Option<Vec<Statement>>,
}

#[derive(Debug)]
pub(crate) struct CatchClause {
    pub(crate) local: mir::LocalId,
    pub(crate) ty: Box<mir::Type>,
    pub(crate) body: Vec<Statement>,
    pub(crate) span: Span,
}

/// One source-pattern decision whose tests must remain separate CFG branches.
///
/// A nested enum value is materialized before its test so the test and every
/// guarded payload projection can name the same immutable local.
#[derive(Debug)]
pub(crate) struct PatternDecision {
    pub(crate) steps: Vec<PatternDecisionStep>,
    pub(crate) then_body: Vec<Statement>,
    pub(crate) else_body: Vec<Statement>,
}

#[derive(Debug)]
pub(crate) enum PatternDecisionStep {
    Materialize { local: mir::LocalId, init: Expr },
    Test(Expr),
}

#[derive(Debug)]
pub(crate) enum StatementKind {
    /// A typed upstream proof established that this control-flow edge has no
    /// runtime predecessor. Keeping it explicit prevents Unit fallthrough
    /// completion from turning an impossible edge into a normal return.
    Unreachable,
    Trap {
        message: String,
    },
    Expr(Expr),
    Return {
        value: Option<Expr>,
    },
    Break {
        target: LoopId,
    },
    Continue {
        target: LoopId,
    },
    ValDecl {
        local: mir::LocalId,
        init: Expr,
    },
    Assign {
        local: mir::LocalId,
        value: Expr,
    },
    GlobalAssign {
        global: mir::GlobalId,
        value: Expr,
    },
    ArraySet {
        array_type: mir::ClassId,
        array: Expr,
        index: Expr,
        value: Expr,
    },
    FieldSet {
        object: Expr,
        index: u32,
        value: Expr,
    },
    Try(Try),
    Throw(Expr),
    If {
        cond: Expr,
        then_body: Vec<Statement>,
        else_body: Option<Vec<Statement>>,
    },
    PatternDecision(PatternDecision),
    While {
        target: LoopId,
        /// Statements evaluated at the start of every condition check. This
        /// is a first-class header region rather than a preheader/body copy.
        condition_setup: Vec<Statement>,
        cond: Expr,
        body: Vec<Statement>,
    },
}

/// Call-preserving structured expression used before CFG normalization.
/// It is fully typed at creation so CFG construction never recovers a result
/// type from an enclosing statement or expected context.
#[derive(Debug, Clone)]
pub(crate) struct Expr {
    pub(crate) ty: mir::Type,
    pub(crate) kind: ExprKind,
}

impl Expr {
    pub(crate) fn new(ty: mir::Type, kind: ExprKind) -> Self {
        Self { ty, kind }
    }

    pub(crate) fn local(local: mir::LocalId, ty: mir::Type) -> Self {
        Self::new(ty, ExprKind::Local(local))
    }

    pub(crate) fn integer(value: mir::MirIntegerConstant) -> Self {
        Self::new(
            mir::Type::Integer(value.kind()),
            ExprKind::IntegerLiteral(value),
        )
    }

    pub(crate) fn integer_unary(operation: mir::IntegerUnaryOperation, operand: Self) -> Self {
        assert_eq!(operand.ty, mir::Type::Integer(operation.kind()));
        Self::new(
            mir::Type::Integer(operation.kind()),
            ExprKind::IntegerUnary {
                operation,
                operand: Box::new(operand),
            },
        )
    }

    pub(crate) fn integer_binary(
        operation: mir::IntegerBinaryOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        let ty = mir::Type::Integer(operation.kind());
        assert_eq!(lhs.ty, ty);
        assert_eq!(rhs.ty, ty);
        Self::new(
            ty,
            ExprKind::IntegerBinary {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
    }

    pub(crate) fn safe_integer_div_rem(
        operation: mir::SafeIntegerDivRemOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        let ty = mir::Type::Integer(operation.kind());
        assert_eq!(lhs.ty, ty);
        assert_eq!(rhs.ty, ty);
        Self::new(
            ty,
            ExprKind::SafeIntegerDivRem {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
    }

    pub(crate) fn integer_compare(
        operation: mir::IntegerComparisonOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        let operand_ty = mir::Type::Integer(operation.operand_kind());
        assert_eq!(lhs.ty, operand_ty);
        assert_eq!(rhs.ty, operand_ty);
        Self::new(
            mir::Type::Boolean,
            ExprKind::IntegerCompare {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
    }

    pub(crate) fn integer_compare_to(
        operation: mir::IntegerCompareToOperation,
        lhs: Self,
        rhs: Self,
    ) -> Self {
        let operand_ty = mir::Type::Integer(operation.operand_kind());
        assert_eq!(lhs.ty, operand_ty);
        assert_eq!(rhs.ty, operand_ty);
        Self::new(
            mir::Type::Integer(operation.result_kind()),
            ExprKind::IntegerCompareTo {
                operation,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
        )
    }

    pub(crate) fn integer_shift(
        operation: mir::IntegerShiftOperation,
        value: Self,
        count: Self,
    ) -> Self {
        assert_eq!(value.ty, mir::Type::Integer(operation.value_kind()));
        assert_eq!(count.ty, mir::Type::Integer(operation.count_kind()));
        Self::new(
            mir::Type::Integer(operation.value_kind()),
            ExprKind::IntegerShift {
                operation,
                value: Box::new(value),
                count: Box::new(count),
            },
        )
    }

    pub(crate) fn integer_conversion(conversion: mir::IntegerConversion, operand: Self) -> Self {
        assert_eq!(operand.ty, mir::Type::Integer(conversion.source_kind()));
        Self::new(
            mir::Type::Integer(conversion.target_kind()),
            ExprKind::IntegerConversion {
                conversion,
                operand: Box::new(operand),
            },
        )
    }

    pub(crate) fn machine_scalar(value: mir::MachineScalarValue) -> Self {
        Self::new(
            mir::Type::MachineScalar(value.kind()),
            ExprKind::MachineScalarLiteral(value),
        )
    }

    pub(crate) fn machine_eq(lhs: Self, rhs: mir::MachineScalarValue) -> Self {
        let kind = rhs.kind();
        assert_eq!(
            lhs.ty,
            mir::Type::MachineScalar(kind),
            "machine scalar equality operands have the same semantic kind"
        );
        Self::new(
            mir::Type::Boolean,
            ExprKind::Binary {
                op: mir::BinOp::MachineEq(kind),
                lhs: Box::new(lhs),
                rhs: Box::new(Self::machine_scalar(rhs)),
            },
        )
    }

    pub(crate) fn variant_test(
        enums: &Arena<mir::EnumDef>,
        operand: Self,
        variant: mir::MirVariantRef,
    ) -> Self {
        variant
            .definition(enums)
            .expect("structured MIR carries a checked variant reference");
        assert!(
            matches!(&operand.ty, mir::Type::Enum(enum_id, _) if *enum_id == variant.enum_id()),
            "VariantTest operand uses the checked enum identity"
        );
        Self::new(
            mir::Type::Boolean,
            ExprKind::VariantTest {
                operand: Box::new(operand),
                variant,
            },
        )
    }

    pub(crate) fn variant_payload_project(
        enums: &Arena<mir::EnumDef>,
        operand: Self,
        field: mir::MirVariantFieldRef,
    ) -> Self {
        let ty = field
            .definition(enums)
            .expect("structured MIR carries a checked variant payload field")
            .ty
            .clone();
        assert!(
            matches!(&operand.ty, mir::Type::Enum(enum_id, _) if *enum_id == field.variant().enum_id()),
            "VariantPayloadProject operand uses the checked enum identity"
        );
        Self::new(
            ty,
            ExprKind::VariantPayloadProject {
                operand: Box::new(operand),
                field,
            },
        )
    }

    pub(crate) fn bool(value: bool) -> Self {
        Self::new(mir::Type::Boolean, ExprKind::BoolLiteral(value))
    }

    pub(crate) fn unit() -> Self {
        Self::new(mir::Type::Unit, ExprKind::UnitLiteral)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ExprKind {
    StringConst(mir::StringConstId),
    IntegerLiteral(mir::MirIntegerConstant),
    MachineScalarLiteral(mir::MachineScalarValue),
    CharLiteral(char),
    CharCode(Box<Expr>),
    CharFromCodeUnchecked(Box<Expr>),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        struct_id: mir::StructId,
        args: Vec<Expr>,
    },
    StructConstruct {
        struct_id: mir::StructId,
        fields: Vec<Expr>,
    },
    ClassNew {
        class_id: mir::ClassId,
        publish_release: bool,
        initializer: mir::Callee,
        args: Vec<Expr>,
    },
    ClosureAlloc {
        class: mir::ClosureClassId,
        captures: Vec<ClosureCaptureInit>,
    },
    ClosureCapture {
        closure: Box<Expr>,
        class: mir::ClosureClassId,
        index: u32,
    },
    Local(mir::LocalId),
    GlobalRead(mir::GlobalId),
    InitializationUnitAddress(mir::InitializationUnitId),
    PtrFromNonZeroULong {
        operand: Box<Expr>,
        pointee: Box<mir::Type>,
    },
    PtrToULong(Box<Expr>),
    PtrCast {
        operand: Box<Expr>,
        pointee: Box<mir::Type>,
    },
    PtrLoad {
        pointer: Box<Expr>,
        pointee: Box<mir::Type>,
        offset: Option<Box<Expr>>,
    },
    PtrStore {
        pointer: Box<Expr>,
        pointee: Box<mir::Type>,
        offset: Option<Box<Expr>>,
        value: Box<Expr>,
    },
    PtrOffset {
        pointer: Box<Expr>,
        pointee: Box<mir::Type>,
        offset: Box<Expr>,
        subtract: bool,
    },
    AddressOf {
        local: mir::LocalId,
        pointee: Box<mir::Type>,
    },
    GlobalAddress {
        global: mir::GlobalId,
        pointee: Box<mir::Type>,
    },
    SizeOf(Box<mir::Type>),
    AlignOf(Box<mir::Type>),
    FunctionAddress {
        callback: mir::CallbackBridgeId,
    },
    ForeignCallbackRegister {
        bridge: mir::ForeignCallbackBridgeId,
        closure: Box<Expr>,
    },
    ForeignCallbackOperation {
        operation: mir::ForeignCallbackOperation,
        callback: Box<Expr>,
    },
    Retype {
        operand: Box<Expr>,
        ty: Box<mir::Type>,
    },
    FieldAccess {
        receiver: Box<Expr>,
        index: u32,
    },
    ReleaseFieldLoad {
        class: mir::ClassId,
        index: u32,
    },
    Call(Call),
    Box(Box<Expr>),
    Unbox(Box<Expr>),
    IsInstance {
        operand: Box<Expr>,
        check_ty: Box<mir::Type>,
    },
    /// Zeroed internal storage, reached only after the managed negative-size guard.
    /// The generated loop publishes the array after every element is initialized.
    ArrayAllocate {
        array_type: mir::ClassId,
        count: Box<Expr>,
    },
    ArrayLiteral {
        array_type: mir::ClassId,
        elements: Vec<Expr>,
    },
    ArrayAssembly {
        array_type: mir::ClassId,
        parts: Vec<ArrayAssemblyPart>,
    },
    ArrayGet {
        array_type: mir::ClassId,
        array: Box<Expr>,
        index: Box<Expr>,
    },
    ArrayLen {
        array_type: mir::ClassId,
        operand: Box<Expr>,
    },
    ArrayClone {
        source_type: mir::ClassId,
        target_type: mir::ClassId,
        operand: Box<Expr>,
    },
    Binary {
        op: mir::BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    ShortCircuit {
        op: LogicOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: mir::UnOp,
        operand: Box<Expr>,
    },
    IntegerUnary {
        operation: mir::IntegerUnaryOperation,
        operand: Box<Expr>,
    },
    IntegerBinary {
        operation: mir::IntegerBinaryOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    SafeIntegerDivRem {
        operation: mir::SafeIntegerDivRemOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    IntegerCompare {
        operation: mir::IntegerComparisonOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    IntegerCompareTo {
        operation: mir::IntegerCompareToOperation,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    IntegerShift {
        operation: mir::IntegerShiftOperation,
        value: Box<Expr>,
        count: Box<Expr>,
    },
    IntegerConversion {
        conversion: mir::IntegerConversion,
        operand: Box<Expr>,
    },
    VariantConstruct {
        variant: mir::MirVariantRef,
        fields: Vec<Expr>,
    },
    VariantTest {
        operand: Box<Expr>,
        variant: mir::MirVariantRef,
    },
    VariantPayloadProject {
        operand: Box<Expr>,
        field: mir::MirVariantFieldRef,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ClosureCaptureInit {
    pub(crate) field: u32,
    pub(crate) value: Expr,
}

impl ClosureCaptureInit {
    pub(crate) const fn new(field: u32, value: Expr) -> Self {
        Self { field, value }
    }
}

#[derive(Debug, Clone)]
pub enum ArrayAssemblyPart {
    Element(Expr),
    CopyArray(Expr),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub(crate) struct Call {
    pub(crate) target: mir::CallTarget,
    pub(crate) args: Vec<Expr>,
    pub(crate) return_ty: mir::Type,
}
