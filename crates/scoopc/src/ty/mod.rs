//! 编译器内部类型表示（early stage）。
//!
//! 目标（T0401）：
//! - 在编译器内部引入稳定的 `TypeId`/`TypeKind` 结构，作为 typecheck 的基础设施
//! - 显式区分引用类型（GC-managed）与值类型（copy 语义）
//! - 支持最小 builtin：`Any`/`String`/`Nothing`/`Unit`/`Bool`/`Option<T>` 与整数族 `Int/UInt/IntN/UIntN`
//!
//! 当前阶段只提供数据结构与格式化输出；类型推断/求解、subtyping 等语义在后续任务实现。

use std::collections::HashMap;
use std::fmt;

/// `TypeStore` 内部类型表的索引。
///
/// 说明：
/// - 目前用 `u32` 足够覆盖编译期需要的类型数量
/// - 后续若引入跨 session 的类型缓存或增量编译，可再调整表示
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(u32);

impl TypeId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// 编译器内部的类型种类。
///
/// 这里把“引用类型 vs 值类型”作为第一层分类，便于后续：
/// - 决定布局与 ABI（value types 可内联，ref types 走对象头/指针）
/// - 决定 GC 扫描策略（ref types 需要追踪；value types 递归含 ref 字段时另行处理）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Ref(RefTypeKind),
    Value(ValueTypeKind),
}

impl TypeKind {
    pub fn is_ref(&self) -> bool {
        matches!(self, TypeKind::Ref(_))
    }

    pub fn is_value(&self) -> bool {
        matches!(self, TypeKind::Value(_))
    }
}

/// 引用类型（GC-managed）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RefTypeKind {
    /// 顶层类型：所有引用类型的 supertype。
    ///
    /// 说明：值类型装箱到 `Any` 属于后续任务（PLAN §4.3）。
    Any,

    /// `String`：内建字符串类型（引用类型，GC-managed）。
    ///
    /// 说明：该类型在源级可由 sysroot 声明，但其布局/语义由编译器与运行时固定。
    String,

    /// 名义引用类型（class/interface/effect 等）。
    Nominal(NominalType),
}

/// 名义类型（nominal type）的最小表示。
///
/// 说明：
/// - 早期阶段（T0403）仅需要 “FQN + type args” 来完成 TypeRef lowering 与 arity 检查；
/// - 更丰富的信息（字段/方法、布局、vtable 等）会在后续阶段逐步接入。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NominalType {
    pub fqn: String,
    pub args: Vec<TypeId>,
}

/// 值类型（copy 语义）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueTypeKind {
    /// `Unit`：0 元 tuple 的语义等价物（spec §2.3.3）。
    Unit,
    /// `Nothing`：bottom / uninhabited（例如 `Raise.raise` 的返回类型）。
    Nothing,

    /// `Bool`：内建布尔类型（值类型）。
    ///
    /// 说明：该类型在源级可由 sysroot 声明，但其布局/语义由编译器与运行时固定。
    Bool,

    /// word-sized 整数（随 target 指针宽度变化，spec §2.3.4）。
    Int,
    /// word-sized 无符号整数。
    UInt,
    /// 固定位宽有符号整数，例如 `Int32`。
    IntN(u16),
    /// 固定位宽无符号整数，例如 `UInt64`。
    UIntN(u16),

    /// `Option<T>`：nullable sugar `T?` 的 desugar 目标（spec §2.4）。
    Option(TypeId),

    /// Tuple 类型（为后续 tuple/Unit 表达式类型检查做准备）。
    Tuple(Vec<TypeId>),

    /// 名义值类型（struct/enum 等）。
    Nominal(NominalType),
}

/// 类型表：负责分配 `TypeId` 并存储 `TypeKind`。
///
/// 当前阶段采用最简单的“push-only arena”。去重/哈希化 interning 可在后续需要时再引入。
#[derive(Debug, Default)]
pub struct TypeStore {
    kinds: Vec<TypeKind>,
    index: HashMap<TypeKind, TypeId>,
}

impl TypeStore {
    pub fn new() -> Self {
        Self {
            kinds: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn kind(&self, id: TypeId) -> &TypeKind {
        &self.kinds[id.0 as usize]
    }

    pub fn intern(&mut self, kind: TypeKind) -> TypeId {
        if let Some(id) = self.index.get(&kind).copied() {
            return id;
        }

        let id = TypeId(u32::try_from(self.kinds.len()).expect("too many types"));
        self.kinds.push(kind.clone());
        self.index.insert(kind, id);
        id
    }

    pub fn display<'a>(&'a self, id: TypeId) -> TypeDisplay<'a> {
        TypeDisplay { store: self, id }
    }

    pub fn is_ref(&self, id: TypeId) -> bool {
        self.kind(id).is_ref()
    }

    pub fn is_value(&self, id: TypeId) -> bool {
        self.kind(id).is_value()
    }

    /// 构造并返回一组常用 builtin 类型的 `TypeId`。
    pub fn intern_builtins(&mut self) -> BuiltinTypes {
        BuiltinTypes {
            any: self.intern(TypeKind::Ref(RefTypeKind::Any)),
            string: self.intern(TypeKind::Ref(RefTypeKind::String)),
            unit: self.intern(TypeKind::Value(ValueTypeKind::Unit)),
            nothing: self.intern(TypeKind::Value(ValueTypeKind::Nothing)),
            bool_: self.intern(TypeKind::Value(ValueTypeKind::Bool)),
            int: self.intern(TypeKind::Value(ValueTypeKind::Int)),
            uint: self.intern(TypeKind::Value(ValueTypeKind::UInt)),
        }
    }

    pub fn ty_int_n(&mut self, bits: u16) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::IntN(bits)))
    }

    pub fn ty_uint_n(&mut self, bits: u16) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::UIntN(bits)))
    }

    pub fn ty_option(&mut self, inner: TypeId) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Option(inner)))
    }

    pub fn ty_tuple(&mut self, elements: Vec<TypeId>) -> TypeId {
        self.intern(TypeKind::Value(ValueTypeKind::Tuple(elements)))
    }
}

/// `TypeStore` 中 builtin 类型的 ID 集合。
#[derive(Debug, Clone, Copy)]
pub struct BuiltinTypes {
    pub any: TypeId,
    pub string: TypeId,
    pub unit: TypeId,
    pub nothing: TypeId,
    pub bool_: TypeId,
    pub int: TypeId,
    pub uint: TypeId,
}

/// `TypeId` 的可格式化视图（需要 `TypeStore` 才能递归打印）。
pub struct TypeDisplay<'a> {
    store: &'a TypeStore,
    id: TypeId,
}

impl fmt::Display for TypeDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_type(self.store, self.id, f, 0)
    }
}

fn format_type(store: &TypeStore, id: TypeId, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
    // 防御性：后续引入递归类型（例如自引用 struct）时避免栈爆。
    if depth > 64 {
        return write!(f, "<type-recursion>");
    }

    match store.kind(id) {
        TypeKind::Ref(RefTypeKind::Any) => write!(f, "Any"),
        TypeKind::Ref(RefTypeKind::String) => write!(f, "String"),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => format_nominal(store, n, f, depth),
        TypeKind::Value(ValueTypeKind::Unit) => write!(f, "Unit"),
        TypeKind::Value(ValueTypeKind::Nothing) => write!(f, "Nothing"),
        TypeKind::Value(ValueTypeKind::Bool) => write!(f, "Bool"),
        TypeKind::Value(ValueTypeKind::Int) => write!(f, "Int"),
        TypeKind::Value(ValueTypeKind::UInt) => write!(f, "UInt"),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => write!(f, "Int{bits}"),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => write!(f, "UInt{bits}"),
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            write!(f, "Option<")?;
            format_type(store, *inner, f, depth + 1)?;
            write!(f, ">")
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            write!(f, "(")?;
            for (idx, element) in elements.iter().copied().enumerate() {
                if idx != 0 {
                    write!(f, ", ")?;
                }
                format_type(store, element, f, depth + 1)?;
            }
            if elements.len() == 1 {
                // 单元素 tuple 需要 trailing comma 以避免与括号表达式混淆。
                write!(f, ",")?;
            }
            write!(f, ")")
        }
        TypeKind::Value(ValueTypeKind::Nominal(n)) => format_nominal(store, n, f, depth),
    }
}

fn format_nominal(
    store: &TypeStore,
    nominal: &NominalType,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
) -> fmt::Result {
    write!(f, "{}", nominal.fqn)?;
    if !nominal.args.is_empty() {
        write!(f, "<")?;
        for (idx, arg) in nominal.args.iter().copied().enumerate() {
            if idx != 0 {
                write!(f, ", ")?;
            }
            format_type(store, arg, f, depth + 1)?;
        }
        write!(f, ">")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_display_formats_builtins_and_composites() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        assert_eq!(tys.display(builtins.any).to_string(), "Any");
        assert_eq!(tys.display(builtins.string).to_string(), "String");
        assert_eq!(tys.display(builtins.unit).to_string(), "Unit");
        assert_eq!(tys.display(builtins.nothing).to_string(), "Nothing");
        assert_eq!(tys.display(builtins.bool_).to_string(), "Bool");
        assert_eq!(tys.display(builtins.int).to_string(), "Int");
        assert_eq!(tys.display(builtins.uint).to_string(), "UInt");

        let int32 = tys.ty_int_n(32);
        let uint64 = tys.ty_uint_n(64);
        assert_eq!(tys.display(int32).to_string(), "Int32");
        assert_eq!(tys.display(uint64).to_string(), "UInt64");

        let opt_int32 = tys.ty_option(int32);
        assert_eq!(tys.display(opt_int32).to_string(), "Option<Int32>");

        let tuple = tys.ty_tuple(vec![builtins.int, builtins.uint]);
        assert_eq!(tys.display(tuple).to_string(), "(Int, UInt)");
    }

    #[test]
    fn type_kind_knows_ref_vs_value() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();
        let opt_int = tys.ty_option(builtins.int);

        assert!(tys.is_ref(builtins.any));
        assert!(!tys.is_value(builtins.any));

        assert!(tys.is_ref(builtins.string));
        assert!(!tys.is_value(builtins.string));

        assert!(tys.is_value(builtins.bool_));
        assert!(!tys.is_ref(builtins.bool_));

        assert!(tys.is_value(builtins.int));
        assert!(!tys.is_ref(builtins.int));

        assert!(tys.is_value(opt_int));
    }
}
