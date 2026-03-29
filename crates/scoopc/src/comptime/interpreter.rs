//! const/comptime 解释器（v0）。
//!
//! 目标（TODO T1202c）：
//! - 支持 `const fun` 调用（仅 Pure；由 typecheck headers 做最小门禁）；
//! - 支持函数体内的局部 `val`、`return` 语句、以及 block 的“最后表达式返回”；
//! - 支持 `const val` initializer 的常量折叠（用于 `tests/fixtures/comptime` 回归）。
//!
//! 非目标（后续任务逐步补齐）：
//! - 闭包/lambda、effects、循环/控制流（`if/when/while`）、`perform/handle`；
//! - 泛型实例化与重载决议（当前仅按“函数名 + 参数个数”做最小选择）。

use std::collections::HashMap;
use std::ops::ControlFlow;

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;

use super::eval::{ConstEvalHost, eval_const_expr, eval_const_expr_with_host, value_kind};
use super::{ConstEvalCtx, ConstEvalError, ConstValue};

/// const 解释器配置项（v0）。
#[derive(Debug, Clone, Copy)]
pub struct ConstEvalOptions {
    /// 最大递归深度（避免无限递归导致栈溢出）。
    pub recursion_limit: usize,
}

impl Default for ConstEvalOptions {
    fn default() -> Self {
        Self {
            recursion_limit: 64,
        }
    }
}

/// 一个 `const val` 的求值结果（用于 dump/fixtures）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstBinding {
    pub name: String,
    pub value: ConstValue,
}

/// 计算一个文件中所有 `const val` 的 initializer。
///
/// 说明：
/// - 该函数不做 parse；调用方需先通过 parser 拿到 AST；
/// - 当前仅支持同一文件内的 `const fun` 调用（按 name+arity 最小选择）。
pub fn eval_const_bindings_in_file<'a>(
    source: &'a SourceFile,
    file: &'a ast::File,
) -> Result<Vec<ConstBinding>, ConstEvalError> {
    let mut interp =
        ConstInterpreter::with_options(ConstEvalCtx::new(source), ConstEvalOptions::default());
    interp.register_file(file);
    interp.eval_const_bindings(file)
}

/// `const fun` 调用与局部求值的解释器状态。
struct ConstInterpreter<'a> {
    ctx: ConstEvalCtx<'a>,
    options: ConstEvalOptions,
    call_depth: usize,
    /// 作用域栈（后进先出）：局部 val/参数/顶层 const val 都放在这里。
    scopes: Vec<HashMap<String, ConstValue>>,
    /// 该文件中所有顶层函数（按名称聚合；用于判断“存在但非 const”）。
    funs_by_name: HashMap<String, Vec<&'a ast::FunDecl>>,
    /// 该文件中所有顶层类型声明（按名称聚合；用于反射 intrinsics v0）。
    types_by_name: HashMap<String, Vec<&'a ast::TypeDecl>>,
}

impl<'a> ConstInterpreter<'a> {
    fn with_options(ctx: ConstEvalCtx<'a>, options: ConstEvalOptions) -> Self {
        Self {
            ctx,
            options,
            call_depth: 0,
            scopes: vec![HashMap::new()],
            funs_by_name: HashMap::new(),
            types_by_name: HashMap::new(),
        }
    }

    fn register_file(&mut self, file: &'a ast::File) {
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    let name = fun.name.text(self.ctx.source).to_string();
                    self.funs_by_name.entry(name).or_default().push(fun);
                }
                ast::Item::Type(ty) => {
                    let name = ty.name.text(self.ctx.source).to_string();
                    self.types_by_name.entry(name).or_default().push(ty);
                }
                ast::Item::TypeAlias(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Object(_)
                | ast::Item::Val(_) => {}
            }
        }
    }

    fn eval_const_bindings(
        &mut self,
        file: &'a ast::File,
    ) -> Result<Vec<ConstBinding>, ConstEvalError> {
        let mut out = Vec::new();

        for item in &file.items {
            let ast::Item::Val(v) = item else { continue };
            if !v.modifiers.contains(&ast::Modifier::Const) {
                continue;
            }

            // `const val` 目前只支持名字绑定。
            let Some(name_ident) = v.name() else {
                return Err(ConstEvalError::UnsupportedStmt {
                    kind: "const val pattern binding",
                    span: v.span.into(),
                });
            };
            if v.kind != ast::ValKind::Val {
                return Err(ConstEvalError::UnsupportedStmt {
                    kind: "const var",
                    span: v.span.into(),
                });
            }
            let Some(init) = v.init.as_ref() else {
                return Err(ConstEvalError::MissingInitializer {
                    kind: "const val",
                    span: v.span.into(),
                });
            };

            let name = name_ident.text(self.ctx.source).to_string();
            let value = eval_const_expr_with_host(self.ctx, self, init)?;

            // 顶层 const val 也进入环境：后续 const val/const fun 可引用它。
            self.define_local(name.clone(), value.clone());
            out.push(ConstBinding { name, value });
        }

        Ok(out)
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define_local(&mut self, name: String, value: ConstValue) {
        let scope = self.scopes.last_mut().expect("at least one scope");
        scope.insert(name, value);
    }

    fn lookup(&self, name: &str) -> Option<ConstValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    fn call_const_fun(
        &mut self,
        call_span: Span,
        callee_name: &str,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        let Some(candidates) = self.funs_by_name.get(callee_name) else {
            return Err(ConstEvalError::UnknownConstFun {
                name: callee_name.to_string(),
                span: call_span.into(),
            });
        };

        // 仅允许调用 `const fun`。
        let const_candidates = candidates
            .iter()
            .copied()
            .filter(|f| f.modifiers.contains(&ast::Modifier::Const))
            .collect::<Vec<_>>();
        if const_candidates.is_empty() {
            return Err(ConstEvalError::CalleeNotConstFun {
                name: callee_name.to_string(),
                span: call_span.into(),
            });
        }

        let arity = args.len();
        let arity_matches = const_candidates
            .into_iter()
            .filter(|f| f.params.len() == arity)
            .collect::<Vec<_>>();

        let fun = match arity_matches.as_slice() {
            [] => {
                // 早期阶段只按 arity 匹配；默认参数/命名参数/重载决议留给后续阶段。
                let expected = candidates
                    .iter()
                    .filter(|f| f.modifiers.contains(&ast::Modifier::Const))
                    .map(|f| f.params.len())
                    .min()
                    .unwrap_or(0);
                return Err(ConstEvalError::ConstFunArityMismatch {
                    name: callee_name.to_string(),
                    expected,
                    found: arity,
                    span: call_span.into(),
                });
            }
            [one] => *one,
            _ => {
                return Err(ConstEvalError::ConstFunAmbiguous {
                    name: callee_name.to_string(),
                    arity,
                    span: call_span.into(),
                });
            }
        };

        self.eval_fun_call(call_span, fun, args)
    }

    fn call_fun_or_intrinsic(
        &mut self,
        call_span: Span,
        callee_name: &str,
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        // T1219：平台反射 `getPlatform()`：既可在 comptime 求值，也可在运行期查询。
        //
        // 说明：
        // - const 解释器只负责“编译期求值”路径；运行期调用由后续 lowering/codegen 处理。
        // - 当前阶段先把 runtime 视为等同于 compile target（host）。
        if callee_name == "getPlatform" {
            if !type_args.is_empty() {
                return Err(ConstEvalError::UnsupportedConstFunSignature {
                    reason: "explicit type args",
                    span: call_span.into(),
                });
            }
            if !args.is_empty() {
                return Err(ConstEvalError::ConstFunArityMismatch {
                    name: "getPlatform".to_string(),
                    expected: 0,
                    found: args.len(),
                    span: call_span.into(),
                });
            }
            return Ok(host_platform_const_value());
        }

        // T1204：反射 intrinsics（comptime 执行时由解释器内建实现）。
        match callee_name {
            "nameOf" | "sizeOf" | "alignOf" | "fieldsOf" | "variantsOf" | "superTypesOf"
            | "annotationsOf" => {
                return self.call_reflection_intrinsics(call_span, callee_name, type_args, args);
            }
            "paramsOf" => {
                return self.call_params_of_intrinsic(call_span, type_args, args);
            }
            _ => {}
        }

        // v0：解释器不支持泛型 const fun；显式 type args 只允许用于 intrinsics。
        if !type_args.is_empty() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "explicit type args",
                span: call_span.into(),
            });
        }

        self.call_const_fun(call_span, callee_name, args)
    }

    fn call_reflection_intrinsics(
        &mut self,
        call_span: Span,
        name: &str,
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        if type_args.len() != 1 || !args.is_empty() {
            return Err(ConstEvalError::ReflectionBadCall {
                name: name.to_string(),
                reason: "期望形态为 `<T>()`（1 个类型实参 + 0 个值实参）",
                span: call_span.into(),
            });
        }

        let ty_arg = &type_args[0];
        let (full_name, simple_name, ty_span) = self.type_ref_path_name_and_simple(ty_arg)?;

        match name {
            "nameOf" => Ok(ConstValue::String(full_name)),
            "sizeOf" => {
                let Some(size) = size_of_builtin_ty_bytes(&simple_name) else {
                    return Err(ConstEvalError::ReflectionSizeOfUnsupportedType {
                        name: full_name,
                        span: ty_span.into(),
                    });
                };
                Ok(ConstValue::Int(super::ConstInt::new(
                    self.ctx.default_int_ty,
                    size as u128,
                )))
            }
            "alignOf" => {
                let Some(align) = align_of_builtin_ty_bytes(&simple_name) else {
                    return Err(ConstEvalError::ReflectionAlignOfUnsupportedType {
                        name: full_name,
                        span: ty_span.into(),
                    });
                };
                Ok(ConstValue::Int(super::ConstInt::new(
                    self.ctx.default_int_ty,
                    align as u128,
                )))
            }
            "fieldsOf" => {
                let decls = self.types_by_name.get(&simple_name).ok_or_else(|| {
                    ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    }
                })?;
                let decl = match decls.as_slice() {
                    [one] => *one,
                    _ => {
                        return Err(ConstEvalError::ReflectionAmbiguousType {
                            name: full_name.clone(),
                            span: ty_span.into(),
                        });
                    }
                };
                if decl.kind != ast::TypeKind::Struct && decl.kind != ast::TypeKind::Class {
                    return Err(ConstEvalError::ReflectionUnsupportedTarget {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    });
                }

                let mut fields: Vec<ConstValue> = Vec::new();
                let mut seen: std::collections::BTreeSet<String> =
                    std::collections::BTreeSet::new();

                // 1) 主构造 `val/var` 参数声明的字段
                if let Some(ctor) = decl.primary_ctor.as_ref() {
                    for p in &ctor.params {
                        if p.kind.is_none() {
                            continue;
                        }
                        let fname = p.name.text(self.ctx.source).to_string();
                        if !seen.insert(fname.clone()) {
                            return Err(ConstEvalError::ReflectionDuplicateField {
                                field: fname,
                                span: ty_span.into(),
                            });
                        }
                        let index = fields.len();
                        fields.push(self.mk_field_meta(
                            fname,
                            p.ty.as_ref(),
                            index,
                            &p.annotations,
                        )?);
                    }
                }

                // 2) type body 里“看起来像 backing field 的属性声明”
                if let Some(body) = decl.body.as_ref() {
                    for m in &body.members {
                        let ast::TypeMember::Property(p) = m else {
                            continue;
                        };

                        // v0：只把“无 delegate、无自定义 getter/setter”的属性当作字段。
                        if p.delegate.is_some() || p.getter.is_some() || p.setter.is_some() {
                            continue;
                        }

                        let fname = p.name.text(self.ctx.source).to_string();
                        if !seen.insert(fname.clone()) {
                            return Err(ConstEvalError::ReflectionDuplicateField {
                                field: fname,
                                span: ty_span.into(),
                            });
                        }
                        let index = fields.len();
                        fields.push(self.mk_field_meta(
                            fname,
                            p.ty.as_ref(),
                            index,
                            &p.annotations,
                        )?);
                    }
                }

                Ok(ConstValue::Tuple(fields))
            }
            "variantsOf" => {
                let decls = self.types_by_name.get(&simple_name).ok_or_else(|| {
                    ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    }
                })?;
                let decl = match decls.as_slice() {
                    [one] => *one,
                    _ => {
                        return Err(ConstEvalError::ReflectionAmbiguousType {
                            name: full_name.clone(),
                            span: ty_span.into(),
                        });
                    }
                };
                if decl.kind != ast::TypeKind::Enum {
                    return Err(ConstEvalError::ReflectionVariantsOfUnsupportedTarget {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    });
                }

                let mut variants: Vec<ConstValue> = Vec::new();
                if let Some(body) = decl.body.as_ref() {
                    for m in &body.members {
                        let ast::TypeMember::EnumVariant(v) = m else {
                            continue;
                        };
                        let index = variants.len();
                        variants.push(self.mk_variant_meta(v, index)?);
                    }
                }
                Ok(ConstValue::Tuple(variants))
            }
            "superTypesOf" => {
                let decls = self.types_by_name.get(&simple_name).ok_or_else(|| {
                    ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    }
                })?;
                let decl = match decls.as_slice() {
                    [one] => *one,
                    _ => {
                        return Err(ConstEvalError::ReflectionAmbiguousType {
                            name: full_name.clone(),
                            span: ty_span.into(),
                        });
                    }
                };

                let mut supers: Vec<ConstValue> = Vec::with_capacity(decl.supertypes.len());
                for st in &decl.supertypes {
                    supers.push(self.mk_type_meta(Some(&st.ty))?);
                }
                Ok(ConstValue::Tuple(supers))
            }
            "annotationsOf" => {
                let decls = self.types_by_name.get(&simple_name).ok_or_else(|| {
                    ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    }
                })?;
                let decl = match decls.as_slice() {
                    [one] => *one,
                    _ => {
                        return Err(ConstEvalError::ReflectionAmbiguousType {
                            name: full_name.clone(),
                            span: ty_span.into(),
                        });
                    }
                };

                let mut anns: Vec<ConstValue> = Vec::new();
                for a in &decl.annotations {
                    // `annotationsOf<T>()` 只返回“类型本身”的注解：忽略 use-site target。
                    if a.use_site_target.is_some() {
                        continue;
                    }
                    anns.push(self.mk_annotation_meta(a)?);
                }
                Ok(ConstValue::Tuple(anns))
            }
            _ => Err(ConstEvalError::ReflectionBadCall {
                name: name.to_string(),
                reason: "unknown reflection intrinsic",
                span: call_span.into(),
            }),
        }
    }

    fn call_params_of_intrinsic(
        &mut self,
        call_span: Span,
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        if !type_args.is_empty() || args.len() != 1 {
            return Err(ConstEvalError::ReflectionBadCall {
                name: "paramsOf".to_string(),
                reason: "期望形态为 `(fn)`（0 个类型实参 + 1 个值实参）",
                span: call_span.into(),
            });
        }

        // v0：允许两种形态提供“函数句柄”：
        // - `FunctionMeta { name: \"foo\" }`（与 sysroot 声明一致）
        // - `"foo"`（便于 tests/fixtures 写最小用例）
        let fn_name: String = match &args[0] {
            ConstValue::String(s) => s.clone(),
            ConstValue::Struct(super::ConstStruct { fields, .. }) => match fields.get("name") {
                Some(ConstValue::String(s)) => s.clone(),
                _ => {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "FunctionMeta{name:String} 或 String",
                        found: value_kind(&args[0]),
                        span: call_span.into(),
                    });
                }
            },
            _ => {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "FunctionMeta{name:String} 或 String",
                    found: value_kind(&args[0]),
                    span: call_span.into(),
                });
            }
        };

        let decls = self.funs_by_name.get(&fn_name).ok_or_else(|| {
            ConstEvalError::ReflectionUnknownFunction {
                name: fn_name.clone(),
                span: call_span.into(),
            }
        })?;
        let fun = match decls.as_slice() {
            [one] => *one,
            _ => {
                return Err(ConstEvalError::ReflectionAmbiguousFunction {
                    name: fn_name.clone(),
                    span: call_span.into(),
                });
            }
        };

        let mut params: Vec<ConstValue> = Vec::with_capacity(fun.params.len());
        for (idx, p) in fun.params.iter().enumerate() {
            params.push(self.mk_param_meta(p, idx)?);
        }
        Ok(ConstValue::Tuple(params))
    }

    fn mk_type_kind(&self, variant: &'static str) -> ConstValue {
        ConstValue::Enum(super::ConstEnum {
            ty: Some("TypeKind".to_string()),
            variant: variant.to_string(),
            payload: Vec::new(),
        })
    }

    fn mk_annotation_list(
        &self,
        anns: &[ast::AnnotationUse],
        ignore_use_site_target: bool,
    ) -> Result<Vec<ConstValue>, ConstEvalError> {
        let mut out: Vec<ConstValue> = Vec::new();
        for a in anns {
            if ignore_use_site_target && a.use_site_target.is_some() {
                continue;
            }
            out.push(self.mk_annotation_meta(a)?);
        }
        Ok(out)
    }

    fn lookup_unique_type_decl(&self, simple_name: &str) -> Option<&'a ast::TypeDecl> {
        let decls = self.types_by_name.get(simple_name)?;
        match decls.as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }

    /// 把一个类型引用“降级”为 TypeMeta。
    ///
    /// 说明：
    /// - const 解释器在当前阶段没有完整的 name resolution / type env，因此这里采用“尽力而为”的策略：
    ///   - `name`：基于 AST 的稳定 pretty string（不保证全限定名）；
    ///   - `kind/annotations`：仅当该类型在当前文件中有唯一声明时补齐；否则回退为 `Primitive` + 空注解；
    /// - 这保证了 fixtures 可以稳定读取 `field.type.name`，并把“精确元信息”留给后续任务（resolve/typecheck）补齐。
    fn mk_type_meta(&self, ty: Option<&ast::TypeRef>) -> Result<ConstValue, ConstEvalError> {
        let name = ty
            .and_then(|t| self.type_ref_to_string(t))
            .unwrap_or_else(|| "Any".to_string());

        let (kind, annotations) = match ty {
            Some(ast::TypeRef::Tuple(t)) if t.elements.is_empty() => {
                (self.mk_type_kind("Tuple"), Vec::new())
            }
            Some(ast::TypeRef::Path(p)) => {
                let simple = p
                    .segments
                    .last()
                    .map(|s| s.text(self.ctx.source))
                    .unwrap_or("");
                if let Some(decl) = self.lookup_unique_type_decl(simple) {
                    let kind = match decl.kind {
                        ast::TypeKind::Struct => self.mk_type_kind("Struct"),
                        ast::TypeKind::Enum => self.mk_type_kind("Enum"),
                        ast::TypeKind::Class => self.mk_type_kind("Class"),
                        ast::TypeKind::Interface => self.mk_type_kind("Interface"),
                        ast::TypeKind::Effect => self.mk_type_kind("Effect"),
                    };
                    let annotations = self.mk_annotation_list(&decl.annotations, true)?;
                    (kind, annotations)
                } else {
                    (self.mk_type_kind("Primitive"), Vec::new())
                }
            }
            Some(_) | None => (self.mk_type_kind("Primitive"), Vec::new()),
        };

        let mut fields = std::collections::BTreeMap::new();
        fields.insert("name".to_string(), ConstValue::String(name));
        fields.insert("kind".to_string(), kind);
        fields.insert("annotations".to_string(), ConstValue::Tuple(annotations));
        Ok(ConstValue::Struct(super::ConstStruct {
            ty: "TypeMeta".to_string(),
            fields,
        }))
    }

    /// 构造一个 FieldMeta 常量值（供 `fieldsOf<T>()` / `variantsOf<T>()` 返回）。
    fn mk_field_meta(
        &self,
        name: String,
        ty: Option<&ast::TypeRef>,
        index: usize,
        annotations: &[ast::AnnotationUse],
    ) -> Result<ConstValue, ConstEvalError> {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "index".to_string(),
            ConstValue::Int(super::ConstInt::new(self.ctx.default_int_ty, index as u128)),
        );
        fields.insert("name".to_string(), ConstValue::String(name));
        fields.insert("type".to_string(), self.mk_type_meta(ty)?);
        fields.insert(
            "annotations".to_string(),
            ConstValue::Tuple(self.mk_annotation_list(annotations, false)?),
        );
        Ok(ConstValue::Struct(super::ConstStruct {
            ty: "FieldMeta".to_string(),
            fields,
        }))
    }

    fn mk_param_meta(&self, p: &ast::Param, index: usize) -> Result<ConstValue, ConstEvalError> {
        let pname = p.name.text(self.ctx.source).to_string();

        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "index".to_string(),
            ConstValue::Int(super::ConstInt::new(self.ctx.default_int_ty, index as u128)),
        );
        fields.insert("name".to_string(), ConstValue::String(pname));
        fields.insert("type".to_string(), self.mk_type_meta(p.ty.as_ref())?);
        fields.insert(
            "annotations".to_string(),
            ConstValue::Tuple(self.mk_annotation_list(&p.annotations, false)?),
        );
        Ok(ConstValue::Struct(super::ConstStruct {
            ty: "ParamMeta".to_string(),
            fields,
        }))
    }

    fn mk_variant_meta(
        &self,
        v: &ast::EnumVariantDecl,
        index: usize,
    ) -> Result<ConstValue, ConstEvalError> {
        let vname = v.name.text(self.ctx.source).to_string();

        let mut field_metas: Vec<ConstValue> = Vec::with_capacity(v.params.len());
        for (idx, p) in v.params.iter().enumerate() {
            let fname = p.name.text(self.ctx.source).to_string();
            field_metas.push(self.mk_field_meta(fname, p.ty.as_ref(), idx, &p.annotations)?);
        }

        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "index".to_string(),
            ConstValue::Int(super::ConstInt::new(self.ctx.default_int_ty, index as u128)),
        );
        fields.insert("name".to_string(), ConstValue::String(vname));
        fields.insert("fields".to_string(), ConstValue::Tuple(field_metas));
        fields.insert(
            "annotations".to_string(),
            ConstValue::Tuple(self.mk_annotation_list(&v.annotations, false)?),
        );
        Ok(ConstValue::Struct(super::ConstStruct {
            ty: "VariantMeta".to_string(),
            fields,
        }))
    }

    fn mk_annotation_meta(&self, a: &ast::AnnotationUse) -> Result<ConstValue, ConstEvalError> {
        let name = a
            .path
            .iter()
            .map(|id| id.text(self.ctx.source))
            .collect::<Vec<_>>()
            .join(".");
        let simple = a
            .path
            .last()
            .map(|id| id.text(self.ctx.source).to_string())
            .unwrap_or_default();

        // spec §15.6：`AnnotationMeta.args` 应当是“按参数名解析后的 arguments（含默认值）”。
        //
        // 说明：
        // - v0 阶段只保证 compile-time constant 形态可读；
        // - 若无法定位到唯一的 annotation class 声明，则回退为“仅按语法顺序读取提供的 args”。
        let ctor_params = self.lookup_annotation_ctor_params(&simple);
        let mut positional_index: usize = 0;
        let mut provided_order: Vec<(String, ConstValue)> = Vec::with_capacity(a.args.len());
        let mut provided_by_name: std::collections::BTreeMap<String, ConstValue> =
            std::collections::BTreeMap::new();

        for arg in &a.args {
            let arg_name = match arg.name {
                Some(id) => id.text(self.ctx.source).to_string(),
                None => {
                    let name = ctor_params
                        .and_then(|ps| ps.get(positional_index))
                        .map(|p| p.name.text(self.ctx.source).to_string())
                        .unwrap_or_else(|| format!("_{positional_index}"));
                    positional_index += 1;
                    name
                }
            };

            // T1209/T1218：当前阶段只保证“compile-time constant”形态可读：
            // - 字面量 / 常量表达式
            // - 数组字面量（v0 用 tuple 承载）
            // - enum unit variant（`Enum.Variant`）
            // - class literal（`TypeName::class`，v0 视为类型名字符串常量）
            let value = eval_const_expr(self.ctx, &arg.value)?;
            provided_by_name.insert(arg_name.clone(), value.clone());
            provided_order.push((arg_name, value));
        }

        let args: Vec<ConstValue> = match ctor_params {
            Some(params) => {
                let mut remaining = provided_by_name;
                let mut out: Vec<ConstValue> = Vec::new();

                // 1) 按 ctor 参数顺序输出（含默认值）。
                for p in params {
                    let pname = p.name.text(self.ctx.source).to_string();
                    if let Some(v) = remaining.remove(&pname) {
                        out.push(self.mk_annotation_arg_meta_value(pname, v));
                        continue;
                    }
                    if let Some(default_value) = p.default_value.as_ref() {
                        let v = eval_const_expr(self.ctx, default_value)?;
                        out.push(self.mk_annotation_arg_meta_value(pname, v));
                    }
                }

                // 2) 未能匹配到 ctor 参数名的 args：为了稳定性，按名字排序追加。
                for (k, v) in remaining {
                    out.push(self.mk_annotation_arg_meta_value(k, v));
                }
                out
            }
            None => provided_order
                .into_iter()
                .map(|(k, v)| self.mk_annotation_arg_meta_value(k, v))
                .collect(),
        };

        let mut fields = std::collections::BTreeMap::new();
        fields.insert("name".to_string(), ConstValue::String(name));
        fields.insert("args".to_string(), ConstValue::Tuple(args));
        Ok(ConstValue::Struct(super::ConstStruct {
            ty: "AnnotationMeta".to_string(),
            fields,
        }))
    }

    fn mk_annotation_arg_meta_value(&self, arg_name: String, value: ConstValue) -> ConstValue {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("name".to_string(), ConstValue::String(arg_name));
        fields.insert("value".to_string(), value);
        ConstValue::Struct(super::ConstStruct {
            ty: "AnnotationArgMeta".to_string(),
            fields,
        })
    }

    fn lookup_annotation_ctor_params(
        &self,
        annotation_simple_name: &str,
    ) -> Option<&'a [ast::Param]> {
        let decls = self.types_by_name.get(annotation_simple_name)?;
        let decl = match decls.as_slice() {
            [one] => *one,
            _ => return None,
        };
        if !decl.modifiers.contains(&ast::Modifier::Annotation) {
            return None;
        }
        let ctor = decl.primary_ctor.as_ref()?;
        Some(ctor.params.as_slice())
    }

    /// 把 `TypeRef` 格式化为稳定的字符串（用于 `TypeMeta.name`）。
    ///
    /// 说明：
    /// - 这里输出的是“语法层面”的名字（基于 AST），并不保证是全限定名；
    /// - 后续接入 resolve/typecheck 后，可把它升级为 FQN + 泛型实例信息。
    fn type_ref_to_string(&self, ty: &ast::TypeRef) -> Option<String> {
        match ty {
            ast::TypeRef::Path(p) => {
                let mut out = p
                    .segments
                    .iter()
                    .map(|id| id.text(self.ctx.source))
                    .collect::<Vec<_>>()
                    .join(".");
                if !p.args.is_empty() {
                    let inner = p
                        .args
                        .iter()
                        .filter_map(|a| self.type_ref_to_string(a))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push('<');
                    out.push_str(&inner);
                    out.push('>');
                }
                Some(out)
            }
            ast::TypeRef::Nullable { inner, .. } => {
                self.type_ref_to_string(inner).map(|s| format!("{s}?"))
            }
            ast::TypeRef::Tuple(t) if t.elements.is_empty() => Some("Unit".to_string()),
            // v0：不支持把这些类型表达成 TypeMeta。
            ast::TypeRef::Tuple(_)
            | ast::TypeRef::Star { .. }
            | ast::TypeRef::EffectRowArg { .. }
            | ast::TypeRef::Function(_) => None,
        }
    }

    fn type_ref_path_name_and_simple(
        &self,
        ty: &ast::TypeRef,
    ) -> Result<(String, String, Span), ConstEvalError> {
        let ast::TypeRef::Path(p) = ty else {
            return Err(ConstEvalError::ReflectionTypeArgNotSupported {
                found: "non-path type",
                span: ty.span().into(),
            });
        };

        let mut full = String::new();
        for (idx, seg) in p.segments.iter().enumerate() {
            if idx > 0 {
                full.push('.');
            }
            full.push_str(seg.text(self.ctx.source));
        }

        let simple = p
            .segments
            .last()
            .map(|s| s.text(self.ctx.source).to_string())
            .unwrap_or_default();

        Ok((full, simple, p.span))
    }

    fn eval_fun_call(
        &mut self,
        call_span: Span,
        fun: &'a ast::FunDecl,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        if self.call_depth >= self.options.recursion_limit {
            return Err(ConstEvalError::RecursionLimitExceeded {
                name: fun.name.text(self.ctx.source).to_string(),
                limit: self.options.recursion_limit,
                span: call_span.into(),
            });
        }

        // 解释器入口做一次“最小签名门禁”，避免把复杂语义带入 v0。
        if fun.receiver.is_some() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "extension receiver",
                span: fun.span.into(),
            });
        }
        if !fun.type_params.is_empty() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "generic type params",
                span: fun.span.into(),
            });
        }
        if fun.eff_param.is_some() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "effect row param",
                span: fun.span.into(),
            });
        }
        if fun.params.len() != args.len() {
            return Err(ConstEvalError::ConstFunArityMismatch {
                name: fun.name.text(self.ctx.source).to_string(),
                expected: fun.params.len(),
                found: args.len(),
                span: call_span.into(),
            });
        }

        self.call_depth += 1;
        self.push_scope();

        // 参数绑定写入当前 frame scope。
        for (param, arg) in fun.params.iter().zip(args) {
            let name = param.name.text(self.ctx.source).to_string();
            self.define_local(name, arg);
        }

        let ret = match &fun.body {
            ast::FunBody::Block(b) => match self.eval_block(b)? {
                ControlFlow::Break(v) | ControlFlow::Continue(v) => v,
            },
            ast::FunBody::Missing => {
                return Err(ConstEvalError::UnsupportedConstFunSignature {
                    reason: "missing body",
                    span: fun.span.into(),
                });
            }
        };

        self.pop_scope();
        self.call_depth -= 1;
        Ok(ret)
    }

    fn eval_block(
        &mut self,
        block: &ast::Block,
    ) -> Result<ControlFlow<ConstValue, ConstValue>, ConstEvalError> {
        // block 自带一个子作用域（与 resolver/typecheck 的“block 内声明仅在该 block 内可见”一致）。
        self.push_scope();

        let mut last_value = ConstValue::Unit;
        for stmt in &block.stmts {
            match self.eval_stmt(stmt)? {
                ControlFlow::Break(ret) => {
                    self.pop_scope();
                    return Ok(ControlFlow::Break(ret));
                }
                ControlFlow::Continue(Some(v)) => last_value = v,
                ControlFlow::Continue(None) => {}
            }
        }

        self.pop_scope();
        Ok(ControlFlow::Continue(last_value))
    }

    fn eval_stmt(
        &mut self,
        stmt: &ast::Stmt,
    ) -> Result<ControlFlow<ConstValue, Option<ConstValue>>, ConstEvalError> {
        match &stmt.kind {
            ast::StmtKind::Empty => Ok(ControlFlow::Continue(None)),
            ast::StmtKind::Expr(e) => {
                let v = eval_const_expr_with_host(self.ctx, self, e)?;
                Ok(ControlFlow::Continue(Some(v)))
            }
            ast::StmtKind::Val(v) => {
                if v.kind != ast::ValKind::Val {
                    return Err(ConstEvalError::UnsupportedStmt {
                        kind: "local var",
                        span: v.span.into(),
                    });
                }
                let Some(name) = v.name() else {
                    return Err(ConstEvalError::UnsupportedStmt {
                        kind: "local val pattern binding",
                        span: v.span.into(),
                    });
                };
                let Some(init) = v.init.as_ref() else {
                    return Err(ConstEvalError::MissingInitializer {
                        kind: "local val",
                        span: v.span.into(),
                    });
                };

                let value = eval_const_expr_with_host(self.ctx, self, init)?;
                self.define_local(name.text(self.ctx.source).to_string(), value);
                Ok(ControlFlow::Continue(None))
            }
            ast::StmtKind::Return { value, .. } => {
                let v = match value {
                    Some(expr) => eval_const_expr_with_host(self.ctx, self, expr)?,
                    None => ConstValue::Unit,
                };
                Ok(ControlFlow::Break(v))
            }
            ast::StmtKind::While { .. } => Err(ConstEvalError::UnsupportedStmt {
                kind: "while",
                span: stmt.span.into(),
            }),
            ast::StmtKind::For(_) => Err(ConstEvalError::UnsupportedStmt {
                kind: "for",
                span: stmt.span.into(),
            }),
            ast::StmtKind::Break { .. } => Err(ConstEvalError::UnsupportedStmt {
                kind: "break",
                span: stmt.span.into(),
            }),
            ast::StmtKind::Continue { .. } => Err(ConstEvalError::UnsupportedStmt {
                kind: "continue",
                span: stmt.span.into(),
            }),
            ast::StmtKind::ComptimeBlock { body, .. } => match self.eval_block(body)? {
                ControlFlow::Break(ret) => Ok(ControlFlow::Break(ret)),
                ControlFlow::Continue(v) => Ok(ControlFlow::Continue(Some(v))),
            },
            ast::StmtKind::ComptimeIf(ci) => match self.eval_comptime_if(ci)? {
                ControlFlow::Break(ret) => Ok(ControlFlow::Break(ret)),
                ControlFlow::Continue(v) => Ok(ControlFlow::Continue(Some(v))),
            },
            ast::StmtKind::ComptimeFor(cf) => match self.eval_comptime_for(cf)? {
                ControlFlow::Break(ret) => Ok(ControlFlow::Break(ret)),
                ControlFlow::Continue(v) => Ok(ControlFlow::Continue(Some(v))),
            },
            ast::StmtKind::Missing => Err(ConstEvalError::UnsupportedStmt {
                kind: "missing stmt",
                span: stmt.span.into(),
            }),
        }
    }

    fn eval_comptime_if(
        &mut self,
        ci: &ast::ComptimeIf,
    ) -> Result<ControlFlow<ConstValue, ConstValue>, ConstEvalError> {
        // `comptime if`：在编译期求值条件，仅执行被选中的分支（未选中分支不求值）。
        let cond_v = eval_const_expr_with_host(self.ctx, self, &ci.cond)?;
        let ConstValue::Bool(cond_b) = cond_v else {
            return Err(ConstEvalError::OperandTypeMismatch {
                expected: "Bool",
                found: value_kind(&cond_v),
                span: ci.cond.span.into(),
            });
        };

        if cond_b {
            return self.eval_block(&ci.then_branch);
        }

        match &ci.else_branch {
            None => Ok(ControlFlow::Continue(ConstValue::Unit)),
            Some(else_branch) => match &**else_branch {
                ast::ComptimeIfElse::Block(b) => self.eval_block(b),
                ast::ComptimeIfElse::If(nested) => self.eval_comptime_if(nested),
            },
        }
    }

    fn eval_comptime_for(
        &mut self,
        cf: &ast::ComptimeFor,
    ) -> Result<ControlFlow<ConstValue, ConstValue>, ConstEvalError> {
        // `comptime for (x in xs) { ... }`：
        // - 先在编译期求值 iter；
        // - 对可迭代对象进行“展开执行”，每次迭代把 binder 绑定到当前元素；
        // - v0：仅支持整数范围 `a..b` 与 tuple/array（以 ConstValue::Tuple 承载）。
        let binder_name = cf.binder.text(self.ctx.source).to_string();

        // 1) 整数范围：`a..b`
        if let ast::ExprKind::Binary {
            lhs,
            op: ast::BinaryOp::RangeInclusive,
            rhs,
            ..
        } = &cf.iter.kind
        {
            let lv = eval_const_expr_with_host(self.ctx, self, lhs)?;
            let li = match lv {
                ConstValue::Int(i) => i,
                other => {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "整数",
                        found: value_kind(&other),
                        span: lhs.span.into(),
                    });
                }
            };

            let rv = eval_const_expr_with_host(self.ctx, self, rhs)?;
            let ri = match rv {
                ConstValue::Int(i) => i,
                other => {
                    return Err(ConstEvalError::OperandTypeMismatch {
                        expected: "整数",
                        found: value_kind(&other),
                        span: rhs.span.into(),
                    });
                }
            };
            if li.ty != ri.ty {
                return Err(ConstEvalError::OperandTypeMismatch {
                    expected: "相同的整数类型",
                    found: "不同位宽/符号位的整数",
                    span: cf.iter.span.into(),
                });
            }

            let mut last_value = ConstValue::Unit;

            if li.ty.signed {
                let mut cur = li.as_i128();
                let end = ri.as_i128();
                while cur <= end {
                    self.push_scope();
                    self.define_local(
                        binder_name.clone(),
                        ConstValue::Int(super::ConstInt::new(li.ty, cur as u128)),
                    );
                    match self.eval_block(&cf.body)? {
                        ControlFlow::Break(ret) => {
                            self.pop_scope();
                            return Ok(ControlFlow::Break(ret));
                        }
                        ControlFlow::Continue(v) => {
                            last_value = v;
                        }
                    }
                    self.pop_scope();

                    let Some(next) = cur.checked_add(1) else {
                        break;
                    };
                    cur = next;
                }
            } else {
                let mut cur = li.as_u128();
                let end = ri.as_u128();
                while cur <= end {
                    self.push_scope();
                    self.define_local(
                        binder_name.clone(),
                        ConstValue::Int(super::ConstInt::new(li.ty, cur)),
                    );
                    match self.eval_block(&cf.body)? {
                        ControlFlow::Break(ret) => {
                            self.pop_scope();
                            return Ok(ControlFlow::Break(ret));
                        }
                        ControlFlow::Continue(v) => {
                            last_value = v;
                        }
                    }
                    self.pop_scope();

                    let Some(next) = cur.checked_add(1) else {
                        break;
                    };
                    cur = next;
                }
            }

            return Ok(ControlFlow::Continue(last_value));
        }

        // 2) tuple/array（v0：统一用 Tuple 承载，见 comptime::eval）
        let iter_v = eval_const_expr_with_host(self.ctx, self, &cf.iter)?;
        let ConstValue::Tuple(items) = iter_v else {
            return Err(ConstEvalError::OperandTypeMismatch {
                expected: "Tuple（可迭代）",
                found: value_kind(&iter_v),
                span: cf.iter.span.into(),
            });
        };

        let mut last_value = ConstValue::Unit;
        for item in items {
            self.push_scope();
            self.define_local(binder_name.clone(), item);

            match self.eval_block(&cf.body)? {
                ControlFlow::Break(ret) => {
                    self.pop_scope();
                    return Ok(ControlFlow::Break(ret));
                }
                ControlFlow::Continue(v) => {
                    last_value = v;
                }
            }

            self.pop_scope();
        }

        Ok(ControlFlow::Continue(last_value))
    }
}

fn size_of_builtin_ty_bytes(name: &str) -> Option<usize> {
    match name {
        // scalar/value types
        "Bool" => Some(std::mem::size_of::<bool>()),
        "Unit" => Some(std::mem::size_of::<()>()),
        "Int" => Some(std::mem::size_of::<isize>()),
        "UInt" | "UIntPtr" => Some(std::mem::size_of::<usize>()),
        "Int8" | "UInt8" | "Byte" => Some(1),
        "Int16" | "UInt16" | "Short" | "UShort" => Some(2),
        "Int32" | "UInt32" => Some(4),
        "Int64" | "UInt64" | "Long" | "ULong" => Some(8),

        // 引用类型：v0 先把它们视为“指针大小”。
        "String" => Some(std::mem::size_of::<usize>()),

        _ => None,
    }
}

fn align_of_builtin_ty_bytes(name: &str) -> Option<usize> {
    match name {
        // scalar/value types
        "Bool" => Some(std::mem::align_of::<bool>()),
        "Unit" => Some(std::mem::align_of::<()>()),
        "Int" => Some(std::mem::align_of::<isize>()),
        "UInt" | "UIntPtr" => Some(std::mem::align_of::<usize>()),
        "Int8" | "UInt8" | "Byte" => Some(std::mem::align_of::<u8>()),
        "Int16" | "UInt16" | "Short" | "UShort" => Some(std::mem::align_of::<u16>()),
        "Int32" | "UInt32" => Some(std::mem::align_of::<u32>()),
        "Int64" | "UInt64" | "Long" | "ULong" => Some(std::mem::align_of::<u64>()),

        // 引用类型：v0 先把它们视为“指针”。
        "String" => Some(std::mem::align_of::<usize>()),

        _ => None,
    }
}

/// 返回当前编译目标（v0：host target）的平台信息（spec §6.4 / TODO T1219）。
///
/// 设计说明：
/// - LLVM 后端（inkwell）默认关闭；CI 也不要求安装 LLVM，因此这里不能依赖 LLVM API。
/// - v0 先用 Cargo 提供的目标 cfg 信息构造一个“LLVM 风格”的 triple 字符串；
/// - 之后再按 `arch-vendor-os-env` 的惯例做拆分，缺失字段则回退为空串。
fn host_platform_const_value() -> ConstValue {
    let triple = host_target_triple_string();
    let (arch, vendor, os, env) = decompose_llvm_like_triple(&triple);

    let mut fields: std::collections::BTreeMap<String, ConstValue> =
        std::collections::BTreeMap::new();
    fields.insert("triple".to_string(), ConstValue::String(triple));
    fields.insert("arch".to_string(), ConstValue::String(arch));
    fields.insert("vendor".to_string(), ConstValue::String(vendor));
    fields.insert("os".to_string(), ConstValue::String(os));
    fields.insert("env".to_string(), ConstValue::String(env));

    ConstValue::Struct(super::ConstStruct {
        ty: "Platform".to_string(),
        fields,
    })
}

fn host_target_triple_string() -> String {
    let arch = option_env!("CARGO_CFG_TARGET_ARCH").unwrap_or("unknown");
    let vendor = option_env!("CARGO_CFG_TARGET_VENDOR").unwrap_or("unknown");
    let os_cfg = option_env!("CARGO_CFG_TARGET_OS").unwrap_or("unknown");
    let os = normalize_target_os_for_llvm_triple(os_cfg);
    let env = option_env!("CARGO_CFG_TARGET_ENV").unwrap_or("");

    if env.is_empty() {
        format!("{arch}-{vendor}-{os}")
    } else {
        format!("{arch}-{vendor}-{os}-{env}")
    }
}

fn normalize_target_os_for_llvm_triple(os_cfg: &str) -> &str {
    // Rust `cfg(target_os = "macos")` 的字符串与 LLVM triple 的 OS 段并不一致：
    // - Rust: macos
    // - LLVM: darwin
    match os_cfg {
        "macos" => "darwin",
        other => other,
    }
}

fn decompose_llvm_like_triple(triple: &str) -> (String, String, String, String) {
    // LLVM triple 约定形态：arch-vendor-os[-env]。
    //
    // 说明：
    // - 本函数不做严格校验（spec 允许 implementation-defined validation）；
    // - 多余段落（如 `arch-vendor-os-env-abi`）目前忽略，保留最常用的前四段。
    let mut parts = triple.split('-');
    let arch = parts.next().unwrap_or("").to_string();
    let vendor = parts.next().unwrap_or("").to_string();
    let os = parts.next().unwrap_or("").to_string();
    let env = parts.next().unwrap_or("").to_string();
    (arch, vendor, os, env)
}

impl ConstEvalHost for ConstInterpreter<'_> {
    fn resolve_ident(&mut self, name: &str) -> Option<ConstValue> {
        self.lookup(name)
    }

    fn call_fun(
        &mut self,
        call_span: Span,
        callee_name: &str,
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        self.call_fun_or_intrinsic(call_span, callee_name, type_args, args)
    }
}
