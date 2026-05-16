//! HIR lowering 的通用 helper（TODO T0103b）。
//!
//! 说明：
//! - 该模块集中放置跨 lowering 分支复用的 helper（例如 FQN 拼接、closure capture 计算、注解参数解析）；
//! - 同时把 early-stage 的“临时特判/兼容逻辑”（delegated properties、@Extern、struct/enum layout 收集等）
//!   收拢到少数入口函数中，降低后续拆分 `expr.rs`/`stmt.rs`/`block.rs` 的重复与循环依赖风险。

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::mir::{InstanceKey, TemplateKey};
use crate::resolve::Index;
use crate::source::SourceFile;
use crate::span::Span;
use crate::stable_id::{
    StableCanonicalKey, StableConeKey, StableDefKey, StableDefNamespace, StableTemplateKey,
    StableTypeParamKey, canonical_callable_signature_key, canonical_property_getter_signature_key,
    stable_template_symbol_suffix,
};
use crate::syntax::int_literal::parse_int_literal;
use crate::syntax::string_literal::parse_string_literal_utf8;
use crate::ty::{
    BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore, ValueTypeKind,
};

use super::types::*;
use super::{HirLowering, HirLoweringSetup};

use super::super::{
    Block, CallArg, Capture, ClassCtor, ClassCtorDelegation, ClassCtorKind, ClassCtorParam,
    ClassField, ClassInit, ClassInitIndex, ClassInitStep, CtorCallInfo, CtorCallSiteIndex,
    EFFECT_ROW_PARAM_DECL_FILE, EnumLayout, EnumLayoutIndex, EnumRepr, EnumVariantFieldLayout,
    EnumVariantLayout, ExternAbi, ExternFun, ExternFunIndex, InterpolatedStringPart, LiteralKind,
    MemberAccess, MemberRef, ObjectInit, ObjectInitIndex, ObjectInitStep, ObjectProperty, Param,
    StmtKind, StructCLayout, StructFieldLayout, StructLayout, StructLayoutIndex, SymbolId,
    ValueRef, WhenPat,
};

pub(crate) type GenericTemplateSymbolSuffixIndex = HashMap<TemplateKey, String>;

mod annotations;
mod closures;
mod decls;
mod generic_funs;
mod generic_layouts;
mod generic_signatures;

#[allow(unused_imports)]
pub use {
    annotations::*, closures::*, decls::*, generic_funs::*, generic_layouts::*,
    generic_signatures::*,
};
