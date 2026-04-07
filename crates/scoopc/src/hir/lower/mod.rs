//! AST → HIR 的最小 lowering（TODO T0701）。
//!
//! 说明：
//! - 这里的 lowering 仅用于 `dump-hir` 的调试输出，因此实现上优先保证“稳定输出 + 不 panic”；。
//! - 完整 lowering（含类型推断结果、更多语法节点）会在后续任务（TODO T0702+）逐步补齐。

mod types;
mod util;

pub use types::{HirLowerError, LoweredHir};

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
    Block, CallArg, Capture, ClassCtor, ClassCtorDelegation, ClassCtorKind, ClassCtorParam,
    ClassField, ClassInit, ClassInitIndex, ClassInitStep, ClosureExpr, ClosureId,
    CtorCallSiteIndex, EffectOpRef, EnumLayout, EnumLayoutIndex, EnumRepr, EnumVariantFieldLayout,
    EnumVariantLayout, Expr, ExprKind, File, FunDecl, HandleArm, HandleArmKind, HandleBinder,
    HandleExpr, HandleOp, Item, LiteralKind, MemberAccess, MemberRef, ObjectInit, ObjectInitIndex,
    ObjectInitStep, ObjectProperty, Param, Stmt, StmtKind, StructCLayout, StructFieldLayout,
    StructLayout, StructLayoutIndex, StructLitField, SymbolId, ValDecl, ValueRef, WhenArm, WhenPat,
};

use types::*;
use util::*;

/// HIR lowering 的上下文（按单文件构建，用于 `dump-hir` 与 HIR fixtures）。
struct HirLowering<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
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

    fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        type_kinds: &'a HashMap<String, ast::TypeKind>,
        delegated_properties: &'a DelegatedPropertyIndex,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
    ) -> Self {
        Self {
            source,
            file,
            index,
            type_kinds,
            delegated_properties,
            default_arg_funs: HashMap::new(),
            ctor_call_sites: HashMap::new(),
            top_level_vars: HashMap::new(),
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
            ast::Item::ExtensionProperty(p) => Item::Todo {
                span: p.span,
                kind: "extension_property",
            },
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
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    /// 降低一个 class/object 的 member `fun` 为“顶层函数形态”（显式 `this` 参数）。
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
            is_const: is_const_fun,
            ty,
            params,
            return_ty,
            body,
        }
    }

    fn rewrite_async_fun_block(&mut self, mut block: Block, wrap_tail_expr: bool) -> Block {
        // T0623：把 `async fun` 的返回值包装成 task 句柄：
        // - `return expr` → `return __scoop_task_spawn_int(expr)`（early stage 仅支持 Int）；
        // - block tail expr（隐式返回）同样做一次包装。

        for stmt in &mut block.stmts {
            match &mut stmt.kind {
                StmtKind::While { body, .. } => {
                    // 这里用 `replace` 把 body move 出来，避免对 Block 增加 Default 约束。
                    let placeholder = Block {
                        span: body.span,
                        ty: body.ty,
                        stmts: Vec::new(),
                    };
                    let old = std::mem::replace(body, placeholder);
                    *body = self.rewrite_async_fun_block(old, false);
                }
                StmtKind::Return { value } => {
                    if let Some(v) = value.take() {
                        *value = Some(self.wrap_task_spawn_int_call(stmt.span, v));
                    }
                }
                _ => {}
            }
        }

        if wrap_tail_expr {
            // 隐式返回：若 block 末尾是表达式语句，则将其值包装为 task handle。
            if let Some(last) = block.stmts.last_mut() {
                if let StmtKind::Expr(expr) = &mut last.kind {
                    // 用占位符把 expr move 出来，避免对 Expr 增加 Default 约束。
                    let expr_span = expr.span;
                    let expr_ty = expr.ty;
                    let old = std::mem::replace(
                        expr,
                        Expr {
                            span: expr_span,
                            ty: expr_ty,
                            kind: ExprKind::Missing,
                        },
                    );
                    *expr = self.wrap_task_spawn_int_call(expr_span, old);
                }
            }
        }

        // 重新计算 block 类型：保持与 `lower_block` 的规则一致。
        block.ty = block
            .stmts
            .last()
            .and_then(|s| match &s.kind {
                StmtKind::Expr(e) => Some(e.ty),
                _ => None,
            })
            .unwrap_or(self.builtins.unit);

        block
    }

    fn wrap_task_spawn_int_call(&mut self, at: Span, value: Expr) -> Expr {
        // `__scoop_task_spawn_int(value)` → task handle (`UInt`)。
        let fqn = Self::TASK_SPAWN_INT_FQN.to_string();
        let callee = Expr {
            span: at,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn,
            }),
        };

        Expr {
            span: at,
            ty: self.builtins.uint,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: vec![CallArg::Positional(value)],
            },
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
        // T1317c：数组字面量 `[...]` 的 lowering 依赖“期望的容器类型”（Array vs MutableArray）。
        // 这里从显式的类型注解（若存在）向 initializer 传播该 hint。
        let init_expected = ExpectedExpr {
            array_lit_target: v
                .ty
                .as_ref()
                .and_then(|ty| self.array_lit_target_from_type_ref(ty)),
        };
        let init = v
            .init
            .as_ref()
            .map(|e| self.lower_expr_with_expected(pkg_prefix, e, init_expected));

        let declared_ty = v.ty.as_ref().map(|t| self.lower_type_ref(t));

        let ty = declared_ty
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

        // T1023：顶层 `@ThreadLocal/@Global var` 需要后端生成静态存储。
        if scope == ValScope::TopLevel && v.kind == ast::ValKind::Var {
            if let Some(fqn) = top_level_fqn.as_ref() {
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

    fn lower_block(&mut self, pkg_prefix: &str, b: &ast::Block) -> Block {
        let mut stmts = Vec::with_capacity(b.stmts.len());
        for s in &b.stmts {
            stmts.push(self.lower_stmt(pkg_prefix, s));
        }

        // 当前阶段：用 block 最后一条“表达式语句”的类型作为 block 类型，否则视为 Unit。
        let ty = stmts
            .last()
            .and_then(|s| match &s.kind {
                StmtKind::Expr(e) => Some(e.ty),
                _ => None,
            })
            .unwrap_or(self.builtins.unit);

        Block {
            span: b.span,
            ty,
            stmts,
        }
    }

    fn lower_stmt(&mut self, pkg_prefix: &str, s: &ast::Stmt) -> Stmt {
        let (kind, ty) = match &s.kind {
            ast::StmtKind::Empty => (StmtKind::Empty, self.builtins.unit),
            ast::StmtKind::Expr(e) => {
                // 赋值在 AST 中以表达式节点承载，但在 HIR 中视为语句（便于后续 MIR lowering）。
                if let ast::ExprKind::Assign { lhs, eq_span, rhs } = &e.kind {
                    if let Some(call) =
                        self.try_lower_delegated_property_assign(pkg_prefix, e.span, lhs, rhs)
                    {
                        (StmtKind::Expr(call), self.builtins.unit)
                    } else {
                        let lhs = self.lower_expr(pkg_prefix, lhs);
                        let rhs = self.lower_expr(pkg_prefix, rhs);
                        (
                            StmtKind::Assign {
                                lhs,
                                eq_span: *eq_span,
                                rhs,
                            },
                            self.builtins.unit,
                        )
                    }
                } else {
                    let e = self.lower_expr(pkg_prefix, e);
                    (StmtKind::Expr(e), self.builtins.unit)
                }
            }
            ast::StmtKind::Val(v) => {
                let v = self.lower_val_decl(pkg_prefix, v, ValScope::Local);
                (StmtKind::Val(v), self.builtins.unit)
            }
            ast::StmtKind::Return { value, .. } => {
                let value = value.as_ref().map(|e| self.lower_expr(pkg_prefix, e));
                (StmtKind::Return { value }, self.builtins.nothing)
            }
            ast::StmtKind::Missing => (StmtKind::Todo("missing_stmt"), self.builtins.unit),
            ast::StmtKind::While { cond, body, .. } => (
                StmtKind::While {
                    cond: self.lower_expr(pkg_prefix, cond),
                    body: self.lower_block(pkg_prefix, body),
                },
                self.builtins.unit,
            ),
            ast::StmtKind::For(_) => (StmtKind::Todo("for"), self.builtins.unit),
            ast::StmtKind::Break { break_span } => (
                StmtKind::Break {
                    break_span: *break_span,
                },
                self.builtins.unit,
            ),
            ast::StmtKind::Continue { continue_span } => (
                StmtKind::Continue {
                    continue_span: *continue_span,
                },
                self.builtins.unit,
            ),
            ast::StmtKind::ComptimeBlock { .. } => {
                (StmtKind::Todo("comptime_block"), self.builtins.unit)
            }
            ast::StmtKind::ComptimeIf(_) => (StmtKind::Todo("comptime_if"), self.builtins.unit),
            ast::StmtKind::ComptimeFor(_) => (StmtKind::Todo("comptime_for"), self.builtins.unit),
        };

        Stmt {
            span: s.span,
            ty,
            kind,
        }
    }

    fn try_lower_delegated_property_assign(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
    ) -> Option<Expr> {
        let ast::ExprKind::MemberAccess { receiver, member } = &lhs.kind else {
            return None;
        };
        let Some(ast::ResolvedMemberRef::Value { fqn }) = member.resolved.as_ref() else {
            return None;
        };
        let info = self.delegated_properties.get(fqn).cloned()?;

        match info {
            DelegatedPropertyInfo::Observable(info) => self
                .lower_observable_delegated_property_assign(
                    pkg_prefix,
                    span,
                    member.span,
                    receiver,
                    rhs,
                    fqn,
                    &info,
                ),
            DelegatedPropertyInfo::Vetoable(info) => self.lower_vetoable_delegated_property_assign(
                pkg_prefix,
                span,
                member.span,
                receiver,
                rhs,
                fqn,
                &info,
            ),
            DelegatedPropertyInfo::Generic(info) => {
                let receiver = self.lower_expr(pkg_prefix, receiver);
                let this_ref = receiver.clone();
                let delegate = self.lower_generic_delegated_property_delegate_access_expr(
                    member.span,
                    receiver.clone(),
                    &info,
                );
                let callee = Expr {
                    span: member.span,
                    ty: self.builtins.any,
                    kind: ExprKind::MemberAccess {
                        receiver: Box::new(delegate),
                        member: MemberAccess {
                            span: member.span,
                            name: "setValue".to_string(),
                            resolved: None,
                        },
                    },
                };

                let meta = self.lower_property_meta_ref_expr(member.span, &info.property_meta_fqn);
                let value = self.lower_expr(pkg_prefix, rhs);

                Some(Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Call {
                        callee: Box::new(callee),
                        args: vec![
                            CallArg::Positional(this_ref),
                            CallArg::Positional(meta),
                            CallArg::Positional(value),
                        ],
                    },
                })
            }

            DelegatedPropertyInfo::Lazy(_) | DelegatedPropertyInfo::MapBacked => None,
        }
    }

    fn lower_expr(&mut self, pkg_prefix: &str, e: &ast::Expr) -> Expr {
        self.lower_expr_with_expected(pkg_prefix, e, ExpectedExpr::default())
    }

    /// lowering 表达式并携带“期望类型 hint”。
    ///
    /// 注意：该 hint 仅用于把 `[...]` 降到稳定的 builder/intrinsics 调用形态（TODO T1317c），
    /// 不等价于完整 typecheck 的 expected-type 推断。
    fn lower_expr_with_expected(
        &mut self,
        pkg_prefix: &str,
        e: &ast::Expr,
        expected: ExpectedExpr,
    ) -> Expr {
        let (kind, ty) = match &e.kind {
            ast::ExprKind::Missing => (ExprKind::Missing, self.builtins.any),
            ast::ExprKind::IntLit => (ExprKind::Literal(LiteralKind::Int), self.builtins.int),
            ast::ExprKind::StringLit => {
                (ExprKind::Literal(LiteralKind::String), self.builtins.string)
            }
            ast::ExprKind::UnitLit => (ExprKind::Literal(LiteralKind::Unit), self.builtins.unit),
            ast::ExprKind::ArrayLit { elements } => match expected.array_lit_target {
                Some(target) => self.lower_array_lit_expr(pkg_prefix, e.span, elements, target),
                None => (ExprKind::Todo("array_lit"), self.builtins.any),
            },
            ast::ExprKind::ClassLit { .. } => (ExprKind::Todo("class_lit"), self.builtins.string),
            ast::ExprKind::InterpolatedString { raw, parts } => {
                let parts = parts
                    .iter()
                    .map(|p| match p {
                        ast::InterpolatedStringPart::Text { span } => {
                            super::InterpolatedStringPart::Text { span: *span }
                        }
                        ast::InterpolatedStringPart::Expr { expr } => {
                            super::InterpolatedStringPart::Expr {
                                expr: self.lower_expr(pkg_prefix, expr),
                            }
                        }
                    })
                    .collect();
                (
                    ExprKind::InterpolatedString { raw: *raw, parts },
                    self.builtins.string,
                )
            }
            ast::ExprKind::Ident(id) => self.lower_ident_expr(id),
            ast::ExprKind::Block(b) => {
                let b = self.lower_block(pkg_prefix, b);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::UnsafeBlock { body, .. } => {
                // `@Unsafe { ... }` 仅影响 typecheck 的 unsafe context，
                // 在 HIR/codegen 层面当前可按普通 block 表达式处理（T1004）。
                let b = self.lower_block(pkg_prefix, body);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::SafeBlock { body, .. } => {
                // `@Safe { ... }` 同样仅影响 typecheck 的 unsafe context，
                // 在 HIR/codegen 层面当前可按普通 block 表达式处理（T1021）。
                let b = self.lower_block(pkg_prefix, body);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::TypeApply { callee, .. } => {
                // v0：HIR 暂不承载显式类型实参；先把它视为 callee 的透明包装。
                // 反射 intrinsics 的 type args 语义目前由 comptime 解释器消费（T1204）。
                let inner = self.lower_expr(pkg_prefix, callee);
                (inner.kind, inner.ty)
            }
            ast::ExprKind::Call { callee, args } => {
                // 扩展函数调用（T0312）：把 `receiver.ext(args...)` 降糖为普通顶层调用：
                // `ext(receiver, args...)`。
                //
                // 说明：
                // - 运行期 codegen 当前只直接支持 `TopLevel` callee（以及少量特殊 member call）；
                // - 这里在 lowering 阶段提前把 extension call 改写为顶层调用，避免后端无法识别 `MemberAccess` callee。
                if let Some((kind, ty)) = (|| {
                    let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
                        return None;
                    };
                    let ast::ResolvedMemberRef::ExtensionFun { fqn } = member.resolved.as_ref()?
                    else {
                        return None;
                    };

                    let sig = self.fun_sig_by_fqn(fqn);
                    // expected-type hint 目前只用于数组字面量 `[...]` 的 lowering（Array vs MutableArray）。
                    // receiver 不是数组字面量时无需解析签名里的 receiver TypeRef，避免跨文件 span 误用。
                    let receiver_is_array_lit =
                        matches!(receiver.kind, ast::ExprKind::ArrayLit { .. });
                    let receiver_expected = ExpectedExpr {
                        array_lit_target: match receiver_is_array_lit {
                            true => sig
                                .as_ref()
                                .and_then(|sig| sig.receiver.as_ref())
                                .and_then(|ty| self.array_lit_target_from_type_ref(ty)),
                            false => None,
                        },
                    };
                    let receiver =
                        self.lower_expr_with_expected(pkg_prefix, receiver, receiver_expected);

                    let mut lowered_args = Vec::with_capacity(args.len() + 1);
                    lowered_args.push(CallArg::Positional(receiver));
                    let sig = sig;
                    let mut positional_index = 0usize;
                    for arg in args {
                        let expected = self.expected_expr_for_fun_call_arg(
                            sig.as_ref(),
                            arg,
                            positional_index,
                        );
                        if !matches!(arg.kind, ast::ExprKind::NamedArg { .. }) {
                            positional_index = positional_index.saturating_add(1);
                        }
                        lowered_args
                            .push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
                    }

                    let callee = Expr {
                        span: callee.span,
                        ty: self.builtins.any,
                        kind: ExprKind::VarRef(ValueRef::TopLevel {
                            id: self.symbols.intern_top_level(fqn.clone()),
                            fqn: fqn.clone(),
                        }),
                    };

                    Some((
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: lowered_args,
                        },
                        self.builtins.any,
                    ))
                })() {
                    (kind, ty)
                } else if let Some((kind, ty)) =
                    self.try_lower_effect_op_call_expr(pkg_prefix, callee, args)
                {
                    (kind, ty)
                } else if let Some((kind, ty)) = (|| {
                    // T1508a：直连成员函数调用（final/private）：把 `receiver.method(args...)`
                    // 降糖为顶层调用 `Owner.method(receiver, args...)`。
                    //
                    // 注意：
                    // - 这里刻意只改写 value receiver 的 member call；type receiver（companion dispatch）
                    //   会走其它 lowering 路径（当前阶段可能仍保持为 `MemberAccess` 供后续任务落地）。
                    // - `GC.pin/unpin` 等少量内建 member call 依赖后端 `MemberAccess` special-case，
                    //   不能在这里改写为顶层调用。
                    let ast::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
                        return None;
                    };
                    let ast::ResolvedMemberRef::Fun { fqn } = member.resolved.as_ref()? else {
                        return None;
                    };

                    if fqn == "scoop.core.GC.pin"
                        || fqn == "scoop.core.GC.unpin"
                        || fqn == "scoop.core.GC.handleNew"
                        || fqn == "scoop.core.GC.handleGet"
                        || fqn == "scoop.core.GC.handleDrop"
                    {
                        return None;
                    }

                    // T1508a/T1508c：对 class/object/interface member method 做降糖；
                    // struct/value type 的 member call 语义留给其它任务（保持既有 HIR fixtures 稳定）。
                    let Some((owner_fqn, _)) = fqn.as_str().rsplit_once('.') else {
                        return None;
                    };
                    let owner_is_class =
                        matches!(self.type_kinds.get(owner_fqn), Some(ast::TypeKind::Class));
                    let owner_is_interface = matches!(
                        self.type_kinds.get(owner_fqn),
                        Some(ast::TypeKind::Interface)
                    );
                    let owner_is_object = self.index.object_types.contains(owner_fqn);
                    if !owner_is_class && !owner_is_interface && !owner_is_object {
                        return None;
                    }

                    // resolver 的 type receiver（例如 `TypeName.member` / `EffectName.op`）约定为：
                    // receiver ident 不写回 `resolved`。这里用该信号避免把 companion dispatch 误改写为
                    // “带隐式 receiver 参数”的普通 member call。
                    if let ast::ExprKind::Ident(id) = &receiver.kind {
                        if id.resolved.is_none() && self.source.slice(id.span) != "this" {
                            return None;
                        }
                    }

                    let receiver = self.lower_expr(pkg_prefix, receiver);

                    let mut lowered_args = Vec::with_capacity(args.len() + 1);
                    lowered_args.push(CallArg::Positional(receiver));
                    for arg in args {
                        lowered_args.push(self.lower_call_arg_with_expected(
                            pkg_prefix,
                            arg,
                            ExpectedExpr::default(),
                        ));
                    }

                    let callee = Expr {
                        span: callee.span,
                        ty: self.builtins.any,
                        kind: ExprKind::VarRef(ValueRef::TopLevel {
                            id: self.symbols.intern_top_level(fqn.clone()),
                            fqn: fqn.clone(),
                        }),
                    };

                    Some((
                        ExprKind::Call {
                            callee: Box::new(callee),
                            args: lowered_args,
                        },
                        self.builtins.any,
                    ))
                })() {
                    (kind, ty)
                } else if let Some((kind, ty)) =
                    self.try_lower_default_args_call_expr(pkg_prefix, e.span, callee, args)
                {
                    (kind, ty)
                } else {
                    // T1312：class ctor call 仍会被降低为 `UnresolvedIdent`，
                    // 但 codegen 需要知道它的 ctor candidates（来自 resolver 的 `ValueIdent.call`）。
                    if let ast::ExprKind::Ident(id) = &callee.kind {
                        if let Some(call) = id.call.as_ref() {
                            let mut ctor_candidates: Vec<String> = call
                                .candidates
                                .iter()
                                .filter_map(|c| match c {
                                    ast::CallCandidate::Constructor { ty_fqn } => {
                                        Some(ty_fqn.clone())
                                    }
                                    ast::CallCandidate::Fun { .. } => None,
                                })
                                .collect();
                            if !ctor_candidates.is_empty() {
                                ctor_candidates.sort();
                                ctor_candidates.dedup();
                                self.ctor_call_sites
                                    .entry(id.span)
                                    .or_insert(ctor_candidates);
                            }
                        }
                    }

                    let callee_fqn = self.callee_top_level_fqn(callee);
                    let sig = callee_fqn.and_then(|fqn| self.fun_sig_by_fqn(fqn));
                    let callee = Box::new(self.lower_expr(pkg_prefix, callee));
                    let mut positional_index = 0usize;
                    let mut lowered_args: Vec<CallArg> = Vec::with_capacity(args.len());
                    for arg in args {
                        let expected = self.expected_expr_for_fun_call_arg(
                            sig.as_ref(),
                            arg,
                            positional_index,
                        );
                        if !matches!(arg.kind, ast::ExprKind::NamedArg { .. }) {
                            positional_index = positional_index.saturating_add(1);
                        }
                        lowered_args
                            .push(self.lower_call_arg_with_expected(pkg_prefix, arg, expected));
                    }
                    (
                        ExprKind::Call {
                            callee,
                            args: lowered_args,
                        },
                        self.builtins.any,
                    )
                }
            }
            // Appendix B.5.5：spread 仅在调用实参语境下有意义；HIR v0 暂不承载该语义。
            ast::ExprKind::SpreadArg { .. } => (ExprKind::Todo("spread_arg"), self.builtins.any),
            ast::ExprKind::NamedArg { .. } => (ExprKind::Todo("named_arg"), self.builtins.any),
            ast::ExprKind::TupleLit { elements } => {
                let elements: Vec<Expr> = elements
                    .iter()
                    .map(|e| self.lower_expr(pkg_prefix, e))
                    .collect();
                let ty = if elements.is_empty() {
                    self.builtins.unit
                } else {
                    self.types.ty_tuple(elements.iter().map(|e| e.ty).collect())
                };
                (ExprKind::TupleLit { elements }, ty)
            }
            ast::ExprKind::Lambda(lam) => self.lower_lambda_expr(pkg_prefix, e.span, lam),
            ast::ExprKind::StructLit { ty, fields } => {
                self.lower_struct_lit_expr(pkg_prefix, e.span, ty, fields)
            }
            ast::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond = Box::new(self.lower_expr(pkg_prefix, cond));
                let then_branch = Box::new(self.lower_expr(pkg_prefix, then_branch));
                let else_branch = else_branch
                    .as_ref()
                    .map(|e| Box::new(self.lower_expr(pkg_prefix, e)));
                (
                    ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    },
                    self.builtins.any,
                )
            }
            ast::ExprKind::When { subject, arms } => {
                let subject = Box::new(self.lower_expr(pkg_prefix, subject));
                let arms = arms
                    .iter()
                    .map(|a| self.lower_when_arm(pkg_prefix, a))
                    .collect();
                (ExprKind::When { subject, arms }, self.builtins.any)
            }
            ast::ExprKind::Handle {
                body,
                arms,
                finally,
            } => {
                let handle = self.lower_handle_expr(pkg_prefix, body, arms, finally.as_ref());
                (ExprKind::Handle(handle), self.builtins.any)
            }
            ast::ExprKind::Async { body } => {
                // T0619：`async { ... }` 是 `Async` effect 的语法糖。
                //
                // 当前阶段（最小可回归落点）：
                // - 直接 lower 为一个 `handle` 表达式（immediate-resume arm），拦截 `Async.await`；
                // - handler 的语义为“同步 join”：`await task` 会调用 runtime helper 取回 `Int` 结果并立即恢复；
                // - 更完整的 executor/跨线程 resume 语义留给后续任务（T0917）。

                let body = self.lower_block(pkg_prefix, body);

                // synth：构造 `scoop.core.Async` 的 TypePath（不依赖 import）。
                let synth_span = e.span;
                let effect_path = ast::TypePath {
                    span: synth_span,
                    segments: vec![
                        ast::Ident::synthetic(synth_span, "scoop"),
                        ast::Ident::synthetic(synth_span, "core"),
                        ast::Ident::synthetic(synth_span, "Async"),
                    ],
                    args: Vec::new(),
                };
                let effect_ty = self.lower_type_path(&effect_path);

                // 为避免与真实源码 binding span 冲突，这里为 binder/resume 分配两个零长度 span。
                let binder_decl_span = Span::new(e.span.start, e.span.start);
                let resume_decl_span = Span::new(e.span.end, e.span.end);

                let binder_id = self.intern_local_symbol(binder_decl_span, false);
                let resume_id = self.intern_local_symbol(resume_decl_span, false);

                let binder_name = "value".to_string();
                let resume_name = "resume".to_string();

                let binder = HandleBinder {
                    span: binder_decl_span,
                    id: binder_id,
                    name: binder_name.clone(),
                    // NOTE: 当前阶段 Task 运行期表示为 word-sized handle；这里用 `UInt` 承载它。
                    ty: self.builtins.uint,
                };

                let op = HandleOp {
                    span: e.span,
                    effect_ty,
                    op: EffectOpRef {
                        span: e.span,
                        fqn: Self::ASYNC_AWAIT_FQN.to_string(),
                    },
                    binders: vec![binder],
                };

                let resume_ref = ValueRef::Local {
                    id: resume_id,
                    name: resume_name.clone(),
                    decl_span: resume_decl_span,
                };
                let binder_ref = ValueRef::Local {
                    id: binder_id,
                    name: binder_name.clone(),
                    decl_span: binder_decl_span,
                };

                let resume_callee = Expr {
                    span: resume_decl_span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(resume_ref),
                };
                let binder_arg = Expr {
                    span: binder_decl_span,
                    ty: self.builtins.uint,
                    kind: ExprKind::VarRef(binder_ref),
                };

                // `await task` 的最小可执行语义：调用 sysroot task helper 取回结果（目前只支持 `Int`）。
                let join_fqn = Self::TASK_JOIN_INT_FQN.to_string();
                let join_callee = Expr {
                    span: binder_decl_span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(join_fqn.clone()),
                        fqn: join_fqn,
                    }),
                };
                let join_call = Expr {
                    span: binder_decl_span,
                    ty: self.builtins.int,
                    kind: ExprKind::Call {
                        callee: Box::new(join_callee),
                        args: vec![CallArg::Positional(binder_arg)],
                    },
                };

                let arm_body = Expr {
                    span: e.span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Call {
                        callee: Box::new(resume_callee),
                        args: vec![CallArg::Positional(join_call)],
                    },
                };

                let arm = HandleArm {
                    span: e.span,
                    op,
                    kind: HandleArmKind::ImmediateResume { resume: resume_id },
                    body: arm_body,
                };

                let handle = HandleExpr {
                    body,
                    arms: vec![arm],
                    finally: None,
                };
                (ExprKind::Handle(handle), self.builtins.any)
            }
            ast::ExprKind::Spawn { body } => {
                // T0620：`spawn { ... }`（结构化并发最小模型）。
                //
                // 当前阶段（最小可回归落点）：
                // - `spawn` 的 body 先按普通 block 表达式执行并产出一个 `Int` 值；
                // - 该值通过 runtime helper 包装为一个 task handle（后续可替换为更完整的 `Task<T>` 模型）。
                //
                // NOTE: 这里刻意不使用 lambda/closure，以避免依赖 closure codegen（尚未接入）。

                let body = self.lower_block(pkg_prefix, body);
                let body_expr = Expr {
                    span: body.span,
                    ty: body.ty,
                    kind: ExprKind::Block(body),
                };

                let fqn = Self::TASK_SPAWN_INT_FQN.to_string();
                let callee = Expr {
                    span: e.span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(fqn.clone()),
                        fqn,
                    }),
                };

                (
                    ExprKind::Call {
                        callee: Box::new(callee),
                        args: vec![CallArg::Positional(body_expr)],
                    },
                    // task handle：用 word-sized `UInt` 表示。
                    self.builtins.uint,
                )
            }
            ast::ExprKind::Await { await_span, expr } => {
                // T0619：`await expr`（async/await）作为 `Async.await(...)` 的语法糖。
                //
                // NOTE: 这里直接 lower 为 HIR `Perform`，不依赖 resolver 对 `Async.await` 的成员解析写回；
                // 这样能避免“语法糖节点需要合成表达式 ident”的复杂度。
                let inner = self.lower_expr(pkg_prefix, expr);
                let op = EffectOpRef {
                    span: *await_span,
                    fqn: Self::ASYNC_AWAIT_FQN.to_string(),
                };
                (
                    ExprKind::Perform {
                        op,
                        args: vec![CallArg::Positional(inner)],
                    },
                    self.builtins.int,
                )
            }
            ast::ExprKind::Join { join_span, expr } => {
                // T0620：`join expr`（结构化并发最小模型）。
                //
                // 当前阶段：join 仅支持 `Int` 句柄，并返回 `Int`（后续会替换为 `await Task<T>`）。
                let inner = self.lower_expr(pkg_prefix, expr);

                let fqn = Self::TASK_JOIN_INT_FQN.to_string();
                let callee = Expr {
                    span: *join_span,
                    ty: self.builtins.any,
                    kind: ExprKind::VarRef(ValueRef::TopLevel {
                        id: self.symbols.intern_top_level(fqn.clone()),
                        fqn,
                    }),
                };

                (
                    ExprKind::Call {
                        callee: Box::new(callee),
                        args: vec![CallArg::Positional(inner)],
                    },
                    self.builtins.int,
                )
            }
            ast::ExprKind::MemberAccess { receiver, member } => {
                self.lower_member_access_expr(pkg_prefix, receiver, member)
            }
            ast::ExprKind::SpliceField { .. } => {
                (ExprKind::Todo("splice_field"), self.builtins.any)
            }
            ast::ExprKind::SafeMemberAccess { .. } => {
                (ExprKind::Todo("safe_member_access"), self.builtins.any)
            }
            ast::ExprKind::NotNullAssert { .. } => {
                (ExprKind::Todo("not_null_assert"), self.builtins.any)
            }
            ast::ExprKind::Unary { op, op_span, expr } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let ty = match op {
                    ast::UnaryOp::Not => {
                        if expr.ty == self.builtins.bool_ {
                            self.builtins.bool_
                        } else {
                            self.builtins.any
                        }
                    }
                    ast::UnaryOp::Neg | ast::UnaryOp::BitNot => {
                        if self.is_integer_type(expr.ty) {
                            expr.ty
                        } else {
                            self.builtins.any
                        }
                    }
                };
                (
                    ExprKind::Unary {
                        op: *op,
                        op_span: *op_span,
                        expr,
                    },
                    ty,
                )
            }
            ast::ExprKind::Binary {
                lhs,
                op,
                op_span,
                rhs,
            } => {
                let lhs = Box::new(self.lower_expr(pkg_prefix, lhs));
                let rhs = Box::new(self.lower_expr(pkg_prefix, rhs));
                let ty = self.lower_binary_expr_type(&lhs, &rhs, *op);
                (
                    ExprKind::Binary {
                        lhs,
                        op: *op,
                        op_span: *op_span,
                        rhs,
                    },
                    ty,
                )
            }
            ast::ExprKind::Assign { .. } => (ExprKind::Todo("assign"), self.builtins.any),
            ast::ExprKind::TypeCheck {
                expr,
                op,
                op_span,
                ty,
            } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let target_ty = self.lower_type_ref(ty);
                (
                    ExprKind::TypeCheck {
                        expr,
                        op: *op,
                        op_span: *op_span,
                        target_ty,
                    },
                    self.builtins.bool_,
                )
            }
            ast::ExprKind::Cast {
                expr,
                op,
                op_span,
                ty,
            } => {
                let expr = Box::new(self.lower_expr(pkg_prefix, expr));
                let target_ty = self.lower_type_ref(ty);
                let out_ty = match op {
                    ast::CastOp::As => target_ty,
                    ast::CastOp::AsQ => self.types.ty_option(target_ty),
                };
                (
                    ExprKind::Cast {
                        expr,
                        op: *op,
                        op_span: *op_span,
                        target_ty,
                    },
                    out_ty,
                )
            }
            ast::ExprKind::WithUpdate { .. } => (ExprKind::Todo("with_update"), self.builtins.any),
        };

        Expr {
            span: e.span,
            ty,
            kind,
        }
    }

    /// 从一个 `TypeRef` 判定数组字面量的目标容器类型（Array vs MutableArray）。
    fn array_lit_target_from_type_ref(&self, ty: &ast::TypeRef) -> Option<ArrayLitTarget> {
        // 注意：`TypeRef` 可能来自其它源文件（例如通过 `Index::FunSig` 跨文件查询得到的签名）。
        // 当前 HIR lowering 仍以“单个 SourceFile 负责 span → 文本切片”为前提，因此当 span 不在
        // 当前文件范围内时，我们只能保守放弃该 hint，避免越界 panic。
        let span = ty.span();
        let text = self.source.text();
        if span.end > text.len() {
            return None;
        }
        // UTF-8 防线：跨文件 span（或内部 bug）可能导致 start/end 落在非字符边界上，
        // 直接 slice 会 panic。这里同样保守放弃 hint。
        if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) {
            return None;
        }

        let fqn = self
            .index
            .type_ref_to_fqn_in_file(self.source, self.file, ty)?;
        match fqn.as_str() {
            "scoop.core.Array" => Some(ArrayLitTarget::Array),
            "scoop.core.MutableArray" => Some(ArrayLitTarget::MutableArray),
            // T1317f2：`List/MutableList` 在 sysroot 中作为 `Array/MutableArray` 的 typealias。
            // lowering 阶段只需要知道“数组字面量目标容器类型”，因此这里把别名也视为等价目标。
            "scoop.core.List" => Some(ArrayLitTarget::Array),
            "scoop.core.MutableList" => Some(ArrayLitTarget::MutableArray),
            // T1317f4：stdlib `Set/MutableSet/MapView/MutableMap` 当前阶段以数组为底座（typealias）。
            // 这里同样把它们视为 array literal 的等价目标，便于写 `val s: MutableSet = []` 等用例。
            "scoop.collections.Set" => Some(ArrayLitTarget::Array),
            "scoop.collections.MapView" => Some(ArrayLitTarget::Array),
            "scoop.collections.MutableSet" => Some(ArrayLitTarget::MutableArray),
            "scoop.collections.MutableMap" => Some(ArrayLitTarget::MutableArray),
            _ => None,
        }
    }

    /// 根据 FQN 获取函数签名（用于从函数参数类型向下传播 expected-type hint）。
    fn fun_sig_by_fqn(&self, fqn: &str) -> Option<crate::resolve::FunSig> {
        let syms = self.index.by_fqn.get(fqn)?;
        let overload = syms.fun.first()?;
        Some(overload.sig.clone())
    }

    /// 尝试从 callee 表达式中提取“顶层函数 FQN”（用于向实参传播期望类型）。
    fn callee_top_level_fqn<'b>(&self, callee: &'b ast::Expr) -> Option<&'b str> {
        // `callee<T>()`：HIR v0 把 `TypeApply` 视为 callee 的透明包装。
        let callee = match &callee.kind {
            ast::ExprKind::TypeApply { callee, .. } => callee.as_ref(),
            _ => callee,
        };
        let ast::ExprKind::Ident(id) = &callee.kind else {
            return None;
        };
        let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
            return None;
        };
        Some(fqn.as_str())
    }

    /// 为一次函数调用的某个实参计算 expected-type hint（目前仅用于数组字面量）。
    fn expected_expr_for_fun_call_arg(
        &self,
        sig: Option<&crate::resolve::FunSig>,
        arg: &ast::Expr,
        positional_index: usize,
    ) -> ExpectedExpr {
        // expected-type hint 目前只用于数组字面量 `[...]` 的 lowering（Array vs MutableArray）。
        //
        // 注意：`FunSig` 的参数 `TypeRef` 可能来自**其它源文件**（sysroot/stdlib/多文件编译单元），
        // 其 span 无法用当前文件的 `SourceFile` 回切；因此我们必须避免在“非数组字面量实参”
        // 的场景下无谓地解析参数类型，防止跨文件 span 误用导致 panic。
        let arg_is_array_lit = match &arg.kind {
            ast::ExprKind::ArrayLit { .. } => true,
            ast::ExprKind::NamedArg { value, .. } => {
                matches!(value.kind, ast::ExprKind::ArrayLit { .. })
            }
            _ => false,
        };
        if !arg_is_array_lit {
            return ExpectedExpr {
                array_lit_target: None,
            };
        }

        let array_lit_target = match (sig, &arg.kind) {
            (Some(sig), ast::ExprKind::NamedArg { name, .. }) => {
                let name = name.text(self.source);
                sig.params
                    .iter()
                    .find(|p| p.name == name)
                    .and_then(|p| p.ty.as_ref())
                    .and_then(|ty| self.array_lit_target_from_type_ref(ty))
            }
            (Some(sig), _) => sig
                .params
                .get(positional_index)
                .and_then(|p| p.ty.as_ref())
                .and_then(|ty| self.array_lit_target_from_type_ref(ty)),
            _ => None,
        };

        ExpectedExpr { array_lit_target }
    }

    /// 将 `[...]` 降到统一的 builder/intrinsics 调用形态（TODO T1317c）。
    ///
    /// 形态（概念上）：
    /// ```text
    /// [e0, e1, e2]
    /// =>
    /// {
    ///   val __array_builder = __scoop_array_builder_new()
    ///   __scoop_array_builder_push(__array_builder, e0)
    ///   __scoop_array_builder_push(__array_builder, e1)
    ///   __scoop_array_builder_push(__array_builder, e2)
    ///   __scoop_array_builder_build_array(__array_builder) // or build_mutable_array
    /// }
    /// ```
    fn lower_array_lit_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        elements: &[ast::Expr],
        target: ArrayLitTarget,
    ) -> (ExprKind, TypeId) {
        // 说明：HIR v0 的 `LiteralKind::Int` 依赖 span 回切解析数值，因此这里避免生成
        // “需要携带长度/索引常量”的形态，改用 push-based builder 语义承载元素顺序。
        let builder_decl_span = Span::new(span.start, span.start);
        let builder_id = self.intern_local_symbol(builder_decl_span, false);
        let builder_name = "__array_builder".to_string();

        let new_fqn = Self::ARRAY_BUILDER_NEW_FQN.to_string();
        let new_callee = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(new_fqn.clone()),
                fqn: new_fqn,
            }),
        };
        let new_call = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(new_callee),
                args: Vec::new(),
            },
        };

        let builder_decl = ValDecl {
            span,
            id: Some(builder_id),
            name: Some(builder_name.clone()),
            mutable: false,
            ty: self.builtins.any,
            init: Some(new_call),
        };

        let mut stmts: Vec<Stmt> = Vec::with_capacity(elements.len() + 2);
        stmts.push(Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Val(builder_decl),
        });

        for element in elements {
            let element_expr = self.lower_expr(pkg_prefix, element);
            let builder_ref = Expr {
                span: builder_decl_span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::Local {
                    id: builder_id,
                    name: builder_name.clone(),
                    decl_span: builder_decl_span,
                }),
            };

            let push_fqn = Self::ARRAY_BUILDER_PUSH_FQN.to_string();
            let push_callee = Expr {
                span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(ValueRef::TopLevel {
                    id: self.symbols.intern_top_level(push_fqn.clone()),
                    fqn: push_fqn,
                }),
            };
            let push_call = Expr {
                span,
                ty: self.builtins.unit,
                kind: ExprKind::Call {
                    callee: Box::new(push_callee),
                    args: vec![
                        CallArg::Positional(builder_ref),
                        CallArg::Positional(element_expr),
                    ],
                },
            };
            stmts.push(Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Expr(push_call),
            });
        }

        let build_fqn = match target {
            ArrayLitTarget::Array => Self::ARRAY_BUILDER_BUILD_ARRAY_FQN,
            ArrayLitTarget::MutableArray => Self::ARRAY_BUILDER_BUILD_MUTABLE_ARRAY_FQN,
        }
        .to_string();
        let build_callee = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(build_fqn.clone()),
                fqn: build_fqn,
            }),
        };
        let builder_ref = Expr {
            span: builder_decl_span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: builder_id,
                name: builder_name,
                decl_span: builder_decl_span,
            }),
        };
        let build_call = Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(build_callee),
                args: vec![CallArg::Positional(builder_ref)],
            },
        };
        stmts.push(Stmt {
            span,
            ty: build_call.ty,
            kind: StmtKind::Expr(build_call),
        });

        (
            ExprKind::Block(Block {
                span,
                ty: self.builtins.any,
                stmts,
            }),
            self.builtins.any,
        )
    }

    fn lower_call_arg_with_expected(
        &mut self,
        pkg_prefix: &str,
        arg: &ast::Expr,
        expected: ExpectedExpr,
    ) -> CallArg {
        match &arg.kind {
            ast::ExprKind::NamedArg { name, value, .. } => CallArg::Named {
                name: name.text(self.source).to_string(),
                name_span: name.span,
                value: self.lower_expr_with_expected(pkg_prefix, value, expected),
            },
            _ => CallArg::Positional(self.lower_expr_with_expected(pkg_prefix, arg, expected)),
        }
    }

    fn alloc_closure_id(&mut self) -> ClosureId {
        let id = ClosureId(self.next_closure);
        self.next_closure = self.next_closure.saturating_add(1);
        id
    }

    fn lower_lambda_expr(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        lam: &ast::LambdaExpr,
    ) -> (ExprKind, TypeId) {
        let id = self.alloc_closure_id();

        let params: Vec<Param> = lam
            .params
            .iter()
            .map(|p| {
                let name = p.name.text(self.source).to_string();
                let ty =
                    p.ty.as_ref()
                        .map(|t| self.lower_type_ref(t))
                        .unwrap_or(self.builtins.any);
                Param {
                    span: p.name.span,
                    id: self.intern_local_symbol(p.name.span, false),
                    name,
                    ty,
                }
            })
            .collect();

        let body = Box::new(self.lower_expr(pkg_prefix, lam.body.as_ref()));
        let captures = compute_closure_captures(&params, body.as_ref(), &self.local_mutability);
        (
            ExprKind::Closure(ClosureExpr {
                span,
                id,
                captures,
                params,
                body,
            }),
            self.builtins.any,
        )
    }

    /// 把 AST 的 struct literal（`Type { field: expr, ... }`）降低为 HIR 表示。
    ///
    /// 说明：
    /// - 当前 lowering 不做字段存在性/类型匹配检查（这些属于 typecheck，参见 TODO T0423）；
    /// - 这里只保留“目标类型 + 字段初始化表达式列表”，供早期 LLVM codegen（T0811）构造值。
    fn lower_struct_lit_expr(
        &mut self,
        pkg_prefix: &str,
        _span: Span,
        ty: &ast::TypePath,
        fields: &[ast::StructLitField],
    ) -> (ExprKind, TypeId) {
        let ty_id = self.lower_type_path(ty);

        let lowered_fields = fields
            .iter()
            .map(|f| StructLitField {
                span: f.span,
                name: f.name.text(self.source).to_string(),
                name_span: f.name.span,
                colon_span: f.colon_span,
                value: self.lower_expr(pkg_prefix, &f.value),
            })
            .collect::<Vec<_>>();

        (
            ExprKind::StructLit {
                ty: ty_id,
                fields: lowered_fields,
            },
            ty_id,
        )
    }

    fn lower_member_access_expr(
        &mut self,
        pkg_prefix: &str,
        receiver: &ast::Expr,
        member: &ast::MemberIdent,
    ) -> (ExprKind, TypeId) {
        // delegated property lowering（spec §10.4）：
        // `receiver.prop` → `receiver.prop$delegate.getValue(receiver, <PropertyMeta const>)`
        if let Some(ast::ResolvedMemberRef::Value { fqn }) = member.resolved.as_ref() {
            if let Some(info) = self.delegated_properties.get(fqn).cloned() {
                match info {
                    DelegatedPropertyInfo::Lazy(info) => {
                        return (
                            self.lower_lazy_delegated_property_get(
                                pkg_prefix,
                                member.span,
                                receiver,
                                &info,
                            ),
                            self.builtins.any,
                        );
                    }
                    DelegatedPropertyInfo::Generic(info) => {
                        let receiver = self.lower_expr(pkg_prefix, receiver);
                        let this_ref = receiver.clone();

                        let delegate = self.lower_generic_delegated_property_delegate_access_expr(
                            member.span,
                            receiver,
                            &info,
                        );
                        let callee = Expr {
                            span: member.span,
                            ty: self.builtins.any,
                            kind: ExprKind::MemberAccess {
                                receiver: Box::new(delegate),
                                member: MemberAccess {
                                    span: member.span,
                                    name: "getValue".to_string(),
                                    resolved: None,
                                },
                            },
                        };
                        let meta =
                            self.lower_property_meta_ref_expr(member.span, &info.property_meta_fqn);

                        return (
                            ExprKind::Call {
                                callee: Box::new(callee),
                                args: vec![
                                    CallArg::Positional(this_ref),
                                    CallArg::Positional(meta),
                                ],
                            },
                            self.builtins.any,
                        );
                    }
                    DelegatedPropertyInfo::Observable(info) => {
                        return self.lower_observable_vetoable_delegated_property_get(
                            pkg_prefix,
                            member.span,
                            receiver,
                            fqn,
                            info.name,
                            info.ty,
                            info.mutex_field_fqn,
                        );
                    }
                    DelegatedPropertyInfo::Vetoable(info) => {
                        return self.lower_observable_vetoable_delegated_property_get(
                            pkg_prefix,
                            member.span,
                            receiver,
                            fqn,
                            info.name,
                            info.ty,
                            info.mutex_field_fqn,
                        );
                    }
                    DelegatedPropertyInfo::MapBacked => {
                        // map-backed：值在初始化时被拷贝到真实字段，后续只读；
                        // 读取不需要额外同步，按普通字段访问处理。
                    }
                }
            }
        }

        let receiver = Box::new(self.lower_expr(pkg_prefix, receiver));

        let resolved = member
            .resolved
            .as_ref()
            .map(|r| self.lower_resolved_member_ref(r));

        let member = MemberAccess {
            span: member.span,
            name: self.source.slice(member.span).to_string(),
            resolved,
        };

        (
            ExprKind::MemberAccess { receiver, member },
            self.builtins.any,
        )
    }

    fn delegated_property_ty(&mut self, ty: Option<&ast::TypeRef>) -> TypeId {
        ty.map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any)
    }

    fn member_access_to_class_field(
        &mut self,
        span: Span,
        receiver: Expr,
        member_name: String,
        field_fqn: String,
    ) -> Expr {
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: MemberAccess {
                    span,
                    name: member_name,
                    resolved: Some(MemberRef::Value {
                        id: self.symbols.intern_top_level(field_fqn.clone()),
                        fqn: field_fqn,
                    }),
                },
            },
        }
    }

    fn lower_lazy_delegated_property_get(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: &ast::Expr,
        info: &LazyDelegatedPropertyInfo,
    ) -> ExprKind {
        let receiver = self.lower_expr(pkg_prefix, receiver);

        let inited_name = format!("{}$lazy_inited", info.name);
        let value_name = format!("{}$lazy_value", info.name);

        // `LazyThreadSafetyMode.None`：沿用早期阶段的“无锁 + bool 标记”实现；
        // 其它模式：通过 `scoop.sync.Mutex` 保障并发可见性（lock/unlock 作为 acquire/release）。
        if info.mode == StdLazyThreadSafetyMode::None {
            let subject = self.member_access_to_class_field(
                span,
                receiver.clone(),
                inited_name,
                info.inited_field_fqn.clone(),
            );

            let true_body = self.member_access_to_class_field(
                span,
                receiver.clone(),
                value_name.clone(),
                info.value_field_fqn.clone(),
            );

            let init_value = self.lower_expr(pkg_prefix, &info.initializer_body);
            let assign_value = Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Assign {
                    lhs: self.member_access_to_class_field(
                        span,
                        receiver.clone(),
                        value_name.clone(),
                        info.value_field_fqn.clone(),
                    ),
                    eq_span: span,
                    rhs: init_value,
                },
            };

            let assign_inited = Stmt {
                span,
                ty: self.builtins.unit,
                kind: StmtKind::Assign {
                    lhs: self.member_access_to_class_field(
                        span,
                        receiver.clone(),
                        format!("{}$lazy_inited", info.name),
                        info.inited_field_fqn.clone(),
                    ),
                    eq_span: span,
                    rhs: Expr {
                        span,
                        ty: self.builtins.bool_,
                        kind: ExprKind::Literal(LiteralKind::Bool(true)),
                    },
                },
            };

            let tail = Stmt {
                span,
                ty: self.builtins.any,
                kind: StmtKind::Expr(self.member_access_to_class_field(
                    span,
                    receiver,
                    value_name,
                    info.value_field_fqn.clone(),
                )),
            };

            let false_body = Expr {
                span,
                ty: self.delegated_property_ty(info.ty.as_ref()),
                kind: ExprKind::Block(Block {
                    span,
                    ty: self.builtins.any,
                    stmts: vec![assign_value, assign_inited, tail],
                }),
            };

            return ExprKind::When {
                subject: Box::new(subject),
                arms: vec![
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: true },
                        guard: None,
                        arrow_span: span,
                        body: true_body,
                    },
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: false },
                        guard: None,
                        arrow_span: span,
                        body: false_body,
                    },
                ],
            };
        }

        let mutex_fqn = info.mutex_field_fqn.as_ref().cloned().unwrap_or_else(|| {
            // 若出现缺失，回退到一个可预测的合成字段名（保持不 panic）。
            format!("__missing__{}.{}$lazy_mutex", pkg_prefix, info.name)
        });
        let mutex_name = format!("{}$lazy_mutex", info.name);
        let mutex_field =
            self.member_access_to_class_field(span, receiver.clone(), mutex_name, mutex_fqn);

        let lock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_LOCK_FQN,
                vec![mutex_field.clone()],
                self.builtins.unit,
            )),
        };
        let unlock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_UNLOCK_FQN,
                vec![mutex_field],
                self.builtins.unit,
            )),
        };

        let inited_expr = self.member_access_to_class_field(
            span,
            receiver.clone(),
            inited_name,
            info.inited_field_fqn.clone(),
        );

        let value_expr = self.member_access_to_class_field(
            span,
            receiver.clone(),
            value_name.clone(),
            info.value_field_fqn.clone(),
        );

        let value_ty = self.delegated_property_ty(info.ty.as_ref());

        // 说明：早期 LLVM codegen 的 `when` 结果合流（phi）对“arm body 内部再产生分支”的情况
        // 仍较脆弱（会触发 dominance/CFG 校验失败）。为保证 run-pass 可回归，
        // 这里把 lazy 的控制流拆成：
        // 1) `lock(mutex)`
        // 2) `when (inited) { true -> Unit; false -> <init path> }`（Unit）
        // 3) 在锁内读取 value → `out`
        // 4) `unlock(mutex)` 并返回 `out`
        let out_decl_span = Span::new(span.end, span.end);
        let out_id = self.intern_local_symbol(out_decl_span, false);
        let out_name = "$lazy_out".to_string();
        let out_decl = ValDecl {
            span: out_decl_span,
            id: Some(out_id),
            name: Some(out_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(value_expr.clone()),
        };
        let out_ref_expr = Expr {
            span,
            ty: value_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: out_id,
                name: out_name,
                decl_span: out_decl_span,
            }),
        };

        let false_arm_unit = match info.mode {
            StdLazyThreadSafetyMode::Synchronized => {
                // Synchronized：在锁内完成 initializer + 发布（只执行一次）。
                let computed_span = Span::new(span.start, span.start);
                let computed_id = self.intern_local_symbol(computed_span, false);
                let computed_name = "$lazy_computed".to_string();
                let computed_decl = ValDecl {
                    span: computed_span,
                    id: Some(computed_id),
                    name: Some(computed_name.clone()),
                    mutable: false,
                    ty: value_ty,
                    init: Some(self.lower_expr(pkg_prefix, &info.initializer_body)),
                };
                let computed_ref = Expr {
                    span: computed_span,
                    ty: value_ty,
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: computed_id,
                        name: computed_name,
                        decl_span: computed_span,
                    }),
                };

                let assign_value = Stmt {
                    span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Assign {
                        lhs: value_expr.clone(),
                        eq_span: span,
                        rhs: computed_ref,
                    },
                };
                let assign_inited = Stmt {
                    span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Assign {
                        lhs: inited_expr.clone(),
                        eq_span: span,
                        rhs: Expr {
                            span,
                            ty: self.builtins.bool_,
                            kind: ExprKind::Literal(LiteralKind::Bool(true)),
                        },
                    },
                };

                Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Block(Block {
                        span,
                        ty: self.builtins.unit,
                        stmts: vec![
                            Stmt {
                                span: computed_decl.span,
                                ty: self.builtins.unit,
                                kind: StmtKind::Val(computed_decl),
                            },
                            assign_value,
                            assign_inited,
                        ],
                    }),
                }
            }
            StdLazyThreadSafetyMode::Publication => {
                // Publication：释放锁后执行 initializer，再二次加锁“发布”（允许 initializer 并发执行多次）。
                let computed_span = Span::new(span.start, span.start);
                let computed_id = self.intern_local_symbol(computed_span, false);
                let computed_name = "$lazy_computed".to_string();
                let computed_decl = ValDecl {
                    span: computed_span,
                    id: Some(computed_id),
                    name: Some(computed_name.clone()),
                    mutable: false,
                    ty: value_ty,
                    init: Some(self.lower_expr(pkg_prefix, &info.initializer_body)),
                };
                let computed_ref = Expr {
                    span: computed_span,
                    ty: value_ty,
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: computed_id,
                        name: computed_name,
                        decl_span: computed_span,
                    }),
                };

                let assign_value = Stmt {
                    span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Assign {
                        lhs: value_expr.clone(),
                        eq_span: span,
                        rhs: computed_ref,
                    },
                };
                let assign_inited = Stmt {
                    span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Assign {
                        lhs: inited_expr.clone(),
                        eq_span: span,
                        rhs: Expr {
                            span,
                            ty: self.builtins.bool_,
                            kind: ExprKind::Literal(LiteralKind::Bool(true)),
                        },
                    },
                };

                let publish_when = Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::When {
                        subject: Box::new(inited_expr.clone()),
                        arms: vec![
                            WhenArm {
                                span,
                                pat: WhenPat::BoolLit { span, value: true },
                                guard: None,
                                arrow_span: span,
                                body: Expr {
                                    span,
                                    ty: self.builtins.unit,
                                    kind: ExprKind::Literal(LiteralKind::Unit),
                                },
                            },
                            WhenArm {
                                span,
                                pat: WhenPat::BoolLit { span, value: false },
                                guard: None,
                                arrow_span: span,
                                body: Expr {
                                    span,
                                    ty: self.builtins.unit,
                                    kind: ExprKind::Block(Block {
                                        span,
                                        ty: self.builtins.unit,
                                        stmts: vec![assign_value, assign_inited],
                                    }),
                                },
                            },
                        ],
                    },
                };

                Expr {
                    span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Block(Block {
                        span,
                        ty: self.builtins.unit,
                        stmts: vec![
                            unlock_stmt.clone(),
                            Stmt {
                                span: computed_decl.span,
                                ty: self.builtins.unit,
                                kind: StmtKind::Val(computed_decl),
                            },
                            lock_stmt.clone(),
                            Stmt {
                                span,
                                ty: self.builtins.unit,
                                kind: StmtKind::Expr(publish_when),
                            },
                        ],
                    }),
                }
            }
            StdLazyThreadSafetyMode::None => unreachable!("handled above"),
        };

        let outer_when_unit = Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::When {
                subject: Box::new(inited_expr.clone()),
                arms: vec![
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: true },
                        guard: None,
                        arrow_span: span,
                        body: Expr {
                            span,
                            ty: self.builtins.unit,
                            kind: ExprKind::Literal(LiteralKind::Unit),
                        },
                    },
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: false },
                        guard: None,
                        arrow_span: span,
                        body: false_arm_unit,
                    },
                ],
            },
        };

        ExprKind::Block(Block {
            span,
            ty: value_ty,
            stmts: vec![
                lock_stmt,
                Stmt {
                    span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Expr(outer_when_unit),
                },
                Stmt {
                    span: out_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(out_decl),
                },
                unlock_stmt,
                Stmt {
                    span,
                    ty: value_ty,
                    kind: StmtKind::Expr(out_ref_expr),
                },
            ],
        })
    }

    fn lower_observable_vetoable_delegated_property_get(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        receiver: &ast::Expr,
        property_fqn: &str,
        property_name: String,
        ty: Option<ast::TypeRef>,
        mutex_field_fqn: Option<String>,
    ) -> (ExprKind, TypeId) {
        // observable/vetoable（T1326b）：
        // - 读取需要具备并发可见性（避免 data race）；
        // - 早期阶段通过一个 per-property 的 `Mutex` 保护 backing field 读写。
        let receiver = self.lower_expr(pkg_prefix, receiver);
        let value_ty = self.delegated_property_ty(ty.as_ref());

        let mutex_fqn = mutex_field_fqn.unwrap_or_else(|| {
            // 若出现缺失，回退到一个可预测的合成字段名（保持不 panic）。
            format!("__missing__{}.{}$delegate_mutex", pkg_prefix, property_name)
        });
        let mutex_name = format!("{property_name}$delegate_mutex");
        let mutex_field =
            self.member_access_to_class_field(span, receiver.clone(), mutex_name, mutex_fqn);

        let lock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_LOCK_FQN,
                vec![mutex_field.clone()],
                self.builtins.unit,
            )),
        };
        let unlock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_UNLOCK_FQN,
                vec![mutex_field],
                self.builtins.unit,
            )),
        };

        let out_decl_span = Span::new(span.end, span.end);
        let out_id = self.intern_local_symbol(out_decl_span, false);
        let out_name = "$delegate_out".to_string();
        let out_decl = ValDecl {
            span: out_decl_span,
            id: Some(out_id),
            name: Some(out_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(self.member_access_to_class_field(
                span,
                receiver,
                property_name,
                property_fqn.to_string(),
            )),
        };
        let out_ref_expr = Expr {
            span,
            ty: value_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: out_id,
                name: out_name,
                decl_span: out_decl_span,
            }),
        };

        (
            ExprKind::Block(Block {
                span,
                ty: value_ty,
                stmts: vec![
                    lock_stmt,
                    Stmt {
                        span: out_decl.span,
                        ty: self.builtins.unit,
                        kind: StmtKind::Val(out_decl),
                    },
                    unlock_stmt,
                    Stmt {
                        span,
                        ty: value_ty,
                        kind: StmtKind::Expr(out_ref_expr),
                    },
                ],
            }),
            value_ty,
        )
    }

    fn lower_observable_delegated_property_assign(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        member_span: Span,
        receiver: &ast::Expr,
        rhs: &ast::Expr,
        property_fqn: &str,
        info: &ObservableDelegatedPropertyInfo,
    ) -> Option<Expr> {
        if info.on_change.params.len() != 2 {
            return None;
        }

        let value_ty = self.delegated_property_ty(info.ty.as_ref());
        let receiver = self.lower_expr(pkg_prefix, receiver);

        let old_param = &info.on_change.params[0];
        let new_param = &info.on_change.params[1];
        let old_name = old_param.name.text(self.source).to_string();
        let new_name = new_param.name.text(self.source).to_string();

        let old_id = self.intern_local_symbol(old_param.name.span, false);
        let new_id = self.intern_local_symbol(new_param.name.span, false);

        let field_access = |this: &mut Self, recv: Expr| -> Expr {
            this.member_access_to_class_field(
                member_span,
                recv,
                info.name.clone(),
                property_fqn.to_string(),
            )
        };

        let new_decl = ValDecl {
            span: new_param.name.span,
            id: Some(new_id),
            name: Some(new_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(self.lower_expr(pkg_prefix, rhs)),
        };

        // 读写并发可见性：用 per-property mutex 保护 backing field。
        let mutex_fqn = info.mutex_field_fqn.as_ref().cloned().unwrap_or_else(|| {
            // 若出现缺失，回退到一个可预测的合成字段名（保持不 panic）。
            format!("__missing__{}.{}$delegate_mutex", pkg_prefix, info.name)
        });
        let mutex_name = format!("{}$delegate_mutex", info.name);
        let mutex_field =
            self.member_access_to_class_field(member_span, receiver.clone(), mutex_name, mutex_fqn);

        let lock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_LOCK_FQN,
                vec![mutex_field.clone()],
                self.builtins.unit,
            )),
        };
        let unlock_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Expr(self.call_top_level_fun(
                span,
                Self::SYNC_MUTEX_UNLOCK_FQN,
                vec![mutex_field],
                self.builtins.unit,
            )),
        };

        let old_decl = ValDecl {
            span: old_param.name.span,
            id: Some(old_id),
            name: Some(old_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(field_access(self, receiver.clone())),
        };

        let assign = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Assign {
                lhs: field_access(self, receiver.clone()),
                eq_span: member_span,
                rhs: Expr {
                    span,
                    ty: value_ty,
                    kind: ExprKind::VarRef(ValueRef::Local {
                        id: new_id,
                        name: new_name,
                        decl_span: new_param.name.span,
                    }),
                },
            },
        };

        let callback_body = self.lower_expr(pkg_prefix, &info.on_change.body);

        let block = Block {
            span,
            ty: self.builtins.unit,
            stmts: vec![
                Stmt {
                    span: new_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(new_decl),
                },
                lock_stmt,
                Stmt {
                    span: old_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(old_decl),
                },
                assign,
                unlock_stmt,
                Stmt {
                    span: callback_body.span,
                    ty: callback_body.ty,
                    kind: StmtKind::Expr(callback_body),
                },
            ],
        };

        Some(Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Block(block),
        })
    }

    fn lower_vetoable_delegated_property_assign(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        member_span: Span,
        receiver: &ast::Expr,
        rhs: &ast::Expr,
        property_fqn: &str,
        info: &VetoableDelegatedPropertyInfo,
    ) -> Option<Expr> {
        if info.on_change.params.len() != 2 {
            return None;
        }

        let value_ty = self.delegated_property_ty(info.ty.as_ref());
        let receiver = self.lower_expr(pkg_prefix, receiver);

        let old_param = &info.on_change.params[0];
        let new_param = &info.on_change.params[1];
        let old_name = old_param.name.text(self.source).to_string();
        let new_name = new_param.name.text(self.source).to_string();

        let old_id = self.intern_local_symbol(old_param.name.span, false);
        let new_id = self.intern_local_symbol(new_param.name.span, false);

        let field_access = |this: &mut Self, recv: Expr| -> Expr {
            this.member_access_to_class_field(
                member_span,
                recv,
                info.name.clone(),
                property_fqn.to_string(),
            )
        };

        let new_decl = ValDecl {
            span: new_param.name.span,
            id: Some(new_id),
            name: Some(new_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(self.lower_expr(pkg_prefix, rhs)),
        };

        // 读写并发可见性：用 per-property mutex 保护 backing field。
        let mutex_fqn = info.mutex_field_fqn.as_ref().cloned().unwrap_or_else(|| {
            // 若出现缺失，回退到一个可预测的合成字段名（保持不 panic）。
            format!("__missing__{}.{}$delegate_mutex", pkg_prefix, info.name)
        });
        let mutex_name = format!("{}$delegate_mutex", info.name);
        let mutex_field =
            self.member_access_to_class_field(member_span, receiver.clone(), mutex_name, mutex_fqn);

        let lock_stmt = |this: &mut Self, mutex: Expr| -> Stmt {
            Stmt {
                span,
                ty: this.builtins.unit,
                kind: StmtKind::Expr(this.call_top_level_fun(
                    span,
                    Self::SYNC_MUTEX_LOCK_FQN,
                    vec![mutex],
                    this.builtins.unit,
                )),
            }
        };
        let unlock_stmt = |this: &mut Self, mutex: Expr| -> Stmt {
            Stmt {
                span,
                ty: this.builtins.unit,
                kind: StmtKind::Expr(this.call_top_level_fun(
                    span,
                    Self::SYNC_MUTEX_UNLOCK_FQN,
                    vec![mutex],
                    this.builtins.unit,
                )),
            }
        };

        // 先在锁内读取 old，再解锁执行回调（避免把用户回调放在锁内导致死锁/泄漏）。
        let old_decl = ValDecl {
            span: old_param.name.span,
            id: Some(old_id),
            name: Some(old_name.clone()),
            mutable: false,
            ty: value_ty,
            init: Some(field_access(self, receiver.clone())),
        };

        let ok_expr = self.lower_expr(pkg_prefix, &info.on_change.body);

        let new_ref = Expr {
            span,
            ty: value_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: new_id,
                name: new_name,
                decl_span: new_param.name.span,
            }),
        };

        let assign_stmt = Stmt {
            span,
            ty: self.builtins.unit,
            kind: StmtKind::Assign {
                lhs: field_access(self, receiver.clone()),
                eq_span: member_span,
                rhs: new_ref,
            },
        };

        let true_block = Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Block(Block {
                span,
                ty: self.builtins.unit,
                stmts: vec![
                    lock_stmt(self, mutex_field.clone()),
                    assign_stmt,
                    unlock_stmt(self, mutex_field.clone()),
                    Stmt {
                        span,
                        ty: self.builtins.unit,
                        kind: StmtKind::Expr(Expr {
                            span,
                            ty: self.builtins.unit,
                            kind: ExprKind::Literal(LiteralKind::Unit),
                        }),
                    },
                ],
            }),
        };

        let false_unit = Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Literal(LiteralKind::Unit),
        };

        let when_expr = Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::When {
                subject: Box::new(ok_expr),
                arms: vec![
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: true },
                        guard: None,
                        arrow_span: span,
                        body: true_block,
                    },
                    WhenArm {
                        span,
                        pat: WhenPat::BoolLit { span, value: false },
                        guard: None,
                        arrow_span: span,
                        body: false_unit,
                    },
                ],
            },
        };

        let block = Block {
            span,
            ty: self.builtins.unit,
            stmts: vec![
                Stmt {
                    span: new_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(new_decl),
                },
                lock_stmt(self, mutex_field.clone()),
                Stmt {
                    span: old_decl.span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Val(old_decl),
                },
                unlock_stmt(self, mutex_field),
                Stmt {
                    span,
                    ty: self.builtins.unit,
                    kind: StmtKind::Expr(when_expr),
                },
            ],
        };

        Some(Expr {
            span,
            ty: self.builtins.unit,
            kind: ExprKind::Block(block),
        })
    }

    fn lower_generic_delegated_property_delegate_access_expr(
        &mut self,
        span: Span,
        receiver: Expr,
        info: &GenericDelegatedPropertyInfo,
    ) -> Expr {
        Expr {
            span,
            ty: self.builtins.any,
            kind: ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: MemberAccess {
                    span,
                    name: format!("{}$delegate", info.name),
                    resolved: Some(MemberRef::Value {
                        id: self
                            .symbols
                            .intern_top_level(info.delegate_field_fqn.clone()),
                        fqn: info.delegate_field_fqn.clone(),
                    }),
                },
            },
        }
    }

    fn lower_property_meta_ref_expr(&mut self, span: Span, fqn: &str) -> Expr {
        let ty = self.intern_nominal(Self::PROPERTY_META_FQN.to_string(), Vec::new(), None);
        Expr {
            span,
            ty,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.to_string()),
                fqn: fqn.to_string(),
            }),
        }
    }

    fn lower_resolved_member_ref(&mut self, resolved: &ast::ResolvedMemberRef) -> MemberRef {
        match resolved {
            ast::ResolvedMemberRef::Value { fqn } => MemberRef::Value {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::Fun { fqn } => MemberRef::Fun {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::ExtensionValue { fqn } => MemberRef::ExtensionValue {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
            ast::ResolvedMemberRef::ExtensionFun { fqn } => MemberRef::ExtensionFun {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        }
    }

    fn lower_when_arm(&mut self, pkg_prefix: &str, arm: &ast::WhenArm) -> WhenArm {
        WhenArm {
            span: arm.span,
            pat: self.lower_when_pat(&arm.pat),
            guard: arm.guard.as_ref().map(|e| self.lower_expr(pkg_prefix, e)),
            arrow_span: arm.arrow_span,
            body: self.lower_expr(pkg_prefix, &arm.body),
        }
    }

    fn lower_when_pat(&mut self, pat: &ast::WhenPat) -> WhenPat {
        match pat {
            ast::WhenPat::Else { span } => WhenPat::Else { span: *span },
            ast::WhenPat::Or { span, pats } => WhenPat::Or {
                span: *span,
                pats: pats.iter().map(|p| self.lower_when_pat(p)).collect(),
            },
            ast::WhenPat::Wildcard { span } => WhenPat::Wildcard { span: *span },
            ast::WhenPat::Rest { span } => WhenPat::Rest { span: *span },
            ast::WhenPat::Is { is_span, ty } => WhenPat::Is {
                span: Span::new(is_span.start, ty.span().end),
                ty: self.lower_type_ref(ty),
            },
            ast::WhenPat::Bind { ident } => WhenPat::Bind {
                span: ident.span,
                id: self.intern_local_symbol(ident.span, false),
                name: ident.text(self.source).to_string(),
            },
            ast::WhenPat::Tuple { span, elements } => WhenPat::Tuple {
                span: *span,
                elements: elements.iter().map(|e| self.lower_when_pat(e)).collect(),
            },
            ast::WhenPat::Variant { span, name, args } => WhenPat::Variant {
                span: *span,
                name_span: name.span,
                name: name.text(self.source).to_string(),
                args: args.iter().map(|a| self.lower_when_pat(a)).collect(),
            },
            ast::WhenPat::IntLit { span } => WhenPat::IntLit { span: *span },
            ast::WhenPat::StringLit { span } => WhenPat::StringLit { span: *span },
            ast::WhenPat::BoolLit { span } => {
                let value = self.source.slice(*span) == "true";
                WhenPat::BoolLit { span: *span, value }
            }
        }
    }

    /// 尝试把一个调用表达式识别为“effect op call”（例如 `Raise.raise(e)`），并 lower 为 HIR `Perform`。
    ///
    /// 若不匹配该形态，则返回 None，让外层按普通 `Call` 处理。
    fn try_lower_effect_op_call_expr(
        &mut self,
        pkg_prefix: &str,
        callee: &ast::Expr,
        args: &[ast::Expr],
    ) -> Option<(ExprKind, TypeId)> {
        let ast::ExprKind::MemberAccess { member, .. } = &callee.kind else {
            return None;
        };
        let Some(ast::ResolvedMemberRef::Fun { fqn }) = member.resolved.as_ref() else {
            return None;
        };
        if !self.is_effect_op_fqn(fqn) {
            return None;
        }

        let op = EffectOpRef {
            span: member.span,
            fqn: fqn.clone(),
        };
        let args = args
            .iter()
            .map(|arg| self.lower_call_arg(pkg_prefix, arg))
            .collect();
        Some((ExprKind::Perform { op, args }, self.builtins.any))
    }

    fn is_effect_op_fqn(&self, fqn: &str) -> bool {
        let Some(syms) = self.index.by_fqn.get(fqn) else {
            return false;
        };
        syms.fun
            .iter()
            .any(|o| o.sig.kind == ast::FunDeclKind::EffectOp)
    }

    fn lower_handle_expr(
        &mut self,
        pkg_prefix: &str,
        body: &ast::Block,
        arms: &[ast::HandleArm],
        finally: Option<&ast::Block>,
    ) -> HandleExpr {
        let body = self.lower_block(pkg_prefix, body);
        let arms = arms
            .iter()
            .map(|arm| self.lower_handle_arm(pkg_prefix, arm))
            .collect();
        let finally = finally.map(|b| self.lower_block(pkg_prefix, b));
        HandleExpr {
            body,
            arms,
            finally,
        }
    }

    fn lower_handle_arm(&mut self, pkg_prefix: &str, arm: &ast::HandleArm) -> HandleArm {
        let kind = match arm.kind {
            ast::HandleArmKind::NonResuming => HandleArmKind::NonResuming,
            ast::HandleArmKind::ImmediateResume { resume_span } => HandleArmKind::ImmediateResume {
                resume: self.intern_local_symbol(resume_span, false),
            },
            ast::HandleArmKind::EscapeContinuation { k_span } => {
                HandleArmKind::EscapeContinuation {
                    continuation: self.intern_local_symbol(k_span, false),
                }
            }
        };
        HandleArm {
            span: arm.span,
            op: self.lower_handle_op(pkg_prefix, &arm.op),
            kind,
            body: self.lower_expr(pkg_prefix, &arm.body),
        }
    }

    fn lower_handle_op(&mut self, _pkg_prefix: &str, op: &ast::HandleOp) -> HandleOp {
        let effect_ty = self.lower_type_path(&op.effect);
        let effect_fqn = self.index.type_ref_to_fqn_in_file(
            self.source,
            self.file,
            &ast::TypeRef::Path(op.effect.clone()),
        );

        let op_name = op.op.text(self.source).to_string();
        let op_fqn = match effect_fqn {
            Some(effect_fqn) => format!("{effect_fqn}.{op_name}"),
            None => format!("{}.{}", self.source.slice(op.effect.span), op_name),
        };

        let binders = op
            .binders
            .iter()
            .map(|b| self.lower_handle_binder(b))
            .collect();

        HandleOp {
            span: op.span,
            effect_ty,
            op: EffectOpRef {
                span: op.op.span,
                fqn: op_fqn,
            },
            binders,
        }
    }

    fn lower_handle_binder(&mut self, b: &ast::HandleBinder) -> HandleBinder {
        let ty =
            b.ty.as_ref()
                .map(|t| self.lower_type_ref(t))
                .unwrap_or(self.builtins.any);
        HandleBinder {
            span: b.span,
            id: self.intern_local_symbol(b.name.span, false),
            name: b.name.text(self.source).to_string(),
            ty,
        }
    }

    fn lower_call_arg(&mut self, pkg_prefix: &str, arg: &ast::Expr) -> CallArg {
        match &arg.kind {
            ast::ExprKind::NamedArg { name, value, .. } => CallArg::Named {
                name: name.text(self.source).to_string(),
                name_span: name.span,
                value: self.lower_expr(pkg_prefix, value),
            },
            _ => CallArg::Positional(self.lower_expr(pkg_prefix, arg)),
        }
    }

    /// 若该调用点满足“尾部默认参数可补齐”的规则，则把调用表达式 lowering 为一个 block：
    ///
    /// ```text
    /// f(a0, a1)   // 省略尾部默认参数
    /// =>
    /// {
    ///   val p0 = a0
    ///   val p1 = a1
    ///   val p2 = <default>
    ///   val p3 = <default>
    ///   f(p0, p1, p2, p3)
    /// }
    /// ```
    ///
    /// 说明：
    /// - 这样可以保证 default value 里对“更早参数”的引用能工作（通过局部 `val` 绑定）；
    /// - 也能保证“实参表达式”不会因简单替换而被重复求值（求值顺序与一次性语义可控）。
    fn try_lower_default_args_call_expr(
        &mut self,
        pkg_prefix: &str,
        call_span: Span,
        callee: &ast::Expr,
        args: &[ast::Expr],
    ) -> Option<(ExprKind, TypeId)> {
        // 仅处理：顶层函数直接调用 `foo(...)`。
        let callee = match &callee.kind {
            // `callee<T>()`：HIR v0 视为透明包装（同 `lower_expr(TypeApply)`）。
            ast::ExprKind::TypeApply { callee, .. } => callee.as_ref(),
            _ => callee,
        };
        let ast::ExprKind::Ident(id) = &callee.kind else {
            return None;
        };
        let ast::ResolvedValueRef::TopLevel { fqn } = id.resolved.as_ref()? else {
            return None;
        };
        let info = self.default_arg_funs.get(fqn).cloned()?;

        let provided = args.len();
        let total = info.params.len();
        if provided >= total {
            return None;
        }
        if provided < info.required {
            return None;
        }

        // Kotlin-like：命名实参之后不能再出现位置实参（与 typecheck 对齐；不支持 trailing-lambda 例外）。
        let mut seen_named = false;
        let mut positional_count = 0usize;
        for arg in args {
            match &arg.kind {
                ast::ExprKind::NamedArg { .. } => {
                    seen_named = true;
                }
                _ => {
                    if seen_named {
                        return None;
                    }
                    positional_count += 1;
                }
            }
        }
        if positional_count > total {
            return None;
        }

        // 将调用点的实参映射到形参槽位：
        // - 位置实参：按序绑定到 [0..positional_count)
        // - 命名实参：按 name 查找形参槽位
        let mut param_to_arg: Vec<Option<usize>> = vec![None; total];
        for arg_idx in 0..positional_count {
            *param_to_arg.get_mut(arg_idx)? = Some(arg_idx);
        }
        for (arg_idx, arg) in args.iter().enumerate().skip(positional_count) {
            let ast::ExprKind::NamedArg { name, .. } = &arg.kind else {
                return None;
            };
            let name_text = name.text(self.source).to_string();
            let slot_idx = info.params.iter().position(|p| p.name == name_text)?;
            let slot = param_to_arg.get_mut(slot_idx)?;
            if slot.is_some() {
                // 同一形参不能被重复赋值（位置+命名/命名重复）。
                return None;
            }
            *slot = Some(arg_idx);
        }

        // 未填充的槽位必须有默认值。
        for (idx, param) in info.params.iter().enumerate() {
            if param_to_arg.get(idx)?.is_some() {
                continue;
            }
            if param.default_value.is_none() {
                return None;
            }
        }

        // 反向映射：arg_idx -> param_idx（用于按调用点顺序求值实参）。
        let mut arg_to_param: Vec<Option<usize>> = vec![None; args.len()];
        for (param_idx, arg_idx) in param_to_arg.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let slot = arg_to_param.get_mut(arg_idx)?;
            if slot.is_some() {
                return None;
            }
            *slot = Some(param_idx);
        }
        if arg_to_param.iter().any(|x| x.is_none()) {
            return None;
        }

        // 1) 先把“已提供的实参表达式”按参数名绑定为局部 val，避免重复求值。
        //    - 求值顺序：严格按调用点源码顺序（positional + named 的排列）。
        // 2) 再按形参顺序求值缺失的默认参数，并同样绑定为局部 val（供后续默认值引用）。
        let mut stmts: Vec<Stmt> = Vec::with_capacity(total + 1);

        for (arg_idx, arg) in args.iter().enumerate() {
            let param_idx = arg_to_param.get(arg_idx).copied().flatten()?;
            let param = info.params.get(param_idx)?;
            let arg_value = match &arg.kind {
                ast::ExprKind::NamedArg { value, .. } => value.as_ref(),
                _ => arg,
            };
            let expected = ExpectedExpr {
                array_lit_target: param
                    .ty_ref
                    .as_ref()
                    .and_then(|t| self.array_lit_target_from_type_ref(t)),
            };
            let init = self.lower_expr_with_expected(pkg_prefix, arg_value, expected);
            let param_ty = param
                .ty_ref
                .as_ref()
                .map(|t| self.lower_type_ref(t))
                .unwrap_or(self.builtins.any);
            let id = self.intern_local_symbol(param.decl_span, false);
            let decl = ValDecl {
                span: call_span,
                id: Some(id),
                name: Some(param.name.clone()),
                mutable: false,
                ty: param_ty,
                init: Some(init),
            };
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(decl),
            });
        }

        for (param_idx, param) in info.params.iter().enumerate() {
            if param_to_arg.get(param_idx)?.is_some() {
                continue;
            }
            let default_value = param.default_value.as_ref()?;
            let expected = ExpectedExpr {
                array_lit_target: param
                    .ty_ref
                    .as_ref()
                    .and_then(|t| self.array_lit_target_from_type_ref(t)),
            };
            let init = self.lower_expr_with_expected(pkg_prefix, default_value, expected);
            let param_ty = param
                .ty_ref
                .as_ref()
                .map(|t| self.lower_type_ref(t))
                .unwrap_or(self.builtins.any);
            let id = self.intern_local_symbol(param.decl_span, false);
            let decl = ValDecl {
                span: call_span,
                id: Some(id),
                name: Some(param.name.clone()),
                mutable: false,
                ty: param_ty,
                init: Some(init),
            };
            stmts.push(Stmt {
                span: call_span,
                ty: self.builtins.unit,
                kind: StmtKind::Val(decl),
            });
        }

        // 最后一条语句：调用“完整参数形态”的原函数。
        let callee_id = self.symbols.intern_top_level(fqn.clone());
        let callee_expr = Expr {
            span: callee.span,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: callee_id,
                fqn: fqn.clone(),
            }),
        };

        let mut full_args: Vec<CallArg> = Vec::with_capacity(total);
        for param in &info.params {
            let id = self.intern_local_symbol(param.decl_span, false);
            let vref = ValueRef::Local {
                id,
                name: param.name.clone(),
                decl_span: param.decl_span,
            };
            full_args.push(CallArg::Positional(Expr {
                span: param.decl_span,
                ty: self.builtins.any,
                kind: ExprKind::VarRef(vref),
            }));
        }

        let call_expr = Expr {
            span: call_span,
            ty: self.builtins.any,
            kind: ExprKind::Call {
                callee: Box::new(callee_expr),
                args: full_args,
            },
        };
        stmts.push(Stmt {
            span: call_span,
            ty: call_expr.ty,
            kind: StmtKind::Expr(call_expr),
        });

        Some((
            ExprKind::Block(Block {
                span: call_span,
                ty: self.builtins.any,
                stmts,
            }),
            self.builtins.any,
        ))
    }

    fn lower_ident_expr(&mut self, id: &ast::ValueIdent) -> (ExprKind, TypeId) {
        let text = self.source.slice(id.span);
        if text == "true" {
            return (
                ExprKind::Literal(LiteralKind::Bool(true)),
                self.builtins.bool_,
            );
        }
        if text == "false" {
            return (
                ExprKind::Literal(LiteralKind::Bool(false)),
                self.builtins.bool_,
            );
        }

        let Some(resolved) = id.resolved.as_ref() else {
            // 典型场景：enum variant ctor 的 callee（`Some(1)`）/0-参数 variant 值（`None`）；
            // resolver 会保留为“未 resolve”，让 typecheck 在期望类型语境下决议。
            return (
                ExprKind::UnresolvedIdent {
                    name: text.to_string(),
                },
                self.builtins.any,
            );
        };

        let resolved = match resolved {
            ast::ResolvedValueRef::Local { name, decl_span } => ValueRef::Local {
                id: self.intern_local_symbol(*decl_span, false),
                name: name.clone(),
                decl_span: *decl_span,
            },
            ast::ResolvedValueRef::TopLevel { fqn } => ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        };

        (ExprKind::VarRef(resolved), self.builtins.any)
    }

    fn is_integer_type(&self, ty: TypeId) -> bool {
        if ty == self.builtins.int || ty == self.builtins.uint {
            return true;
        }

        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::IntN(_) | ValueTypeKind::UIntN(_))
        )
    }

    /// 对齐 typecheck 阶段的最小规则：整数二元运算要求“相同的整数类型”，但允许一侧是整数字面量。
    ///
    /// 说明：HIR lowering 目前仅用于 dump/fixtures 与早期 codegen，因此这里的规则只覆盖：
    /// - 算术/位运算：`T op T -> T`（一侧为 int literal 时可吸收为另一侧的整数类型）
    /// - 移位：`T << Int -> T` / `T >> Int -> T`
    /// - 比较：`T < T -> Bool` 等
    /// - 相等：`T == T -> Bool` / `Bool == Bool -> Bool`
    fn lower_binary_expr_type(&self, lhs: &Expr, rhs: &Expr, op: ast::BinaryOp) -> TypeId {
        let unify_int_same_type = |lhs: &Expr, rhs: &Expr| -> Option<TypeId> {
            if lhs.ty == rhs.ty && self.is_integer_type(lhs.ty) {
                return Some(lhs.ty);
            }

            let lhs_is_int_lit = matches!(lhs.kind, ExprKind::Literal(LiteralKind::Int));
            let rhs_is_int_lit = matches!(rhs.kind, ExprKind::Literal(LiteralKind::Int));

            if lhs_is_int_lit && self.is_integer_type(rhs.ty) {
                return Some(rhs.ty);
            }
            if rhs_is_int_lit && self.is_integer_type(lhs.ty) {
                return Some(lhs.ty);
            }

            None
        };

        match op {
            // arithmetic + bitwise: T op T -> T
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => unify_int_same_type(lhs, rhs).unwrap_or(self.builtins.any),

            // shifts: T << Int -> T
            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if self.is_integer_type(lhs.ty) && rhs.ty == self.builtins.int {
                    lhs.ty
                } else {
                    self.builtins.any
                }
            }

            // comparisons: T < T -> Bool
            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                if unify_int_same_type(lhs, rhs).is_some() {
                    self.builtins.bool_
                } else {
                    self.builtins.any
                }
            }

            // equality: (T == T) -> Bool; (Bool == Bool) -> Bool
            ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
                if lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_ {
                    return self.builtins.bool_;
                }
                if unify_int_same_type(lhs, rhs).is_some() {
                    return self.builtins.bool_;
                }
                self.builtins.any
            }

            // boolean logic: Bool op Bool -> Bool
            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                if lhs.ty == self.builtins.bool_ && rhs.ty == self.builtins.bool_ {
                    self.builtins.bool_
                } else {
                    self.builtins.any
                }
            }

            // range/progression：语义由 stdlib/lowering 补齐；HIR dump 阶段先降级为 Any。
            ast::BinaryOp::RangeInclusive => self.builtins.any,

            // elvis not lowered in current HIR dump mode
            ast::BinaryOp::Elvis => self.builtins.any,
        }
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
    let (file, member_funs, mut ctor_call_sites, top_level_vars) = {
        let mut ctx = HirLowering::new(
            source,
            &ast,
            &index,
            &type_kinds,
            &delegated_properties,
            &mut types,
            builtins,
        );
        let file = ctx.lower_file();
        let member_funs = ctx.collect_member_funs(&pkg_prefix);
        let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
        let top_level_vars = std::mem::take(&mut ctx.top_level_vars);
        (file, member_funs, ctor_call_sites, top_level_vars)
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
    let struct_layouts = collect_struct_layouts(&pairs, &index);
    // T0813：早期 LLVM codegen 需要知道 enum 的 variant tag 与 payload 字段类型，用于生成判别与解构。
    let enum_layouts = collect_enum_layouts(&pairs, &index);
    Ok(LoweredHir {
        file,
        member_funs,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_libs,
        top_level_vars,
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
    let (file_hir, member_funs, mut ctor_call_sites, top_level_vars) = {
        let mut ctx = HirLowering::new(
            source,
            file,
            index,
            &type_kinds,
            &delegated_properties,
            &mut types,
            builtins,
        );
        let file_hir = ctx.lower_file();
        let member_funs = ctx.collect_member_funs(&pkg_prefix);
        let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
        let top_level_vars = std::mem::take(&mut ctx.top_level_vars);
        (file_hir, member_funs, ctor_call_sites, top_level_vars)
    };

    let (object_inits, object_ctor_call_sites) =
        collect_object_inits(source, file, index, &type_kinds, &mut types, builtins);
    ctor_call_sites.extend(object_ctor_call_sites);
    let (class_inits, class_ctor_call_sites) =
        collect_class_inits(source, file, index, &type_kinds, &mut types, builtins);
    ctor_call_sites.extend(class_ctor_call_sites);
    let extern_funs = collect_extern_funs(source, file);
    let extern_libs = collect_extern_libs(compilation_unit);
    let struct_layouts = collect_struct_layouts(compilation_unit, index);
    let enum_layouts = collect_enum_layouts(compilation_unit, index);

    Ok(LoweredHir {
        file: file_hir,
        member_funs,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_libs,
        top_level_vars,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
    })
}

/// 在“给定编译单元（多个源文件）”的上下文中，把多个文件一起 lowering 为一个 `LoweredHir`。
///
/// 用途（T1315a）：
/// - `scoop build/run` 注入 `stdlib/*.scoop` 后，需要让这些文件里的顶层函数在后端可见；
/// - 当前 LLVM 后端仍以“单一 SourceFile”切片读取字面量文本（Span -> SourceFile），
///   因此本入口会拒绝 **非入口文件** 中出现 `Int/String/插值文本` 等“需要源文本切片”的字面量，
///   避免 silent miscompile。
pub fn lower_for_compilation_unit_multi_files(
    entry_source: &SourceFile,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
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

    for (source, file) in files_to_lower {
        let (file_hir, file_member_funs, file_ctor_call_sites, file_top_level_vars) = {
            let mut ctx = HirLowering::new(
                source,
                file,
                index,
                &type_kinds,
                &delegated_properties,
                &mut types,
                builtins,
            );
            let file_hir = ctx.lower_file();
            // multi-file lowering 当前只允许入口文件包含“需要源文本切片”的字面量；
            // member_funs 同样可能包含字面量，因此只为入口文件收集即可。
            let file_member_funs = if source.path() == entry_source.path() {
                let pkg_prefix = package_prefix(source, file.package.as_ref());
                ctx.collect_member_funs(&pkg_prefix)
            } else {
                Vec::new()
            };
            let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
            let file_top_level_vars = std::mem::take(&mut ctx.top_level_vars);
            (
                file_hir,
                file_member_funs,
                ctor_call_sites,
                file_top_level_vars,
            )
        };

        if source.path() != entry_source.path() && file_contains_source_backed_literals(&file_hir) {
            return Err(HirLowerError::MultiFileNonEntrySourceBackedLiteral {
                path: source.path().display().to_string(),
            });
        }

        // `CtorCallSiteIndex` 当前以 `Span` 作为 key（offset-only，不含文件标识）。
        // 为避免跨文件 span 冲突导致错误 codegen，当前阶段只保留入口文件的 ctor call-sites。
        if source.path() == entry_source.path() {
            ctor_call_sites.extend(file_ctor_call_sites);
        }

        top_level_vars.extend(file_top_level_vars);
        member_funs.extend(file_member_funs);
        items.extend(file_hir.items);
    }

    // side tables：保持与 `lower_for_compilation_unit` 一致的收集逻辑（先降 HIR，再补充 layout/init/extern）。
    let extern_funs = files_to_lower
        .iter()
        .flat_map(|(source, file)| collect_extern_funs(source, file))
        .collect();
    let extern_libs = collect_extern_libs(compilation_unit);
    let struct_layouts = collect_struct_layouts(compilation_unit, index);
    let enum_layouts = collect_enum_layouts(compilation_unit, index);

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

    Ok(LoweredHir {
        file: File { items },
        member_funs,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_libs,
        top_level_vars,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
    })
}

/// 将给定的 `ast::FunDecl` 在“已绑定 type params”的语境下降低为 HIR（用于单态化，T0712）。
///
/// 说明：
/// - 该函数假设调用方已在 AST 上运行 resolver（headers + bodies），以便 `ValueIdent.resolved` 等信息可用；
/// - `type_bindings` 用于把函数声明处的 `T` 等 type params 映射为具体 `TypeId`（调用点推断结果）。
pub(crate) fn lower_fun_with_type_bindings(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    type_kinds: &HashMap<String, ast::TypeKind>,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    fun: &ast::FunDecl,
    type_bindings: impl IntoIterator<Item = (String, TypeId)>,
) -> FunDecl {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let compilation_unit = [(source, file)];
    let delegated_properties = collect_delegated_properties(&compilation_unit);
    let mut ctx = HirLowering::new(
        source,
        file,
        index,
        type_kinds,
        &delegated_properties,
        types,
        builtins,
    );
    ctx.push_type_param_bindings(type_bindings);
    let out = ctx.lower_fun_decl_with_bound_type_params(&pkg_prefix, fun);
    ctx.pop_type_params();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

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
        let lowered =
            lower_for_compilation_unit_multi_files(&src_main, &index, &unit, &files_to_lower)
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
}
