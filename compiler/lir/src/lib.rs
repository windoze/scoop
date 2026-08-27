//! LIR definitions and LIR meta: the data channel between LIR and codegen.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4 and
//! `docs/milestone2/DESIGN.md` section 2.4.
//!
//! LIR contains nothing Scoop-specific: functions are basic blocks of
//! explicit instructions over locals and temporaries, and codegen
//! translates them mechanically. All locals are stack slots (alloca);
//! SSA construction is left to LLVM's mem2reg.

use la_arena::{Arena, Idx};

pub type GlobalId = Idx<Global>;
pub type LocalId = Idx<Local>;
pub type TempId = Idx<Temp>;
pub type BlockId = Idx<BasicBlock>;
pub type EnumDefId = Idx<EnumDef>;

/// Symbol of the TypeDescriptor global for `String` (runtime spec 2.2).
pub const STRING_TD_SYMBOL: &str = "scoop_td_String";

/// Runtime trap (M3: `!!` on `None`; M8: real exceptions).
pub const TRAP_SYMBOL: &str = "scoop_rt_trap";

/// Runtime array clone (spec 10.4 conversions).
pub const ARRAY_CLONE_SYMBOL: &str = "scoop_rt_array_clone";

/// A type after layout resolution: maps directly onto LLVM types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LirType {
    Void,
    I1,
    I64,
    Ptr,
    /// struct / tuple values: an LLVM literal struct.
    Aggregate(Vec<LirType>),
    /// An array object: pointer to `{ td, i64 size, inline elements }`
    /// (spec 10.1). The payload is the element layout.
    Array(Box<LirType>),
    /// An enum value; the representation is fixed by
    /// `EnumDef::repr` (niche pointer or tagged union, spec 7.4).
    Enum(EnumDefId),
}

impl LirType {
    pub fn dump(&self) -> String {
        match self {
            LirType::Void => "void".to_string(),
            LirType::I1 => "i1".to_string(),
            LirType::I64 => "i64".to_string(),
            LirType::Ptr => "ptr".to_string(),
            LirType::Aggregate(elements) => {
                let inner: Vec<String> = elements.iter().map(LirType::dump).collect();
                format!("{{{}}}", inner.join(", "))
            }
            LirType::Enum(id) => format!("enum{}", id.into_raw()),
            LirType::Array(inner) => format!("[{}]", inner.dump()),
        }
    }
}

#[derive(Debug)]
pub struct Module {
    pub globals: Arena<Global>,
    /// Enum definitions with fixed representations (indexed by
    /// `EnumDefId`).
    pub enums: Arena<EnumDef>,
    pub functions: Vec<Function>,
    /// Symbol of the entry function (`scoop_main`).
    pub entry_symbol: String,
    pub meta: LirMeta,
}

/// Per-Cone LIR metadata (impl spec 2.4): type layouts.
#[derive(Debug)]
pub struct LirMeta {
    pub layouts: Vec<Layout>,
    /// TypeDescriptors to emit (runtime spec 2.2): classes, boxed
    /// value types, and interfaces (symbols serve as itable keys).
    /// Emission order is significant: `parent` / interface symbols
    /// must refer to entries in this list (or to
    /// `STRING_TD_SYMBOL`).
    pub type_descriptors: Vec<TypeDescriptor>,
}

/// Everything codegen needs to emit one `ScoopTypeDescriptor`
/// global (see runtime/include/scoop_rt.h for the field order).
#[derive(Debug)]
pub struct TypeDescriptor {
    /// Name for dumps and the default `toString`.
    pub name: String,
    /// Global symbol, e.g. `scoop_td_Point`.
    pub symbol: String,
    /// `type_id` (codegen assigns small integers, starting after the
    /// built-ins).
    pub size: u64,
    pub align: u64,
    pub ref_offsets: Vec<u64>,
    /// Symbol of the parent TypeDescriptor (classes: base class;
    /// boxed value types / interfaces: none).
    pub parent: Option<String>,
    /// vtable slot symbols (functions or `scoop_rt_any_*`).
    pub vtable: Vec<String>,
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface_symbol: String,
    pub slots: Vec<String>,
}

#[derive(Debug)]
pub struct Layout {
    pub name: String,
    pub size: u64,
    pub align: u64,
    pub kind: LayoutKind,
}

#[derive(Debug)]
pub enum LayoutKind {
    Plain {
        /// Byte offsets of reference fields (for the GC bitmap, M9).
        ref_field_offsets: Vec<u64>,
    },
    /// Array layouts: the GC scans `size` elements inline when the
    /// element type is a reference (elements start after header +
    /// size, spec 10.1).
    Array { element_is_ref: bool },
    /// Enum layouts keep per-variant reference offsets: scanning an
    /// enum value depends on its tag (runtime spec 2.2, see
    /// docs/milestone4/DESIGN.md section 7).
    Enum { variants: Vec<VariantLayout> },
}

#[derive(Debug)]
pub struct VariantLayout {
    pub ref_field_offsets: Vec<u64>,
}

#[derive(Debug)]
pub struct Global {
    pub symbol: String,
    pub init: GlobalInit,
}

/// An enum definition with its representation fixed by lir-lower.
#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    pub repr: EnumRepr,
}

#[derive(Debug)]
pub enum EnumRepr {
    /// Niche optimization (spec 7.4): two variants, one without
    /// payload, the other a single reference field — the value is a
    /// bare pointer, `None`-style variant is 0.
    Niche {
        /// Index of the payload-carrying variant.
        payload_variant: u32,
    },
    /// `{ i64 tag, [N x i8] payload }`.
    Tagged {
        /// Field types of each variant, in declaration order.
        variants: Vec<Vec<LirType>>,
        payload_size: u64,
        payload_align: u64,
    },
}

#[derive(Debug)]
pub enum GlobalInit {
    /// A `ScoopString` constant: header points at `STRING_TD_SYMBOL`.
    StringConst(String),
    /// A NUL-terminated C string (e.g. trap messages).
    CString(String),
}

/// A local variable's stack slot.
#[derive(Debug)]
pub struct Local {
    pub name: String,
    pub ty: LirType,
}

/// A temporary SSA-ish value produced by an instruction.
#[derive(Debug)]
pub struct Temp {
    pub ty: LirType,
}

#[derive(Debug)]
pub struct Function {
    pub symbol: String,
    /// Parameter types; arguments are SSA values (`Value::Param`).
    pub params: Vec<LirType>,
    pub return_ty: LirType,
    pub locals: Arena<Local>,
    pub temps: Arena<Temp>,
    pub blocks: Arena<BasicBlock>,
    /// Entry block; every function has exactly one.
    pub entry: BlockId,
}

impl Function {
    /// The type of a value in this function.
    pub fn value_ty(&self, globals: &Arena<Global>, value: Value) -> LirType {
        match value {
            Value::Local(id) => self.locals[id].ty.clone(),
            Value::Temp(id) => self.temps[id].ty.clone(),
            Value::Param(index) => self.params[index as usize].clone(),
            Value::IntConst(_) => LirType::I64,
            Value::BoolConst(_) => LirType::I1,
            Value::Global(id) => {
                let _ = &globals[id];
                LirType::Ptr
            }
        }
    }
}

#[derive(Debug)]
pub struct BasicBlock {
    pub name: String,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

/// A value usable as an instruction operand.
#[derive(Debug, Clone, Copy)]
pub enum Value {
    /// Contents of a local's stack slot (loaded implicitly).
    Local(LocalId),
    /// A function parameter (0-based).
    Param(u32),
    Temp(TempId),
    IntConst(i64),
    BoolConst(bool),
    /// Address of a global constant.
    Global(GlobalId),
}

#[derive(Debug)]
pub enum Instruction {
    /// `out = <op> lhs, rhs` (integer or boolean; the type is on `out`).
    BinOp {
        out: TempId,
        op: BinOp,
        lhs: Value,
        rhs: Value,
    },
    /// `out = -operand` / `out = !operand`.
    UnaryOp {
        out: TempId,
        op: UnOp,
        operand: Value,
    },
    /// Build an aggregate value (struct / tuple construction, or the
    /// Unit value with zero elements).
    MakeAggregate { out: TempId, elements: Vec<Value> },
    /// Extract field / element `index` from an aggregate value.
    ExtractValue {
        out: TempId,
        aggregate: Value,
        index: u32,
    },
    /// `store value -> local`'s stack slot.
    Store { local: LocalId, value: Value },
    /// Heap field store: `object` is a `ptr` to a heap object laid
    /// out as `{ ptr header, fields... }` (the same indexing as
    /// `ExtractValue` on a `ptr` aggregate: 0 = header, 1..=n =
    /// flattened fields, base-class fields first). `index` must be
    /// >= 1.
    HeapStore {
        object: Value,
        index: u32,
        value: Value,
    },
    /// Direct call. `out` is `None` exactly when the callee returns
    /// void; runtime functions with results produce a Temp of the
    /// result type.
    Call {
        out: Option<TempId>,
        symbol: String,
        args: Vec<Value>,
    },
    /// Indirect call through a function table (vtable / itable
    /// dispatch, impl spec 2.9): `table` is a `ptr` to the first slot,
    /// the callee is `table[slot]`. Arguments include the receiver.
    CallIndirect {
        out: Option<TempId>,
        table: Value,
        slot: u32,
        args: Vec<Value>,
    },
    /// Array operations. The element layout is the `Array(...)` type
    /// of the array operand (or of `out` for `ArrayAlloc`).
    /// Allocate an array object and store the elements in order.
    ArrayAlloc { out: TempId, elements: Vec<Value> },
    /// `array.size` (result `I64`).
    ArrayLen { out: TempId, operand: Value },
    /// Bounds-checked element read (traps out of range).
    ArrayGet {
        out: TempId,
        array: Value,
        index: Value,
    },
    /// Bounds-checked element write (traps out of range).
    ArraySet {
        array: Value,
        index: Value,
        value: Value,
    },
    /// `Array(m)` / `MutableArray(a)` conversion (memcpy snapshot).
    ArrayClone { out: TempId, operand: Value },
    /// Enum operations. The representation (niche pointer or tagged
    /// union) is fixed by `EnumDef::repr`, so codegen translates these
    /// mechanically.
    /// Construct a variant value.
    EnumWrap {
        out: TempId,
        enum_id: EnumDefId,
        variant: u32,
        fields: Vec<Value>,
    },
    /// Read the variant tag (result `I64`; niche: null test).
    EnumTag {
        out: TempId,
        enum_id: EnumDefId,
        operand: Value,
    },
    /// Read field `index` of variant `variant`; only emitted on paths
    /// where the tag is known to match.
    EnumField {
        out: TempId,
        enum_id: EnumDefId,
        variant: u32,
        index: u32,
        operand: Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    SDiv,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug)]
pub enum Terminator {
    Br(BlockId),
    CondBr {
        cond: Value,
        then_block: BlockId,
        else_block: BlockId,
    },
    /// `value` is absent exactly for void (Unit-returning) functions.
    Return {
        value: Option<Value>,
    },
    /// After a noreturn call (e.g. the trap function).
    Unreachable,
}

/// Indented text dump for golden tests (`scoopc build --emit=lir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::StringConst(value) => {
                out.push_str(&format!("  global @{} = {:?}\n", global.symbol, value));
            }
            GlobalInit::CString(value) => {
                out.push_str(&format!("  global @{} = c{:?}\n", global.symbol, value));
            }
        }
    }
    for (_, def) in module.enums.iter() {
        match &def.repr {
            EnumRepr::Niche { payload_variant } => out.push_str(&format!(
                "  enum {} niche(payload_variant={})\n",
                def.name, payload_variant
            )),
            EnumRepr::Tagged {
                variants,
                payload_size,
                payload_align,
            } => {
                let variants: Vec<String> = variants
                    .iter()
                    .map(|fields| {
                        let inner: Vec<String> = fields.iter().map(LirType::dump).collect();
                        format!("({})", inner.join(", "))
                    })
                    .collect();
                out.push_str(&format!(
                    "  enum {} tagged size={} align={} variants={}\n",
                    def.name,
                    payload_size,
                    payload_align,
                    variants.join(" ")
                ));
            }
        }
    }
    for function in &module.functions {
        let params: Vec<String> = function.params.iter().map(LirType::dump).collect();
        out.push_str(&format!(
            "  fun @{}({}) -> {}\n",
            function.symbol,
            params.join(", "),
            function.return_ty.dump()
        ));
        for (id, local) in function.locals.iter() {
            out.push_str(&format!(
                "    local %{} {}: {}\n",
                id.into_raw(),
                local.name,
                local.ty.dump()
            ));
        }
        for (block_id, block) in function.blocks.iter() {
            let _ = block_id;
            out.push_str(&format!("  block {}\n", block.name));
            for instruction in &block.instructions {
                dump_instruction(function, instruction, &mut out);
            }
            match &block.terminator {
                Terminator::Br(target) => {
                    out.push_str(&format!("    br @{}\n", block_name(function, *target)))
                }
                Terminator::CondBr {
                    cond,
                    then_block,
                    else_block,
                } => out.push_str(&format!(
                    "    cbr {} then @{} else @{}\n",
                    value_name(*cond),
                    block_name(function, *then_block),
                    block_name(function, *else_block)
                )),
                Terminator::Return { value } => match value {
                    Some(value) => out.push_str(&format!("    ret {}\n", value_name(*value))),
                    None => out.push_str("    ret\n"),
                },
                Terminator::Unreachable => out.push_str("    unreachable\n"),
            }
        }
    }
    for td in &module.meta.type_descriptors {
        out.push_str(&format!(
            "  td {} @{} size={} vtable={} itables={}\n",
            td.name,
            td.symbol,
            td.size,
            td.vtable.len(),
            td.itables.len()
        ));
    }
    for layout in &module.meta.layouts {
        match &layout.kind {
            LayoutKind::Plain { ref_field_offsets } => out.push_str(&format!(
                "  layout {} size={} align={} refs={:?}\n",
                layout.name, layout.size, layout.align, ref_field_offsets
            )),
            LayoutKind::Array { element_is_ref } => out.push_str(&format!(
                "  layout {} size={} align={} array(element_is_ref={})\n",
                layout.name, layout.size, layout.align, element_is_ref
            )),
            LayoutKind::Enum { variants } => out.push_str(&format!(
                "  layout {} size={} align={} enum-refs={:?}\n",
                layout.name,
                layout.size,
                layout.align,
                variants
                    .iter()
                    .map(|v| v.ref_field_offsets.clone())
                    .collect::<Vec<_>>()
            )),
        }
    }
    out.push_str(&format!("  entry @{}\n", module.entry_symbol));
    out
}

fn block_name(function: &Function, id: BlockId) -> String {
    function.blocks[id].name.clone()
}

fn value_name(value: Value) -> String {
    match value {
        Value::Local(id) => format!("local{}", id.into_raw()),
        Value::Param(index) => format!("param{index}"),
        Value::Temp(id) => format!("t{}", id.into_raw()),
        Value::IntConst(value) => format!("{value}"),
        Value::BoolConst(value) => format!("{value}"),
        Value::Global(id) => format!("global{}", id.into_raw()),
    }
}

fn dump_instruction(function: &Function, instruction: &Instruction, buf: &mut String) {
    match instruction {
        Instruction::BinOp { out, op, lhs, rhs } => buf.push_str(&format!(
            "    t{} = {:?} {}, {} : {}\n",
            out.into_raw(),
            op,
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::UnaryOp { out, op, operand } => buf.push_str(&format!(
            "    t{} = {:?} {} : {}\n",
            out.into_raw(),
            op,
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::MakeAggregate { out, elements } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            buf.push_str(&format!(
                "    t{} = aggregate ({}) : {}\n",
                out.into_raw(),
                elements.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ExtractValue {
            out,
            aggregate,
            index,
        } => buf.push_str(&format!(
            "    t{} = extract {}, {} : {}\n",
            out.into_raw(),
            value_name(*aggregate),
            index,
            function.temps[*out].ty.dump()
        )),
        Instruction::HeapStore {
            object,
            index,
            value,
        } => buf.push_str(&format!(
            "    heap_store {} {} {}\n",
            value_name(*object),
            index,
            value_name(*value)
        )),
        Instruction::Store { local, value } => buf.push_str(&format!(
            "    store {} -> local{}\n",
            value_name(*value),
            local.into_raw()
        )),
        Instruction::Call { out, symbol, args } => {
            let args: Vec<String> = args.iter().map(|a| value_name(*a)).collect();
            match out {
                Some(temp) => buf.push_str(&format!(
                    "    t{} = call @{}({}) : {}\n",
                    temp.into_raw(),
                    symbol,
                    args.join(", "),
                    function.temps[*temp].ty.dump()
                )),
                None => buf.push_str(&format!("    call @{}({})\n", symbol, args.join(", "))),
            }
        }
        Instruction::CallIndirect {
            out,
            table,
            slot,
            args,
        } => {
            let args: Vec<String> = args.iter().map(|a| value_name(*a)).collect();
            match out {
                Some(temp) => buf.push_str(&format!(
                    "    t{} = call_indirect {}[{}]({}) : {}\n",
                    temp.into_raw(),
                    value_name(*table),
                    slot,
                    args.join(", "),
                    function.temps[*temp].ty.dump()
                )),
                None => buf.push_str(&format!(
                    "    call_indirect {}[{}]({})\n",
                    value_name(*table),
                    slot,
                    args.join(", ")
                )),
            }
        }
        Instruction::ArrayAlloc { out, elements } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            buf.push_str(&format!(
                "    t{} = array_alloc ({}) : {}\n",
                out.into_raw(),
                elements.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ArrayLen { out, operand } => buf.push_str(&format!(
            "    t{} = array_len {} : {}\n",
            out.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArrayGet { out, array, index } => buf.push_str(&format!(
            "    t{} = array_get {} {} : {}\n",
            out.into_raw(),
            value_name(*array),
            value_name(*index),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArraySet {
            array,
            index,
            value,
        } => buf.push_str(&format!(
            "    array_set {} {} {}\n",
            value_name(*array),
            value_name(*index),
            value_name(*value)
        )),
        Instruction::ArrayClone { out, operand } => buf.push_str(&format!(
            "    t{} = array_clone {} : {}\n",
            out.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::EnumWrap {
            out,
            enum_id,
            variant,
            fields,
        } => {
            let fields: Vec<String> = fields.iter().map(|f| value_name(*f)).collect();
            buf.push_str(&format!(
                "    t{} = enum_wrap e{} v{} ({}) : {}\n",
                out.into_raw(),
                enum_id.into_raw(),
                variant,
                fields.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::EnumTag {
            out,
            enum_id,
            operand,
        } => buf.push_str(&format!(
            "    t{} = enum_tag e{} {} : {}\n",
            out.into_raw(),
            enum_id.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::EnumField {
            out,
            enum_id,
            variant,
            index,
            operand,
        } => buf.push_str(&format!(
            "    t{} = enum_field e{} v{} f{} {} : {}\n",
            out.into_raw(),
            enum_id.into_raw(),
            variant,
            index,
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
    }
}
