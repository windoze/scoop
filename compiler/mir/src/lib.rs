//! MIR definitions and MIR meta: the data channel between MIR and LIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone2/DESIGN.md` section 2.3.
//!
//! M2 notes: structural equality on aggregates is already expanded by
//! mir-lower into primitive comparisons and runtime calls, so MIR
//! `BinOp` only contains primitive operations. Control flow stays
//! structured (if/while); basic blocks appear only in LIR.

use la_arena::{Arena, Idx};
use scoop_ast::Span;

pub type FunctionId = Idx<Function>;
pub type StringConstId = Idx<StringConst>;
pub type StructId = Idx<StructDef>;
pub type EnumId = Idx<EnumDef>;
pub type ClassId = Idx<ClassDef>;
pub type InterfaceId = Idx<InterfaceDef>;
pub type LocalId = Idx<Local>;

/// Mangled symbol of the program entry point (called by the C runtime).
pub const ENTRY_SYMBOL: &str = "scoop_main";

/// Mangle a user function name (entry point maps to `ENTRY_SYMBOL`).
pub fn mangle_function(name: &str, is_entry: bool) -> String {
    if is_entry {
        ENTRY_SYMBOL.to_string()
    } else {
        format!("scoop.{name}")
    }
}

/// Mangle a monomorphized instance: `scoop.<name>$<encoded type args>`.
pub fn mangle_instance(module: &Module, name: &str, type_args: &[Type]) -> String {
    let args: Vec<String> = type_args.iter().map(|t| encode_type(module, t)).collect();
    format!("scoop.{name}${}", args.join("_"))
}

/// Mangle one overload of a name shared by several functions (M7):
/// `scoop.<name>.<encoded params>` — `scoop.show.I`,
/// `scoop.println.S`; a zero-parameter overload gets an empty encoding
/// (`scoop.f.`). `.` introduces the overload encoding while `$` stays
/// reserved for monomorphized instances, so the two never collide.
pub fn mangle_overload(module: &Module, name: &str, params: &[Type]) -> String {
    format!("scoop.{name}.{}", encode_params(module, params))
}

/// The `_`-joined parameter encoding shared by overload mangling and
/// dispatch signature keys.
pub fn encode_params(module: &Module, params: &[Type]) -> String {
    params
        .iter()
        .map(|t| encode_type(module, t))
        .collect::<Vec<_>>()
        .join("_")
}

/// Compact type encoding for mangling (e.g. `scoop.identity$I`).
pub fn encode_type(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "U".to_string(),
        Type::Int => "I".to_string(),
        Type::Boolean => "B".to_string(),
        Type::String => "S".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
        Type::Class(id) => module.classes[*id].name.clone(),
        Type::Interface(id) => module.interfaces[*id].name.clone(),
        Type::Any => "Any".to_string(),
        Type::Array(inner) => format!("A{}X", encode_type(module, inner)),
        Type::MutableArray(inner) => format!("M{}X", encode_type(module, inner)),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| encode_type(module, t)).collect();
            format!("T{}X", inner.join("_"))
        }
        Type::Enum(id, args) => {
            let name = &module.enums[*id].name;
            if args.is_empty() {
                format!("E{name}")
            } else {
                let inner: Vec<String> = args.iter().map(|t| encode_type(module, t)).collect();
                format!("E{}_{}X", name, inner.join("_"))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    Int,
    Boolean,
    String,
    Struct(StructId),
    /// A reference type declared with `class`.
    Class(ClassId),
    /// An interface type (dispatch through itables, impl spec 2.9).
    Interface(InterfaceId),
    /// The root of all types; boxed value types live behind it.
    Any,
    /// Built-in array types (M5, see hir::Type). Invariant (spec 10.4).
    Array(Box<Type>),
    MutableArray(Box<Type>),
    Tuple(Vec<Type>),
    /// An instantiated enum type (including `Option<T>` since M4).
    Enum(EnumId, Vec<Type>),
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// An instantiated enum definition (M4): variants with concrete field
/// types. `name` is the mangled instance name (e.g. `Option$I`).
#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<VariantDef>,
}

#[derive(Debug)]
pub struct VariantDef {
    pub name: String,
    /// Fields in declaration order (named and positional forms both
    /// normalized; positional fields carry `_1`-style names).
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassModifier {
    Final,
    Open,
    Abstract,
}

/// A class definition with its dispatch layout fixed by mir-lower
/// (impl spec 2.9).
#[derive(Debug)]
pub struct ClassDef {
    pub modifier: ClassModifier,
    pub name: String,
    /// Constructor properties in declaration order.
    pub fields: Vec<Field>,
    pub base_class: Option<ClassId>,
    pub interfaces: Vec<InterfaceId>,
    /// vtable slots: 0..2 are the `Any` defaults
    /// (`RuntimeFn::AnyEquals/AnyHashCode/AnyToString`), then user
    /// methods in vtable order (overrides share the base slot).
    pub vtable: Vec<TableSlot>,
    /// itable entries, one per implemented interface (pointer-keyed
    /// lookup at runtime).
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug)]
pub enum TableSlot {
    Function(FunctionId),
    Runtime(RuntimeFn),
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface: InterfaceId,
    pub slots: Vec<TableSlot>,
}

#[derive(Debug)]
pub struct InterfaceDef {
    pub name: String,
    /// Method names in declaration order (itable slot indices).
    pub methods: Vec<String>,
}

#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

#[derive(Debug)]
pub struct Module {
    pub functions: Arena<Function>,
    /// User functions in declaration order (builtins have no MIR body).
    pub top_level: Vec<FunctionId>,
    pub strings: Arena<StringConst>,
    pub structs: Arena<StructDef>,
    pub enums: Arena<EnumDef>,
    pub classes: Arena<ClassDef>,
    pub interfaces: Arena<InterfaceDef>,
    pub entry: FunctionId,
    pub meta: MirMeta,
}

/// Per-Cone MIR metadata (impl spec 2.3). M2: still no class hierarchy,
/// hence no dispatch tables, but the structure exists.
#[derive(Debug, Default)]
pub struct MirMeta {
    pub dispatch_tables: Vec<DispatchTable>,
}

#[derive(Debug)]
pub struct DispatchTable {
    pub owner: FunctionId,
    pub entries: Vec<FunctionId>,
}

#[derive(Debug)]
pub struct StringConst {
    pub value: String,
    /// Mangled global symbol, e.g. `scoop.str.0`.
    pub symbol: String,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    /// Mangled symbol; `scoop.<name>`, `scoop.<name>$<args>` for
    /// monomorphized instances, or `scoop_main` for the entry.
    pub symbol: String,
    pub params: Vec<Param>,
    pub return_ty: Type,
    pub body: Body,
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
    pub statements: Vec<Statement>,
}

#[derive(Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Debug)]
pub struct Try {
    pub body: Vec<Statement>,
    pub catches: Vec<CatchClause>,
    pub finally_body: Option<Vec<Statement>>,
}

#[derive(Debug)]
pub struct CatchClause {
    pub local: LocalId,
    pub ty: Box<Type>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug)]
pub enum StatementKind {
    Expr(Expr),
    Return {
        /// Absent in `Unit` functions (bare `return`).
        value: Option<Expr>,
    },
    ValDecl {
        local: LocalId,
        init: Expr,
    },
    Assign {
        local: LocalId,
        value: Expr,
    },
    /// `array[index] = value` (only `MutableArray`).
    ArraySet {
        array: Expr,
        index: Expr,
        value: Expr,
    },
    /// `obj.field = value`: heap field store (class `var` property;
    /// `index` is the flattened field index — base fields first).
    FieldSet {
        object: Expr,
        index: u32,
        value: Expr,
    },
    /// `try { } catch ... finally { }`; catches are ordered.
    Try(Try),
    /// `throw expr` (throws the evaluated exception object).
    Throw(Expr),
    If {
        cond: Expr,
        then_body: Vec<Statement>,
        /// The else branch genuinely may not exist.
        else_body: Option<Vec<Statement>>,
    },
    While {
        cond: Expr,
        body: Vec<Statement>,
    },
}

#[derive(Debug)]
pub enum Expr {
    StringConst(StringConstId),
    IntLiteral(i64),
    BoolLiteral(bool),
    UnitLiteral,
    TupleLiteral(Vec<Expr>),
    StructInit {
        struct_id: StructId,
        args: Vec<Expr>,
    },
    /// Class instantiation; mir-lower generates a constructor
    /// function per class and this becomes a plain call to it.
    ClassInit {
        class_id: ClassId,
        args: Vec<Expr>,
    },
    Local(LocalId),
    /// Field or element access; `index` is 0-based for both structs
    /// and tuples.
    FieldAccess {
        receiver: Box<Expr>,
        index: u32,
    },
    Call(Call),
    /// Box a value type into `Any` / an interface (spec 4.4.4).
    Box(Box<Expr>),
    /// Unbox a reference back to a value type.
    Unbox(Box<Expr>),
    /// `expr is T` (result `Int`-as-bool). The checked type is in
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
    /// `[e1, ...]` (the kind, Array vs MutableArray, is fixed by the
    /// producing context — LIR types record it).
    ArrayLiteral(Vec<Expr>),
    /// Subscript read; result is the element type.
    ArrayGet {
        array: Box<Expr>,
        index: Box<Expr>,
    },
    /// `array.size`; result is `Int`.
    ArrayLen(Box<Expr>),
    /// `Array(m)` / `MutableArray(a)` conversion: memcpy snapshot.
    ArrayClone(Box<Expr>),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    /// Variant construction; `ty` is the instantiated enum type.
    /// `fields` are the variant's field values in declaration order.
    VariantConstruct {
        ty: Type,
        variant: u32,
        fields: Vec<Expr>,
    },
    /// Read the variant tag of an enum value (Int).
    EnumTag(Box<Expr>),
    /// Read field `index` of variant `variant` from an enum value.
    /// Only evaluated on a path where the tag is known to match.
    EnumField {
        operand: Box<Expr>,
        variant: u32,
        index: u32,
    },
}

#[derive(Debug)]
pub struct Call {
    pub target: CallTarget,
    pub args: Vec<Expr>,
}

#[derive(Debug)]
pub struct CallTarget {
    pub kind: CallKind,
    /// Fully resolved callee.
    pub callee: Callee,
}

#[derive(Debug)]
pub enum CallKind {
    Direct,
    /// vtable slot (load `td` from the receiver, load `vtable[slot]`).
    Virtual {
        slot: u32,
    },
    /// itable lookup (`scoop_rt_itable_lookup(td, iface_td)`), then
    /// `slot` within the returned table.
    Interface {
        interface: InterfaceId,
        slot: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    /// A user function defined in this Cone.
    User(FunctionId),
    /// A runtime function (see `RuntimeFn::symbol`).
    Runtime(RuntimeFn),
}

/// Runtime functions callable from generated code. The output shims
/// are temporary until M11 (docs/milestone1/DESIGN.md 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFn {
    /// `scoop_rt_box(td, payload, size)`
    Box,
    /// `scoop_rt_is_instance(obj, td)`
    IsInstance,
    /// `scoop_rt_itable_lookup(td, iface_td)`
    ITableLookup,
    /// The `Any` vtable defaults (slots 0..2).
    AnyEquals,
    AnyHashCode,
    AnyToString,
    /// Primitive output intrinsics backing core's `print`/`println`
    /// overloads (M7, docs/milestone7/DESIGN.md section 2).
    Write,
    IntToString,
    BoolToString,
    StringConcat,
    StringEq,
    /// Noreturn runtime trap, called with a message string constant
    /// (M4: `!!` on `None`; M8: real exceptions).
    Trap,
}

impl RuntimeFn {
    pub fn symbol(self) -> &'static str {
        match self {
            RuntimeFn::Box => "scoop_rt_box",
            RuntimeFn::IsInstance => "scoop_rt_is_instance",
            RuntimeFn::ITableLookup => "scoop_rt_itable_lookup",
            RuntimeFn::AnyEquals => "scoop_rt_any_equals",
            RuntimeFn::AnyHashCode => "scoop_rt_any_hashcode",
            RuntimeFn::AnyToString => "scoop_rt_any_tostring",
            RuntimeFn::Write => "scoop_rt_print",
            RuntimeFn::IntToString => "scoop_rt_int_to_string",
            RuntimeFn::BoolToString => "scoop_rt_bool_to_string",
            RuntimeFn::StringConcat => "scoop_rt_string_concat",
            RuntimeFn::StringEq => "scoop_rt_string_eq",
            RuntimeFn::Trap => "scoop_rt_trap",
        }
    }
}

/// Primitive operations only: aggregate equality has been expanded by
/// mir-lower (docs/milestone2/DESIGN.md 2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    IntAdd,
    IntSub,
    IntMul,
    IntDiv,
    IntLt,
    IntLe,
    IntGt,
    IntGe,
    IntEq,
    IntNe,
    BoolEq,
    BoolNe,
    /// Short-circuit boolean operators; LIR lowers them to branches.
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    IntNeg,
    BoolNot,
}

/// Indented text dump for golden tests (`scoopc build --emit=mir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, def) in module.structs.iter() {
        let fields: Vec<String> = def
            .fields
            .iter()
            .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
            .collect();
        out.push_str(&format!("  struct {} ({})\n", def.name, fields.join(", ")));
    }
    for (_, def) in module.enums.iter() {
        out.push_str(&format!("  enum {}\n", def.name));
        for variant in &def.variants {
            let fields: Vec<String> = variant
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, type_name(module, &f.ty)))
                .collect();
            out.push_str(&format!("    {}({})\n", variant.name, fields.join(", ")));
        }
    }
    for (_, def) in module.classes.iter() {
        out.push_str(&format!(
            "  class {} vtable={} itables={}\n",
            def.name,
            def.vtable.len(),
            def.itables.len()
        ));
    }
    for (_, def) in module.interfaces.iter() {
        out.push_str(&format!("  interface {}\n", def.name));
    }
    for &id in &module.top_level {
        let function = &module.functions[id];
        let params: Vec<String> = function
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, type_name(module, &p.ty)))
            .collect();
        out.push_str(&format!(
            "  fun {} @{}({}) -> {}\n",
            function.name,
            function.symbol,
            params.join(", "),
            type_name(module, &function.return_ty)
        ));
        dump_statements(
            module,
            &function.body.locals,
            &function.body.statements,
            2,
            &mut out,
        );
    }
    for (_, string) in module.strings.iter() {
        out.push_str(&format!("  str @{} {:?}\n", string.symbol, string.value));
    }
    out.push_str(&format!("  entry @{ENTRY_SYMBOL}\n"));
    out
}

/// Render a type for dumps.
pub fn type_name(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
        Type::Class(id) => module.classes[*id].name.clone(),
        Type::Interface(id) => module.interfaces[*id].name.clone(),
        Type::Any => "Any".to_string(),
        Type::Array(inner) => format!("Array<{}>", type_name(module, inner)),
        Type::MutableArray(inner) => format!("MutableArray<{}>", type_name(module, inner)),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| type_name(module, t)).collect();
            format!("({})", inner.join(", "))
        }
        Type::Enum(id, args) => {
            let name = &module.enums[*id].name;
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(|t| type_name(module, t)).collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
    }
}

fn dump_statements(
    module: &Module,
    locals: &Arena<Local>,
    statements: &[Statement],
    indent: usize,
    out: &mut String,
) {
    for statement in statements {
        let pad = "  ".repeat(indent);
        match &statement.kind {
            StatementKind::Expr(expr) => dump_expr(module, locals, expr, indent, out),
            StatementKind::Return { value } => {
                out.push_str(&format!("{pad}return\n"));
                if let Some(value) = value {
                    dump_expr(module, locals, value, indent + 1, out);
                }
            }
            StatementKind::ValDecl { local, init } => {
                let local = &locals[*local];
                let keyword = if local.mutable { "var" } else { "val" };
                out.push_str(&format!(
                    "{pad}{keyword} {}: {}\n",
                    local.name,
                    type_name(module, &local.ty)
                ));
                dump_expr(module, locals, init, indent + 1, out);
            }
            StatementKind::Try(try_) => {
                out.push_str(&format!("{pad}try\n"));
                dump_statements(module, locals, &try_.body, indent + 1, out);
                for catch in &try_.catches {
                    out.push_str(&format!(
                        "{pad}catch {}: {}\n",
                        locals[catch.local].name,
                        type_name(module, &catch.ty)
                    ));
                    dump_statements(module, locals, &catch.body, indent + 1, out);
                }
                if let Some(finally_body) = &try_.finally_body {
                    out.push_str(&format!("{pad}finally\n"));
                    dump_statements(module, locals, finally_body, indent + 1, out);
                }
            }
            StatementKind::Throw(expr) => {
                out.push_str(&format!("{pad}throw\n"));
                dump_expr(module, locals, expr, indent + 1, out);
            }
            StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                out.push_str(&format!("{pad}field_set {index}\n"));
                dump_expr(module, locals, object, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::ArraySet {
                array,
                index,
                value,
            } => {
                out.push_str(&format!("{pad}array_set\n"));
                dump_expr(module, locals, array, indent + 1, out);
                dump_expr(module, locals, index, indent + 1, out);
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::Assign { local, value } => {
                out.push_str(&format!("{pad}assign {}\n", locals[*local].name));
                dump_expr(module, locals, value, indent + 1, out);
            }
            StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                out.push_str(&format!("{pad}if\n"));
                dump_expr(module, locals, cond, indent + 1, out);
                dump_statements(module, locals, then_body, indent + 1, out);
                if let Some(else_body) = else_body {
                    out.push_str(&format!("{pad}else\n"));
                    dump_statements(module, locals, else_body, indent + 1, out);
                }
            }
            StatementKind::While { cond, body } => {
                out.push_str(&format!("{pad}while\n"));
                dump_expr(module, locals, cond, indent + 1, out);
                dump_statements(module, locals, body, indent + 1, out);
            }
        }
    }
}

fn dump_expr(module: &Module, locals: &Arena<Local>, expr: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::StringConst(id) => {
            out.push_str(&format!(
                "{pad}StringConst @{}\n",
                module.strings[*id].symbol
            ));
        }
        Expr::IntLiteral(value) => out.push_str(&format!("{pad}IntLiteral {value}\n")),
        Expr::BoolLiteral(value) => out.push_str(&format!("{pad}BoolLiteral {value}\n")),
        Expr::UnitLiteral => out.push_str(&format!("{pad}UnitLiteral\n")),
        Expr::TupleLiteral(elements) => {
            out.push_str(&format!("{pad}TupleLiteral\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        Expr::ClassInit { class_id, args } => {
            out.push_str(&format!(
                "{pad}ClassInit {}\n",
                module.classes[*class_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        Expr::StructInit { struct_id, args } => {
            out.push_str(&format!(
                "{pad}StructInit {}\n",
                module.structs[*struct_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        Expr::Local(local) => out.push_str(&format!("{pad}Local {}\n", locals[*local].name)),
        Expr::FieldAccess { receiver, index } => {
            out.push_str(&format!("{pad}FieldAccess {index}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        Expr::Call(call) => {
            let callee = match &call.target.callee {
                Callee::User(id) => format!("@{}", module.functions[*id].symbol),
                Callee::Runtime(function) => format!("@{}", function.symbol()),
            };
            out.push_str(&format!("{pad}Call {callee} direct\n"));
            for arg in &call.args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        Expr::Box(operand) => {
            out.push_str(&format!("{pad}Box\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::Unbox(operand) => {
            out.push_str(&format!("{pad}Unbox\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::IsInstance { operand, check_ty } => {
            out.push_str(&format!(
                "{pad}IsInstance {}\n",
                type_name(module, check_ty)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::Cast { operand, optional } => {
            out.push_str(&format!("{pad}Cast optional={optional}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::ArrayLiteral(elements) => {
            out.push_str(&format!("{pad}ArrayLiteral\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        Expr::ArrayGet { array, index } => {
            out.push_str(&format!("{pad}ArrayGet\n"));
            dump_expr(module, locals, array, indent + 1, out);
            dump_expr(module, locals, index, indent + 1, out);
        }
        Expr::ArrayLen(operand) => {
            out.push_str(&format!("{pad}ArrayLen\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::ArrayClone(operand) => {
            out.push_str(&format!("{pad}ArrayClone\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::Binary { op, lhs, rhs } => {
            out.push_str(&format!("{pad}Binary {op:?}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        Expr::Unary { op, operand } => {
            out.push_str(&format!("{pad}Unary {op:?}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::VariantConstruct {
            ty,
            variant,
            fields,
        } => {
            out.push_str(&format!(
                "{pad}VariantConstruct {} v{}\n",
                type_name(module, ty),
                variant
            ));
            for field in fields {
                dump_expr(module, locals, field, indent + 1, out);
            }
        }
        Expr::EnumTag(operand) => {
            out.push_str(&format!("{pad}EnumTag\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        Expr::EnumField {
            operand,
            variant,
            index,
        } => {
            out.push_str(&format!("{pad}EnumField v{variant} f{index}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
    }
}
