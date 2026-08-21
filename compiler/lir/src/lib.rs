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

/// Symbol of the TypeDescriptor global for `String` (runtime spec 2.2).
pub const STRING_TD_SYMBOL: &str = "scoop_td_String";

/// A type after layout resolution: maps directly onto LLVM types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LirType {
    Void,
    I1,
    I64,
    Ptr,
    /// struct / tuple values: an LLVM literal struct.
    Aggregate(Vec<LirType>),
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
        }
    }
}

#[derive(Debug)]
pub struct Module {
    pub globals: Arena<Global>,
    pub functions: Vec<Function>,
    /// Symbol of the entry function (`scoop_main`).
    pub entry_symbol: String,
    pub meta: LirMeta,
}

/// Per-Cone LIR metadata (impl spec 2.4): type layouts.
#[derive(Debug)]
pub struct LirMeta {
    pub layouts: Vec<Layout>,
}

#[derive(Debug)]
pub struct Layout {
    pub name: String,
    pub size: u64,
    pub align: u64,
    /// Byte offsets of reference fields (for the GC bitmap, M9).
    pub ref_field_offsets: Vec<u64>,
}

#[derive(Debug)]
pub struct Global {
    pub symbol: String,
    pub init: GlobalInit,
}

#[derive(Debug)]
pub enum GlobalInit {
    /// A `ScoopString` constant: header points at `STRING_TD_SYMBOL`.
    StringConst(String),
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
    /// Direct call. `out` is `None` exactly when the callee returns
    /// void (all M2 user functions do); runtime functions with results
    /// produce a Temp of the result type.
    Call {
        out: Option<TempId>,
        symbol: String,
        args: Vec<Value>,
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
    /// All M2 functions return void (Unit).
    Return,
}

/// Indented text dump for golden tests (`scoopc build --emit=lir`).
pub fn dump(module: &Module) -> String {
    let mut out = String::from("Module\n");
    for (_, global) in module.globals.iter() {
        match &global.init {
            GlobalInit::StringConst(value) => {
                out.push_str(&format!("  global @{} = {:?}\n", global.symbol, value));
            }
        }
    }
    for function in &module.functions {
        out.push_str(&format!("  fun @{}\n", function.symbol));
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
                Terminator::Return => out.push_str("    ret\n"),
            }
        }
    }
    for layout in &module.meta.layouts {
        out.push_str(&format!(
            "  layout {} size={} align={} refs={:?}\n",
            layout.name, layout.size, layout.align, layout.ref_field_offsets
        ));
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
    }
}
