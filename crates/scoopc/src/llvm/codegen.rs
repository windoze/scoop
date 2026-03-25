//! LLVM IR 生成（早期阶段，T0808～T0810）。
//!
//! 当前落点：支持把**入口函数 `fun main`** 与其调用到的顶层函数降低到单个 LLVM module：
//! - 入口保持为 `i32 @main()`（C ABI），其返回值作为进程退出码；
//! - 额外生成（或声明）被调用的顶层函数（先按简单 C ABI）。
//!
//! 表达式/语句子集（当前只覆盖早期最小回归需要）：
//! - 整数/布尔字面量；
//! - 一元运算：`!`、`-`、`~`；
//! - 二元运算：算术/比较/位运算/移位（含 shift count mask）；
//! - 局部绑定：`val`/`var`（映射为 `alloca` + `load/store`）；
//! - 赋值语句：`x = expr`（仅支持 local `var`）；
//! - `return`（以及“block 最后表达式”作为隐式返回）。
//! - `when`（T0813：仅支持 enum tag 判别 + variant binder；不支持 guard/or-pattern）。
//!
//! 非目标（后续任务逐步补齐）：
//! - if/loop 等更复杂控制流（依赖 MIR/CFG codegen 任务）。

use std::collections::HashMap;

use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicTypeEnum;
use inkwell::types::IntType;
use inkwell::types::StructType;
use inkwell::values::AggregateValueEnum;
use inkwell::values::BasicValue;
use inkwell::values::BasicValueEnum;
use inkwell::values::FunctionValue;
use inkwell::values::GlobalValue;
use inkwell::values::IntValue;
use inkwell::values::PointerValue;

use crate::ast;
use crate::hir;
use crate::llvm::target::HostTargetInfo;
use crate::source::SourceFile;
use crate::ty::layout::{NicheDomain, NicheStorage, TargetLayout, TypeLayout};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::LlvmEmitError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntTy {
    bits: u32,
    signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CgTy {
    Unit,
    Bool,
    Int(IntTy),
    Tuple(TypeId),
    Struct(TypeId),
    Enum(TypeId),
    /// runtime 字符串对象（early stage）
    ///
    /// 说明：
    /// - 当前阶段把 `scoop.core.String` 映射为 `*const ScoopString`（C ABI）；
    /// - 该指针的指向对象目前允许来自：字符串字面量生成的栈上 `ScoopString`；
    /// - 更完整的 String 对象头/GC 语义将由后续任务补齐（T09/T12）。
    String,
    /// 通用引用类型（Any / class / interface / function / union ...）。
    ///
    /// 当前阶段的 codegen 约定：
    /// - 一律用 `i8*`（opaque pointer）表示；
    /// - 值类型向引用类型的隐式转换需要装箱（T0817：先只支持 `Int -> Any`）。
    ///
    /// 未来将替换为带对象头（type descriptor/flags/size）的具体布局（PLAN §8.2/§9.1）。
    Ref,
}

// boxing / lint 的启发式阈值（与 typecheck::layout.rs 保持一致）。
const ENUM_BOX_DISPARITY_RATIO: u64 = 4;
const ENUM_BOX_INLINE_THRESHOLD_WORDS: u64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CgEnumRepr {
    TaggedUnion,
    /// niche 优化：无显式 tag，通过 payload 的非法值编码 `None`。
    Niche { storage: NicheStorage, none_value: u64 },
}

#[derive(Debug, Clone)]
struct CgEnumVariant {
    name: String,
    tag: u32,
    boxed: bool,
    fields: Vec<CgTy>,
}

#[derive(Debug, Clone)]
struct CgEnumLayout {
    repr: CgEnumRepr,
    variants: Vec<CgEnumVariant>,
}

#[derive(Debug, Clone, Copy)]
struct CgValue<'ctx> {
    ty: CgTy,
    value: Option<BasicValueEnum<'ctx>>,
}

impl<'ctx> CgValue<'ctx> {
    fn unit() -> Self {
        Self {
            ty: CgTy::Unit,
            value: None,
        }
    }

    fn int(value: IntValue<'ctx>, ty: IntTy) -> Self {
        Self {
            ty: CgTy::Int(ty),
            value: Some(value.into()),
        }
    }

    fn bool(value: IntValue<'ctx>) -> Self {
        Self {
            ty: CgTy::Bool,
            value: Some(value.into()),
        }
    }

    fn as_int(self) -> Option<(IntValue<'ctx>, IntTy)> {
        match self.ty {
            CgTy::Int(ty) => match self.value? {
                BasicValueEnum::IntValue(v) => Some((v, ty)),
                _ => None,
            },
            _ => None,
        }
    }

    fn as_bool(self) -> Option<IntValue<'ctx>> {
        match self.ty {
            CgTy::Bool => match self.value? {
                BasicValueEnum::IntValue(v) => Some(v),
                _ => None,
            },
            _ => None,
        }
    }
}

/// 一个局部变量（`val`/`var`）在 LLVM 里的存储形态。
///
/// 当前阶段（T0809）统一用栈分配（`alloca`）承载 locals，并用 `load/store` 实现读写。
#[derive(Debug, Clone, Copy)]
struct CgLocal<'ctx> {
    ty: CgTy,
    ptr: PointerValue<'ctx>,
    mutable: bool,
}

#[derive(Debug, Default)]
struct Env<'ctx> {
    scopes: Vec<HashMap<hir::SymbolId, CgLocal<'ctx>>>,
}

impl<'ctx> Env<'ctx> {
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn insert(&mut self, id: hir::SymbolId, local: CgLocal<'ctx>) {
        if let Some(frame) = self.scopes.last_mut() {
            frame.insert(id, local);
        }
    }

    fn get(&self, id: hir::SymbolId) -> Option<CgLocal<'ctx>> {
        for frame in self.scopes.iter().rev() {
            if let Some(local) = frame.get(&id).copied() {
                return Some(local);
            }
        }
        None
    }
}

pub(crate) struct MainCodegen<'a, 'ctx> {
    context: &'ctx Context,
    module: &'a Module<'ctx>,
    builder: &'a Builder<'ctx>,
    host: &'a HostTargetInfo,
    source: &'a SourceFile,
    types: &'a TypeStore,
    struct_layouts: &'a hir::StructLayoutIndex,
    enum_layouts: &'a hir::EnumLayoutIndex,
    object_inits: &'a hir::ObjectInitIndex,
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    env: Env<'ctx>,
    /// `TypeId -> TypeLayout`（仅用于 codegen 侧的 niche 决策；不追求覆盖所有类型语法）。
    type_layout_cache: HashMap<TypeId, TypeLayout>,
    /// `Option<T>` niche 表示的 `None` 编码（用于嵌套 niche）。
    option_niche_cache: HashMap<TypeId, Option<(NicheStorage, u64)>>,
    /// `enum/Option` 的 codegen 表示选择与 boxing 决策缓存。
    enum_cg_layout_cache: HashMap<TypeId, CgEnumLayout>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn new(
        context: &'ctx Context,
        module: &'a Module<'ctx>,
        builder: &'a Builder<'ctx>,
        host: &'a HostTargetInfo,
        source: &'a SourceFile,
        types: &'a TypeStore,
        struct_layouts: &'a hir::StructLayoutIndex,
        enum_layouts: &'a hir::EnumLayoutIndex,
        object_inits: &'a hir::ObjectInitIndex,
        fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    ) -> Self {
        Self {
            context,
            module,
            builder,
            host,
            source,
            types,
            struct_layouts,
            enum_layouts,
            object_inits,
            fun_index,
            env: Env::default(),
            type_layout_cache: HashMap::new(),
            option_niche_cache: HashMap::new(),
            enum_cg_layout_cache: HashMap::new(),
        }
    }

    pub(crate) fn declare_top_level_fun(
        &self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        if let Some(existing) = self.module.get_function(&fun.fqn) {
            return Ok(existing);
        }

        let llvm_params = fun
            .params
            .iter()
            .map(|p| self.llvm_param_ty(p.span, p.ty))
            .collect::<Result<Vec<_>, _>>()?;

        let return_cg = self
            .cg_ty_of(fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            })?;

        let fn_ty = match return_cg {
            CgTy::Unit => self.context.void_type().fn_type(&llvm_params, false),
            CgTy::Bool => self.context.bool_type().fn_type(&llvm_params, false),
            CgTy::Int(int_ty) => self.int_type(int_ty).fn_type(&llvm_params, false),
            CgTy::String | CgTy::Ref | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "function return type",
                    at: fun.span.into(),
                });
            }
        };

        Ok(self.module.add_function(&fun.fqn, fn_ty, None))
    }

    pub(crate) fn codegen_top_level_fun(
        mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(body) = fun.body.as_ref() else {
            // extern / declaration-only：由调用点按需声明即可，这里不生成 body。
            return Ok(());
        };

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);

        self.env.push_scope();
        self.codegen_fun_params(fun, llvm_fun)?;

        let declared_return_cg =
            self.cg_ty_of(fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "function return type",
                    at: fun.span.into(),
                })?;
        let ret_v = self.codegen_block_as_return_value(body, declared_return_cg)?;
        self.emit_return(fun.span, declared_return_cg, ret_v)?;

        self.env.pop_scope();
        Ok(())
    }

    pub(crate) fn codegen_main_exit_code(
        mut self,
        fun: &hir::FunDecl,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        self.env.push_scope();

        let exit = match fun.body.as_ref() {
            Some(body) => self.codegen_block_as_exit_code(body, fun.return_ty)?,
            None => self.context.i32_type().const_int(0, false),
        };

        self.env.pop_scope();
        Ok(exit)
    }

    fn codegen_block_as_exit_code(
        &mut self,
        block: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // block 是表达式：若末尾是表达式语句，则它的值作为 block value。
        let mut tail_value: Option<CgValue<'ctx>> = None;

        self.env.push_scope();

        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    tail_value = None;
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    tail_value = None;
                }
                hir::StmtKind::Expr(expr) => {
                    let v = self.codegen_expr(expr)?;
                    if is_last {
                        tail_value = Some(v);
                    } else {
                        tail_value = None;
                    }
                }
                hir::StmtKind::Return { value } => {
                    let exit = match value {
                        Some(expr) => {
                            let v = self.codegen_expr(expr)?;
                            self.coerce_exit_code(expr.span, v)?
                        }
                        None => self.context.i32_type().const_int(0, false),
                    };

                    self.env.pop_scope();
                    return Ok(exit);
                }
                // 控制流留待后续任务（需要 function-level CFG/MIR codegen）。
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        // 隐式返回：当函数声明了整数/Bool 返回类型时，允许用 block tail value 作为返回值。
        let exit = if let Some(v) = tail_value {
            match self.cg_ty_of(declared_return_ty) {
                Some(CgTy::Int(_) | CgTy::Bool) => self.coerce_exit_code(block.span, v)?,
                _ => self.context.i32_type().const_int(0, false),
            }
        } else {
            self.context.i32_type().const_int(0, false)
        };

        self.env.pop_scope();
        Ok(exit)
    }

    fn codegen_val_decl(&mut self, decl: &hir::ValDecl) -> Result<(), LlvmEmitError> {
        let Some(id) = decl.id else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "anonymous val binding",
                at: decl.span.into(),
            });
        };

        let target_ty = self
            .cg_ty_of(decl.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "val type",
                at: decl.span.into(),
            })?;

        let init = match decl.init.as_ref() {
            Some(expr) => self.codegen_expr_in_expected_context(expr, Some(target_ty))?,
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "val without initializer",
                    at: decl.span.into(),
                });
            }
        };

        // T0809：局部变量统一降为 alloca + store/load；`val/var` 仅在“是否允许赋值”上有差异。
        let name = decl.name.as_deref().unwrap_or("local");
        let ptr = self.create_entry_alloca(decl.span, name, target_ty)?;
        self.store_local_value(decl.span, ptr, target_ty, init)?;
        self.env.insert(
            id,
            CgLocal {
                ty: target_ty,
                ptr,
                mutable: decl.mutable,
            },
        );
        Ok(())
    }

    fn codegen_expr_in_expected_context(
        &mut self,
        expr: &hir::Expr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::UnresolvedIdent { name } => {
                self.codegen_unresolved_ident(expr.span, name, expected)
            }
            hir::ExprKind::Call { callee, args } => {
                self.codegen_call(expr.span, callee, args, expected)
            }
            _ => self.codegen_expr(expr),
        }
    }

    fn codegen_assign_stmt(
        &mut self,
        eq_span: crate::span::Span,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<(), LlvmEmitError> {
        let hir::ExprKind::VarRef(vref) = &lhs.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "assignment lhs",
                at: lhs.span.into(),
            });
        };

        let hir::ValueRef::Local { id, .. } = vref else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "assignment to non-local",
                at: lhs.span.into(),
            });
        };

        let local = self
            .env
            .get(*id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown local value",
                at: lhs.span.into(),
            })?;

        if !local.mutable {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "assignment to immutable local",
                at: eq_span.into(),
            });
        }

        let rhs_v = self.codegen_expr(rhs)?;
        self.store_local_value(eq_span, local.ptr, local.ty, rhs_v)?;
        Ok(())
    }

    fn codegen_expr(&mut self, expr: &hir::Expr) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Missing | hir::ExprKind::Todo(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "expression",
                    at: expr.span.into(),
                })
            }
            hir::ExprKind::UnresolvedIdent { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident (missing expected type context)",
                at: expr.span.into(),
            }),
            hir::ExprKind::Literal(lit) => self.codegen_literal(expr.span, expr.ty, lit),
            hir::ExprKind::VarRef(v) => self.codegen_var_ref(expr.span, v),
            hir::ExprKind::StructLit { ty, fields } => {
                self.codegen_struct_lit(expr.span, *ty, fields)
            }
            hir::ExprKind::TupleLit { elements } => {
                self.codegen_tuple_lit(expr.span, expr.ty, elements)
            }
            hir::ExprKind::InterpolatedString { raw, parts } => {
                self.codegen_interpolated_string(expr.span, *raw, parts)
            }
            hir::ExprKind::Unary {
                op, expr: inner, ..
            } => self.codegen_unary(expr.span, *op, inner),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.codegen_binary(expr.span, *op, lhs, rhs)
            }
            hir::ExprKind::Block(block) => self.codegen_block_value(block),
            hir::ExprKind::Call { callee, args } => {
                self.codegen_call(expr.span, callee, args, None)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.codegen_member_access(expr.span, receiver, member)
            }
            hir::ExprKind::When { subject, arms } => {
                self.codegen_when_expr(expr.span, subject, arms)
            }

            // 后续任务接入 MIR/CFG codegen
            hir::ExprKind::Closure(_)
            | hir::ExprKind::If { .. }
            | hir::ExprKind::Perform { .. }
            | hir::ExprKind::Handle(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "expression kind",
                at: expr.span.into(),
            }),
        }
    }

    fn codegen_call(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 1) 普通顶层函数调用：`foo(args...)`
        if let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind {
            // T0822：最小 I/O（sysroot `print/println(String)`）直接映射到 runtime 符号。
            if fqn == "scoop.core.print" || fqn == "scoop.core.println" {
                return self.codegen_sysroot_print_like(span, callee.span, fqn, args);
            }
            return self.codegen_top_level_fun_call(span, callee.span, fqn, args);
        }

        // 1.5) 内建 String API（early stage）：`receiver.trimIndent()`
        //
        // 说明：
        // - `trimIndent` 在语言层面是 `String` 的 `const fun`（spec §8.4）；
        // - 编译期折叠由 TODO T1216 负责；此处只负责运行期 fallback：调用 runtime 实现。
        if let hir::ExprKind::MemberAccess { receiver, member } = &callee.kind {
            if member.name == "trimIndent" {
                return self.codegen_string_trim_indent(span, receiver, args);
            }
        }

        // 2) enum variant ctor：`Some(x)` 这类调用在 resolver 阶段不会 resolve，
        //    需要依赖“期望类型语境”才能决定属于哪个 enum。
        if let hir::ExprKind::UnresolvedIdent { name } = &callee.kind {
            let Some(CgTy::Enum(enum_ty)) = expected else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum variant ctor call without expected enum type",
                    at: callee.span.into(),
                });
            };
            return self.codegen_enum_variant_ctor_call(span, enum_ty, name, args);
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "call callee",
            at: callee.span.into(),
        })
    }

    fn codegen_string_trim_indent(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent arity mismatch",
                at: span.into(),
            });
        }

        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::String))?;
        let coerced = self.coerce_value(receiver.span, recv, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent receiver type",
                at: receiver.span.into(),
            });
        };

        let rt_fun = self.declare_runtime_trim_indent();
        let call = self
            .builder
            .build_call(rt_fun, &[recv_ptr.into()], "rt_trim_indent")?;
        let ret =
            call.try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "trimIndent return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::PointerValue(out_ptr) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    fn codegen_sysroot_print_like(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println named arg",
                at: span.into(),
            });
        };

        let rt_name = match fqn {
            "scoop.core.print" => "scoop_print",
            "scoop.core.println" => "scoop_println",
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "unknown sysroot print/println callee",
                    at: callee_span.into(),
                });
            }
        };

        // 说明：
        // - sysroot 中允许 `print/println` 以 overload set 的形式声明（例如 `String` 与 `Int`）；
        // - HIR 当前阶段不保留“已选定 overload”的信息，因此这里以实参 codegen 后的 `CgTy`
        //   来决定使用哪条 lowering 路径。
        let v = self.codegen_expr_in_expected_context(expr, Some(CgTy::String))?;
        let str_ptr = match v.ty {
            CgTy::String => {
                let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
                let Some(raw) = coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println arg value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(str_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println arg type",
                        at: expr.span.into(),
                    });
                };
                str_ptr
            }
            CgTy::Int(from_ty) => {
                let (raw_int, _) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println int arg value",
                    at: expr.span.into(),
                })?;
                self.codegen_int_to_scoop_string(expr.span, raw_int, from_ty)?
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println arg type",
                    at: expr.span.into(),
                });
            }
        };

        let rt_fun = self.declare_runtime_print_like(rt_name);
        let _ = self
            .builder
            .build_call(rt_fun, &[str_ptr.into()], "rt_print")?;
        Ok(CgValue::unit())
    }

    fn codegen_int_to_scoop_string(
        &mut self,
        at: crate::span::Span,
        raw_int: IntValue<'ctx>,
        from_ty: IntTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if from_ty.bits > 64 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "integer width for print/println",
                at: at.into(),
            });
        }

        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();

        // 先把整数提升/截断到 i64/u64，再调用 runtime 格式化到临时 buffer。
        let to_ty = IntTy {
            bits: 64,
            signed: from_ty.signed,
        };
        let int64 = self.cast_int(raw_int, from_ty, to_ty)?;

        // i64 最长：`-9223372036854775808`（20 字符），预留更宽裕的 cap。
        let cap = i64_ty.const_int(64, false);
        let buf = self
            .builder
            .build_array_alloca(i8_ty, cap, "print_int_buf")?;

        let fmt_name = if from_ty.signed {
            "scoop_format_i64"
        } else {
            "scoop_format_u64"
        };
        let fmt_fun = self.declare_runtime_format_int(fmt_name);
        let call_site = self.builder.build_call(
            fmt_fun,
            &[int64.into(), buf.into(), cap.into()],
            "print_fmt_int",
        )?;
        let len = call_site
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "print int length",
                at: at.into(),
            })?
            .into_int_value();

        // 构造一个 `ScoopString { len, data }`（放在 entry block，便于复用/避免 alloca 位置敏感问题）。
        let scoop_str_ty = self.llvm_scoop_string_type();
        let str_ptr = self.create_entry_alloca_raw(at, "scoop_str_int", scoop_str_ty.into())?;

        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 0, "print_int_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "print_int_data_gep")?;

        let _ = self.builder.build_store(len_ptr, len)?;
        let _ = self.builder.build_store(data_ptr, buf)?;

        Ok(str_ptr)
    }

    fn codegen_top_level_fun_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let sig_fun =
            self.fun_index
                .get(fqn)
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call callee type",
                    at: callee_span.into(),
                })?;

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call arity mismatch",
                at: span.into(),
            });
        }

        let mut llvm_args = Vec::with_capacity(args.len());
        for (idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named call arg",
                    at: span.into(),
                });
            };

            let target_cg = self.cg_ty_of(sig_fun.params[idx].ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg type",
                    at: expr.span.into(),
                },
            )?;
            let v = self.codegen_expr(expr)?;
            let coerced = self.coerce_value(expr.span, v, target_cg)?;
            llvm_args.push(self.as_llvm_arg_value(expr.span, target_cg, coerced)?);
        }

        let llvm_fun = match self.module.get_function(fqn) {
            Some(f) => f,
            None => self.declare_top_level_fun(sig_fun)?,
        };
        let call_site = self.builder.build_call(llvm_fun, &llvm_args, "call")?;

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call return type",
                    at: span.into(),
                })?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::bool(value))
            }
            CgTy::Int(int_ty) => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::int(value, int_ty))
            }
            CgTy::String | CgTy::Ref | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "call return type",
                    at: span.into(),
                })
            }
        }
    }

    fn codegen_unresolved_ident(
        &mut self,
        span: crate::span::Span,
        name: &str,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 0-参数 enum variant 值：`None`
        let Some(CgTy::Enum(enum_ty)) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident without expected enum type",
                at: span.into(),
            });
        };

        let cg_layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = cg_layout
            .variants
            .iter()
            .find(|v| v.name == name)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown enum variant",
                at: span.into(),
            })?;
        let tag = variant.tag;
        let field_count = variant.fields.len();

        if field_count != 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "non-zero-arity enum variant used as value",
                at: span.into(),
            });
        }

        self.build_enum_value(span, enum_ty, tag, None)
    }

    fn codegen_enum_variant_ctor_call(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        variant_name: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = layout
            .variants
            .iter()
            .find(|v| v.name == variant_name)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown enum variant",
                at: span.into(),
            })?
            .clone();

        if variant.fields.len() != args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant ctor arity mismatch",
                at: span.into(),
            });
        }

        // 先把所有实参在“字段期望类型”下 codegen 并做最小 coercion，避免后续重复走 codegen。
        let mut field_values: Vec<(CgTy, CgValue<'ctx>)> = Vec::with_capacity(args.len());
        for (idx, (field_cg, arg)) in variant.fields.iter().copied().zip(args.iter()).enumerate() {
            let hir::CallArg::Positional(arg_expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named enum ctor arg",
                    at: span.into(),
                });
            };

            let v = self.codegen_expr_in_expected_context(arg_expr, Some(field_cg))?;
            let coerced = self.coerce_value(arg_expr.span, v, field_cg)?;
            field_values.push((field_cg, coerced));

            // 提前在 debug 名称里体现 index，便于排查（不影响语义）。
            let _ = idx;
        }

        // 1) boxed variant：把 payload fields 聚合成一个 payload struct，存到栈上并把指针写入 enum payload。
        if variant.boxed {
            let payload_struct_ty =
                self.llvm_enum_boxed_payload_struct_type(span, enum_ty, &variant)?;
            let mut payload: AggregateValueEnum<'ctx> = payload_struct_ty.get_undef().into();

            for (idx, (field_cg, field_v)) in field_values.iter().enumerate() {
                // Unit 没有运行期值；当前阶段不允许把 Unit 作为 enum payload 字段。
                if matches!(field_cg, CgTy::Unit) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum boxed payload field (unit)",
                        at: span.into(),
                    });
                }
                let raw = field_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum boxed payload field value",
                    at: span.into(),
                })?;
                payload = self.builder.build_insert_value(
                    payload,
                    raw,
                    idx as u32,
                    &format!("enum_payload_field_{idx}"),
                )?;
            }

            let tmp_name = format!(
                "boxed_enum_payload_{}_{}",
                enum_ty.as_u32(),
                sanitize_llvm_ident(&variant.name)
            );
            let payload_ptr = self.create_entry_alloca_raw(span, &tmp_name, payload_struct_ty.into())?;
            let _ = self.builder.build_store(payload_ptr, payload.as_basic_value_enum())?;

            let word_ty = self.int_type(self.enum_payload_ty());
            let payload_word =
                self.builder
                    .build_ptr_to_int(payload_ptr, word_ty, "boxed_enum_payload_ptr")?;
            return self.build_enum_value(span, enum_ty, variant.tag, Some(payload_word));
        }

        // 2) inline（非 boxed）variant：当前阶段仍采用 “word payload” 承载的小 payload。
        if variant.fields.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant payload (multi-field, not boxed)",
                at: span.into(),
            });
        }

        let payload = if let Some((field_cg, field_v)) = field_values.first().copied() {
            Some(self.coerce_enum_payload_word(span, field_v, field_cg)?)
        } else {
            None
        };

        self.build_enum_value(span, enum_ty, variant.tag, payload)
    }

    fn target_layout(&self) -> TargetLayout {
        // 说明：与 typecheck::layout.rs 一致，当前阶段用 host pointer size/align 作为 layout。
        TargetLayout::host()
    }

    fn type_layout(&mut self, ty: TypeId) -> TypeLayout {
        if let Some(layout) = self.type_layout_cache.get(&ty).copied() {
            return layout;
        }

        let target = self.target_layout();

        let layout = match self.types.kind(ty) {
            TypeKind::Ref(_) => TypeLayout::new(target.pointer_size, target.pointer_align).with_niche(NicheDomain {
                storage: NicheStorage::Pointer,
                next: 0,
                end: target.pointer_align.max(1),
            }),
            TypeKind::Param(_) => TypeLayout::new(target.pointer_size, target.pointer_align),
            TypeKind::Value(v) => match v {
                ValueTypeKind::Unit | ValueTypeKind::Nothing => TypeLayout::new(0, 1),
                ValueTypeKind::Bool => TypeLayout::new(1, 1).with_niche(NicheDomain {
                    storage: NicheStorage::U8,
                    next: 2,
                    end: 256,
                }),
                ValueTypeKind::Int | ValueTypeKind::UInt => {
                    TypeLayout::new(target.pointer_size, target.pointer_align)
                }
                ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => {
                    let size = (u64::from(*bits) + 7) / 8;
                    let align = size.clamp(1, target.pointer_align.max(1));
                    TypeLayout::new(size, align)
                }
                ValueTypeKind::Tuple(elements) => self.aggregate_fields_layout_for_type_ids(elements),
                ValueTypeKind::Option(inner) => self.option_type_layout(ty, *inner),
                ValueTypeKind::Nominal(_) => {
                    // 当前 codegen 只在 niche/boxing 决策里需要 layout 信息；nominal struct/enum 的精确布局
                    // 将在对应任务里补齐。这里按“opaque word-sized”兜底，避免过度耦合。
                    TypeLayout::new(target.pointer_size, target.pointer_align)
                }
            },
        };

        self.type_layout_cache.insert(ty, layout);
        layout
    }

    fn option_type_layout(&mut self, option_ty: TypeId, inner: TypeId) -> TypeLayout {
        // 注意：该函数只负责“niche 传播”与 `None` 编码缓存（供后续 codegen 使用）。
        if self.option_niche_cache.contains_key(&option_ty) {
            return *self
                .type_layout_cache
                .get(&option_ty)
                .unwrap_or(&TypeLayout::new(self.target_layout().pointer_size, self.target_layout().pointer_align));
        }

        let target = self.target_layout();
        let inner_layout = self.type_layout(inner);

        // niche path：inner 提供可用 niche domain。
        if let Some(mut domain) = inner_layout.niche {
            if let Some(none_value) = domain.take_one() {
                self.option_niche_cache
                    .insert(option_ty, Some((domain.storage, none_value)));

                let layout = TypeLayout::new(inner_layout.size, inner_layout.align).with_niche(domain);
                self.type_layout_cache.insert(option_ty, layout);
                return layout;
            }
        }

        // tagged union fallback：不携带 niche。
        self.option_niche_cache.insert(option_ty, None);

        // 说明：当前 codegen 的 enum 表示仍采用 `{ tag: i32, payload: word }`，因此这里返回一个
        // “足够大”的布局即可；精确大小与 tag type 选择后续任务再统一。
        let tag_size = 4u64;
        let tag_align = 4u64;
        let payload_size = target.pointer_size;
        let payload_align = target.pointer_align;
        let payload_offset = align_to(tag_size, payload_align);
        let align = payload_align.max(tag_align);
        let size = align_to(payload_offset + payload_size, align);
        let layout = TypeLayout::new(size, align);
        self.type_layout_cache.insert(option_ty, layout);
        layout
    }

    fn aggregate_fields_layout_for_type_ids(&mut self, fields: &[TypeId]) -> TypeLayout {
        let mut size = 0u64;
        let mut align = 1u64;
        for &field in fields {
            let l = self.type_layout(field);
            size = align_to(size, l.align);
            size = size.saturating_add(l.size);
            align = align.max(l.align);
        }
        size = align_to(size, align);
        TypeLayout::new(size, align)
    }

    fn cg_enum_layout(&mut self, at: crate::span::Span, enum_ty: TypeId) -> Result<&CgEnumLayout, LlvmEmitError> {
        if !self.enum_cg_layout_cache.contains_key(&enum_ty) {
            let computed = self.compute_cg_enum_layout(at, enum_ty)?;
            self.enum_cg_layout_cache.insert(enum_ty, computed);
        }
        Ok(self
            .enum_cg_layout_cache
            .get(&enum_ty)
            .expect("just inserted"))
    }

    fn compute_cg_enum_layout(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
    ) -> Result<CgEnumLayout, LlvmEmitError> {
        match self.types.kind(enum_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                // 确保 option niche 缓存已被填充（用于 nested niche）。
                let _ = self.type_layout(enum_ty);
                let repr = match self.option_niche_cache.get(&enum_ty).copied().flatten() {
                    Some((storage, none_value)) => CgEnumRepr::Niche { storage, none_value },
                    None => CgEnumRepr::TaggedUnion,
                };

                let inner_cg = self.cg_ty_of(*inner).ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Option<T> inner type",
                    at: at.into(),
                })?;

                Ok(CgEnumLayout {
                    repr,
                    variants: vec![
                        CgEnumVariant {
                            name: "Some".to_string(),
                            tag: 0,
                            boxed: false,
                            fields: vec![inner_cg],
                        },
                        CgEnumVariant {
                            name: "None".to_string(),
                            tag: 1,
                            boxed: false,
                            fields: Vec::new(),
                        },
                    ],
                })
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                let hir_layout = self
                    .enum_layouts
                    .get(&nominal.fqn)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum layout",
                        at: at.into(),
                    })?;

                let mut variants: Vec<CgEnumVariant> = Vec::with_capacity(hir_layout.variants.len());
                let mut payload_layouts: Vec<TypeLayout> = Vec::with_capacity(hir_layout.variants.len());
                for v in &hir_layout.variants {
                    let mut fields = Vec::with_capacity(v.fields.len());
                    for f in &v.fields {
                        let cg = self.cg_ty_of_type_fqn(f.span, f.ty_fqn.as_deref())?;
                        fields.push(cg);
                    }
                    payload_layouts.push(self.aggregate_fields_layout_for_cg_tys(&fields)?);
                    variants.push(CgEnumVariant {
                        name: v.name.clone(),
                        tag: v.tag,
                        boxed: false,
                        fields,
                    });
                }

                // boxing：复用 typecheck 的启发式规则（ratio + inline threshold）。
                let target = self.target_layout();
                let (max_size, second_size) = largest_two_sizes(&payload_layouts);
                let inline_threshold = target.pointer_size.saturating_mul(ENUM_BOX_INLINE_THRESHOLD_WORDS);
                let disparity = if second_size == 0 {
                    max_size >= inline_threshold
                } else {
                    max_size >= inline_threshold
                        && max_size >= second_size.saturating_mul(ENUM_BOX_DISPARITY_RATIO)
                };

                if disparity {
                    for (v, payload) in variants.iter_mut().zip(payload_layouts.iter()) {
                        if payload.size == max_size && max_size > target.pointer_size {
                            v.boxed = true;
                        }
                    }
                }

                Ok(CgEnumLayout {
                    repr: CgEnumRepr::TaggedUnion,
                    variants,
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum type id",
                at: at.into(),
            }),
        }
    }

    fn aggregate_fields_layout_for_cg_tys(
        &self,
        fields: &[CgTy],
    ) -> Result<TypeLayout, LlvmEmitError> {
        let mut size = 0u64;
        let mut align = 1u64;
        for &field in fields {
            let field_layout = self.cg_ty_layout(field)?;
            size = align_to(size, field_layout.align);
            size = size.saturating_add(field_layout.size);
            align = align.max(field_layout.align);
        }
        size = align_to(size, align);
        Ok(TypeLayout::new(size, align))
    }

    fn cg_ty_layout(&self, ty: CgTy) -> Result<TypeLayout, LlvmEmitError> {
        let target = self.target_layout();
        Ok(match ty {
            CgTy::Unit => TypeLayout::new(0, 1),
            // 当前阶段 Bool 在 LLVM 中用 i1 表示，但 layout/lint/niche 计算按“存储为 u8”建模。
            CgTy::Bool => TypeLayout::new(1, 1),
            CgTy::Int(int_ty) => {
                let size = (u64::from(int_ty.bits) + 7) / 8;
                let align = size.clamp(1, target.pointer_align.max(1));
                TypeLayout::new(size, align)
            }
            CgTy::String => TypeLayout::new(target.pointer_size, target.pointer_align),
            CgTy::Ref => TypeLayout::new(target.pointer_size, target.pointer_align),
            // 兜底：composite 在当前阶段按 word-sized opaque 处理，避免错误放大。
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                TypeLayout::new(target.pointer_size, target.pointer_align)
            }
        })
    }

    fn enum_payload_ty(&self) -> IntTy {
        IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        }
    }

    fn coerce_enum_payload_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        value_ty: CgTy,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let payload_ty = self.enum_payload_ty();
        let payload_int_ty = self.int_type(payload_ty);

        match value_ty {
            CgTy::Unit => Ok(payload_int_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload bool",
                    at: at.into(),
                })?;
                Ok(self
                    .builder
                    .build_int_z_extend(b, payload_int_ty, "enum_payload_bool")?)
            }
            CgTy::Int(from) => {
                let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload int",
                    at: at.into(),
                })?;
                if from.bits > payload_ty.bits {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload larger than word",
                        at: at.into(),
                    });
                }
                Ok(self.cast_int(v, from, payload_ty)?)
            }
            CgTy::String => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload string",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload string",
                        at: at.into(),
                    });
                };
                Ok(self
                    .builder
                    .build_ptr_to_int(ptr, payload_int_ty, "enum_payload_str_ptr")?)
            }
            CgTy::Ref => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload ref",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload ref",
                        at: at.into(),
                    });
                };
                Ok(self
                    .builder
                    .build_ptr_to_int(ptr, payload_int_ty, "enum_payload_ref_ptr")?)
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload (non-scalar)",
                    at: at.into(),
                })
            }
        }
    }

    fn build_enum_value(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        tag: u32,
        payload: Option<IntValue<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 注意：`cg_enum_layout(...)` 返回的是对缓存表的引用；为了避免与后续 `&mut self` 调用产生借用冲突，
        // 这里先把需要的字段拷贝出来再继续。
        let (repr, some_field) = {
            let layout = self.cg_enum_layout(at, enum_ty)?;
            let repr = layout.repr;
            let some_field = layout
                .variants
                .iter()
                .find(|v| v.name == "Some")
                .and_then(|v| v.fields.first())
                .copied();
            (repr, some_field)
        };

        match repr {
            CgEnumRepr::TaggedUnion => {
                let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
                let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();

                let tag_ty = self.context.i32_type();
                let payload_ty = self.int_type(self.enum_payload_ty());

                agg = self.builder.build_insert_value(
                    agg,
                    tag_ty.const_int(u64::from(tag), false),
                    0,
                    "enum_tag",
                )?;

                let payload_v = payload.unwrap_or_else(|| payload_ty.const_int(0, false));
                agg = self
                    .builder
                    .build_insert_value(agg, payload_v, 1, "enum_payload")?;

                Ok(CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(agg.as_basic_value_enum()),
                })
            }
            CgEnumRepr::Niche { storage, none_value } => {
                // 说明：niche 表示下 `tag` 不参与运行期布局；caller 只需要保证：
                // - `None`：payload 传 None（使用 `none_value` 作为编码）；
                // - `Some(x)`：payload 传 Some(word(x))。
                let word_ty = self.int_type(self.enum_payload_ty());
                let encoded = payload.unwrap_or_else(|| word_ty.const_int(none_value, false));

                let raw: BasicValueEnum<'ctx> = match storage {
                    NicheStorage::Pointer => {
                        // 存储类型取 `Some` variant 的字段类型（通常为指针）。
                        let some_field = some_field.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "Option niche payload type",
                            at: at.into(),
                        })?;
                        let llvm_storage_ty = self.llvm_basic_type_of(at, some_field)?;
                        let BasicTypeEnum::PointerType(ptr_ty) = llvm_storage_ty else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Option niche storage (non-pointer)",
                                at: at.into(),
                            });
                        };
                        self.builder
                            .build_int_to_ptr(encoded, ptr_ty, "option_niche_ptr")?
                            .into()
                    }
                    NicheStorage::U8 => self
                        .builder
                        .build_int_truncate(encoded, self.context.i8_type(), "option_niche_u8")?
                        .into(),
                };

                Ok(CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(raw),
                })
            }
        }
    }

    fn codegen_when_expr(
        &mut self,
        span: crate::span::Span,
        subject: &hir::Expr,
        arms: &[hir::WhenArm],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if arms.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when (no arms)",
                at: span.into(),
            });
        }

        let subject_v = self.codegen_expr(subject)?;
        let subject_ty = subject_v.ty;
        let subject_raw = subject_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "when subject value",
            at: subject.span.into(),
        })?;

        // 将 subject 落到一个栈 slot：便于在各 arm 中做 payload 解构（避免跨 block 的 dominance 细节）。
        let subject_ptr = self.create_entry_alloca(span, "when_subject", subject_ty)?;
        self.store_local_value(span, subject_ptr, subject_ty, subject_v)?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let merge_bb = self.context.append_basic_block(func, "when_merge");
        let arm_bbs = (0..arms.len())
            .map(|i| {
                self.context
                    .append_basic_block(func, &format!("when_arm_{i}"))
            })
            .collect::<Vec<_>>();

        let needs_chain = arms
            .iter()
            .any(|arm| arm.guard.is_some() || self.when_pat_contains_or(&arm.pat));

        if needs_chain {
            // guard / or-pattern：用“链式判别 + guard 失败回落到下一个分支”的 CFG。
            //
            // 说明：这条路径不追求最优 CFG（TODO T0825：目标是语义正确）。
            let check_bbs = (0..arms.len())
                .map(|i| {
                    self.context
                        .append_basic_block(func, &format!("when_check_{i}"))
                })
                .collect::<Vec<_>>();
            let bind_bbs = (0..arms.len())
                .map(|i| {
                    self.context
                        .append_basic_block(func, &format!("when_bind_{i}"))
                })
                .collect::<Vec<_>>();
            let no_match_bb = self.context.append_basic_block(func, "when_no_match");

            self.builder.build_unconditional_branch(check_bbs[0])?;

            for (idx, arm) in arms.iter().enumerate() {
                self.builder.position_at_end(check_bbs[idx]);
                let cond = self.codegen_when_pat_cond(span, subject_ty, &arm.pat, subject_ptr)?;
                let else_bb = if idx + 1 < arms.len() {
                    check_bbs[idx + 1]
                } else {
                    no_match_bb
                };
                self.builder
                    .build_conditional_branch(cond, bind_bbs[idx], else_bb)?;
            }

            self.builder.position_at_end(no_match_bb);
            self.builder.build_unreachable()?;

            // 生成各 arm body，并把结果汇合到 merge。
            let mut out_ty: Option<CgTy> = None;
            let mut incoming: Vec<(inkwell::basic_block::BasicBlock<'ctx>, CgValue<'ctx>)> =
                Vec::new();

            for (idx, arm) in arms.iter().enumerate() {
                let else_bb = if idx + 1 < arms.len() {
                    check_bbs[idx + 1]
                } else {
                    no_match_bb
                };

                // 先在 bind block 中完成 pattern binder + guard 判定，再决定是否进入 arm body。
                self.builder.position_at_end(bind_bbs[idx]);

                self.env.push_scope();
                self.bind_when_pat(span, subject_ty, &arm.pat, subject_ptr)?;

                if let Some(guard) = &arm.guard {
                    let gv = self.codegen_expr_in_expected_context(guard, Some(CgTy::Bool))?;
                    let gb = gv.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "when guard value",
                        at: guard.span.into(),
                    })?;
                    self.builder
                        .build_conditional_branch(gb, arm_bbs[idx], else_bb)?;
                } else {
                    self.builder.build_unconditional_branch(arm_bbs[idx])?;
                }

                // arm body：在同一作用域内生成（binder 可用）。
                self.builder.position_at_end(arm_bbs[idx]);

                let v = self.codegen_expr(&arm.body)?;
                match out_ty {
                    None => out_ty = Some(v.ty),
                    Some(prev) if prev == v.ty => {}
                    Some(_) => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when arm type mismatch",
                            at: arm.body.span.into(),
                        });
                    }
                }

                self.builder.build_unconditional_branch(merge_bb)?;
                self.env.pop_scope();

                incoming.push((arm_bbs[idx], v));
            }

            self.builder.position_at_end(merge_bb);

            let out_ty = out_ty.unwrap_or(CgTy::Unit);
            return match out_ty {
                CgTy::Unit => Ok(CgValue::unit()),
                CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                    let phi_ty = self.llvm_basic_type_of(span, out_ty)?;
                    let phi = self.builder.build_phi(phi_ty, "when_phi")?;

                    for (bb, v) in incoming {
                        let raw = v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "when arm value",
                            at: span.into(),
                        })?;
                        phi.add_incoming(&[(&raw, bb)]);
                    }

                    Ok(CgValue {
                        ty: out_ty,
                        value: Some(phi.as_basic_value()),
                    })
                }
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                    Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when result type",
                        at: span.into(),
                    })
                }
            };
        }

        // 生成分派：enum/bool 优先降到 LLVM switch；tuple 仍用分支链并做字段比较。
        match subject_ty {
            CgTy::Enum(enum_ty) => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::Variant { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (enum)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                // 注意：避免持有 `cg_enum_layout(...)` 的借用跨越后续 builder 调用。
                let (repr, variants) = {
                    let cg_layout = self.cg_enum_layout(span, enum_ty)?;
                    (cg_layout.repr, cg_layout.variants.clone())
                };

                let tag = match repr {
                    CgEnumRepr::TaggedUnion => {
                        let subject_struct = subject_raw.into_struct_value();
                        self.builder
                            .build_extract_value(subject_struct, 0, "when_tag")?
                            .into_int_value()
                    }
                    CgEnumRepr::Niche { storage, none_value } => {
                        let is_none = match storage {
                            NicheStorage::Pointer => {
                                let ptr = subject_raw.into_pointer_value();
                                let word_ty = self.int_type(self.enum_payload_ty());
                                let as_int = self
                                    .builder
                                    .build_ptr_to_int(ptr, word_ty, "option_ptr_as_int")?;
                                let expected = word_ty.const_int(none_value, false);
                                self.builder.build_int_compare(
                                    IntPredicate::EQ,
                                    as_int,
                                    expected,
                                    "option_is_none",
                                )?
                            }
                            NicheStorage::U8 => {
                                let v = subject_raw.into_int_value();
                                let expected = self.context.i8_type().const_int(none_value, false);
                                self.builder.build_int_compare(
                                    IntPredicate::EQ,
                                    v,
                                    expected,
                                    "option_is_none",
                                )?
                            }
                        };

                        let some_tag = self.context.i32_type().const_int(0, false);
                        let none_tag = self.context.i32_type().const_int(1, false);
                        self.builder
                            .build_select(is_none, none_tag, some_tag, "option_tag")?
                            .into_int_value()
                    }
                };

                let tag_ty = self.context.i32_type();
                let default_bb = self.context.append_basic_block(func, "when_no_match");

                let mut cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                    Vec::with_capacity(variants.len());
                for variant in &variants {
                    let Some(target_idx) =
                        self.when_first_matching_arm_for_enum_variant(arms, &variant.name)
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when missing enum arm",
                            at: span.into(),
                        });
                    };
                    cases.push((
                        tag_ty.const_int(u64::from(variant.tag), false),
                        arm_bbs[target_idx],
                    ));
                }

                self.builder.build_switch(tag, default_bb, &cases)?;
                self.builder.position_at_end(default_bb);
                self.builder.build_unreachable()?;
            }
            CgTy::Bool => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::BoolLit { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (bool)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let b = subject_raw.into_int_value();
                let bool_ty = self.context.bool_type();
                let default_bb = self.context.append_basic_block(func, "when_no_match");

                let Some(false_idx) = self.when_first_matching_arm_for_bool(arms, false) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when missing bool arm (false)",
                        at: span.into(),
                    });
                };
                let Some(true_idx) = self.when_first_matching_arm_for_bool(arms, true) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when missing bool arm (true)",
                        at: span.into(),
                    });
                };

                let cases = [
                    (bool_ty.const_int(0, false), arm_bbs[false_idx]),
                    (bool_ty.const_int(1, false), arm_bbs[true_idx]),
                ];
                self.builder.build_switch(b, default_bb, &cases)?;
                self.builder.position_at_end(default_bb);
                self.builder.build_unreachable()?;
            }
            CgTy::Tuple(tuple_ty) => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::Tuple { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (tuple)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let check_bbs = (0..arms.len())
                    .map(|i| {
                        self.context
                            .append_basic_block(func, &format!("when_check_{i}"))
                    })
                    .collect::<Vec<_>>();
                let no_match_bb = self.context.append_basic_block(func, "when_no_match");

                self.builder.build_unconditional_branch(check_bbs[0])?;

                for (idx, arm) in arms.iter().enumerate() {
                    self.builder.position_at_end(check_bbs[idx]);

                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. } => {
                            self.builder.build_unconditional_branch(arm_bbs[idx])?;
                        }
                        hir::WhenPat::Tuple { elements, .. } => {
                            let cond =
                                self.codegen_when_tuple_pat_cond(span, tuple_ty, elements, subject_ptr)?;
                            let else_bb = if idx + 1 < arms.len() {
                                check_bbs[idx + 1]
                            } else {
                                no_match_bb
                            };
                            self.builder
                                .build_conditional_branch(cond, arm_bbs[idx], else_bb)?;
                        }
                        _ => unreachable!("tuple patterns validated above"),
                    }
                }

                self.builder.position_at_end(no_match_bb);
                self.builder.build_unreachable()?;
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when subject type",
                    at: subject.span.into(),
                });
            }
        }

        // 生成各 arm body，并把结果汇合到 merge。
        let mut out_ty: Option<CgTy> = None;
        let mut incoming: Vec<(inkwell::basic_block::BasicBlock<'ctx>, CgValue<'ctx>)> = Vec::new();

        for (idx, arm) in arms.iter().enumerate() {
            self.builder.position_at_end(arm_bbs[idx]);

            self.env.push_scope();
            self.bind_when_pat(span, subject_ty, &arm.pat, subject_ptr)?;

            let v = self.codegen_expr(&arm.body)?;
            match out_ty {
                None => out_ty = Some(v.ty),
                Some(prev) if prev == v.ty => {}
                Some(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when arm type mismatch",
                        at: arm.body.span.into(),
                    });
                }
            }

            self.builder.build_unconditional_branch(merge_bb)?;
            self.env.pop_scope();

            incoming.push((arm_bbs[idx], v));
        }

        self.builder.position_at_end(merge_bb);

        let out_ty = out_ty.unwrap_or(CgTy::Unit);
        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let phi_ty = self.llvm_basic_type_of(span, out_ty)?;
                let phi = self.builder.build_phi(phi_ty, "when_phi")?;

                for (bb, v) in incoming {
                    let raw = v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "when arm value",
                        at: span.into(),
                    })?;
                    phi.add_incoming(&[(&raw, bb)]);
                }

                Ok(CgValue {
                    ty: out_ty,
                    value: Some(phi.as_basic_value()),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when result type",
                    at: span.into(),
                })
            }
        }
    }

    fn bind_when_pat(
        &mut self,
        at: crate::span::Span,
        subject_ty: CgTy,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Or { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Rest { .. }
            | hir::WhenPat::Is { .. }
            | hir::WhenPat::IntLit { .. }
            | hir::WhenPat::StringLit { .. }
            | hir::WhenPat::BoolLit { .. } => Ok(()),
            hir::WhenPat::Bind { id, name, .. } => {
                // `x -> ...`：绑定整个 subject。
                let ptr = self.create_entry_alloca(at, name, subject_ty)?;
                let llvm_ty = self.llvm_basic_type_of(at, subject_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, subject_ptr, "bind_subject")?;
                let v = CgValue {
                    ty: subject_ty,
                    value: Some(loaded),
                };
                self.store_local_value(at, ptr, subject_ty, v)?;
                self.env.insert(
                    *id,
                    CgLocal {
                        ty: subject_ty,
                        ptr,
                        mutable: false,
                    },
                );
                Ok(())
            }
            hir::WhenPat::Variant { name, args, .. } => {
                let CgTy::Enum(enum_ty) = subject_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant pattern subject type",
                        at: pat.span().into(),
                    });
                };

                let (repr, variant) = {
                    let cg_layout = self.cg_enum_layout(at, enum_ty)?;
                    let repr = cg_layout.repr;
                    let variant = cg_layout
                        .variants
                        .iter()
                        .find(|v| v.name == *name)
                        .cloned()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "when unknown enum variant",
                            at: pat.span().into(),
                        })?;
                    (repr, variant)
                };

                // 解析 `..`：parser/typecheck 已保证它最多出现一次且必须出现在最后一个位置。
                let (prefix_pats, has_rest) = match args.last() {
                    Some(hir::WhenPat::Rest { .. }) => (&args[..args.len().saturating_sub(1)], true),
                    _ => (args.as_slice(), false),
                };

                let expected_arity = variant.fields.len();
                let found_arity = prefix_pats.len();
                if (!has_rest && expected_arity != found_arity) || (has_rest && found_arity > expected_arity) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant arity mismatch",
                        at: pat.span().into(),
                    });
                }

                if prefix_pats.is_empty() {
                    return Ok(());
                }

                // boxed variant：payload 是指向“payload struct”的指针（存放所有字段）。
                if variant.boxed {
                    let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?.into_struct_type();
                    let loaded =
                        self.builder
                            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;
                    let raw_struct = loaded.into_struct_value();
                    let payload_word = self
                        .builder
                        .build_extract_value(raw_struct, 1, "when_payload")?
                        .into_int_value();

                    let payload_struct_ty =
                        self.llvm_enum_boxed_payload_struct_type(at, enum_ty, &variant)?;
                    let payload_ptr = self.builder.build_int_to_ptr(
                        payload_word,
                        payload_struct_ty.ptr_type(AddressSpace::default()),
                        "when_payload_ptr",
                    )?;
                    let payload_loaded = self
                        .builder
                        .build_load(payload_struct_ty, payload_ptr, "load_when_payload")?
                        .into_struct_value();

                    for (idx, arg_pat) in prefix_pats.iter().enumerate() {
                        let field_cg = *variant
                            .fields
                            .get(idx)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "when boxed payload field index",
                                at: arg_pat.span().into(),
                            })?;

                        match arg_pat {
                            hir::WhenPat::Bind { id, name, .. } => {
                                let raw = self.builder.build_extract_value(
                                    payload_loaded,
                                    idx as u32,
                                    "when_payload_field",
                                )?;
                                let extracted =
                                    self.cg_value_from_loaded(arg_pat.span(), field_cg, raw)?;

                                let ptr = self.create_entry_alloca(at, name, field_cg)?;
                                self.store_local_value(at, ptr, field_cg, extracted)?;
                                self.env.insert(
                                    *id,
                                    CgLocal {
                                        ty: field_cg,
                                        ptr,
                                        mutable: false,
                                    },
                                );
                            }
                            hir::WhenPat::Wildcard { .. } => {}
                            hir::WhenPat::Rest { .. } => break,
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "when variant arg pattern",
                                    at: arg_pat.span().into(),
                                });
                            }
                        }
                    }

                    return Ok(());
                }

                // niche enum（当前仅 Option<T>）：payload 就是 enum 本身。
                if matches!(repr, CgEnumRepr::Niche { .. }) {
                    if variant.fields.len() != 1 {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "niche enum variant arity",
                            at: pat.span().into(),
                        });
                    }

                    let field_cg = variant.fields[0];
                    let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;

                    // 存储类型可能与字段类型不同（例如 `Option<Bool>` 存储为 u8）。
                    let extracted = match field_cg {
                        CgTy::Bool => {
                            let b = self.builder.build_int_truncate(
                                loaded.into_int_value(),
                                self.context.bool_type(),
                                "option_bool_from_u8",
                            )?;
                            CgValue::bool(b)
                        }
                        CgTy::String => CgValue {
                            ty: CgTy::String,
                            value: Some(loaded.into_pointer_value().into()),
                        },
                        CgTy::Ref => CgValue {
                            ty: CgTy::Ref,
                            value: Some(loaded.into_pointer_value().into()),
                        },
                        CgTy::Unit
                        | CgTy::Int(_)
                        | CgTy::Tuple(_)
                        | CgTy::Struct(_)
                        | CgTy::Enum(_) => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "niche enum payload type",
                                at: pat.span().into(),
                            });
                        }
                    };

                    // niche enum 的 binder 只能绑定第一个字段（且 rest 可能忽略其余）。
                    let Some(first_pat) = prefix_pats.first() else {
                        return Ok(());
                    };
                    match first_pat {
                        hir::WhenPat::Bind { id, name, .. } => {
                            let ptr = self.create_entry_alloca(at, name, field_cg)?;
                            self.store_local_value(at, ptr, field_cg, extracted)?;
                            self.env.insert(
                                *id,
                                CgLocal {
                                    ty: field_cg,
                                    ptr,
                                    mutable: false,
                                },
                            );
                        }
                        hir::WhenPat::Wildcard { .. } | hir::WhenPat::Rest { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when variant arg pattern",
                                at: first_pat.span().into(),
                            });
                        }
                    }

                    return Ok(());
                }

                // inline tagged union：仍只支持 “小 payload”（单字段标量）。
                if variant.fields.len() != 1 || prefix_pats.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant payload (inline, unsupported arity)",
                        at: pat.span().into(),
                    });
                }

                let field_cg = variant.fields[0];
                let arg_pat = &prefix_pats[0];

                let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?.into_struct_type();
                let loaded =
                    self.builder
                        .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;
                let raw_struct = loaded.into_struct_value();
                let payload_raw = self
                    .builder
                    .build_extract_value(raw_struct, 1, "when_payload")?
                    .into_int_value();

                // 当前阶段 payload 固定为 word-sized int；按字段类型截断/转换。
                let extracted = match field_cg {
                    CgTy::Unit => CgValue::unit(),
                    CgTy::Bool => {
                        let b = self.builder.build_int_truncate(
                            payload_raw,
                            self.context.bool_type(),
                            "payload_to_bool",
                        )?;
                        CgValue::bool(b)
                    }
                    CgTy::Int(int_ty) => {
                        let from = self.enum_payload_ty();
                        let casted = self.cast_int(payload_raw, from, int_ty)?;
                        CgValue::int(casted, int_ty)
                    }
                    CgTy::String => {
                        let ptr = self.builder.build_int_to_ptr(
                            payload_raw,
                            self.llvm_scoop_string_ptr_type(),
                            "payload_to_str_ptr",
                        )?;
                        CgValue {
                            ty: CgTy::String,
                            value: Some(ptr.into()),
                        }
                    }
                    CgTy::Ref => {
                        let ptr = self.builder.build_int_to_ptr(
                            payload_raw,
                            self.context.i8_type().ptr_type(AddressSpace::default()),
                            "payload_to_ref_ptr",
                        )?;
                        CgValue {
                            ty: CgTy::Ref,
                            value: Some(ptr.into()),
                        }
                    }
                    CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when payload (non-scalar)",
                            at: arg_pat.span().into(),
                        });
                    }
                };

                match arg_pat {
                    hir::WhenPat::Bind { id, name, .. } => {
                        let ptr = self.create_entry_alloca(at, name, field_cg)?;
                        self.store_local_value(at, ptr, field_cg, extracted)?;
                        self.env.insert(
                            *id,
                            CgLocal {
                                ty: field_cg,
                                ptr,
                                mutable: false,
                            },
                        );
                    }
                    hir::WhenPat::Wildcard { .. } | hir::WhenPat::Rest { .. } => {}
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when variant arg pattern",
                            at: arg_pat.span().into(),
                        });
                    }
                }

                Ok(())
            }
            hir::WhenPat::Tuple { elements, .. } => {
                let CgTy::Tuple(tuple_ty) = subject_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple pattern subject type",
                        at: pat.span().into(),
                    });
                };

                let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty)
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple type id",
                        at: pat.span().into(),
                    });
                };

                let mut has_rest = false;
                for (idx, elem_pat) in elements.iter().enumerate() {
                    if matches!(elem_pat, hir::WhenPat::Rest { .. }) {
                        if idx + 1 != elements.len() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when tuple pattern rest position",
                                at: elem_pat.span().into(),
                            });
                        }
                        has_rest = true;
                        break;
                    }
                }

                let pat_arity = if has_rest {
                    elements.len().saturating_sub(1)
                } else {
                    elements.len()
                };

                if (!has_rest && pat_arity != tuple_elems.len())
                    || (has_rest && pat_arity > tuple_elems.len())
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple pattern arity mismatch",
                        at: pat.span().into(),
                    });
                }

                let llvm_tuple_ty = self.llvm_tuple_type(at, tuple_ty)?;
                let loaded =
                    self.builder
                        .build_load(llvm_tuple_ty, subject_ptr, "load_when_tuple")?;
                let tuple_v = loaded.into_struct_value();

                for (idx, elem_pat) in elements.iter().enumerate() {
                    if matches!(elem_pat, hir::WhenPat::Rest { .. }) {
                        break;
                    }
                    let elem_ty =
                        self.lookup_tuple_element(tuple_ty, idx as u32, elem_pat.span())?;

                    let extracted_v = if elem_ty == CgTy::Unit {
                        CgValue::unit()
                    } else {
                        let raw = self.builder.build_extract_value(
                            tuple_v,
                            idx as u32,
                            "when_tuple_elem",
                        )?;
                        self.cg_value_from_loaded(elem_pat.span(), elem_ty, raw)?
                    };

                    match elem_pat {
                        hir::WhenPat::Bind { .. } => {
                            // 直接把元素作为 subject 绑定（避免额外临时 slot）。
                            let hir::WhenPat::Bind { id, name, .. } = elem_pat else {
                                unreachable!()
                            };
                            let ptr = self.create_entry_alloca(at, name, elem_ty)?;
                            self.store_local_value(at, ptr, elem_ty, extracted_v)?;
                            self.env.insert(
                                *id,
                                CgLocal {
                                    ty: elem_ty,
                                    ptr,
                                    mutable: false,
                                },
                            );
                        }
                        hir::WhenPat::Tuple { .. } | hir::WhenPat::Variant { .. } => {
                            // 递归绑定：需要一个临时 slot 让子 pattern 能 load/extract。
                            let tmp_name = format!("when_tuple_elem_{idx}");
                            let tmp_ptr = self.create_entry_alloca(at, &tmp_name, elem_ty)?;
                            self.store_local_value(at, tmp_ptr, elem_ty, extracted_v)?;
                            self.bind_when_pat(at, elem_ty, elem_pat, tmp_ptr)?;
                        }
                        _ => {}
                    }
                }

                Ok(())
            }
        }
    }

    fn when_pat_contains_or(&self, pat: &hir::WhenPat) -> bool {
        match pat {
            hir::WhenPat::Or { .. } => true,
            hir::WhenPat::Tuple { elements, .. } => {
                elements.iter().any(|p| self.when_pat_contains_or(p))
            }
            hir::WhenPat::Variant { args, .. } => args.iter().any(|p| self.when_pat_contains_or(p)),
            _ => false,
        }
    }

    fn codegen_when_pat_cond(
        &mut self,
        at: crate::span::Span,
        subject_ty: CgTy,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match subject_ty {
            CgTy::Enum(enum_ty) => self.codegen_when_pat_cond_for_enum(at, enum_ty, pat, subject_ptr),
            CgTy::Bool => self.codegen_when_pat_cond_for_bool(at, pat, subject_ptr),
            CgTy::Tuple(tuple_ty) => {
                self.codegen_when_pat_cond_for_tuple(at, tuple_ty, pat, subject_ptr)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when subject type",
                at: at.into(),
            }),
        }
    }

    fn codegen_when_pat_cond_for_enum(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 注意：避免持有 `cg_enum_layout(...)` 的借用跨越后续 builder 调用。
        let (repr, variants) = {
            let cg_layout = self.cg_enum_layout(at, enum_ty)?;
            (cg_layout.repr, cg_layout.variants.clone())
        };
        let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
        let loaded = self
            .builder
            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;

        let tag = match repr {
            CgEnumRepr::TaggedUnion => {
                let raw_struct = loaded.into_struct_value();
                self.builder
                    .build_extract_value(raw_struct, 0, "when_tag")?
                    .into_int_value()
            }
            CgEnumRepr::Niche { storage, none_value } => {
                let is_none = match storage {
                    NicheStorage::Pointer => {
                        let ptr = loaded.into_pointer_value();
                        let word_ty = self.int_type(self.enum_payload_ty());
                        let as_int = self
                            .builder
                            .build_ptr_to_int(ptr, word_ty, "option_ptr_as_int")?;
                        let expected = word_ty.const_int(none_value, false);
                        self.builder.build_int_compare(
                            IntPredicate::EQ,
                            as_int,
                            expected,
                            "option_is_none",
                        )?
                    }
                    NicheStorage::U8 => {
                        let v = loaded.into_int_value();
                        let expected = self.context.i8_type().const_int(none_value, false);
                        self.builder.build_int_compare(
                            IntPredicate::EQ,
                            v,
                            expected,
                            "option_is_none",
                        )?
                    }
                };

                let some_tag = self.context.i32_type().const_int(0, false);
                let none_tag = self.context.i32_type().const_int(1, false);
                self.builder
                    .build_select(is_none, none_tag, some_tag, "option_tag")?
                    .into_int_value()
            }
        };

        self.codegen_when_pat_cond_for_enum_with_tag(at, &variants, tag, pat)
    }

    fn codegen_when_pat_cond_for_enum_with_tag(
        &self,
        at: crate::span::Span,
        variants: &[CgEnumVariant],
        tag: IntValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. } | hir::WhenPat::Wildcard { .. } | hir::WhenPat::Bind { .. } => {
                Ok(self.context.bool_type().const_int(1, false))
            }
            hir::WhenPat::Variant { name, args, .. } => {
                let Some(variant) = variants.iter().find(|v| v.name == *name) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when unknown enum variant",
                        at: pat.span().into(),
                    });
                };
                let _ = args;

                let expected = self
                    .context
                    .i32_type()
                    .const_int(u64::from(variant.tag), false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    tag,
                    expected,
                    "when_enum_tag_eq",
                )?)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_enum_with_tag(at, variants, tag, p)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (enum)",
                at: pat.span().into(),
            }),
        }
    }

    fn codegen_when_pat_cond_for_bool(
        &mut self,
        at: crate::span::Span,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let loaded = self
            .builder
            .build_load(self.context.bool_type(), subject_ptr, "load_when_bool")?
            .into_int_value();
        self.codegen_when_pat_cond_for_bool_with_value(at, loaded, pat)
    }

    fn codegen_when_pat_cond_for_bool_with_value(
        &self,
        _at: crate::span::Span,
        value: IntValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. } | hir::WhenPat::Wildcard { .. } | hir::WhenPat::Bind { .. } => {
                Ok(self.context.bool_type().const_int(1, false))
            }
            hir::WhenPat::BoolLit { value: expected, .. } => {
                let expected = self.context.bool_type().const_int(*expected as u64, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    expected,
                    "when_bool_eq",
                )?)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_bool_with_value(_at, value, p)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (bool)",
                at: pat.span().into(),
            }),
        }
    }

    fn codegen_when_pat_cond_for_tuple(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. } | hir::WhenPat::Wildcard { .. } | hir::WhenPat::Bind { .. } => {
                Ok(self.context.bool_type().const_int(1, false))
            }
            hir::WhenPat::Tuple { elements, .. } => {
                self.codegen_when_tuple_pat_cond(at, tuple_ty, elements, subject_ptr)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_tuple(at, tuple_ty, p, subject_ptr)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (tuple)",
                at: pat.span().into(),
            }),
        }
    }

    fn when_first_matching_arm_for_enum_variant(
        &self,
        arms: &[hir::WhenArm],
        variant_name: &str,
    ) -> Option<usize> {
        for (idx, arm) in arms.iter().enumerate() {
            match &arm.pat {
                hir::WhenPat::Else { .. }
                | hir::WhenPat::Wildcard { .. }
                | hir::WhenPat::Bind { .. } => return Some(idx),
                hir::WhenPat::Variant { name, .. } if name == variant_name => return Some(idx),
                _ => {}
            }
        }
        None
    }

    fn when_first_matching_arm_for_bool(
        &self,
        arms: &[hir::WhenArm],
        value: bool,
    ) -> Option<usize> {
        for (idx, arm) in arms.iter().enumerate() {
            match &arm.pat {
                hir::WhenPat::Else { .. }
                | hir::WhenPat::Wildcard { .. }
                | hir::WhenPat::Bind { .. } => return Some(idx),
                hir::WhenPat::BoolLit { value: v, .. } if *v == value => return Some(idx),
                _ => {}
            }
        }
        None
    }

    fn codegen_when_tuple_pat_cond(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        elements: &[hir::WhenPat],
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let mut rest_idx: Option<usize> = None;
        for (idx, pat) in elements.iter().enumerate() {
            if matches!(pat, hir::WhenPat::Rest { .. }) {
                rest_idx = Some(idx);
                break;
            }
        }

        if let Some(rest) = rest_idx {
            if rest + 1 != elements.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when tuple pattern rest position",
                    at: elements[rest].span().into(),
                });
            }
        }

        let pat_arity = rest_idx.unwrap_or(elements.len());
        if (rest_idx.is_none() && pat_arity != tuple_elems.len())
            || (rest_idx.is_some() && pat_arity > tuple_elems.len())
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when tuple pattern arity mismatch",
                at: at.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(at, tuple_ty)?;
        let loaded = self
            .builder
            .build_load(llvm_tuple_ty, subject_ptr, "load_when_tuple")?;
        let tuple_v = loaded.into_struct_value();

        let mut cond = self.context.bool_type().const_int(1, false);
        for (idx, elem_pat) in elements.iter().enumerate().take(pat_arity) {
            let elem_ty = self.lookup_tuple_element(tuple_ty, idx as u32, elem_pat.span())?;
            let elem_cond = self.codegen_when_pat_cond_for_tuple_elem(
                at, tuple_ty, idx, elem_ty, tuple_v, elem_pat,
            )?;
            cond = self.builder.build_and(cond, elem_cond, "when_tuple_and")?;
        }
        Ok(cond)
    }

    fn codegen_when_pat_cond_for_tuple_elem(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        elem_idx: usize,
        elem_ty: CgTy,
        tuple_v: inkwell::values::StructValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. }
            | hir::WhenPat::Rest { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::BoolLit { value, .. } => {
                let CgTy::Bool = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem bool pattern type",
                        at: pat.span().into(),
                    });
                };
                let raw = self
                    .builder
                    .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?
                    .into_int_value();
                let expected = self.context.bool_type().const_int(*value as u64, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    raw,
                    expected,
                    "when_tuple_bool_eq",
                )?)
            }
            hir::WhenPat::IntLit { span } => {
                let CgTy::Int(int_ty) = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem int pattern type",
                        at: pat.span().into(),
                    });
                };
                let raw = self
                    .builder
                    .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?
                    .into_int_value();
                let text = self.source.slice(*span);
                let value = parse_int_literal_decimal(text);
                let value = mask_to_bits(value, int_ty.bits) as u64;
                let expected = self.int_type(int_ty).const_int(value, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    raw,
                    expected,
                    "when_tuple_int_eq",
                )?)
            }
            hir::WhenPat::Tuple { elements, .. } => {
                let CgTy::Tuple(nested_tuple_ty) = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem tuple pattern type",
                        at: pat.span().into(),
                    });
                };

                let TypeKind::Value(ValueTypeKind::Tuple(_)) = self.types.kind(nested_tuple_ty)
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple type id",
                        at: pat.span().into(),
                    });
                };

                // 由于 extractvalue 返回的是一个“by-value tuple struct”，我们先把它落到临时 slot，
                // 再复用 `codegen_when_tuple_pat_cond` 的逻辑生成递归比较。
                let nested_raw = self.builder.build_extract_value(
                    tuple_v,
                    elem_idx as u32,
                    "when_tuple_elem",
                )?;
                let nested_value = self.cg_value_from_loaded(pat.span(), elem_ty, nested_raw)?;
                let tmp_name = format!("when_tuple_nested_{}_{}", tuple_ty.as_u32(), elem_idx);
                let tmp_ptr = self.create_entry_alloca(at, &tmp_name, elem_ty)?;
                self.store_local_value(at, tmp_ptr, elem_ty, nested_value)?;
                self.codegen_when_tuple_pat_cond(at, nested_tuple_ty, elements, tmp_ptr)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when tuple pattern",
                at: pat.span().into(),
            }),
        }
    }

    fn llvm_param_ty(
        &self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        let cg = self
            .cg_ty_of(ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function param type",
                at: span.into(),
            })?;

        Ok(match cg {
            CgTy::Unit => self.context.i8_type().into(),
            CgTy::Bool => self.context.bool_type().into(),
            CgTy::Int(int_ty) => self.int_type(int_ty).into(),
            CgTy::String => self.llvm_scoop_string_ptr_type().into(),
            CgTy::Ref => self
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .into(),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "function param type",
                    at: span.into(),
                });
            }
        })
    }

    fn as_llvm_arg_value(
        &self,
        span: crate::span::Span,
        param_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<inkwell::values::BasicMetadataValueEnum<'ctx>, LlvmEmitError> {
        Ok(match param_ty {
            CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => value
                .value
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg value",
                    at: span.into(),
                })?
                .into(),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg value",
                    at: span.into(),
                });
            }
        })
    }

    fn codegen_fun_params(
        &mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in fun.params.iter().enumerate() {
            let target_ty = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "param type",
                    at: param.span.into(),
                })?;

            let ptr = self.create_entry_alloca(param.span, &param.name, target_ty)?;
            let init = match target_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Bool => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_int_value();
                    CgValue::bool(raw)
                }
                CgTy::Int(int_ty) => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_int_value();
                    CgValue::int(raw, int_ty)
                }
                CgTy::String => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_pointer_value();
                    CgValue {
                        ty: CgTy::String,
                        value: Some(raw.into()),
                    }
                }
                CgTy::Ref => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_pointer_value();
                    CgValue {
                        ty: CgTy::Ref,
                        value: Some(raw.into()),
                    }
                }
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "param type",
                        at: param.span.into(),
                    });
                }
            };

            self.store_local_value(param.span, ptr, target_ty, init)?;
            self.env.insert(
                param.id,
                CgLocal {
                    ty: target_ty,
                    ptr,
                    mutable: false,
                },
            );
        }
        Ok(())
    }

    fn codegen_block_as_return_value(
        &mut self,
        block: &hir::Block,
        declared_return_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let mut tail_value: Option<CgValue<'ctx>> = None;

        self.env.push_scope();

        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    tail_value = None;
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    tail_value = None;
                }
                hir::StmtKind::Expr(expr) => {
                    let v = self.codegen_expr(expr)?;
                    tail_value = if is_last { Some(v) } else { None };
                }
                hir::StmtKind::Return { value } => {
                    let out = match value {
                        Some(expr) => {
                            let v = self.codegen_expr(expr)?;
                            if declared_return_ty == CgTy::Unit {
                                CgValue::unit()
                            } else {
                                self.coerce_value(expr.span, v, declared_return_ty)?
                            }
                        }
                        None => self.default_value(declared_return_ty),
                    };

                    self.env.pop_scope();
                    return Ok(out);
                }
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        let out = if let Some(v) = tail_value {
            if declared_return_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                self.coerce_value(block.span, v, declared_return_ty)?
            }
        } else {
            self.default_value(declared_return_ty)
        };

        self.env.pop_scope();
        Ok(out)
    }

    fn default_value(&self, ty: CgTy) -> CgValue<'ctx> {
        match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(self.context.bool_type().const_int(0, false)),
            CgTy::Int(int_ty) => CgValue::int(self.int_type(int_ty).const_int(0, false), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(self.llvm_scoop_string_ptr_type().const_null().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(
                    self.context
                        .i8_type()
                        .ptr_type(AddressSpace::default())
                        .const_null()
                        .into(),
                ),
            },
            // 说明：当前阶段不支持 tuple/struct 作为函数返回类型，因此这里仅提供占位值；
            // 若后续误用，会在 emit/store 阶段触发结构化错误而非 panic。
            CgTy::Tuple(ty) => CgValue {
                ty: CgTy::Tuple(ty),
                value: None,
            },
            CgTy::Struct(ty) => CgValue {
                ty: CgTy::Struct(ty),
                value: None,
            },
            CgTy::Enum(ty) => CgValue {
                ty: CgTy::Enum(ty),
                value: None,
            },
        }
    }

    fn emit_return(
        &mut self,
        span: crate::span::Span,
        declared_return_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match declared_return_ty {
            CgTy::Unit => {
                self.builder.build_return(None)?;
                Ok(())
            }
            CgTy::Bool | CgTy::Int(_) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "return value",
                        at: span.into(),
                    });
                };
                self.builder.build_return(Some(&raw))?;
                Ok(())
            }
            CgTy::String | CgTy::Ref | CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "aggregate return type",
                    at: span.into(),
                })
            }
        }
    }

    fn codegen_block_value(&mut self, block: &hir::Block) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.env.push_scope();

        let mut value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    value = CgValue::unit();
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    value = CgValue::unit();
                }
                hir::StmtKind::Expr(expr) => {
                    let v = self.codegen_expr(expr)?;
                    value = if is_last { v } else { CgValue::unit() };
                }
                // block 作为表达式时，`return` 语义在当前阶段暂不支持（需要 function-level CFG）。
                hir::StmtKind::Return { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "`return` inside block expression",
                        at: stmt.span.into(),
                    });
                }
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement inside block expression",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        self.env.pop_scope();
        Ok(value)
    }

    fn codegen_literal(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        lit: &hir::LiteralKind,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match lit {
            hir::LiteralKind::Unit => Ok(CgValue::unit()),
            hir::LiteralKind::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            hir::LiteralKind::Int => {
                let Some(CgTy::Int(int_ty)) = self.cg_ty_of(ty) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "int literal type",
                        at: span.into(),
                    });
                };
                let text = self.source.slice(span);
                let value = parse_int_literal_decimal(text);
                let value = mask_to_bits(value, int_ty.bits) as u64;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(value, false),
                    int_ty,
                ))
            }
            hir::LiteralKind::String => self.codegen_string_literal(span),
        }
    }

    fn codegen_string_literal(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let text = self.source.slice(span);

        let bytes = match parse_string_literal_bytes(text) {
            Ok(bytes) => bytes,
            Err(StringLiteralParseError::Interpolated) => {
                // 插值字符串（`f"..."`/`f"""..."""`）由后续任务 T0823 lowering 处理；
                // 当前阶段避免“把原始文本当作普通字符串”导致语义错误。
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "interpolated string literal",
                    at: span.into(),
                });
            }
            Err(StringLiteralParseError::Invalid) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "invalid string literal",
                    at: span.into(),
                });
            }
        };

        // 1) 把字节序列落到一个只读全局常量：`[N x i8] @__scoop_str_data_*`
        let data_gv = self.get_or_create_global_bytes(span, &bytes);

        // 2) 构造 `ScoopString { len, data }` 并返回其指针（当前阶段先放在栈上）。
        let scoop_str_ty = self.llvm_scoop_string_type();
        let str_ptr = self.create_entry_alloca_raw(span, "scoop_str_lit", scoop_str_ty.into())?;

        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 0, "str_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "str_data_gep")?;

        let len = self.context.i64_type().const_int(bytes.len() as u64, false);

        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let data_i8_ptr = self.builder.build_pointer_cast(
            data_gv.as_pointer_value(),
            i8_ptr_ty,
            "str_data_ptr",
        )?;

        let _ = self.builder.build_store(len_ptr, len)?;
        let _ = self.builder.build_store(data_ptr, data_i8_ptr)?;

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn codegen_interpolated_string(
        &mut self,
        span: crate::span::Span,
        raw: bool,
        parts: &[hir::InterpolatedStringPart],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 当前阶段的落点：把 f-string 分片后“拼接”为一段连续 UTF-8 字节序列，
        // 以 runtime `ScoopString { len, data }` 的形式返回（data 指向栈上 buffer）。
        //
        // 约束（与 TODO T0823 对齐）：
        // - 仅支持 `{Int}` 与 `{String}`；
        // - 先不支持 format spec / locale；
        // - 先不做 heap 分配：buffer 全部落在栈上（未来接入 `scoop_alloc`/GC 再升级）。

        #[derive(Clone, Copy)]
        struct Segment<'ctx> {
            ptr: PointerValue<'ctx>,
            len: IntValue<'ctx>,
        }

        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = i8_ty.ptr_type(AddressSpace::default());

        // 1) 为结果构造一个 `ScoopString`（固定大小，放在 entry block）
        let scoop_str_ty = self.llvm_scoop_string_type();
        let str_ptr = self.create_entry_alloca_raw(span, "scoop_str_fstr", scoop_str_ty.into())?;

        // 2) 先做一遍：收集所有片段的 (ptr, len)，并计算总长度（运行期）。
        let mut segments: Vec<Segment<'ctx>> = Vec::new();
        let mut total_len = i64_ty.const_zero();

        for part in parts {
            match part {
                hir::InterpolatedStringPart::Text { span: text_span } => {
                    let text = self.source.slice(*text_span);
                    let bytes = parse_f_string_text_bytes(raw, text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "invalid interpolated string text",
                            at: (*text_span).into(),
                        }
                    })?;

                    let gv = self.get_or_create_global_bytes(*text_span, &bytes);
                    let ptr = self.builder.build_pointer_cast(
                        gv.as_pointer_value(),
                        i8_ptr_ty,
                        "fstr_text_ptr",
                    )?;
                    let len = i64_ty.const_int(bytes.len() as u64, false);

                    segments.push(Segment { ptr, len });
                    total_len = self
                        .builder
                        .build_int_add(total_len, len, "fstr_total_len")?;
                }
                hir::InterpolatedStringPart::Expr { expr } => {
                    let v = self.codegen_expr(expr)?;

                    match v.ty {
                        CgTy::String => {
                            let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
                            let Some(raw) = coerced.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let len_ptr = self.builder.build_struct_gep(
                                scoop_str_ty,
                                str_obj_ptr,
                                0,
                                "fstr_part_len_gep",
                            )?;
                            let data_ptr = self.builder.build_struct_gep(
                                scoop_str_ty,
                                str_obj_ptr,
                                1,
                                "fstr_part_data_gep",
                            )?;

                            let len = self
                                .builder
                                .build_load(i64_ty, len_ptr, "fstr_part_len")?
                                .into_int_value();
                            let data = self
                                .builder
                                .build_load(i8_ptr_ty, data_ptr, "fstr_part_data")?
                                .into_pointer_value();

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Int(from_ty) => {
                            if from_ty.bits > 64 {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "integer width for string interpolation",
                                    at: expr.span.into(),
                                });
                            }

                            let (raw_int, _) =
                                v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "integer interpolation expr value",
                                    at: expr.span.into(),
                                })?;

                            // 先把整数提升/截断到 i64/u64，再调用 runtime 格式化到临时 buffer。
                            let to_ty = IntTy {
                                bits: 64,
                                signed: from_ty.signed,
                            };
                            let int64 = self.cast_int(raw_int, from_ty, to_ty)?;

                            // i64 最长：`-9223372036854775808`（20 字符）；
                            // 这里给更宽松的 cap，避免后续扩展时踩坑。
                            let cap = i64_ty.const_int(64, false);
                            let buf =
                                self.builder
                                    .build_array_alloca(i8_ty, cap, "fstr_int_buf")?;

                            let fmt_name = if from_ty.signed {
                                "scoop_format_i64"
                            } else {
                                "scoop_format_u64"
                            };
                            let fmt_fun = self.declare_runtime_format_int(fmt_name);
                            let call_site = self.builder.build_call(
                                fmt_fun,
                                &[int64.into(), buf.into(), cap.into()],
                                "fstr_fmt_int",
                            )?;
                            let len = call_site
                                .try_as_basic_value()
                                .basic()
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation int length",
                                    at: expr.span.into(),
                                })?
                                .into_int_value();

                            segments.push(Segment { ptr: buf, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation expr type",
                                at: expr.span.into(),
                            });
                        }
                    }
                }
            }
        }

        // 3) 为拼接结果分配 buffer，并按顺序 memcpy 各段。
        //
        // 注意：`alloca [0 x i8]` 在某些目标上会导致奇怪的 IR/后端行为；这里保证至少分配 1 byte。
        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            total_len,
            i64_ty.const_zero(),
            "fstr_total_is_zero",
        )?;
        let alloc_len = self
            .builder
            .build_select(
                is_zero,
                i64_ty.const_int(1, false),
                total_len,
                "fstr_alloc_len",
            )?
            .into_int_value();

        let buf = self
            .builder
            .build_array_alloca(i8_ty, alloc_len, "fstr_buf")?;

        let mut cursor = i64_ty.const_zero();
        for (idx, seg) in segments.iter().enumerate() {
            let dst = unsafe {
                self.builder.build_in_bounds_gep(
                    i8_ty,
                    buf,
                    &[cursor],
                    &format!("fstr_dst_{idx}"),
                )?
            };
            let _ = self.builder.build_memcpy(dst, 1, seg.ptr, 1, seg.len)?;
            cursor = self.builder.build_int_add(cursor, seg.len, "fstr_cursor")?;
        }

        // 4) 写回 `ScoopString { len, data }` 并返回其指针。
        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 0, "fstr_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "fstr_data_gep")?;

        let _ = self.builder.build_store(len_ptr, total_len)?;
        let _ = self.builder.build_store(data_ptr, buf)?;

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn codegen_var_ref(
        &mut self,
        span: crate::span::Span,
        v: &hir::ValueRef,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let hir::ValueRef::Local { id, .. } = v else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level value ref",
                at: span.into(),
            });
        };

        let local = self
            .env
            .get(*id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown local value",
                at: span.into(),
            })?;

        match local.ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool => {
                let raw = self
                    .builder
                    .build_load(
                        self.llvm_basic_type_of(span, local.ty)?,
                        local.ptr,
                        "load_bool",
                    )?
                    .into_int_value();
                Ok(CgValue::bool(raw))
            }
            CgTy::Int(int_ty) => {
                let raw = self
                    .builder
                    .build_load(
                        self.llvm_basic_type_of(span, local.ty)?,
                        local.ptr,
                        "load_int",
                    )?
                    .into_int_value();
                Ok(CgValue::int(raw, int_ty))
            }
            CgTy::String => {
                let raw = self
                    .builder
                    .build_load(
                        self.llvm_basic_type_of(span, local.ty)?,
                        local.ptr,
                        "load_str",
                    )?
                    .into_pointer_value();
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(raw.into()),
                })
            }
            CgTy::Ref => {
                let raw = self
                    .builder
                    .build_load(
                        self.llvm_basic_type_of(span, local.ty)?,
                        local.ptr,
                        "load_ref",
                    )?
                    .into_pointer_value();
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(raw.into()),
                })
            }
            CgTy::Tuple(_) => {
                let raw = self.builder.build_load(
                    self.llvm_basic_type_of(span, local.ty)?,
                    local.ptr,
                    "load_tuple",
                )?;
                Ok(CgValue {
                    ty: local.ty,
                    value: Some(raw),
                })
            }
            CgTy::Struct(_) => {
                let raw = self.builder.build_load(
                    self.llvm_basic_type_of(span, local.ty)?,
                    local.ptr,
                    "load_struct",
                )?;
                Ok(CgValue {
                    ty: local.ty,
                    value: Some(raw),
                })
            }
            CgTy::Enum(_) => {
                let raw = self.builder.build_load(
                    self.llvm_basic_type_of(span, local.ty)?,
                    local.ptr,
                    "load_enum",
                )?;
                Ok(CgValue {
                    ty: local.ty,
                    value: Some(raw),
                })
            }
        }
    }

    fn codegen_struct_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        fields: &[hir::StructLitField],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Struct(struct_ty)) = self.cg_ty_of(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal type",
                at: span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal type",
                at: span.into(),
            });
        };

        let layout =
            self.struct_layouts
                .get(&nominal.fqn)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct literal layout",
                    at: span.into(),
                })?;

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();

        for (idx, field) in layout.fields.iter().enumerate() {
            let Some(init) = fields.iter().find(|f| f.name == field.name) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct literal missing field",
                    at: span.into(),
                });
            };

            let field_cg = self.cg_ty_of_type_fqn(init.span, field.ty_fqn.as_deref())?;

            let init_v = self.codegen_expr(&init.value)?;
            let coerced = self.coerce_value(init.value.span, init_v, field_cg)?;

            let raw = match field_cg {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct field value",
                    at: init.value.span.into(),
                })?,
            };

            let name = format!("insert_{}", field.name);
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Struct(struct_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_tuple_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        elements: &[hir::Expr],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Tuple(tuple_ty)) = self.cg_ty_of(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal type",
                at: span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal type",
                at: span.into(),
            });
        };

        if element_tys.len() != elements.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal arity mismatch",
                at: span.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();

        for (idx, (elem_expr, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = self
                .cg_ty_of(*elem_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element type",
                    at: elem_expr.span.into(),
                })?;

            let elem_v = self.codegen_expr(elem_expr)?;
            let coerced = self.coerce_value(elem_expr.span, elem_v, elem_cg)?;

            let raw: BasicValueEnum<'ctx> = match elem_cg {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element value",
                    at: elem_expr.span.into(),
                })?,
            };

            let name = format!("insert_elem_{idx}");
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Tuple(tuple_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_member_access(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match member.resolved.as_ref() {
            Some(hir::MemberRef::Value { fqn, .. }) => {
                // T0828：`object` / `companion object` 静态成员访问（backing field 读取）。
                if self.lookup_object_property_by_fqn(fqn).is_some() {
                    return self.codegen_object_property_access(member.span, fqn);
                }

                // 优先路径：`localStruct.field` —— 用 GEP 从 alloca slot 取字段（更贴近后续可变字段语义）。
                if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind {
                    if let Some(local) = self.env.get(*id) {
                        if let CgTy::Struct(struct_ty) = local.ty {
                            let (field_idx, field_ty) =
                                self.lookup_struct_field(struct_ty, fqn, member.span)?;
                            if field_ty == CgTy::Unit {
                                return Ok(CgValue::unit());
                            }

                            let llvm_struct_ty = self.llvm_struct_type(member.span, struct_ty)?;
                            let field_ptr = self.builder.build_struct_gep(
                                llvm_struct_ty,
                                local.ptr,
                                field_idx,
                                "field_gep",
                            )?;
                            let llvm_field_ty = self.llvm_basic_type_of(member.span, field_ty)?;
                            let loaded =
                                self.builder
                                    .build_load(llvm_field_ty, field_ptr, "load_field")?;
                            return self.cg_value_from_loaded(member.span, field_ty, loaded);
                        }
                    }
                }

                // fallback：先把 receiver 降到值，再用 extractvalue 取字段。
                let recv = self.codegen_expr(receiver)?;
                let CgTy::Struct(struct_ty) = recv.ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "member access receiver type",
                        at: receiver.span.into(),
                    });
                };
                let (field_idx, field_ty) =
                    self.lookup_struct_field(struct_ty, fqn, member.span)?;
                if field_ty == CgTy::Unit {
                    return Ok(CgValue::unit());
                }

                let raw = recv.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "member access receiver value",
                    at: receiver.span.into(),
                })?;
                let struct_v = raw.into_struct_value();
                let extracted =
                    self.builder
                        .build_extract_value(struct_v, field_idx, "extract_field")?;
                return self.cg_value_from_loaded(member.span, field_ty, extracted);
            }
            Some(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "member access target",
                    at: member.span.into(),
                });
            }
            None => {}
        }

        // tuple 元素访问（spec §2.3.3）：`t._0` / `t._1` / ...
        let Some(elem_idx) = parse_tuple_member_index(&member.name) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access target",
                at: member.span.into(),
            });
        };

        // 优先路径：`localTuple._0` —— 用 GEP 从 alloca slot 取元素。
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind {
            if let Some(local) = self.env.get(*id) {
                if let CgTy::Tuple(tuple_ty) = local.ty {
                    let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
                    if elem_ty == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let llvm_tuple_ty = self.llvm_tuple_type(member.span, tuple_ty)?;
                    let elem_ptr = self.builder.build_struct_gep(
                        llvm_tuple_ty,
                        local.ptr,
                        elem_idx,
                        "tuple_elem_gep",
                    )?;
                    let llvm_elem_ty = self.llvm_basic_type_of(member.span, elem_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_elem_ty, elem_ptr, "load_tuple_elem")?;
                    return self.cg_value_from_loaded(member.span, elem_ty, loaded);
                }
            }
        }

        // fallback：先把 receiver 降到值，再用 extractvalue 取元素。
        let recv = self.codegen_expr(receiver)?;
        let CgTy::Tuple(tuple_ty) = recv.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access receiver type",
                at: receiver.span.into(),
            });
        };

        let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let raw = recv.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "member access receiver value",
            at: receiver.span.into(),
        })?;
        let tuple_v = raw.into_struct_value();
        let extracted =
            self.builder
                .build_extract_value(tuple_v, elem_idx, "extract_tuple_elem")?;
        self.cg_value_from_loaded(member.span, elem_ty, extracted)
    }

    fn lookup_object_property_by_fqn(
        &self,
        prop_fqn: &str,
    ) -> Option<(&hir::ObjectInit, &hir::ObjectProperty)> {
        let (owner, name) = prop_fqn.rsplit_once('.')?;
        let obj = self.object_inits.get(owner)?;
        let prop = obj.properties.get(name)?;
        Some((obj, prop))
    }

    fn codegen_object_property_access(
        &mut self,
        at: crate::span::Span,
        prop_fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (object_fqn, prop) = match self.lookup_object_property_by_fqn(prop_fqn) {
            Some((obj, prop)) => (obj.fqn.clone(), prop.clone()),
            None => {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "object property access (missing metadata)",
                at: at.into(),
            });
            }
        };

        if !prop.has_init {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "object property without initializer",
                at: at.into(),
            });
        }

        let prop_cg =
            self.cg_ty_of(prop.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "object property type",
                    at: at.into(),
                })?;

        let init_fn = self.ensure_object_init_function_defined(&object_fqn)?;
        let _ = self.builder.build_call(init_fn, &[], "obj_init")?;

        if prop_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let Some(global) = self.declare_object_property_global(at, prop_fqn, prop_cg)? else {
            return Ok(CgValue::unit());
        };
        let llvm_ty = self.llvm_basic_type_of(at, prop_cg)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, global.as_pointer_value(), "load_obj_prop")?;
        self.cg_value_from_loaded(at, prop_cg, loaded)
    }

    fn ensure_object_init_function_defined(
        &mut self,
        object_fqn: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let Some(obj) = self.object_inits.get(object_fqn) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "object init (missing metadata)",
                at: crate::span::Span::new(0, 0).into(),
            });
        };

        let name = object_init_fn_name(object_fqn);
        let fn_ty = self.context.void_type().fn_type(&[], false);

        let llvm_fun = self
            .module
            .get_function(&name)
            .unwrap_or_else(|| self.module.add_function(&name, fn_ty, None));

        // 已有 body：无需重复生成。
        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        // 在生成 init function body 时，临时切换 builder 的插入点；结束后恢复到调用方位置。
        let saved_block = self.builder.get_insert_block();

        let mut init_codegen = MainCodegen::new(
            self.context,
            self.module,
            self.builder,
            self.host,
            self.source,
            self.types,
            self.struct_layouts,
            self.enum_layouts,
            self.object_inits,
            self.fun_index,
        );
        init_codegen.codegen_object_init_fun_body(obj, llvm_fun)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(llvm_fun)
    }

    fn codegen_object_init_fun_body(
        &mut self,
        obj: &hir::ObjectInit,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let entry = self.context.append_basic_block(llvm_fun, "entry");
        let init_bb = self.context.append_basic_block(llvm_fun, "init");
        let done_bb = self.context.append_basic_block(llvm_fun, "done");

        self.builder.position_at_end(entry);

        let guard = self.declare_object_init_guard(&obj.fqn);
        let guard_val = self
            .builder
            .build_load(self.context.bool_type(), guard.as_pointer_value(), "load_guard")?
            .into_int_value();
        self.builder
            .build_conditional_branch(guard_val, done_bb, init_bb)?;

        self.builder.position_at_end(init_bb);

        // 单线程最小语义：先写入 guard，避免递归初始化导致的重复执行。
        let _ = self
            .builder
            .build_store(guard.as_pointer_value(), self.context.bool_type().const_int(1, false))?;

        self.env.push_scope();
        for step in &obj.steps {
            match step {
                hir::ObjectInitStep::PropertyInit { name, init } => {
                    let Some(prop) = obj.properties.get(name) else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "object property init (missing property)",
                            at: init.span.into(),
                        });
                    };

                    let prop_cg =
                        self.cg_ty_of(prop.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "object property init type",
                                at: init.span.into(),
                            })?;

                    let v = self.codegen_expr_in_expected_context(init, Some(prop_cg))?;

                    // Unit：只执行副作用即可，无需 backing storage。
                    if prop_cg != CgTy::Unit {
                        let prop_fqn = format!("{}.{}", obj.fqn, name);
                        let Some(global) =
                            self.declare_object_property_global(init.span, &prop_fqn, prop_cg)?
                        else {
                            continue;
                        };
                        self.store_local_value(init.span, global.as_pointer_value(), prop_cg, v)?;
                    }
                }
                hir::ObjectInitStep::InitBlock { block } => {
                    let _ = self.codegen_block_value(block)?;
                }
            }
        }
        self.env.pop_scope();

        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        self.builder.build_return(None)?;
        Ok(())
    }

    fn declare_object_init_guard(&self, object_fqn: &str) -> GlobalValue<'ctx> {
        let name = object_guard_global_name(object_fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        let gv = self.module.add_global(self.context.bool_type(), None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.context.bool_type().const_int(0, false));
        gv
    }

    fn declare_object_property_global(
        &mut self,
        at: crate::span::Span,
        prop_fqn: &str,
        prop_cg: CgTy,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        if prop_cg == CgTy::Unit {
            return Ok(None);
        }

        let name = object_prop_global_name(prop_fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(Some(existing));
        }

        let llvm_ty = self.llvm_basic_type_of(at, prop_cg)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);

        let init: BasicValueEnum<'ctx> = match llvm_ty {
            BasicTypeEnum::IntType(ty) => BasicValueEnum::IntValue(ty.const_int(0, false)),
            BasicTypeEnum::PointerType(ty) => BasicValueEnum::PointerValue(ty.const_null()),
            BasicTypeEnum::StructType(ty) => BasicValueEnum::StructValue(ty.const_zero()),
            BasicTypeEnum::ArrayType(ty) => BasicValueEnum::ArrayValue(ty.const_zero()),
            BasicTypeEnum::FloatType(ty) => BasicValueEnum::FloatValue(ty.const_float(0.0)),
            BasicTypeEnum::VectorType(ty) => BasicValueEnum::VectorValue(ty.const_zero()),
            BasicTypeEnum::ScalableVectorType(ty) => {
                BasicValueEnum::ScalableVectorValue(ty.const_zero())
            }
        };
        gv.set_initializer(&init);
        Ok(Some(gv))
    }

    fn lookup_struct_field(
        &self,
        struct_ty: TypeId,
        field_fqn: &str,
        at: crate::span::Span,
    ) -> Result<(u32, CgTy), LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct type id",
                at: at.into(),
            });
        };

        let layout =
            self.struct_layouts
                .get(&nominal.fqn)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct layout",
                    at: at.into(),
                })?;

        let idx = layout
            .fields
            .iter()
            .position(|f| f.fqn == field_fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown struct field",
                at: at.into(),
            })?;

        let field = &layout.fields[idx];
        let field_ty = self.cg_ty_of_type_fqn(field.span, field.ty_fqn.as_deref())?;
        Ok((idx as u32, field_ty))
    }

    fn lookup_tuple_element(
        &self,
        tuple_ty: TypeId,
        elem_idx: u32,
        at: crate::span::Span,
    ) -> Result<CgTy, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let elem_ty =
            elements
                .get(elem_idx as usize)
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element out of bounds",
                    at: at.into(),
                })?;

        self.cg_ty_of(elem_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple element type",
                at: at.into(),
            })
    }

    fn cg_value_from_loaded(
        &self,
        _at: crate::span::Span,
        ty: CgTy,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Ok(match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(raw.into_int_value()),
            CgTy::Int(int_ty) => CgValue::int(raw.into_int_value(), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Tuple(tuple_ty) => CgValue {
                ty: CgTy::Tuple(tuple_ty),
                value: Some(raw),
            },
            CgTy::Struct(struct_ty) => CgValue {
                ty: CgTy::Struct(struct_ty),
                value: Some(raw),
            },
            CgTy::Enum(enum_ty) => CgValue {
                ty: CgTy::Enum(enum_ty),
                value: Some(raw),
            },
        })
    }

    fn codegen_unary(
        &mut self,
        span: crate::span::Span,
        op: ast::UnaryOp,
        expr: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::UnaryOp::Not => {
                let v = self.codegen_expr(expr)?.as_bool().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary ! operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_not(v, "not")?;
                Ok(CgValue::bool(out))
            }
            ast::UnaryOp::Neg => {
                let (v, ty) = self.codegen_expr(expr)?.as_int().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary - operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_int_neg(v, "neg")?;
                Ok(CgValue::int(out, ty))
            }
            ast::UnaryOp::BitNot => {
                let (v, ty) = self.codegen_expr(expr)?.as_int().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary ~ operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_not(v, "bitnot")?;
                Ok(CgValue::int(out, ty))
            }
        }
    }

    fn codegen_binary(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => self.codegen_int_binary_same_type(span, op, lhs, rhs),

            ast::BinaryOp::Shl | ast::BinaryOp::Shr => self.codegen_shift(span, op, lhs, rhs),

            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                self.codegen_int_compare(span, op, lhs, rhs)
            }

            ast::BinaryOp::Eq | ast::BinaryOp::Ne => self.codegen_equality(span, op, lhs, rhs),

            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                self.codegen_bool_logic(span, op, lhs, rhs)
            }

            ast::BinaryOp::Elvis => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "elvis operator",
                at: span.into(),
            }),
        }
    }

    fn codegen_int_binary_same_type(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op rhs",
                    at: span.into(),
                })?;

        let out_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "integer binary op type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, out_ty)?;
        let r = self.cast_int(r_raw, r_ty, out_ty)?;

        let out = match op {
            ast::BinaryOp::Add => self.builder.build_int_add(l, r, "add")?,
            ast::BinaryOp::Sub => self.builder.build_int_sub(l, r, "sub")?,
            ast::BinaryOp::Mul => self.builder.build_int_mul(l, r, "mul")?,
            ast::BinaryOp::Div => {
                if out_ty.signed {
                    self.builder.build_int_signed_div(l, r, "sdiv")?
                } else {
                    self.builder.build_int_unsigned_div(l, r, "udiv")?
                }
            }
            ast::BinaryOp::Rem => {
                if out_ty.signed {
                    self.builder.build_int_signed_rem(l, r, "srem")?
                } else {
                    self.builder.build_int_unsigned_rem(l, r, "urem")?
                }
            }
            ast::BinaryOp::BitAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::BitXor => self.builder.build_xor(l, r, "xor")?,
            ast::BinaryOp::BitOr => self.builder.build_or(l, r, "or")?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, out_ty))
    }

    fn codegen_shift(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (lhs_value, lhs_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift lhs type",
                    at: span.into(),
                })?;

        let rhs_value =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift rhs type",
                    at: span.into(),
                })?;

        let shift_count = self.mask_shift_count(lhs_ty, rhs_value.0)?;

        let out = match op {
            ast::BinaryOp::Shl => self
                .builder
                .build_left_shift(lhs_value, shift_count, "shl")?,
            ast::BinaryOp::Shr => {
                self.builder
                    .build_right_shift(lhs_value, shift_count, lhs_ty.signed, "shr")?
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, lhs_ty))
    }

    fn mask_shift_count(
        &mut self,
        lhs_ty: IntTy,
        rhs: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let lhs_bits = lhs_ty.bits;
        let lhs_int = self.int_type(lhs_ty);

        // 1) 截断为 lhs 的位宽（只取低位，后续再 mask）。
        let rhs_trunc = self
            .builder
            .build_int_truncate(rhs, lhs_int, "shift_rhs_trunc")?;

        // 2) mask：shiftCount & (bitWidth - 1)，避免 LLVM 对“超范围 shift”的 UB。
        let mask = lhs_int.const_int((lhs_bits.saturating_sub(1)) as u64, false);
        Ok(self.builder.build_and(rhs_trunc, mask, "shift_masked")?)
    }

    fn codegen_int_compare(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison rhs",
                    at: span.into(),
                })?;

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "comparison operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match (op, int_ty.signed) {
            (ast::BinaryOp::Lt, true) => IntPredicate::SLT,
            (ast::BinaryOp::Lt, false) => IntPredicate::ULT,
            (ast::BinaryOp::Le, true) => IntPredicate::SLE,
            (ast::BinaryOp::Le, false) => IntPredicate::ULE,
            (ast::BinaryOp::Gt, true) => IntPredicate::SGT,
            (ast::BinaryOp::Gt, false) => IntPredicate::UGT,
            (ast::BinaryOp::Ge, true) => IntPredicate::SGE,
            (ast::BinaryOp::Ge, false) => IntPredicate::UGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp")?,
        ))
    }

    fn codegen_equality(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let lhs_v = self.codegen_expr(lhs)?;
        let rhs_v = self.codegen_expr(rhs)?;

        // Bool == Bool
        if matches!((lhs_v.ty, rhs_v.ty), (CgTy::Bool, CgTy::Bool)) {
            let l = lhs_v.as_bool().unwrap();
            let r = rhs_v.as_bool().unwrap();
            let pred = match op {
                ast::BinaryOp::Eq => IntPredicate::EQ,
                ast::BinaryOp::Ne => IntPredicate::NE,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(self.builder.build_int_compare(
                pred,
                l,
                r,
                "icmp_bool",
            )?));
        }

        // Int == Int（含 int literal 吸收）
        let Some((l_raw, l_ty)) = lhs_v.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "equality lhs",
                at: span.into(),
            });
        };
        let Some((r_raw, r_ty)) = rhs_v.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "equality rhs",
                at: span.into(),
            });
        };

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "equality operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match op {
            ast::BinaryOp::Eq => IntPredicate::EQ,
            ast::BinaryOp::Ne => IntPredicate::NE,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp_eq")?,
        ))
    }

    fn codegen_bool_logic(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let l = self
            .codegen_expr(lhs)?
            .as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bool operator lhs",
                at: span.into(),
            })?;
        let r = self
            .codegen_expr(rhs)?
            .as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bool operator rhs",
                at: span.into(),
            })?;

        let out = match op {
            ast::BinaryOp::LogAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::LogOr => self.builder.build_or(l, r, "or")?,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(out))
    }

    fn coerce_value(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match (value.ty, target) {
            (CgTy::Unit, CgTy::Unit) => Ok(CgValue::unit()),
            (CgTy::Bool, CgTy::Bool) => Ok(value),
            (CgTy::Bool, CgTy::Int(int_ty)) => {
                let v = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "bool value",
                    at: at.into(),
                })?;
                let out =
                    self.builder
                        .build_int_z_extend(v, self.int_type(int_ty), "bool_to_int")?;
                Ok(CgValue::int(out, int_ty))
            }
            (CgTy::Int(from), CgTy::Int(to)) => {
                let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "int value",
                    at: at.into(),
                })?;
                let out = self.cast_int(v, from, to)?;
                Ok(CgValue::int(out, to))
            }
            (CgTy::String, CgTy::String) => Ok(value),
            (CgTy::String, CgTy::Ref) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "string value",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "string value type",
                        at: at.into(),
                    });
                };
                let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
                let casted = self
                    .builder
                    .build_pointer_cast(ptr, i8_ptr_ty, "coerce_str_to_ref")?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(casted.into()),
                })
            }
            (CgTy::Ref, CgTy::Ref) => Ok(value),
            (CgTy::Int(_), CgTy::Ref) => {
                // T0817：值类型装箱到 `Any`（当前阶段先只支持整数族）。
                let (raw_int, from_ty) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "int value",
                    at: at.into(),
                })?;
                let boxed = self.codegen_box_int_to_ref(at, raw_int, from_ty)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Tuple(from), CgTy::Tuple(to)) if from == to => Ok(value),
            (CgTy::Struct(from), CgTy::Struct(to)) if from == to => Ok(value),
            (CgTy::Enum(from), CgTy::Enum(to)) if from == to => Ok(value),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value coercion",
                at: at.into(),
            }),
        }
    }

    fn codegen_box_int_to_ref(
        &mut self,
        at: crate::span::Span,
        value: IntValue<'ctx>,
        value_ty: IntTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        // 约定（early stage）：
        // - box 对象布局：`{ type_desc: i8*, payload: <int> }`
        // - 目前 type_desc 先写 NULL；后续会接入 type descriptor 与 GC（T0907+）。
        //
        // 注意：这里不尝试做“复用 box 类型”或 cache；LLVM named struct 会在 module 内复用。
        let target = self.target_layout();
        let payload_size = (u64::from(value_ty.bits) + 7) / 8;
        let payload_align = payload_size.clamp(1, target.pointer_align.max(1));

        let header_size = target.pointer_size;
        let header_align = target.pointer_align.max(1);
        let payload_offset = align_to(header_size, payload_align);
        let obj_align = header_align.max(payload_align);
        let total_size = align_to(payload_offset.saturating_add(payload_size), obj_align);

        let rt_alloc = self.declare_runtime_alloc();
        let size_v = self
            .context
            .i64_type()
            .const_int(total_size as u64, false);
        let call = self
            .builder
            .build_call(rt_alloc, &[size_v.into()], "rt_alloc_box")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: at.into(),
            });
        };

        // 写入对象头与 payload（type_desc 先为 NULL）。
        let boxed_ty = self.llvm_boxed_int_type(value_ty);
        let boxed_ptr_ty = boxed_ty.ptr_type(AddressSpace::default());
        let boxed_ptr =
            self.builder
                .build_pointer_cast(raw_ptr, boxed_ptr_ty, "boxed_int_ptr")?;

        let type_desc_ptr =
            self.builder
                .build_struct_gep(boxed_ty, boxed_ptr, 0, "boxed_type_desc_gep")?;
        let payload_ptr =
            self.builder
                .build_struct_gep(boxed_ty, boxed_ptr, 1, "boxed_payload_gep")?;

        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let null_desc = i8_ptr_ty.const_null();
        let _ = self.builder.build_store(type_desc_ptr, null_desc)?;
        let _ = self.builder.build_store(payload_ptr, value)?;

        Ok(raw_ptr)
    }

    fn cast_int(
        &mut self,
        value: IntValue<'ctx>,
        from: IntTy,
        to: IntTy,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        if from.bits == to.bits {
            return Ok(value);
        }

        let to_ty = self.int_type(to);
        if to.bits > from.bits {
            if from.signed {
                Ok(self.builder.build_int_s_extend(value, to_ty, "sext")?)
            } else {
                Ok(self.builder.build_int_z_extend(value, to_ty, "zext")?)
            }
        } else {
            Ok(self.builder.build_int_truncate(value, to_ty, "trunc")?)
        }
    }

    fn coerce_exit_code(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();

        match value.ty {
            CgTy::Unit => Ok(i32_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "exit bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i32_ty, "exit_bool")?)
            }
            CgTy::Int(int_ty) => {
                let (v, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "exit int",
                    at: at.into(),
                })?;
                let to = IntTy {
                    bits: 32,
                    signed: int_ty.signed,
                };
                let casted = self.cast_int(v, from, to)?;
                Ok(casted)
            }
            CgTy::String => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "string exit code",
                at: at.into(),
            }),
            CgTy::Ref => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "ref exit code",
                at: at.into(),
            }),
            CgTy::Tuple(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple exit code",
                at: at.into(),
            }),
            CgTy::Struct(_) | CgTy::Enum(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "composite exit code",
                at: at.into(),
            }),
        }
    }

    fn cg_ty_of(&self, ty: TypeId) -> Option<CgTy> {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::String) => Some(CgTy::String),
            TypeKind::Ref(_) => Some(CgTy::Ref),
            TypeKind::Value(ValueTypeKind::Unit) => Some(CgTy::Unit),
            TypeKind::Value(ValueTypeKind::Bool) => Some(CgTy::Bool),
            TypeKind::Value(ValueTypeKind::Int) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UInt) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::Option(_)) => Some(CgTy::Enum(ty)),
            TypeKind::Value(ValueTypeKind::Tuple(_)) => Some(CgTy::Tuple(ty)),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                if self.struct_layouts.contains_key(&nominal.fqn) {
                    return Some(CgTy::Struct(ty));
                }
                if self.enum_layouts.contains_key(&nominal.fqn) {
                    return Some(CgTy::Enum(ty));
                }
                None
            }
            _ => None,
        }
    }

    fn cg_ty_of_type_fqn(
        &self,
        at: crate::span::Span,
        ty_fqn: Option<&str>,
    ) -> Result<CgTy, LlvmEmitError> {
        let Some(ty_fqn) = ty_fqn else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct field type",
                at: at.into(),
            });
        };

        match ty_fqn {
            "scoop.core.Unit" => Ok(CgTy::Unit),
            "scoop.core.Bool" => Ok(CgTy::Bool),
            "scoop.core.String" => Ok(CgTy::String),
            "scoop.core.Int" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            "scoop.core.UInt" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            other => {
                // 固定位宽整数族（与 HIR lowering 的 special-case 规则对齐）。
                if let Some(bits) = other
                    .strip_prefix("scoop.core.Int")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    return Ok(CgTy::Int(IntTy { bits, signed: true }));
                }
                if let Some(bits) = other
                    .strip_prefix("scoop.core.UInt")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    return Ok(CgTy::Int(IntTy {
                        bits,
                        signed: false,
                    }));
                }

                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct field type",
                    at: at.into(),
                })
            }
        }
    }

    fn int_type(&self, ty: IntTy) -> IntType<'ctx> {
        self.context.custom_width_int_type(ty.bits)
    }

    fn llvm_scoop_string_type(&self) -> StructType<'ctx> {
        // 说明：该类型名用于 LLVM module 内部复用，不应与用户类型冲突（使用 runtime 命名空间前缀）。
        const TY_NAME: &str = "scoop.runtime.ScoopString";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct { uint64_t len; const uint8_t *data; } ScoopString;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let len_ty = self.context.i64_type();
        let data_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        ty.set_body(&[len_ty.into(), data_ty.into()], false);
        ty
    }

    fn llvm_scoop_string_ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.llvm_scoop_string_type()
            .ptr_type(AddressSpace::default())
    }

    fn llvm_boxed_int_type(&self, payload: IntTy) -> StructType<'ctx> {
        // 说明：box 类型目前只用于 `Int/UInt/... -> Any` 的最小装箱（T0817）。
        // 未来会扩展为统一的对象头 + type descriptor（T0907+）。
        let name = format!(
            "scoop.runtime.BoxedInt{}_{}",
            payload.bits,
            if payload.signed { "i" } else { "u" }
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            return existing;
        }

        // `{ i8* type_desc, <int> payload }`
        let ty = self.context.opaque_struct_type(&name);
        let type_desc_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        ty.set_body(&[type_desc_ty.into(), self.int_type(payload).into()], false);
        ty
    }

    fn declare_runtime_print_like(&self, name: &str) -> FunctionValue<'ctx> {
        if let Some(existing) = self.module.get_function(name) {
            return existing;
        }

        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] =
            [self.llvm_scoop_string_ptr_type().into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(name, fn_ty, None)
    }

    fn declare_runtime_format_int(&self, name: &str) -> FunctionValue<'ctx> {
        if let Some(existing) = self.module.get_function(name) {
            return existing;
        }

        // `uint64_t scoop_format_{i64,u64}(int64_t value, uint8_t* out, uint64_t cap)`
        //
        // 说明：
        // - 该函数用于 f-string 插值 `{Int}` 的最小 formatting（TODO T0823）；
        // - 由 runtime 实现，避免在 LLVM IR 中直接引入 varargs `snprintf` 调用。
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i64_ty.into(), i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(name, fn_ty, None)
    }

    fn declare_runtime_trim_indent(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_string_trim_indent";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_string_trim_indent(const ScoopString* value)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_alloc(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_alloc";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void *scoop_alloc(uint64_t size)`
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn get_or_create_global_bytes(
        &self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> GlobalValue<'ctx> {
        let name = format!("__scoop_str_data_{}_{}", span.start, span.end);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        let arr_ty = self.context.i8_type().array_type(bytes.len() as u32);
        let gv = self.module.add_global(arr_ty, None, &name);
        let init = self.context.const_string(bytes, false);
        gv.set_initializer(&init);
        gv.set_constant(true);
        gv
    }

    fn llvm_basic_type_of(
        &mut self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        Ok(match ty {
            // 说明：Unit 没有运行期值；当前阶段仅用于“可放入 alloca”与保持 load/store 管线统一。
            CgTy::Unit => self.context.i8_type().into(),
            CgTy::Bool => self.context.bool_type().into(),
            CgTy::Int(int_ty) => self.int_type(int_ty).into(),
            CgTy::String => self.llvm_scoop_string_ptr_type().into(),
            CgTy::Ref => self
                .context
                .i8_type()
                .ptr_type(AddressSpace::default())
                .into(),
            CgTy::Tuple(tuple_ty) => self.llvm_tuple_type(at, tuple_ty)?.into(),
            CgTy::Struct(struct_ty) => self.llvm_struct_type(at, struct_ty)?.into(),
            CgTy::Enum(enum_ty) => self.llvm_enum_value_type(at, enum_ty)?,
        })
    }

    fn llvm_struct_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct type id",
                at: at.into(),
            });
        };

        let layout =
            self.struct_layouts
                .get(&nominal.fqn)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct layout",
                    at: at.into(),
                })?;

        if let Some(existing) = self.context.get_struct_type(&layout.fqn) {
            return Ok(existing);
        }

        let struct_ty = self.context.opaque_struct_type(&layout.fqn);

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(layout.fields.len());
        for field in &layout.fields {
            let field_cg = self.cg_ty_of_type_fqn(field.span, field.ty_fqn.as_deref())?;
            llvm_fields.push(self.llvm_basic_type_of(field.span, field_cg)?);
        }

        struct_ty.set_body(&llvm_fields, false);
        Ok(struct_ty)
    }

    fn llvm_enum_value_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        // 注意：避免持有 `cg_enum_layout(...)` 返回的引用跨越后续 `&mut self` 调用。
        let (repr, some_field) = {
            let cg_layout = self.cg_enum_layout(at, ty)?;
            let repr = cg_layout.repr;
            let some_field = cg_layout
                .variants
                .iter()
                .find(|v| v.name == "Some")
                .and_then(|v| v.fields.first())
                .copied();
            (repr, some_field)
        };

        match repr {
            CgEnumRepr::TaggedUnion => {
                let fqn = match self.types.kind(ty) {
                    TypeKind::Value(ValueTypeKind::Option(_)) => "scoop.core.Option",
                    TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn.as_str(),
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "enum type id",
                            at: at.into(),
                        });
                    }
                };

                if let Some(existing) = self.context.get_struct_type(fqn) {
                    return Ok(existing.into());
                }

                // 最小 rich enum 表示：`{ tag: i32, payload: iN }`
                // - tag：按声明顺序分配的 variant id
                // - payload：当前阶段用 machine word 承载 payload 或 boxed payload 指针
                let enum_ty = self.context.opaque_struct_type(fqn);
                let tag_ty = self.context.i32_type();
                let payload_ty = self.int_type(IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                });
                enum_ty.set_body(&[tag_ty.into(), payload_ty.into()], false);
                Ok(enum_ty.into())
            }
            CgEnumRepr::Niche { storage, .. } => match storage {
                NicheStorage::Pointer => {
                    let some_field = some_field.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "Option niche payload type",
                        at: at.into(),
                    })?;
                    Ok(self.llvm_basic_type_of(at, some_field)?)
                }
                NicheStorage::U8 => Ok(self.context.i8_type().into()),
            },
        }
    }

    fn llvm_tuple_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(elements.len());
        for elem_ty in elements {
            let elem_cg = self
                .cg_ty_of(*elem_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element type",
                    at: at.into(),
                })?;
            llvm_fields.push(self.llvm_basic_type_of(at, elem_cg)?);
        }

        Ok(self.context.struct_type(&llvm_fields, false))
    }

    fn llvm_enum_boxed_payload_struct_type(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        variant: &CgEnumVariant,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let enum_fqn = match self.types.kind(enum_ty) {
            TypeKind::Value(ValueTypeKind::Option(_)) => "scoop.core.Option",
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn.as_str(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum boxed payload type",
                    at: at.into(),
                });
            }
        };

        // 说明：boxed payload 在运行期是一个独立的聚合对象；当前阶段用一个具名 LLVM struct 承载其字段布局，
        // 以便 ctor/binder 双方对齐类型（避免 bitcast 到不一致的匿名 struct）。
        let name = format!(
            "scoop_boxed_payload_{}_{}",
            sanitize_llvm_ident(enum_fqn),
            sanitize_llvm_ident(&variant.name)
        );

        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }

        let payload_ty = self.context.opaque_struct_type(&name);
        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(variant.fields.len());
        for &field_cg in &variant.fields {
            llvm_fields.push(self.llvm_basic_type_of(at, field_cg)?);
        }
        payload_ty.set_body(&llvm_fields, false);
        Ok(payload_ty)
    }

    fn create_entry_alloca(
        &mut self,
        at: crate::span::Span,
        name: &str,
        ty: CgTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let alloca_ty = self.llvm_basic_type_of(at, ty)?;
        self.create_entry_alloca_raw(at, name, alloca_ty)
    }

    fn create_entry_alloca_raw(
        &self,
        at: crate::span::Span,
        name: &str,
        alloca_ty: BasicTypeEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let alloca_builder = self.context.create_builder();
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;
        let entry = func
            .get_first_basic_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function has no entry block",
                at: at.into(),
            })?;

        match entry.get_first_instruction() {
            Some(inst) => alloca_builder.position_before(&inst),
            None => alloca_builder.position_at_end(entry),
        }

        Ok(alloca_builder.build_alloca(alloca_ty, name)?)
    }

    fn store_local_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        // 说明：当前阶段 locals 允许：
        // - 标量：`Unit/Bool/Int*`
        // - struct/enum（值类型）：以 LLVM struct by-value 形式存入栈 slot（`alloca`）
        let v = self.coerce_value(at, value, ty)?;
        match ty {
            CgTy::Unit => {
                let zero = self.context.i8_type().const_int(0, false);
                let _ = self.builder.build_store(ptr, zero)?;
            }
            CgTy::Bool
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let Some(raw) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "store value",
                        at: at.into(),
                    });
                };
                let _ = self.builder.build_store(ptr, raw)?;
            }
        }
        Ok(())
    }
}

fn object_init_fn_name(object_fqn: &str) -> String {
    format!("__scoop_object_init__{object_fqn}")
}

fn object_guard_global_name(object_fqn: &str) -> String {
    format!("__scoop_object_guard__{object_fqn}")
}

fn object_prop_global_name(prop_fqn: &str) -> String {
    format!("__scoop_object_prop__{prop_fqn}")
}

fn align_to(value: u64, align: u64) -> u64 {
    if align <= 1 {
        return value;
    }
    let mask = align - 1;
    (value + mask) & !mask
}

fn largest_two_sizes(layouts: &[TypeLayout]) -> (u64, u64) {
    let mut max = 0u64;
    let mut second = 0u64;
    for l in layouts {
        let s = l.size;
        if s >= max {
            second = max;
            max = s;
            continue;
        }
        if s > second {
            second = s;
        }
    }
    (max, second)
}

fn unify_int_types(
    lhs_is_lit: bool,
    lhs_ty: IntTy,
    rhs_is_lit: bool,
    rhs_ty: IntTy,
) -> Option<IntTy> {
    if lhs_ty == rhs_ty {
        return Some(lhs_ty);
    }
    if lhs_is_lit {
        return Some(rhs_ty);
    }
    if rhs_is_lit {
        return Some(lhs_ty);
    }
    None
}

fn parse_tuple_member_index(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
}

fn sanitize_llvm_ident(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_".to_string()
    } else {
        out
    }
}

fn parse_int_literal_decimal(text: &str) -> u128 {
    let mut out: u128 = 0;
    for ch in text.chars() {
        if ch == '_' {
            continue;
        }
        if let Some(d) = ch.to_digit(10) {
            out = out.saturating_mul(10).saturating_add(u128::from(d));
        }
    }
    out
}

fn mask_to_bits(value: u128, bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return value;
    }
    let mask = (1u128 << bits) - 1;
    value & mask
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StringLiteralParseError {
    Invalid,
    Interpolated,
}

fn parse_string_literal_bytes(text: &str) -> Result<Vec<u8>, StringLiteralParseError> {
    // f-string：留给 T0823 lowering；这里避免误把它当作普通字符串直接输出。
    if text.starts_with("f\"") || text.starts_with("f\"\"\"") {
        return Err(StringLiteralParseError::Interpolated);
    }

    // raw string：""" ... """
    if let Some(rest) = text.strip_prefix("\"\"\"") {
        let inner = rest
            .strip_suffix("\"\"\"")
            .ok_or(StringLiteralParseError::Invalid)?;
        return Ok(inner.as_bytes().to_vec());
    }

    // normal string：" ... "（支持最小转义）
    let inner = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .ok_or(StringLiteralParseError::Invalid)?;

    parse_normal_string_bytes(inner)
}

fn parse_normal_string_bytes(inner: &str) -> Result<Vec<u8>, StringLiteralParseError> {
    let mut out: Vec<u8> = Vec::with_capacity(inner.len());
    let mut chars = inner.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            out.extend_from_slice(s.as_bytes());
            continue;
        }

        let Some(esc) = chars.next() else {
            return Err(StringLiteralParseError::Invalid);
        };

        match esc {
            '\\' => out.push(b'\\'),
            '"' => out.push(b'"'),
            'n' => out.push(b'\n'),
            'r' => out.push(b'\r'),
            't' => out.push(b'\t'),
            '0' => out.push(b'\0'),
            // `\u{...}`（Kotlin-like，early stage）
            'u' => {
                let Some('{') = chars.next() else {
                    return Err(StringLiteralParseError::Invalid);
                };

                let mut hex = String::new();
                let mut closed = false;
                while let Some(c) = chars.next() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    hex.push(c);
                    if hex.len() > 6 {
                        return Err(StringLiteralParseError::Invalid);
                    }
                }

                if !closed || hex.is_empty() {
                    return Err(StringLiteralParseError::Invalid);
                }

                let cp =
                    u32::from_str_radix(&hex, 16).map_err(|_| StringLiteralParseError::Invalid)?;
                let Some(ch) = char::from_u32(cp) else {
                    return Err(StringLiteralParseError::Invalid);
                };

                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                out.extend_from_slice(s.as_bytes());
            }
            // fallback：保守策略——把未知转义当作“转义后字符本身”（便于早期跑通）。
            other => {
                let mut buf = [0u8; 4];
                let s = other.encode_utf8(&mut buf);
                out.extend_from_slice(s.as_bytes());
            }
        }
    }

    Ok(out)
}

fn parse_f_string_text_bytes(raw: bool, text: &str) -> Result<Vec<u8>, StringLiteralParseError> {
    // f-string 的 Text 片段来自 parser 拆分后的“内容区间 slice”，不包含包裹引号。
    // 这里需要补齐两类语义：
    // - `{{` / `}}`：字面量大括号（spec §8.2）；
    // - 非 raw f-string：支持最小转义（与普通字符串一致）。
    if raw {
        let undoubled = undouble_braces(text);
        return Ok(undoubled.into_bytes());
    }

    // 非 raw：先在源码层“去双大括号”，并避免把 `\u{...}` 的 `{}` 当作候选；
    // 再复用普通字符串的转义解析。
    let undoubled = undouble_braces_preserving_escapes(text);
    parse_normal_string_bytes(&undoubled)
}

fn undouble_braces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if matches!(chars.peek(), Some('{')) => {
                let _ = chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some('}')) => {
                let _ = chars.next();
                out.push('}');
            }
            _ => out.push(ch),
        }
    }
    out
}

fn undouble_braces_preserving_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // 转义序列中的 `{`/`}` 不参与 `{{`/`}}` 的消解。
            out.push('\\');
            let Some(next) = chars.next() else {
                break;
            };
            out.push(next);

            // `\u{...}`：把整个 `{...}` 视为转义语法的一部分，原样拷贝。
            if next == 'u' && matches!(chars.peek(), Some('{')) {
                out.push(chars.next().expect("peek 已保证存在"));
                while let Some(c) = chars.next() {
                    out.push(c);
                    if c == '}' {
                        break;
                    }
                }
            }
            continue;
        }

        match ch {
            '{' if matches!(chars.peek(), Some('{')) => {
                let _ = chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some('}')) => {
                let _ = chars.next();
                out.push('}');
            }
            _ => out.push(ch),
        }
    }

    out
}
