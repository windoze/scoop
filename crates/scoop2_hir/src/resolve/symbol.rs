//! 名称解析的身份与命名空间类型。
//!
//! 对应 spec P1 §3（cone/package）、§4.1（可见性）、§4（声明）。
//!
//! - 一个 **cone**（`ConeId`）是编译/可见性/依赖单元（≈ Rust crate，由
//!   `Cone.toml` 描述）；一个 **package** 是 cone 内的源码命名空间。
//! - 每个全限定名（FQN）下有**三个独立命名空间**：类型（`type`，class/
//!   interface/struct/enum/effect/object/typealias）、函数（`fun`，重载集）、
//!   值（`value`，顶层 val/var）。同名可同时存在于不同命名空间。
//! - 函数命名空间是一个**重载集**（`Vec<FunOverload>`，Phase B 后续填充签名）；
//!   同名重载在 resolve 阶段不判重复（签名冲突由 typecheck 负责）。

use scoop2_base::{FileId, Span, Symbol};

use crate::syntax::ast::{Modifier, ModifierKind};

// ---------------------------------------------------------------------------
// Cone（编译/可见性单元）
// ---------------------------------------------------------------------------

/// Cone 身份（在 [`super::Index`] 的 cone 列表中的下标）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct ConeId(pub u32);

/// Cone 种类（影响可见性与链接语义）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConeKind {
    /// 可执行二进制（含 entry point）。
    Bin,
    /// 受信任/不受信任的系统库（sysroot）。
    Syslib,
    /// 普通库。
    Lib,
}

/// 一个 cone 的元信息。
#[derive(Clone, Debug)]
pub struct ConeInfo {
    pub id: ConeId,
    /// cone 名（如 `scoop.core`）。
    pub name: String,
    pub kind: ConeKind,
    /// 跨构建稳定身份（从包名派生；PLAN.md C2——序列化/跨 cone 引用用它，
    /// `id` 只是会话内注册表下标，不跨构建稳定）。
    pub stable_key: scoop2_base::StableConeKey,
}

// ---------------------------------------------------------------------------
// 可见性（spec P1 §4.1）
// ---------------------------------------------------------------------------

/// 可见性修饰符。`public`/`internal`/`private` 三者互斥；默认 `internal`。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Visibility {
    /// 跨 cone 可见（显式导出）。
    Public,
    /// 同 cone 内可见（默认）。
    Internal,
    /// 文件/声明体局部。
    Private,
}

impl Visibility {
    /// 从修饰符列表解析可见性。
    ///
    /// 无可见性修饰符 → `Internal`（默认）。若出现多个可见性修饰符，返回其中之一，
    /// 非法组合的诊断由 collect 阶段单独发出（`invalid_visibility`）。
    pub fn from_modifiers(mods: &[Modifier]) -> Visibility {
        for m in mods {
            match m.kind {
                ModifierKind::Public => return Visibility::Public,
                ModifierKind::Internal => return Visibility::Internal,
                ModifierKind::Private => return Visibility::Private,
                _ => {}
            }
        }
        Visibility::Internal
    }

    /// 计数可见性修饰符的出现次数（>1 即非法组合）。
    pub fn count_modifiers(mods: &[Modifier]) -> usize {
        mods.iter()
            .filter(|m| {
                matches!(
                    m.kind,
                    ModifierKind::Public | ModifierKind::Internal | ModifierKind::Private
                )
            })
            .count()
    }
}

// ---------------------------------------------------------------------------
// 修饰符集合（位集）
// ---------------------------------------------------------------------------

/// 声明修饰符的位集。
///
/// 仅记录哪些修饰符出现；可见性单独由 [`Visibility::from_modifiers`] 解析。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct ModifierSet(u16);

impl ModifierSet {
    /// 修饰符位。
    const PUBLIC: u16 = 1 << 0;
    const INTERNAL: u16 = 1 << 1;
    const PRIVATE: u16 = 1 << 2;
    const OPEN: u16 = 1 << 3;
    const ABSTRACT: u16 = 1 << 4;
    const SEALED: u16 = 1 << 5;
    const OVERRIDE: u16 = 1 << 6;
    const OPERATOR: u16 = 1 << 7;
    const ANNOTATION: u16 = 1 << 8;

    pub fn from_modifiers(mods: &[Modifier]) -> Self {
        let mut bits = 0u16;
        for m in mods {
            let bit = match m.kind {
                ModifierKind::Public => Self::PUBLIC,
                ModifierKind::Internal => Self::INTERNAL,
                ModifierKind::Private => Self::PRIVATE,
                ModifierKind::Open => Self::OPEN,
                ModifierKind::Abstract => Self::ABSTRACT,
                ModifierKind::Sealed => Self::SEALED,
                ModifierKind::Override => Self::OVERRIDE,
                ModifierKind::Operator => Self::OPERATOR,
                ModifierKind::Annotation => Self::ANNOTATION,
            };
            bits |= bit;
        }
        ModifierSet(bits)
    }

    pub fn contains(self, kind: ModifierKind) -> bool {
        let bit = match kind {
            ModifierKind::Public => Self::PUBLIC,
            ModifierKind::Internal => Self::INTERNAL,
            ModifierKind::Private => Self::PRIVATE,
            ModifierKind::Open => Self::OPEN,
            ModifierKind::Abstract => Self::ABSTRACT,
            ModifierKind::Sealed => Self::SEALED,
            ModifierKind::Override => Self::OVERRIDE,
            ModifierKind::Operator => Self::OPERATOR,
            ModifierKind::Annotation => Self::ANNOTATION,
        };
        (self.0 & bit) != 0
    }

    pub fn is_open(self) -> bool {
        (self.0 & Self::OPEN) != 0
    }
    pub fn is_abstract(self) -> bool {
        (self.0 & Self::ABSTRACT) != 0
    }
    pub fn is_sealed(self) -> bool {
        (self.0 & Self::SEALED) != 0
    }
    pub fn is_override(self) -> bool {
        (self.0 & Self::OVERRIDE) != 0
    }
    pub fn is_operator(self) -> bool {
        (self.0 & Self::OPERATOR) != 0
    }
    pub fn is_annotation(self) -> bool {
        (self.0 & Self::ANNOTATION) != 0
    }
}

// ---------------------------------------------------------------------------
// 符号
// ---------------------------------------------------------------------------

/// 符号的命名空间类别。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymbolKind {
    /// 类型命名空间：class/interface/struct/enum/effect。
    Type,
    /// 命名 object（既是类型也是单例值，但登记在类型命名空间）。
    Object,
    /// typealias。
    TypeAlias,
    /// 函数命名空间（重载集的一员）。
    Fun,
    /// 值命名空间：顶层 val/var。
    Value,
    /// 扩展函数（按接收者归类，登记在扩展表）。
    ExtensionFun,
    /// 扩展属性（按接收者归类）。
    ExtensionProperty,
}

/// nominal 类型声明的具体类别（供 typecheck 判定 ref vs value、成员解析等）。
///
/// `Class`/`Interface`/`Object`/`Effect` → 引用类型；`Struct`/`Enum` → 值类型。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NominalCategory {
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
    Object,
}

impl NominalCategory {
    /// 是否引用类型（GC 托管、按引用传递）。
    pub fn is_reference(self) -> bool {
        matches!(
            self,
            NominalCategory::Class
                | NominalCategory::Interface
                | NominalCategory::Object
                | NominalCategory::Effect
        )
    }

    /// 由 AST 的 `TypeKind` 构造（type 声明）。
    pub fn from_ast_type_kind(k: crate::syntax::ast::TypeKind) -> Option<Self> {
        use crate::syntax::ast::TypeKind as T;
        Some(match k {
            T::Class => NominalCategory::Class,
            T::Interface => NominalCategory::Interface,
            T::Struct => NominalCategory::Struct,
            T::Enum => NominalCategory::Enum,
            T::Effect => NominalCategory::Effect,
        })
    }
}

/// 一个已登记的声明符号。
#[derive(Clone, Debug)]
pub struct DeclSymbol {
    pub kind: SymbolKind,
    /// 全限定名（interned 点分串，如 `a.b.f`）；句柄为 [`scoop2_base::Symbol`]。
    pub fqn: Symbol,
    /// 简单名（FQN 最后一段，interned）。
    pub simple_name: Symbol,
    pub span: Span,
    pub file: FileId,
    pub cone: ConeId,
    pub visibility: Visibility,
    pub modifiers: ModifierSet,
}

impl DeclSymbol {
    pub fn is_visible_from(&self, other_cone: ConeId) -> bool {
        match self.visibility {
            Visibility::Public => true,
            Visibility::Internal => self.cone == other_cone,
            Visibility::Private => self.cone == other_cone,
        }
    }
}

/// 一个 FQN 下的三命名空间内容。
///
/// 函数命名空间是重载集（同 FQN 可有多个函数，签名判重在 typecheck）。
#[derive(Clone, Default, Debug)]
pub struct NamespacedSymbols {
    /// 类型命名空间（class/.../object/typealias）。同 FQN 至多一个。
    pub ty: Option<DeclSymbol>,
    /// 函数命名空间（重载集）。顺序为声明顺序。
    pub funs: Vec<DeclSymbol>,
    /// 值命名空间（顶层 val/var）。同 FQN 至多一个。
    pub value: Option<DeclSymbol>,
}

impl NamespacedSymbols {
    pub fn is_empty(&self) -> bool {
        self.ty.is_none() && self.funs.is_empty() && self.value.is_none()
    }
}
