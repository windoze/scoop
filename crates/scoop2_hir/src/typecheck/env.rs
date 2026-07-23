//! [`TypeEnv`]：typecheck 的类型查询环境。
//!
//! 持有 [`TypeStore`]（类型存储）、对 resolve [`Index`](crate::resolve::Index) 与
//! [`Interner`] 的引用，并提供**内建类型表**（标量 / String / Unit / Nothing）。
//! nominal 类型（class/struct/enum/...）的 ref/value 由 [`Index::category`] 决定。

use std::collections::HashMap;

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{FileId, Interner, Symbol};

use crate::resolve::imports::ImportTable;
use crate::resolve::index::Index;
use crate::syntax::ast::{File, ItemKind};
use crate::ty::{TypeId, TypeParamType, TypeStore};

use super::lower::TypeLowering;

/// 一个函数签名（已降级；M2 用于单候选调用）。
#[derive(Clone, Debug)]
pub struct Signature {
    pub params: Vec<TypeId>,
    pub return_ty: TypeId,
    /// 类型参数个数（>0 表示泛型；M3 才支持实例化）。
    pub type_param_count: usize,
}

/// typecheck 类型环境。
pub struct TypeEnv<'i> {
    pub store: TypeStore,
    pub index: &'i Index,
    pub interner: &'i Interner,
    /// FQN → 函数签名重载集（顶层函数；M2）。
    signatures: HashMap<Symbol, Vec<Signature>>,
}

impl<'i> TypeEnv<'i> {
    pub fn new(index: &'i Index, interner: &'i Interner) -> Self {
        Self {
            store: TypeStore::new(),
            index,
            interner,
            signatures: HashMap::new(),
        }
    }

    /// 顶层函数（非扩展、非成员）的签名重载集。
    pub fn signatures(&self, fqn: Symbol) -> Option<&[Signature]> {
        self.signatures.get(&fqn).map(|v| v.as_slice())
    }

    /// 内建标量 / String / Unit / Nothing 名字 → [`TypeId`]。
    /// 接受短名（`Int`）或全限定（`scoop.core.Int`）。
    /// 不含 `Option`/`Array`——它们是 prelude nominal，由 [`super::lower`] 经 Index 解析。
    pub fn builtin(&mut self, name: &str) -> Option<TypeId> {
        let n = name.strip_prefix("scoop.core.").unwrap_or(name);
        let s = &mut self.store;
        Some(match n {
            "Int" => s.int(),
            "UInt" | "UIntPtr" => s.uint(),
            "Bool" => s.bool(),
            "Char" => s.char(),
            "Float64" | "Double" => s.float64(),
            "Float32" => s.float32(),
            "Int8" => s.int_n(8),
            "Int16" | "Short" => s.int_n(16),
            "Int32" => s.int_n(32),
            "Int64" | "Long" => s.int_n(64),
            "UInt8" | "Byte" => s.uint_n(8),
            "UInt16" | "UShort" => s.uint_n(16),
            "UInt32" => s.uint_n(32),
            "UInt64" | "ULong" => s.uint_n(64),
            "String" => s.string(),
            "Unit" => s.unit(),
            "Nothing" => s.nothing(),
            _ => return None,
        })
    }

    /// nominal 类型是否引用类型（按 [`Index::category`]）。
    pub fn is_reference_nominal(&self, fqn: Symbol) -> bool {
        self.index.category(fqn).is_some_and(|c| c.is_reference())
    }
}

/// 把文件的**顶层函数**（非扩展、非成员）签名降级并登记进 `env.signatures`。
/// 成员函数 / 构造器 / 扩展函数的签名在成员调用里程碑补齐。
pub fn register_top_level_signatures(
    env: &mut TypeEnv,
    file: &File,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        let ItemKind::Fun(d) = &item.kind else {
            continue;
        };
        if d.receiver.is_some() {
            continue; // 扩展函数：M2 暂不
        }
        let name_text = env.interner.resolve(d.name.symbol);
        let fqn_text = if package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{package_prefix}.{name_text}")
        };
        let Some(fqn) = env.interner.get(&fqn_text) else {
            continue;
        };
        // 类型参数映射（用于降低签名中的类型参数引用）。
        let tp_map: HashMap<Symbol, TypeParamType> = d
            .type_params
            .iter()
            .flat_map(|tpl| tpl.params.iter())
            .map(|p| {
                (
                    p.name.symbol,
                    TypeParamType {
                        name: p.name.symbol,
                        file: FileId(0),
                        span: p.name.span,
                    },
                )
            })
            .collect();
        let tpc = tp_map.len();
        let unit_ty = env.store.unit();
        let sig = {
            let mut lower =
                TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
            let params: Vec<TypeId> = d
                .params
                .iter()
                .map(|p| match &p.ty {
                    Some(t) => lower.lower(t),
                    None => unit_ty,
                })
                .collect();
            let return_ty = match &d.return_ty {
                Some(t) => lower.lower(t),
                None => unit_ty,
            };
            Signature {
                params,
                return_ty,
                type_param_count: tpc,
            }
        };
        env.signatures.entry(fqn).or_default().push(sig);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::index::Index;

    #[test]
    fn builtin_scalars_and_qualified_form() {
        let idx = Index::new();
        let it = Interner::new();
        let mut env = TypeEnv::new(&idx, &it);
        let i = env.builtin("Int").unwrap();
        let i2 = env.builtin("scoop.core.Int").unwrap();
        assert_eq!(i, i2, "short and qualified resolve to same TypeId");
        assert!(env.store.is_value(i));
        let s = env.builtin("String").unwrap();
        assert!(env.store.is_reference(s));
        let u = env.builtin("Unit").unwrap();
        assert!(env.store.is_unit(u));
        let n = env.builtin("Nothing").unwrap();
        assert!(env.store.is_nothing(n));
        assert!(env.builtin("NotAType").is_none());
        assert_eq!(env.builtin("Byte").unwrap(), env.builtin("UInt8").unwrap());
        assert_eq!(
            env.builtin("Double").unwrap(),
            env.builtin("Float64").unwrap()
        );
        let _ = it;
    }
}
