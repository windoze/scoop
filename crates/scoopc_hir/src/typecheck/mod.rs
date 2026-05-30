//! Typecheck（类型检查）。
//!
//! 当前阶段（T0402）先落地 typecheck 的基础设施：从 sysroot 收集“内建类型/效果”的声明头信息，
//! 形成可查询的类型环境（`TypeEnv`），并为后续这些步骤提供起点：
//! - `TypeRef` → `Type` lowering（T0403）
//! - 顶层签名检查（T0404）
//! - 表达式类型检查（T0405+）

mod annotations;
mod assignable;
mod branch_merge;
mod builtin_annotations;
mod eff_row_subst;
mod expr;
mod headers;
mod inheritance;
mod int_literals;
mod interfaces;
mod layout;
mod lower;
mod overloads;
mod override_effects;
mod properties;
mod signature_match;
mod structs;
mod type_env;
mod val_pat;
mod when_exhaustiveness;
mod when_pat;
mod where_clause;

pub use annotations::{AnnotationError, check_file_annotations};
pub(crate) use assignable::is_type_assignable;
pub(crate) use builtin_annotations::{BuiltinAnnotationKind, builtin_annotation_kind};
pub use expr::{
    ExprTypeError, check_file_exprs, check_file_exprs_with_monomorph_and_type_instantiation_keys,
    check_file_exprs_with_monomorph_keys, check_file_exprs_with_monomorph_requests,
    check_file_exprs_with_type_instantiation_keys,
};
pub use headers::{TypeHeaderError, check_file_headers};
pub use inheritance::{InheritanceError, check_file_inheritance};
pub use interfaces::{InterfaceError, check_file_interfaces};
pub use layout::{LayoutError, check_file_type_layouts};
pub use lower::TypeLowering;
pub use lower::{
    TypeInstantiationKey, TypeLowerError, check_file_type_refs,
    check_file_type_refs_with_type_instantiation_keys,
};
pub use overloads::{OverloadDeclError, check_file_overload_conflicts};
pub use override_effects::{OverrideEffectError, check_file_override_effects};
pub use properties::{PropertyDeclError, check_file_properties};
pub use structs::{StructDeclError, check_file_struct_decls};
pub use type_env::{
    AnnotationRetentionPolicy, AnnotationTargetKind, FileTypeContext, TypeEnv, TypeEnvError,
    TypeSymbol, TypeSymbolKind,
};
pub use where_clause::{WhereClauseError, check_file_where_clauses};
