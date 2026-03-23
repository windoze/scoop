//! Typecheck（类型检查）。
//!
//! 当前阶段（T0402）先落地 typecheck 的基础设施：从 sysroot 收集“内建类型/效果”的声明头信息，
//! 形成可查询的类型环境（`TypeEnv`），为后续：
//! - `TypeRef` → `Type` lowering（T0403）
//! - 顶层签名检查（T0404）
//! - 表达式类型检查（T0405+）
//! 提供起点。

mod type_env;
mod lower;
mod assignable;
mod branch_merge;
mod headers;
mod expr;
mod structs;
mod properties;
mod when_pat;
mod when_exhaustiveness;
mod val_pat;
mod inheritance;
mod interfaces;
mod override_effects;
mod layout;
mod overloads;
mod where_clause;

pub use type_env::{TypeEnv, TypeEnvError, TypeSymbol, TypeSymbolKind};
pub use lower::{check_file_type_refs, TypeLowerError};
pub use headers::{check_file_headers, TypeHeaderError};
pub use expr::{check_file_exprs, ExprTypeError};
pub use structs::{check_file_struct_decls, StructDeclError};
pub use properties::{check_file_properties, PropertyDeclError};
pub use inheritance::{check_file_inheritance, InheritanceError};
pub use interfaces::{check_file_interfaces, InterfaceError};
pub use override_effects::{check_file_override_effects, OverrideEffectError};
pub use layout::{check_file_type_layouts, LayoutError};
pub use overloads::{check_file_overload_conflicts, OverloadDeclError};
pub use where_clause::{check_file_where_clauses, WhereClauseError};
