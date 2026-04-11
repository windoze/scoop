//! AST → HIR 的最小 lowering（TODO T0701）。
//!
//! 说明：
//! - 这里的 lowering 仅用于 `dump-hir` 的调试输出，因此实现上优先保证“稳定输出 + 不 panic”；。
//! - 完整 lowering（含类型推断结果、更多语法节点）会在后续任务（TODO T0702+）逐步补齐。

mod types;
mod util;

mod patterns;
mod sugar;

mod block;
mod expr;
mod stmt;

pub use types::{HirLowerError, LoweredHir};
pub use util::mangle_nominal_fqn;

use std::collections::HashMap;

use crate::ast;
use crate::parser::parse_file;
use crate::resolve::Index;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore,
    ValueTypeKind,
};

use super::{
    Block, CallArg, ClassInitIndex, CtorCallSiteIndex, Expr, ExprKind, File, FunDecl, Item,
    ObjectInitIndex, Param, Stmt, StmtKind, SymbolId, ValDecl, ValueRef,
};

use types::*;
use util::*;

/// HIR lowering 的上下文（按单文件构建，用于 `dump-hir` 与 HIR fixtures）。
struct HirLowering<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    /// typecheck 阶段的 TypeStore（若存在）。
    ///
    /// 用途：把 `ast::File` side table 中记录的 typecheck `TypeId` 重新 intern 到当前 HIR 的
    /// `TypeStore`，从而在 lowering 阶段恢复“表达式最终类型”。
    typecheck_types: Option<&'a TypeStore>,
    /// `type fqn -> ast::TypeKind` 的最小索引，用于决定 nominal type 是 ref 还是 value。
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    /// delegated property（spec §10.4）索引：`Owner.prop` → lowering 所需的合成符号信息。
    ///
    /// 用途：
    /// - `receiver.prop` 读取：降糖为 `receiver.prop$delegate.getValue(receiver, PropertyMeta)`；
    /// - `receiver.prop = value` 写入：降糖为 `receiver.prop$delegate.setValue(receiver, PropertyMeta, value)`。
    ///
    /// 注意：HIR lowering 的目标是 `dump-hir`/fixtures 的稳定输出，因此这里仅生成“调用形状”，
    /// 不要求 `$delegate` 字段/`PropertyMeta` 常量在后端可执行（真实 codegen 留给后续任务）。
    delegated_properties: &'a DelegatedPropertyIndex,
    /// 顶层函数默认参数信息索引：`fqn -> params(default)`（用于 call-site 默认参数补齐，T1305）。
    default_arg_funs: HashMap<String, DefaultArgFunInfo>,
    /// ctor 调用点候选集合：callee span → candidate type fqns。
    ///
    /// 说明：HIR v0 仍把 ctor 调用的 callee 降为 `UnresolvedIdent`，因此需要 side table
    /// 把 resolver 的 call candidates 保留下来，供 LLVM codegen 决定“这是 ctor call”。
    ctor_call_sites: CtorCallSiteIndex,
    /// 顶层可变全局变量（`@ThreadLocal/@Global`）索引（TODO T1023）。
    top_level_vars: super::TopLevelVarIndex,
    /// 顶层 `const val` 索引：供后端在表达式位置按声明类型回放 initializer。
    top_level_consts: super::TopLevelConstIndex,
    symbols: SymbolInterner,
    /// 本文件内的“局部 symbol → 是否可变（var）”信息。
    ///
    /// 用途：closure capture set 需要知道捕获目标是否为 `var`，以便在后续 MIR lowering（T0714）
    /// 侧把可变捕获降为 box/alias 语义。
    local_mutability: HashMap<SymbolId, bool>,
    next_closure: u32,
    /// 类型表（HIR 内所有 `TypeId` 必须来自同一个 store）。
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    /// type parameter 作用域栈：用于 lowering `T` 这类抽象类型引用。
    type_param_scopes: Vec<HashMap<String, TypeId>>,
}

/// 构造 `HirLowering` 时用到的非必需上下文集合。
///
/// 说明：
/// - 这些字段在不同 lowering 入口之间经常成组出现；
/// - 单独打包后可以避免初始化函数参数过多，同时保持调用点语义明确。
struct HirLoweringSetup<'a> {
    typecheck_types: Option<&'a TypeStore>,
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    delegated_properties: &'a DelegatedPropertyIndex,
    builtins: BuiltinTypes,
}

impl<'a> HirLowering<'a> {
    const ASYNC_AWAIT_FQN: &'static str = "scoop.core.Async.await";
    const TASK_SPAWN_INT_FQN: &'static str = "scoop.core.__scoop_task_spawn_int";
    const TASK_JOIN_INT_FQN: &'static str = "scoop.core.__scoop_task_join_int";
    const PROPERTY_META_FQN: &'static str = "scoop.core.PropertyMeta";
    const ARRAY_BUILDER_NEW_FQN: &'static str = "scoop.core.__scoop_array_builder_new";
    const ARRAY_BUILDER_PUSH_FQN: &'static str = "scoop.core.__scoop_array_builder_push";
    const ARRAY_BUILDER_BUILD_ARRAY_FQN: &'static str =
        "scoop.core.__scoop_array_builder_build_array";
    const SYNC_MUTEX_TYPE_FQN: &'static str = "scoop.sync.Mutex";
    const SYNC_MUTEX_CREATE_FQN: &'static str = "scoop.sync.mutexCreate";
    const SYNC_MUTEX_LOCK_FQN: &'static str = "scoop.sync.lock";
    const SYNC_MUTEX_UNLOCK_FQN: &'static str = "scoop.sync.unlock";
    const ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN: &'static str =
        "scoop.core.__scoop_array_builder_build_mutable_array";
    const RAISE_RAISE_FQN: &'static str = "scoop.core.Raise.raise";
    const RUNTIME_ERROR_NULL_ASSERTION_FAILED_FQN: &'static str =
        "scoop.core.RuntimeError.NullAssertionFailed";

    fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        types: &'a mut TypeStore,
        setup: HirLoweringSetup<'a>,
    ) -> Self {
        let HirLoweringSetup {
            typecheck_types,
            type_kinds,
            delegated_properties,
            builtins,
        } = setup;
        Self {
            source,
            file,
            index,
            typecheck_types,
            type_kinds,
            delegated_properties,
            default_arg_funs: HashMap::new(),
            ctor_call_sites: HashMap::new(),
            top_level_vars: HashMap::new(),
            top_level_consts: HashMap::new(),
            symbols: SymbolInterner::default(),
            local_mutability: HashMap::new(),
            next_closure: 0,
            types,
            builtins,
            type_param_scopes: Vec::new(),
        }
    }

    fn intern_local_symbol(&mut self, decl_span: Span, mutable: bool) -> SymbolId {
        let id = self.symbols.intern_local(decl_span);
        match self.local_mutability.get(&id).copied() {
            // 同一 decl_span 不应出现冲突的 mutability，但为了降低与 resolver 交互时的脆弱性：
            // - 若任一方认为它是 `var`，则提升为 `var`；
            // - 否则保持 `val`。
            Some(prev) => {
                let _ = self.local_mutability.insert(id, prev || mutable);
            }
            None => {
                self.local_mutability.insert(id, mutable);
            }
        }
        id
    }

    fn lower_file(&mut self) -> File {
        let pkg_prefix = package_prefix(self.source, self.file.package.as_ref());
        self.default_arg_funs = self.collect_default_arg_funs(&pkg_prefix);
        let mut items = Vec::with_capacity(self.file.items.len());

        for item in &self.file.items {
            items.push(self.lower_item(&pkg_prefix, item));
        }

        File { items }
    }

    /// 扫描当前源文件内的顶层 `fun` 声明，收集“默认参数”信息（供 call-site 默认参数补齐使用）。
    ///
    /// 注意：
    /// - 这里只服务于早期单文件 codegen，因此只索引“当前文件内”的顶层函数；
    /// - 不尝试处理 overload set（同名重载）与泛型函数（它们需要 typecheck 的最终决议信息）。
    fn collect_default_arg_funs(&mut self, pkg_prefix: &str) -> HashMap<String, DefaultArgFunInfo> {
        let mut out: HashMap<String, DefaultArgFunInfo> = HashMap::new();

        for item in &self.file.items {
            let ast::Item::Fun(fun) = item else {
                continue;
            };

            // 当前阶段：默认参数补齐仅针对“非泛型 + 非 receiver”的顶层函数。
            if fun.receiver.is_some() || !fun.type_params.is_empty() {
                continue;
            }
            // 当前阶段：不处理 vararg（其“缺省语义”是空数组/0 个元素，而非 param default_value）。
            if fun.params.iter().any(|p| p.is_vararg) {
                continue;
            }

            let name = fun.name.text(self.source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };

            // 过滤 overload set：同一个 fqn 出现多次时不做索引（避免与 typecheck 的 overload 决议冲突）。
            if out.contains_key(&fqn) {
                let _ = out.remove(&fqn);
                continue;
            }

            let mut params: Vec<DefaultArgParamInfo> = Vec::with_capacity(fun.params.len());
            for p in &fun.params {
                let name = p.name.text(self.source).to_string();
                params.push(DefaultArgParamInfo {
                    decl_span: p.name.span,
                    name,
                    ty_ref: p.ty.clone(),
                    default_value: p.default_value.clone(),
                });
            }

            if !params.iter().any(|p| p.default_value.is_some()) {
                continue;
            }

            let required = params.iter().filter(|p| p.default_value.is_none()).count();
            out.insert(fqn, DefaultArgFunInfo { required, params });
        }

        out
    }

    fn lower_item(&mut self, pkg_prefix: &str, item: &ast::Item) -> Item {
        match item {
            ast::Item::Fun(fun) => Item::Fun(self.lower_fun_decl(pkg_prefix, fun)),
            ast::Item::Val(v) => Item::Val(self.lower_val_decl(pkg_prefix, v, ValScope::TopLevel)),
            ast::Item::TypeAlias(ta) => Item::Todo {
                span: ta.span,
                kind: "typealias",
            },
            ast::Item::ComptimeIf(ci) => Item::Todo {
                span: ci.span,
                kind: "comptime_if_item",
            },
            ast::Item::Type(ty) => Item::Todo {
                span: ty.span,
                kind: "type",
            },
            ast::Item::Object(obj) => Item::Todo {
                span: obj.span,
                kind: "object",
            },
            ast::Item::ExtensionProperty(p) => self.lower_extension_property(pkg_prefix, p),
        }
    }

    /// 收集并 lowering 当前文件内的 class/object member `fun` 声明为可 codegen 的顶层函数形态。
    ///
    /// 说明：
    /// - 该收集发生在 `lower_file()` 之后，作为 side table 保存，避免影响 `dump-hir` 的输出稳定性；
    /// - T1508a 只需要“直连 member call”，因此这里不尝试建模 vtable/itable 的动态分发信息；
    ///   统一把 member method 表达为 `Owner.method(this, ...args) -> ret` 的顶层调用约定。
    fn collect_member_funs(&mut self, pkg_prefix: &str) -> Vec<FunDecl> {
        let mut out: Vec<FunDecl> = Vec::new();

        for item in &self.file.items {
            match item {
                ast::Item::Type(ty) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, ty, pkg_prefix, &mut out);
                }
                ast::Item::Object(obj) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, obj, pkg_prefix, &mut out);
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }

        out
    }

    fn collect_member_funs_in_type_decl(
        &mut self,
        pkg_prefix: &str,
        decl: &ast::TypeDecl,
        prefix: &str,
        out: &mut Vec<FunDecl>,
    ) {
        let local_name = decl.name.text(self.source);
        let owner_fqn = join_prefix(prefix, local_name);

        let Some(body) = &decl.body else {
            return;
        };

        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    out.push(self.lower_member_fun_decl(
                        pkg_prefix,
                        &owner_fqn,
                        &decl.type_params,
                        decl.name.span,
                        fun,
                    ));
                }
                ast::TypeMember::Type(nested) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::Object(obj) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, obj, &owner_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
            }
        }
    }

    fn collect_member_funs_in_object_decl(
        &mut self,
        pkg_prefix: &str,
        obj: &ast::ObjectDecl,
        prefix: &str,
        out: &mut Vec<FunDecl>,
    ) {
        let Some(name) = object_decl_name(self.source, obj) else {
            return;
        };
        let owner_fqn = join_prefix(prefix, &name);

        let this_decl_span = obj.name.as_ref().map(|n| n.span).unwrap_or(obj.span);

        let Some(body) = &obj.body else {
            return;
        };
        for member in &body.members {
            match member {
                ast::TypeMember::Fun(fun) => {
                    out.push(self.lower_member_fun_decl(
                        pkg_prefix,
                        &owner_fqn,
                        &[],
                        this_decl_span,
                        fun,
                    ));
                }
                ast::TypeMember::Type(nested) => {
                    self.collect_member_funs_in_type_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::Object(nested) => {
                    self.collect_member_funs_in_object_decl(pkg_prefix, nested, &owner_fqn, out);
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_) => {}
            }
        }
    }

    fn lower_fun_decl(&mut self, pkg_prefix: &str, fun: &ast::FunDecl) -> FunDecl {
        // 进入函数作用域：先把 type params lower 成 `TypeId`，保证签名与 body 内引用一致。
        self.push_type_params(&fun.type_params);

        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        // 扩展函数的 receiver（`fun T.f(...)`）在 resolver 中会把 `this` 解析为一个局部绑定，
        // 且 decl_span 取 receiver type 的 span（见 `resolve::scopes` 中 `ThisContext.decl_span`）。
        // 为了让 codegen 能把 `this` 当作一个普通局部参数处理，这里把 receiver 显式降为第 0 个参数。
        let mut params: Vec<Param> =
            Vec::with_capacity(fun.params.len() + receiver_ty.is_some() as usize);
        if let Some(receiver) = fun.receiver.as_ref() {
            let span = receiver.span();
            let id = self.intern_local_symbol(span, false);
            let ty = receiver_ty.unwrap_or(self.builtins.any);
            params.push(Param {
                span,
                id,
                name: "this".to_string(),
                ty,
            });
        }

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let elem_ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            // T0113: vararg param type `T` → `Array<T>` (the function body uses it as an array).
            let ty = if p.is_vararg {
                self.types
                    .intern(crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(
                        crate::ty::NominalType {
                            fqn: "scoop.core.Array".to_string(),
                            args: vec![elem_ty],
                            eff: None,
                        },
                    )))
            } else {
                elem_ty
            };
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        // 当前阶段：未接入返回类型推断，缺省时用 `Any` 占位。
        //
        // T0623：`async fun foo(): T` 对外暴露 `Task<T>`：
        // - 早期 HIR 里 task 先用 word-sized `UInt` 句柄承载（与 `spawn/join` 一致）；
        // - 真正的 `Task<T>` nominal type lowering 与 ABI 会在后续任务中补齐。
        let is_async_fun = fun.modifiers.contains(&ast::Modifier::Async);
        let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
        let _inner_return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);
        let return_ty = if is_async_fun {
            self.builtins.uint
        } else {
            _inner_return_ty
        };

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        // receiver 已作为显式参数降入 `params`，因此 HIR 的 function type 不再单独保留 receiver 位。
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                let lowered = self.lower_block(pkg_prefix, b);
                if is_async_fun {
                    Some(self.rewrite_async_fun_block(lowered, true))
                } else {
                    Some(lowered)
                }
            }
            ast::FunBody::Missing => None,
        };

        self.pop_type_params();

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// T0112: Synthesize an extension property's getter as a top-level function.
    ///
    /// The getter function has:
    /// - FQN = property FQN (e.g., `pkg.lastIndex`)
    /// - params = `[this: ReceiverType]`
    /// - return type = declared property type
    /// - body = getter body
    fn lower_extension_property(
        &mut self,
        pkg_prefix: &str,
        prop: &ast::ExtensionPropertyDecl,
    ) -> Item {
        let Some(getter) = &prop.getter else {
            return Item::Todo {
                span: prop.span,
                kind: "extension_property_no_getter",
            };
        };

        self.push_type_params(&prop.type_params);

        let name = self.source.slice(prop.name.span).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = self.lower_type_ref(&prop.receiver);

        // Receiver as first parameter (same as extension functions).
        let receiver_span = prop.receiver.span();
        let receiver_id = self.intern_local_symbol(receiver_span, false);
        let params = vec![Param {
            span: receiver_span,
            id: receiver_id,
            name: "this".to_string(),
            ty: receiver_ty,
        }];

        let return_ty = prop
            .ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);

        let effects = EffectRow::pure();
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            false,
        );

        // Lower getter body.
        let body = match &getter.body {
            ast::AccessorBody::Block(b) => Some(self.lower_block(pkg_prefix, b)),
            ast::AccessorBody::Expr(e) => {
                // `get() = expr` → synthesize a block with the expression as tail stmt.
                let lowered_expr = self.lower_expr(pkg_prefix, e);
                let expr_ty = lowered_expr.ty;
                Some(Block {
                    span: e.span,
                    ty: expr_ty,
                    stmts: vec![Stmt {
                        span: e.span,
                        ty: expr_ty,
                        kind: StmtKind::Expr(lowered_expr),
                    }],
                })
            }
            ast::AccessorBody::Missing => None,
        };

        self.pop_type_params();

        Item::Fun(FunDecl {
            span: prop.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: false,
            ty,
            params,
            return_ty,
            body,
        })
    }

    /// 降低一个 class/object 的 member `fun` 为”顶层函数形态”（显式 `this` 参数）。
    ///
    /// 约定：
    /// - `this_decl_span` 必须与 resolver 为 `this` 写回的 `ResolvedValueRef::Local { decl_span }` 对齐：
    ///   - class member：`decl.name.span`
    ///   - object member：`obj.name.span`（匿名 companion 用 `obj.span`）
    fn lower_member_fun_decl(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        owner_type_params: &[ast::TypeParam],
        this_decl_span: Span,
        fun: &ast::FunDecl,
    ) -> FunDecl {
        // owner type params 在 member 方法体内可见（例如 `class Box<T> { fun get(): T }`）。
        self.push_type_params(owner_type_params);
        self.push_type_params(&fun.type_params);

        let name = fun.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_args: Vec<TypeId> = owner_type_params
            .iter()
            .filter_map(|p| self.lookup_type_param(p.name.text(self.source)))
            .collect();
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_args, None);

        let mut params: Vec<Param> = Vec::with_capacity(fun.params.len() + 1);
        params.push(Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        });

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        // 当前阶段：未接入返回类型推断，缺省时用 `Any` 占位。
        let is_async_fun = fun.modifiers.contains(&ast::Modifier::Async);
        let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
        let _inner_return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);
        let return_ty = if is_async_fun {
            self.builtins.uint
        } else {
            _inner_return_ty
        };

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                let lowered = self.lower_block(pkg_prefix, b);
                if is_async_fun {
                    Some(self.rewrite_async_fun_block(lowered, true))
                } else {
                    Some(lowered)
                }
            }
            ast::FunBody::Missing => None,
        };

        self.pop_type_params(); // fun type params
        self.pop_type_params(); // owner type params

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 在“已绑定 type params”的语境下降低一个函数声明。
    ///
    /// 用途：
    /// - 单态化（monomorphization）生成具体实例：把 `T` 等 type param 直接映射到具体 `TypeId`
    ///   后再构造 HIR（避免再次生成 `TypeKind::Param`）。
    fn lower_fun_decl_with_bound_type_params(
        &mut self,
        pkg_prefix: &str,
        fun: &ast::FunDecl,
    ) -> FunDecl {
        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        let mut params: Vec<Param> =
            Vec::with_capacity(fun.params.len() + receiver_ty.is_some() as usize);
        if let Some(receiver) = fun.receiver.as_ref() {
            let span = receiver.span();
            let id = self.intern_local_symbol(span, false);
            let ty = receiver_ty.unwrap_or(self.builtins.any);
            params.push(Param {
                span,
                id,
                name: "this".to_string(),
                ty,
            });
        }

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let elem_ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            // T0113: vararg param type `T` → `Array<T>` (the function body uses it as an array).
            let ty = if p.is_vararg {
                self.types
                    .intern(crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(
                        crate::ty::NominalType {
                            fqn: "scoop.core.Array".to_string(),
                            args: vec![elem_ty],
                            eff: None,
                        },
                    )))
            } else {
                elem_ty
            };
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        // 当前阶段：未接入返回类型推断，缺省时用 `Any` 占位。
        //
        // T0623：monomorph/hir 视图下同样把 `async fun` 的返回类型降为 task handle（`UInt`）。
        let is_async_fun = fun.modifiers.contains(&ast::Modifier::Async);
        let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
        let _inner_return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);
        let return_ty = if is_async_fun {
            self.builtins.uint
        } else {
            _inner_return_ty
        };

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                let lowered = self.lower_block(pkg_prefix, b);
                if is_async_fun {
                    Some(self.rewrite_async_fun_block(lowered, true))
                } else {
                    Some(lowered)
                }
            }
            ast::FunBody::Missing => None,
        };

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// T0126: 在"已绑定 owner type params"的语境下降低成员方法。
    ///
    /// 与 `lower_member_fun_decl` 的区别：owner type params 已经通过 `push_type_param_bindings`
    /// 预绑定到具体类型（由调用方在调用前完成），因此 `this` 参数和方法体中的 owner type params
    /// 均会直接解析为具体 TypeId。
    ///
    /// `this_concrete_args` 是 owner 的具体类型实参（例如 `[Int]` for `Box<Int>`），用于
    /// 构造 `this` 参数的精确 nominal 类型（codegen 阶段需要从 `this.hir_ty` 提取 type args
    /// 来查找 class field 布局）。
    fn lower_member_fun_decl_with_bound_type_params(
        &mut self,
        pkg_prefix: &str,
        owner_fqn: &str,
        this_decl_span: Span,
        this_concrete_args: &[TypeId],
        fun: &ast::FunDecl,
    ) -> FunDecl {
        // 方法自身的 type params（如果有的话）仍然需要 push；
        // owner 的 type params 已由调用方在 push_type_param_bindings 中绑定。
        self.push_type_params(&fun.type_params);

        let name = fun.name.text(self.source).to_string();
        let fqn = format!("{owner_fqn}.{name}");

        let this_id = self.intern_local_symbol(this_decl_span, false);
        let this_ty = self.intern_nominal(owner_fqn.to_string(), this_concrete_args.to_vec(), None);

        let mut params: Vec<Param> = Vec::with_capacity(fun.params.len() + 1);
        params.push(Param {
            span: this_decl_span,
            id: this_id,
            name: "this".to_string(),
            ty: this_ty,
        });

        for p in &fun.params {
            let name = p.name.text(self.source).to_string();
            let id = self.intern_local_symbol(p.name.span, false);
            let ty =
                p.ty.as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
            params.push(Param {
                span: p.name.span,
                id,
                name,
                ty,
            });
        }

        let is_async_fun = fun.modifiers.contains(&ast::Modifier::Async);
        let is_const_fun = fun.modifiers.contains(&ast::Modifier::Const);
        let _inner_return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);
        let return_ty = if is_async_fun {
            self.builtins.uint
        } else {
            _inner_return_ty
        };

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            None,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
            fun.effects.as_ref().is_some_and(|r| r.closed),
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => {
                let lowered = self.lower_block(pkg_prefix, b);
                if is_async_fun {
                    Some(self.rewrite_async_fun_block(lowered, true))
                } else {
                    Some(lowered)
                }
            }
            ast::FunBody::Missing => None,
        };

        self.pop_type_params(); // fun type params

        FunDecl {
            span: fun.span,
            fqn,
            name,
            source_path: self.source.path().to_path_buf(),
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    fn call_top_level_fun(
        &mut self,
        span: Span,
        fqn: &str,
        args: Vec<Expr>,
        ret_ty: TypeId,
    ) -> Expr {
        let fqn = fqn.to_string();
        let callee = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn,
            }),
        };

        Expr {
            span,
            ty: ret_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: args.into_iter().map(CallArg::Positional).collect(),
            },
        }
    }

    fn lower_val_decl(&mut self, pkg_prefix: &str, v: &ast::ValDecl, scope: ValScope) -> ValDecl {
        // T0124: lower the declared type first so we can pass it as expected type for struct literals.
        let declared_ty_early = v.ty.as_ref().map(|t| self.lower_type_ref(t));

        // T1317c：数组字面量 `[...]` 的 lowering 依赖”期望的容器类型”（Array vs MutableArray）。
        // 这里从显式的类型注解（若存在）向 initializer 传播该 hint。
        let init_expected = ExpectedExpr {
            array_lit_target: v
                .ty
                .as_ref()
                .and_then(|ty| self.array_lit_target_from_type_ref(ty)),
            array_lit_ty: declared_ty_early,
            struct_lit_ty: declared_ty_early,
        };
        let init = v
            .init
            .as_ref()
            .map(|e| self.lower_expr_with_expected(pkg_prefix, e, init_expected));

        let ty = declared_ty_early
            .or_else(|| init.as_ref().map(|e| e.ty))
            .unwrap_or(self.builtins.any);

        let mut top_level_fqn: Option<String> = None;
        let (id, name) = match v.name() {
            Some(id) => {
                let name = id.text(self.source).to_string();
                let sym = match scope {
                    ValScope::TopLevel => {
                        let fqn = if pkg_prefix.is_empty() {
                            name.clone()
                        } else {
                            format!("{pkg_prefix}.{name}")
                        };
                        top_level_fqn = Some(fqn.clone());
                        self.symbols.intern_top_level(fqn)
                    }
                    ValScope::Local => {
                        self.intern_local_symbol(id.span, v.kind == ast::ValKind::Var)
                    }
                };
                (Some(sym), Some(name))
            }
            None => (None, None),
        };

        if scope == ValScope::TopLevel
            && let Some(fqn) = top_level_fqn.as_ref()
        {
            if v.modifiers.contains(&ast::Modifier::Const) && v.kind == ast::ValKind::Val {
                self.top_level_consts.insert(
                    fqn.clone(),
                    super::TopLevelConst {
                        fqn: fqn.clone(),
                        source_path: self.source.path().to_path_buf(),
                        span: v.span,
                        ty,
                        init: init.clone(),
                    },
                );
            }

            // T1023：顶层 `@ThreadLocal/@Global var` 需要后端生成静态存储。
            if v.kind == ast::ValKind::Var {
                const THREAD_LOCAL_FQN: &str = "scoop.core.ThreadLocal";
                const GLOBAL_FQN: &str = "scoop.core.Global";

                let is_thread_local = v
                    .annotations
                    .iter()
                    .any(|ann| self.annotation_use_resolves_to_fqn(ann, THREAD_LOCAL_FQN));
                let is_global = v
                    .annotations
                    .iter()
                    .any(|ann| self.annotation_use_resolves_to_fqn(ann, GLOBAL_FQN));

                let storage = if is_thread_local {
                    Some(super::TopLevelVarStorage::ThreadLocal)
                } else if is_global {
                    Some(super::TopLevelVarStorage::Global)
                } else {
                    None
                };

                if let Some(storage) = storage {
                    self.top_level_vars.insert(
                        fqn.clone(),
                        super::TopLevelVar {
                            fqn: fqn.clone(),
                            source_path: self.source.path().to_path_buf(),
                            span: v.span,
                            storage,
                            ty,
                            init: init.clone(),
                        },
                    );
                }
            }
        }

        ValDecl {
            span: v.span,
            id,
            name,
            mutable: v.kind == ast::ValKind::Var,
            ty,
            init,
        }
    }

    fn annotation_use_resolves_to_fqn(&self, ann: &ast::AnnotationUse, expected_fqn: &str) -> bool {
        // 与 typecheck 保持一致：复用 Index 的 import/package 解析逻辑，避免仅按未限定名匹配导致误判。
        let ty = ast::TypeRef::Path(ast::TypePath {
            span: ann.span,
            segments: ann.path.clone(),
            args: Vec::new(),
        });

        matches!(
            self.index.type_ref_to_fqn_in_file(self.source, self.file, &ty),
            Some(fqn) if fqn == expected_fqn
        )
    }

    fn lower_type_ref(&mut self, t: &ast::TypeRef) -> TypeId {
        match t {
            ast::TypeRef::Path(p) => self.lower_type_path(p),
            ast::TypeRef::Tuple(tt) => {
                if tt.elements.is_empty() {
                    return self.builtins.unit;
                }
                let elements = tt.elements.iter().map(|e| self.lower_type_ref(e)).collect();
                self.types.ty_tuple(elements)
            }
            ast::TypeRef::Nullable { inner, .. } => {
                let inner = self.lower_type_ref(inner);
                self.types.ty_option(inner)
            }
            ast::TypeRef::Function(fun) => {
                let receiver = fun.receiver.as_ref().map(|r| self.lower_type_ref(r));
                let params = fun.params.iter().map(|p| self.lower_type_ref(p)).collect();
                let return_ty = self.lower_type_ref(&fun.return_ty);
                let effects = self.lower_effect_row_expr(fun.effects.as_ref());
                self.types.ty_function(
                    receiver,
                    params,
                    return_ty,
                    effects,
                    fun.effects.as_ref().is_some_and(|r| r.closed),
                )
            }
            ast::TypeRef::Star { .. } | ast::TypeRef::EffectRowArg { .. } => self.builtins.any,
        }
    }

    fn lower_type_path(&mut self, p: &ast::TypePath) -> TypeId {
        // 单段名且无实参：优先解析为当前作用域的 type parameter。
        if p.segments.len() == 1 && p.args.is_empty() {
            let name = p.segments[0].text(self.source);
            if let Some(id) = self.lookup_type_param(name) {
                return id;
            }
        }

        let fqn = self.index.type_ref_to_fqn_in_file(
            self.source,
            self.file,
            &ast::TypeRef::Path(p.clone()),
        );

        let Some(fqn) = fqn else {
            return self.builtins.any;
        };

        // 分离：普通 type args vs use-site effect row arg（`eff ...`）。
        let mut eff_arg: Option<&ast::EffectRowExpr> = None;
        let mut type_args: Vec<&ast::TypeRef> = Vec::new();
        for a in &p.args {
            match a {
                ast::TypeRef::EffectRowArg { row, .. } => {
                    eff_arg.get_or_insert(row);
                }
                other => type_args.push(other),
            }
        }

        // 少数 builtin/special-case：不走 nominal。
        match fqn.as_str() {
            "scoop.core.Any" => return self.builtins.any,
            "scoop.core.String" => return self.builtins.string,
            "scoop.core.Unit" => return self.builtins.unit,
            "scoop.core.Nothing" => return self.builtins.nothing,
            "scoop.core.Bool" => return self.builtins.bool_,
            "scoop.core.Char" => return self.builtins.char_,
            "scoop.core.Float64" => return self.builtins.float64,
            "scoop.core.Float32" => return self.builtins.float32,
            "scoop.core.Int" => return self.builtins.int,
            // T1027：internal atomics（`__AtomicInt`）——与 `Int` 相同布局的内部原子整型。
            "scoop.unsafe.__AtomicInt" => return self.builtins.int,
            "scoop.core.UInt" => return self.builtins.uint,
            "scoop.core.Option" => {
                let inner = type_args
                    .first()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
                return self.types.ty_option(inner);
            }
            _ => {}
        }

        // `Int32`/`UInt64` 这类固定位宽整数：若出现在 sysroot/type env 中，直接 lowering 为内建整数族。
        if let Some(bits) = fqn
            .strip_prefix("scoop.core.Int")
            .and_then(|s| s.parse::<u16>().ok())
        {
            return self.types.ty_int_n(bits);
        }
        if let Some(bits) = fqn
            .strip_prefix("scoop.core.UInt")
            .and_then(|s| s.parse::<u16>().ok())
        {
            return self.types.ty_uint_n(bits);
        }

        let args = type_args
            .iter()
            .map(|a| self.lower_type_ref(a))
            .collect::<Vec<_>>();
        let eff = eff_arg.map(|e| self.lower_effect_row_expr(Some(e)));
        self.intern_nominal(fqn, args, eff)
    }

    fn lower_effect_row_expr(&mut self, expr: Option<&ast::EffectRowExpr>) -> EffectRow {
        let Some(expr) = expr else {
            return EffectRow::pure();
        };
        if expr.terms.is_empty() {
            return EffectRow::pure();
        }

        let mut terms: Vec<TypeId> = Vec::with_capacity(expr.terms.len());
        for term in &expr.terms {
            terms.push(self.lower_type_path(term));
        }
        EffectRow::new(terms)
    }

    fn intern_nominal(&mut self, fqn: String, args: Vec<TypeId>, eff: Option<EffectRow>) -> TypeId {
        let nominal = NominalType { fqn, args, eff };

        // 尝试用 `type_kinds` 判断 struct/enum（value type）vs class/interface/effect（ref type）。
        let kind = self.type_kinds.get(&nominal.fqn).copied();
        match kind {
            Some(ast::TypeKind::Struct | ast::TypeKind::Enum) => self
                .types
                .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
            _ => self
                .types
                .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
        }
    }

    fn push_type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            self.type_param_scopes.push(HashMap::new());
            return;
        }

        let decl_file = self.source.path().to_path_buf();
        let mut frame = HashMap::new();
        for p in params {
            let name = p.name.text(self.source).to_string();
            let id = self.types.ty_param(TypeParamType {
                name: name.clone(),
                decl_file: decl_file.clone(),
                decl_span: p.name.span,
            });
            frame.insert(name, id);
        }
        self.type_param_scopes.push(frame);
    }

    /// 直接注入一组“使用点 type param 绑定”（name → TypeId）。
    ///
    /// 用途：
    /// - 单态化实例生成：把 `T` 等抽象类型替换为调用点推断出的具体类型。
    fn push_type_param_bindings(&mut self, bindings: impl IntoIterator<Item = (String, TypeId)>) {
        let mut frame = HashMap::new();
        for (name, id) in bindings {
            frame.insert(name, id);
        }
        self.type_param_scopes.push(frame);
    }

    fn pop_type_params(&mut self) {
        let _ = self.type_param_scopes.pop();
    }

    fn lookup_type_param(&self, name: &str) -> Option<TypeId> {
        self.type_param_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValScope {
    TopLevel,
    Local,
}

/// 为 `scoop dump-hir` 生成 HIR（最小实现）。
///
/// 流程：
/// 1) parse 源文件为 AST；
/// 2) 构建 sysroot + 当前文件的 `Index`；
/// 3) 运行 resolver（headers + bodies）把绑定结果写回 AST；
/// 4) 在一个新的 `TypeStore` 中 intern builtin types，收集 struct 布局信息，并把 AST 降为 HIR
///    （未覆盖节点用 `Any` 占位）。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredHir, HirLowerError> {
    let mut ast = parse_file(source)?;
    crate::comptime::trim_package_level_comptime_ifs(source, &mut ast)?;

    let index = {
        // 注意：`check_file_bodies` 需要 `&mut ast`，因此这里把构建 index 的临时借用放在独立作用域中，
        // 避免把 `&ast` 存到更长生命周期的容器里导致借用冲突。
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &session.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((source, &ast));
        Index::build(&pairs)?
    };

    let headers = crate::resolve::check_file_headers(source, &ast, &index)?;
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers)?;

    let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));
    let type_kinds = collect_type_decl_kinds(&pairs);
    let delegated_properties = collect_delegated_properties(&pairs);
    let class_vtables = crate::vtable::collect_class_vtables(&pairs, &index)?;
    let (interfaces, class_itables) =
        crate::itable::collect_interfaces_and_class_itables(&pairs, &index, &class_vtables)?;

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    // 先降 HIR（保持 fixtures 中 `TypeId` 分配顺序稳定），再补充 struct 布局索引供后端使用。
    let pkg_prefix = package_prefix(source, ast.package.as_ref());
    let (file, member_funs, mut ctor_call_sites, top_level_vars, top_level_consts) = {
        let mut ctx = HirLowering::new(
            source,
            &ast,
            &index,
            &mut types,
            HirLoweringSetup {
                typecheck_types: None,
                type_kinds: &type_kinds,
                delegated_properties: &delegated_properties,
                builtins,
            },
        );
        let file = ctx.lower_file();
        let member_funs = ctx.collect_member_funs(&pkg_prefix);
        let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
        let top_level_vars = std::mem::take(&mut ctx.top_level_vars);
        let top_level_consts = std::mem::take(&mut ctx.top_level_consts);
        (
            file,
            member_funs,
            ctor_call_sites,
            top_level_vars,
            top_level_consts,
        )
    };

    // T0828：收集 object/companion object 的 once 初始化信息（不影响 HIR dump 输出稳定性）。
    let (object_inits, object_ctor_call_sites) =
        collect_object_inits(source, &ast, &index, &type_kinds, &mut types, builtins);
    ctor_call_sites.extend(object_ctor_call_sites);
    // T1312：收集 class 初始化信息（Appendix B.2.2）。
    let (class_inits, class_ctor_call_sites) =
        collect_class_inits(source, &ast, &index, &type_kinds, &mut types, builtins);
    ctor_call_sites.extend(class_ctor_call_sites);

    // T1006：收集 `@Extern` 外部函数的符号名与 ABI（side table；不影响 dump-hir 输出）。
    let extern_funs = collect_extern_funs(source, &ast);
    let extern_libs = collect_extern_libs(&pairs);

    // T0811：早期 LLVM codegen 需要知道 struct 的字段顺序与字段类型，用于生成字段 GEP 索引。
    let mut struct_layouts = collect_struct_layouts(&pairs, &index);
    // T0813：早期 LLVM codegen 需要知道 enum 的 variant tag 与 payload 字段类型，用于生成判别与解构。
    let mut enum_layouts = collect_enum_layouts(&pairs, &index);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(&pairs, &types));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(&pairs, &types));
    // T0125：泛型 class 的具体实例化 ClassInit。
    let class_inits = {
        let mut ci = class_inits;
        ci.extend(collect_generic_class_instantiation_inits(
            &pairs, &types, &ci,
        ));
        ci
    };
    // T0126：为泛型 class 的具体实例化生成单态化的成员方法 FunDecl。
    let monomorphized_member_funs = collect_generic_class_member_fun_instantiations(
        &pairs,
        &index,
        &type_kinds,
        &mut types,
        builtins,
    );
    let mut member_funs = member_funs;
    member_funs.extend(monomorphized_member_funs);
    Ok(LoweredHir {
        file,
        member_funs,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_libs,
        top_level_vars,
        top_level_consts,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
    })
}

/// 在“给定编译单元（多个源文件）”的上下文中，为其中一个文件生成 HIR。
///
/// 用途：
/// - `.cone` 打包时导出 `api.scoopir`（TODO T1104）需要跨文件可见的类型 kind 信息；
/// - 后续多包编译/链接流程也会复用类似的“多文件 lowering”入口。
///
/// 约定：
/// - `file` 必须已经过 resolver（至少 `check_file_headers + check_file_bodies`），
///   以保证标识符绑定信息已写回 AST；
/// - `compilation_unit` 应包含 sysroot + 当前 cone 的全部源文件（稳定排序可保证输出更可回归）。
pub fn lower_for_compilation_unit(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Result<LoweredHir, HirLowerError> {
    let type_kinds = collect_type_decl_kinds(compilation_unit);
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let class_vtables = crate::vtable::collect_class_vtables(compilation_unit, index)?;
    let (interfaces, class_itables) = crate::itable::collect_interfaces_and_class_itables(
        compilation_unit,
        index,
        &class_vtables,
    )?;

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    // 先降 HIR（保持 `TypeId` 分配顺序稳定），再补充 side tables（layout/extern/object init）。
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let (file_hir, member_funs, mut ctor_call_sites, top_level_vars, top_level_consts) = {
        let mut ctx = HirLowering::new(
            source,
            file,
            index,
            &mut types,
            HirLoweringSetup {
                typecheck_types: None,
                type_kinds: &type_kinds,
                delegated_properties: &delegated_properties,
                builtins,
            },
        );
        let file_hir = ctx.lower_file();
        let member_funs = ctx.collect_member_funs(&pkg_prefix);
        let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
        let top_level_vars = std::mem::take(&mut ctx.top_level_vars);
        let top_level_consts = std::mem::take(&mut ctx.top_level_consts);
        (
            file_hir,
            member_funs,
            ctor_call_sites,
            top_level_vars,
            top_level_consts,
        )
    };

    let (object_inits, object_ctor_call_sites) =
        collect_object_inits(source, file, index, &type_kinds, &mut types, builtins);
    ctor_call_sites.extend(object_ctor_call_sites);
    let (class_inits, class_ctor_call_sites) =
        collect_class_inits(source, file, index, &type_kinds, &mut types, builtins);
    ctor_call_sites.extend(class_ctor_call_sites);
    let extern_funs = collect_extern_funs(source, file);
    let extern_libs = collect_extern_libs(compilation_unit);
    let mut struct_layouts = collect_struct_layouts(compilation_unit, index);
    let mut enum_layouts = collect_enum_layouts(compilation_unit, index);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        compilation_unit,
        &types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        compilation_unit,
        &types,
    ));
    // T0125：泛型 class 的具体实例化 ClassInit。
    let class_inits = {
        let mut ci = class_inits;
        ci.extend(collect_generic_class_instantiation_inits(
            compilation_unit,
            &types,
            &ci,
        ));
        ci
    };

    Ok(LoweredHir {
        file: file_hir,
        member_funs,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_libs,
        top_level_vars,
        top_level_consts,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
    })
}

/// 在”给定编译单元（多个源文件）”的上下文中，把多个文件一起 lowering 为一个 `LoweredHir`。
///
/// 用途（T1315a）：
/// - `scoop build/run` 注入 `stdlib/*.scoop` 后，需要让这些文件里的顶层函数在后端可见；
/// - T0150c：HIR 继续保留本地 span，但不再在 lowering 时 eager parse Int/String 字面量；
///   后续阶段通过“声明所属源文件 + 本地 span”回查原文。
pub fn lower_for_compilation_unit_multi_files(
    entry_source: &SourceFile,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_keys: &[crate::monomorph::MonomorphKey],
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, HirLowerError> {
    let type_kinds = collect_type_decl_kinds(compilation_unit);
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let class_vtables = crate::vtable::collect_class_vtables(compilation_unit, index)?;
    let (interfaces, class_itables) = crate::itable::collect_interfaces_and_class_itables(
        compilation_unit,
        index,
        &class_vtables,
    )?;

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    let mut items: Vec<Item> = Vec::new();
    let mut member_funs: Vec<FunDecl> = Vec::new();
    let mut ctor_call_sites: CtorCallSiteIndex = HashMap::new();
    let mut top_level_vars: super::TopLevelVarIndex = HashMap::new();
    let mut top_level_consts: super::TopLevelConstIndex = HashMap::new();

    for (source, file) in files_to_lower {
        let (
            file_hir,
            file_member_funs,
            file_ctor_call_sites,
            file_top_level_vars,
            file_top_level_consts,
        ) = {
            let mut ctx = HirLowering::new(
                source,
                file,
                index,
                &mut types,
                HirLoweringSetup {
                    typecheck_types: Some(typecheck_types),
                    type_kinds: &type_kinds,
                    delegated_properties: &delegated_properties,
                    builtins,
                },
            );
            let file_hir = ctx.lower_file();
            // 字面量已不再依赖“仅入口文件可切片”的旧路径，因此这里可以稳定收集所有文件的 member_funs。
            let pkg_prefix = package_prefix(source, file.package.as_ref());
            let file_member_funs = ctx.collect_member_funs(&pkg_prefix);
            let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
            let file_top_level_vars = std::mem::take(&mut ctx.top_level_vars);
            let file_top_level_consts = std::mem::take(&mut ctx.top_level_consts);
            (
                file_hir,
                file_member_funs,
                ctor_call_sites,
                file_top_level_vars,
                file_top_level_consts,
            )
        };

        // `CtorCallSiteIndex` 当前以 `Span` 作为 key（offset-only，不含文件标识）。
        // 为避免跨文件 span 冲突导致错误 codegen，当前阶段只保留入口文件的 ctor call-sites。
        if source.path() == entry_source.path() {
            ctor_call_sites.extend(file_ctor_call_sites);
        }

        top_level_vars.extend(file_top_level_vars);
        top_level_consts.extend(file_top_level_consts);
        member_funs.extend(file_member_funs);
        items.extend(file_hir.items);
    }

    // side tables：保持与 `lower_for_compilation_unit` 一致的收集逻辑（先降 HIR，再补充 layout/init/extern）。
    let extern_funs = files_to_lower
        .iter()
        .flat_map(|(source, file)| collect_extern_funs(source, file))
        .collect();
    let extern_libs = collect_extern_libs(compilation_unit);
    let mut struct_layouts = collect_struct_layouts(compilation_unit, index);
    let mut enum_layouts = collect_enum_layouts(compilation_unit, index);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        compilation_unit,
        &types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        compilation_unit,
        &types,
    ));

    let mut object_inits = ObjectInitIndex::new();
    let mut class_inits = ClassInitIndex::new();
    for (source, file) in files_to_lower {
        let (file_object_inits, file_object_ctor_call_sites) =
            collect_object_inits(source, file, index, &type_kinds, &mut types, builtins);
        object_inits.extend(file_object_inits);
        if source.path() == entry_source.path() {
            ctor_call_sites.extend(file_object_ctor_call_sites);
        }

        let (file_class_inits, file_class_ctor_call_sites) =
            collect_class_inits(source, file, index, &type_kinds, &mut types, builtins);
        class_inits.extend(file_class_inits);
        if source.path() == entry_source.path() {
            ctor_call_sites.extend(file_class_ctor_call_sites);
        }
    }
    // T0125：泛型 class 的具体实例化 ClassInit（第一遍：处理文件中已有的泛型 class 实例化类型）。
    class_inits.extend(collect_generic_class_instantiation_inits(
        compilation_unit,
        &types,
        &class_inits,
    ));

    // T0127：为泛型独立函数的具体实例化生成单态化的 FunDecl。
    // 注意：必须在 class member monomorphization 之前运行，因为独立函数的单态化
    // 可能在 TypeStore 中创建新的泛型 class 实例化类型（例如 `Printer<Greeter>`），
    // 这些类型需要被后续的 class member monomorphization 发现。
    let monomorphized_funs = collect_generic_fun_instantiations(
        compilation_unit,
        monomorph_keys,
        index,
        &type_kinds,
        &mut types,
        builtins,
        typecheck_types,
    );
    items.extend(monomorphized_funs.into_iter().map(Item::Fun));

    // T0130：第二遍 class 实例化 —— standalone fun monomorphization 可能在 TypeStore 中
    // 创建了新的泛型 class 实例化类型（例如 `Printer<Greeter>`），这里补充收集。
    class_inits.extend(collect_generic_class_instantiation_inits(
        compilation_unit,
        &types,
        &class_inits,
    ));

    // T0126：为泛型 class 的具体实例化生成单态化的成员方法 FunDecl。
    member_funs.extend(collect_generic_class_member_fun_instantiations(
        compilation_unit,
        index,
        &type_kinds,
        &mut types,
        builtins,
    ));

    Ok(LoweredHir {
        file: File { items },
        member_funs,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_libs,
        top_level_vars,
        top_level_consts,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
    })
}

pub(crate) struct LoweringInputs<'a> {
    pub(crate) source: &'a SourceFile,
    pub(crate) file: &'a ast::File,
    pub(crate) index: &'a Index,
    pub(crate) type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub(crate) types: &'a mut TypeStore,
    pub(crate) builtins: BuiltinTypes,
}

pub(super) struct BoundMemberFunLoweringTarget<'a> {
    pub(super) owner_fqn: &'a str,
    pub(super) this_decl_span: Span,
    pub(super) this_concrete_args: &'a [TypeId],
    pub(super) fun: &'a ast::FunDecl,
}

/// 将给定的 `ast::FunDecl` 在”已绑定 type params”的语境下降低为 HIR（用于单态化，T0712）。
///
/// 说明：
/// - 该函数假设调用方已在 AST 上运行 resolver（headers + bodies），以便 `ValueIdent.resolved` 等信息可用；
/// - `type_bindings` 用于把函数声明处的 `T` 等 type params 映射为具体 `TypeId`（调用点推断结果）。
pub(crate) fn lower_fun_with_type_bindings(
    inputs: LoweringInputs<'_>,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    let LoweringInputs {
        source,
        file,
        index,
        type_kinds,
        types,
        builtins,
    } = inputs;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let compilation_unit = [(source, file)];
    let delegated_properties = collect_delegated_properties(&compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types: None,
            type_kinds,
            delegated_properties: &delegated_properties,
            builtins,
        },
    );
    ctx.push_type_param_bindings(type_bindings);
    let out = ctx.lower_fun_decl_with_bound_type_params(&pkg_prefix, fun);
    ctx.pop_type_params();
    out
}

/// T0126: 将泛型类的成员方法在”已绑定 owner type params”的语境下降低为 HIR。
///
/// 说明：
/// - 类似 `lower_fun_with_type_bindings`，但处理的是 class member method（带隐式 `this` 参数）；
/// - `owner_type_bindings` 映射 owner 的 type params 到具体 TypeId（例如 `T → Int` for `Box<Int>`）；
/// - `this_concrete_args` 是 owner 的具体类型实参（例如 `[Int]` for `Box<Int>`），用于构造 `this` 的精确类型；
/// - 生成的 FunDecl 中 `this` 参数类型为具体的 `owner_fqn<concrete_args>`，所有引用 owner
///   type params 的地方均被替换为具体类型。
pub(crate) fn lower_member_fun_with_type_bindings(
    inputs: LoweringInputs<'_>,
    target: BoundMemberFunLoweringTarget<'_>,
    owner_type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    let LoweringInputs {
        source,
        file,
        index,
        type_kinds,
        types,
        builtins,
    } = inputs;
    let BoundMemberFunLoweringTarget {
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        fun,
    } = target;
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let compilation_unit = [(source, file)];
    let delegated_properties = collect_delegated_properties(&compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        types,
        HirLoweringSetup {
            typecheck_types: None,
            type_kinds,
            delegated_properties: &delegated_properties,
            builtins,
        },
    );
    // 先绑定 owner type params（例如 class Box<T> 的 T → Int），
    // 再由 lower_member_fun_decl_with_bound_type_params 处理方法 body。
    ctx.push_type_param_bindings(owner_type_bindings);
    let out = ctx.lower_member_fun_decl_with_bound_type_params(
        &pkg_prefix,
        owner_fqn,
        this_decl_span,
        this_concrete_args,
        fun,
    );
    ctx.pop_type_params();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::LiteralKind;
    use crate::resolve::Index;
    use crate::typecheck;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn collect_unresolved_member_names_in_expr(expr: &Expr, out: &mut Vec<String>) {
        match &expr.kind {
            ExprKind::MemberAccess { receiver, member } => {
                if member.resolved.is_none() {
                    out.push(member.name.clone());
                }
                collect_unresolved_member_names_in_expr(receiver, out);
            }
            ExprKind::Call { callee, args } => {
                collect_unresolved_member_names_in_expr(callee, out);
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => {
                            collect_unresolved_member_names_in_expr(expr, out)
                        }
                        CallArg::Named { value, .. } => {
                            collect_unresolved_member_names_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::When { subject, arms } => {
                collect_unresolved_member_names_in_expr(subject, out);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        collect_unresolved_member_names_in_expr(guard, out);
                    }
                    collect_unresolved_member_names_in_expr(&arm.body, out);
                }
            }
            ExprKind::Block(block) => collect_unresolved_member_names_in_block(block, out),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_unresolved_member_names_in_expr(cond, out);
                collect_unresolved_member_names_in_expr(then_branch, out);
                if let Some(else_branch) = else_branch.as_deref() {
                    collect_unresolved_member_names_in_expr(else_branch, out);
                }
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. } => collect_unresolved_member_names_in_expr(expr, out),
            ExprKind::Binary { lhs, rhs, .. } => {
                collect_unresolved_member_names_in_expr(lhs, out);
                collect_unresolved_member_names_in_expr(rhs, out);
            }
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    collect_unresolved_member_names_in_expr(&field.value, out);
                }
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    collect_unresolved_member_names_in_expr(element, out);
                }
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                        collect_unresolved_member_names_in_expr(expr, out);
                    }
                }
            }
            ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => {
                            collect_unresolved_member_names_in_expr(expr, out)
                        }
                        CallArg::Named { value, .. } => {
                            collect_unresolved_member_names_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::Handle(handle) => {
                collect_unresolved_member_names_in_block(&handle.body, out);
                for arm in &handle.arms {
                    collect_unresolved_member_names_in_expr(&arm.body, out);
                }
                if let Some(finally) = handle.finally.as_ref() {
                    collect_unresolved_member_names_in_block(finally, out);
                }
            }
            ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::Closure(_)
            | ExprKind::Missing
            | ExprKind::Todo(_) => {}
        }
    }

    fn collect_unresolved_member_names_in_block(block: &Block, out: &mut Vec<String>) {
        for stmt in &block.stmts {
            match &stmt.kind {
                StmtKind::Expr(expr) => collect_unresolved_member_names_in_expr(expr, out),
                StmtKind::Val(val) => {
                    if let Some(init) = val.init.as_ref() {
                        collect_unresolved_member_names_in_expr(init, out);
                    }
                }
                StmtKind::Assign { lhs, rhs, .. } => {
                    collect_unresolved_member_names_in_expr(lhs, out);
                    collect_unresolved_member_names_in_expr(rhs, out);
                }
                StmtKind::While { cond, body } => {
                    collect_unresolved_member_names_in_expr(cond, out);
                    collect_unresolved_member_names_in_block(body, out);
                }
                StmtKind::Return { value } => {
                    if let Some(value) = value.as_ref() {
                        collect_unresolved_member_names_in_expr(value, out);
                    }
                }
                StmtKind::Empty
                | StmtKind::Break { .. }
                | StmtKind::Continue { .. }
                | StmtKind::Todo(_) => {}
            }
        }
    }

    fn collect_top_level_call_fqns_in_expr(expr: &Expr, out: &mut Vec<String>) {
        match &expr.kind {
            ExprKind::Call { callee, args } => {
                if let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind {
                    out.push(fqn.clone());
                }
                collect_top_level_call_fqns_in_expr(callee, out);
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => collect_top_level_call_fqns_in_expr(expr, out),
                        CallArg::Named { value, .. } => {
                            collect_top_level_call_fqns_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::MemberAccess { receiver, .. } => {
                collect_top_level_call_fqns_in_expr(receiver, out);
            }
            ExprKind::When { subject, arms } => {
                collect_top_level_call_fqns_in_expr(subject, out);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        collect_top_level_call_fqns_in_expr(guard, out);
                    }
                    collect_top_level_call_fqns_in_expr(&arm.body, out);
                }
            }
            ExprKind::Block(block) => collect_top_level_call_fqns_in_block(block, out),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_top_level_call_fqns_in_expr(cond, out);
                collect_top_level_call_fqns_in_expr(then_branch, out);
                if let Some(else_branch) = else_branch.as_deref() {
                    collect_top_level_call_fqns_in_expr(else_branch, out);
                }
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. } => collect_top_level_call_fqns_in_expr(expr, out),
            ExprKind::Binary { lhs, rhs, .. } => {
                collect_top_level_call_fqns_in_expr(lhs, out);
                collect_top_level_call_fqns_in_expr(rhs, out);
            }
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    collect_top_level_call_fqns_in_expr(&field.value, out);
                }
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    collect_top_level_call_fqns_in_expr(element, out);
                }
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                        collect_top_level_call_fqns_in_expr(expr, out);
                    }
                }
            }
            ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) => collect_top_level_call_fqns_in_expr(expr, out),
                        CallArg::Named { value, .. } => {
                            collect_top_level_call_fqns_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::Handle(handle) => {
                collect_top_level_call_fqns_in_block(&handle.body, out);
                for arm in &handle.arms {
                    collect_top_level_call_fqns_in_expr(&arm.body, out);
                }
                if let Some(finally) = handle.finally.as_ref() {
                    collect_top_level_call_fqns_in_block(finally, out);
                }
            }
            ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::Closure(_)
            | ExprKind::Missing
            | ExprKind::Todo(_) => {}
        }
    }

    fn collect_top_level_call_fqns_in_block(block: &Block, out: &mut Vec<String>) {
        for stmt in &block.stmts {
            match &stmt.kind {
                StmtKind::Expr(expr) => collect_top_level_call_fqns_in_expr(expr, out),
                StmtKind::Val(val) => {
                    if let Some(init) = val.init.as_ref() {
                        collect_top_level_call_fqns_in_expr(init, out);
                    }
                }
                StmtKind::Assign { lhs, rhs, .. } => {
                    collect_top_level_call_fqns_in_expr(lhs, out);
                    collect_top_level_call_fqns_in_expr(rhs, out);
                }
                StmtKind::While { cond, body } => {
                    collect_top_level_call_fqns_in_expr(cond, out);
                    collect_top_level_call_fqns_in_block(body, out);
                }
                StmtKind::Return { value } => {
                    if let Some(value) = value.as_ref() {
                        collect_top_level_call_fqns_in_expr(value, out);
                    }
                }
                StmtKind::Empty
                | StmtKind::Break { .. }
                | StmtKind::Continue { .. }
                | StmtKind::Todo(_) => {}
            }
        }
    }

    #[test]
    fn lower_minimal_file_smoke() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual("<mem>", "package a\nfun main() { val x: Int = 1; x }");

        let lowered = lower_for_dump(&sess, &src).unwrap();
        assert!(!lowered.file.items.is_empty());
    }

    #[test]
    fn hir_fixture_minimal_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/minimal.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hir/minimal.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_handle_perform_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/handle_perform.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/handle_perform.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_control_flow_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/control_flow.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/control_flow.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_member_access_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/member_access.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/member_access.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_closure_non_capture_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_non_capture.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_non_capture.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn hir_fixture_closure_capture_val_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_capture_val.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/closure_capture_val.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn lower_float_literals_to_typed_hir_literals() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            "package a\nfun main() { val a = 2.75; val b = 0.5f; val c = 1e3 }\n",
        );

        let lowered = lower_for_dump(&sess, &src).unwrap();
        let Item::Fun(fun) = &lowered.file.items[0] else {
            panic!("期望第一个 item 为函数");
        };
        let body = fun.body.as_ref().expect("main 应有函数体");
        let [stmt_a, stmt_b, stmt_c] = body.stmts.as_slice() else {
            panic!("期望 main 中包含三个 val 语句");
        };

        let StmtKind::Val(val_a) = &stmt_a.kind else {
            panic!("第一个语句应为 val");
        };
        let init_a = val_a.init.as_ref().expect("a 应有 initializer");
        assert!(matches!(
            init_a.kind,
            ExprKind::Literal(LiteralKind::Float64(value)) if value == 2.75
        ));
        assert_eq!(lowered.types.display(init_a.ty).to_string(), "Float64");

        let StmtKind::Val(val_b) = &stmt_b.kind else {
            panic!("第二个语句应为 val");
        };
        let init_b = val_b.init.as_ref().expect("b 应有 initializer");
        assert!(matches!(
            init_b.kind,
            ExprKind::Literal(LiteralKind::Float32(value)) if value == 0.5
        ));
        assert_eq!(lowered.types.display(init_b.ty).to_string(), "Float32");

        let StmtKind::Val(val_c) = &stmt_c.kind else {
            panic!("第三个语句应为 val");
        };
        let init_c = val_c.init.as_ref().expect("c 应有 initializer");
        assert!(matches!(
            init_c.kind,
            ExprKind::Literal(LiteralKind::Float64(value)) if value == 1000.0
        ));
        assert_eq!(lowered.types.display(init_c.ty).to_string(), "Float64");
    }

    #[test]
    fn lower_for_compilation_unit_multi_files_includes_non_entry_top_level_funs() {
        let sess = Session::new().unwrap();

        let src_lib = SourceFile::new_virtual(
            "<lib>",
            r#"
package fixtures.t1315a

import scoop.core.*

fun id(x: Int): Int { return x }
"#,
        );
        let src_main = SourceFile::new_virtual(
            "<main>",
            r#"
package fixtures.t1315a

import scoop.core.*

fun main(): Int { return id(1) }
"#,
        );

        let mut ast_lib = parse_file(&src_lib).unwrap();
        let mut ast_main = parse_file(&src_main).unwrap();

        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for f in &sess.sysroot().files {
                pairs.push((&f.source, &f.ast));
            }
            pairs.push((&src_lib, &ast_lib));
            pairs.push((&src_main, &ast_main));
            Index::build(&pairs).unwrap()
        };

        let h_lib = crate::resolve::check_file_headers(&src_lib, &ast_lib, &index).unwrap();
        crate::resolve::check_file_bodies(&src_lib, &mut ast_lib, &index, &h_lib).unwrap();

        let h_main = crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
        crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &h_main).unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&src_lib, &ast_lib));
        unit.push((&src_main, &ast_main));

        let files_to_lower = vec![(&src_lib, &ast_lib), (&src_main, &ast_main)];
        let empty_types = TypeStore::new();
        let lowered = lower_for_compilation_unit_multi_files(
            &src_main,
            &index,
            &unit,
            &files_to_lower,
            &[],
            &empty_types,
        )
        .unwrap();

        let fun_fqns: HashSet<&str> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                _ => None,
            })
            .collect();

        assert!(fun_fqns.contains("fixtures.t1315a.id"));
        assert!(fun_fqns.contains("fixtures.t1315a.main"));
    }

    #[test]
    fn lower_for_compilation_unit_multi_files_preserves_safe_member_access_resolution() {
        let sess = Session::new().unwrap();
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop");
        let source = SourceFile::load(&fixture_path).unwrap();
        let mut ast = parse_file(&source).unwrap();
        crate::comptime::trim_package_level_comptime_ifs(&source, &mut ast).unwrap();

        typecheck::check_file_headers(&source, &ast).unwrap();
        typecheck::check_file_struct_decls(&source, &ast).unwrap();

        let index = {
            let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for f in &sess.sysroot().files {
                unit.push((&f.source, &f.ast));
            }
            unit.push((&source, &ast));
            Index::build(&unit).unwrap()
        };
        let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_properties(&source, &ast, &index, &env).unwrap();
        typecheck::check_file_inheritance(&source, &ast, &index).unwrap();
        typecheck::check_file_interfaces(&source, &ast, &index, &env).unwrap();
        typecheck::check_file_override_effects(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_where_clauses(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_overload_conflicts(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

        let safe_debug = format!("{:?}", ast.safe_member_access_resolved.borrow());
        assert!(safe_debug.contains("User.score"), "{safe_debug}");
        assert!(safe_debug.contains("Config.port"), "{safe_debug}");
        assert!(safe_debug.contains("doubleScore"), "{safe_debug}");

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &sess.sysroot().files {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&source, &ast));

        let lowered = lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &[(&source, &ast)],
            &[],
            &types,
        )
        .unwrap();

        let mut unresolved_member_names = Vec::new();
        let mut top_level_call_fqns = Vec::new();
        for item in &lowered.file.items {
            if let Item::Fun(fun) = item
                && let Some(body) = fun.body.as_ref()
            {
                collect_unresolved_member_names_in_block(body, &mut unresolved_member_names);
                collect_top_level_call_fqns_in_block(body, &mut top_level_call_fqns);
            }
        }

        assert!(!unresolved_member_names.iter().any(|name| name == "score"));
        assert!(!unresolved_member_names.iter().any(|name| name == "port"));
        assert!(top_level_call_fqns.iter().any(|fqn| fqn == "doubleScore"));
    }
}
