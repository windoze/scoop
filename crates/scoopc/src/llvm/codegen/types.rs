//! LLVM codegen 的共享类型与关键不变量。
//!
//! 该模块只放“跨多处 codegen 逻辑共享”的定义（例如 `CgTy`/`CgValue`/enum 表示选择），
//! 以便后续把 `expr/stmt/layout/effect/gc` 等实现分拆到独立文件时，避免循环依赖与散落的常量约定。

use inkwell::values::BasicValueEnum;
use inkwell::values::IntValue;
use inkwell::values::PointerValue;

use crate::ty::TypeId;
use crate::ty::layout::NicheStorage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct IntTy {
    pub(super) bits: u32,
    pub(super) signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CgTy {
    Unit,
    Bool,
    Int(IntTy),
    Tuple(TypeId),
    Struct(TypeId),
    Enum(TypeId),
    /// runtime 字符串对象（early stage）
    ///
    /// 说明：
    /// - `scoop.core.String` 运行期表示为 `ScoopString*`：
    ///   - LLVM 侧使用 `addrspace(1)` 指针，表示其为 GC-managed heap 对象；
    ///   - 对象头为 `ScoopGcObjectHeader`（与 `scoop_alloc` 对齐），其后为 `{ len: i64, data: i8* }`；
    /// - 字符串字面量与 f-string 结果当前都会分配一个 `ScoopString` 对象（T1502b3）。
    String,
    /// 通用引用类型（Any / class / interface / function / union ...）。
    ///
    /// 当前阶段的 codegen 约定：
    /// - 一律用 `i8 addrspace(1)*` 表示（LLVM 文本 IR 在 opaque pointers 下通常显示为 `ptr addrspace(1)`）；
    /// - 值类型向引用类型的隐式转换需要装箱（T0817：先只支持 `Int -> Any`）。
    ///
    /// 未来将替换为带对象头（type descriptor/flags/size）的具体布局（PLAN §8.2/§9.1）。
    Ref,
    /// T1612: bottom type (`Nothing`)。
    ///
    /// `Nothing` 是 uninhabited type：运行时没有值。任何返回类型为 `Nothing` 的表达式都不会
    /// "正常返回"（只能通过 `Raise.raise`、无限循环等控制流终止）。
    ///
    /// 后端不变量：
    /// - `CgTy::Never` 的值不可被 store/load/return/observed；
    /// - 若后端需要占位表示（例如在不可达路径保持 IR 连通），使用 `CgValue::never()`（value: None）。
    Never,
}

/// LLVM GC address space（用于标记 GC-managed 引用指针，后续接入 statepoint/stackmap）。
///
/// 说明：
/// - 约定 `addrspace(1)` 为 GC-managed ref（与运行时 `scoop_alloc` 分配对象一致）；
/// - `addrspace(0)` 保留给“native/unsafe 指针”（例如 malloc buffer、C ABI out pointer、fn_ptr 等）。
pub(super) const GC_ADDRSPACE: u16 = 1;

// boxing / lint 的启发式阈值（与 typecheck::layout.rs 保持一致）。
pub(super) const ENUM_BOX_DISPARITY_RATIO: u64 = 4;
pub(super) const ENUM_BOX_INLINE_THRESHOLD_WORDS: u64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CgEnumRepr {
    TaggedUnion,
    /// niche 优化：无显式 tag，通过 payload 的非法值编码 `None`。
    Niche {
        storage: NicheStorage,
        none_value: u64,
    },
    /// value-only enum：运行期表示就是底层整型标量（spec §2.3.2.1）。
    ValueOnly {
        underlying: IntTy,
    },
}

#[derive(Debug, Clone)]
pub(super) struct CgEnumVariant {
    pub(super) name: String,
    pub(super) tag: u64,
    pub(super) boxed: bool,
    pub(super) fields: Vec<CgTy>,
}

#[derive(Debug, Clone)]
pub(super) struct CgEnumLayout {
    pub(super) repr: CgEnumRepr,
    pub(super) variants: Vec<CgEnumVariant>,
}

/// rich enum / `Option<T>` 的 payload 载体（避免把 GC 指针做 ptr<->int 编码）。
///
/// 约定：
/// - `word`：用于承载 `Bool/Int` 等 word-sized payload，或 boxed payload 的 native 指针（addrspace(0)）；
/// - `gc_ptr`：用于承载 `Ref/String` 等 GC-managed 指针（addrspace(1)），并在 tagged union 表示下写入独立字段。
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CgEnumPayload<'ctx> {
    pub(super) word: Option<IntValue<'ctx>>,
    pub(super) gc_ptr: Option<PointerValue<'ctx>>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CgValue<'ctx> {
    pub(super) ty: CgTy,
    pub(super) value: Option<BasicValueEnum<'ctx>>,
}

impl<'ctx> CgValue<'ctx> {
    pub(super) fn unit() -> Self {
        Self {
            ty: CgTy::Unit,
            value: None,
        }
    }

    pub(super) fn int(value: IntValue<'ctx>, ty: IntTy) -> Self {
        Self {
            ty: CgTy::Int(ty),
            value: Some(value.into()),
        }
    }

    pub(super) fn bool(value: IntValue<'ctx>) -> Self {
        Self {
            ty: CgTy::Bool,
            value: Some(value.into()),
        }
    }

    pub(super) fn as_int(self) -> Option<(IntValue<'ctx>, IntTy)> {
        match self.ty {
            CgTy::Int(ty) => match self.value? {
                BasicValueEnum::IntValue(v) => Some((v, ty)),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn as_bool(self) -> Option<IntValue<'ctx>> {
        match self.ty {
            CgTy::Bool => match self.value? {
                BasicValueEnum::IntValue(v) => Some(v),
                _ => None,
            },
            _ => None,
        }
    }

    /// T1612: placeholder for `Nothing` (bottom type). No runtime value exists.
    pub(super) fn never() -> Self {
        Self {
            ty: CgTy::Never,
            value: None,
        }
    }
}
