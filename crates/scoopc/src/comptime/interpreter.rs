//! const/comptime 解释器（v0）。
//!
//! 目标（TODO T1202c）：
//! - 支持 `const fun` 调用（仅 Pure；由 typecheck headers 做最小门禁）；
//! - 支持函数体内的局部 `val`、`return` 语句、以及 block 的“最后表达式返回”；
//! - 支持 `const val` initializer 的常量折叠（用于 `tests/fixtures/comptime` 回归）。
//!
//! 非目标（后续任务逐步补齐）：
//! - 闭包/lambda、effects、循环/控制流（`if/when/while`）、`perform/handle`；
//! - 更完整的 generic/effect-row contract 与运行期 fallback 语义。

use std::collections::HashMap;
use std::ops::ControlFlow;

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::eval::{ConstEvalHost, eval_const_expr, eval_const_expr_with_host, value_kind};
use super::{ConstEvalCtx, ConstEvalError, ConstFloatTy, ConstIntTy, ConstValue};

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
/// - 调用会在“当前文件 + 传入 sysroot”的 compilation-unit 上下文中执行 resolve/typecheck，
///   再按普通调用主线选中的绑定执行 const/comptime 解释。
pub fn eval_const_bindings_in_file<'a>(
    sysroot: &'a crate::sysroot::Sysroot,
    source: &'a SourceFile,
    file: &'a ast::File,
) -> Result<Vec<ConstBinding>, ConstEvalError> {
    eval_const_bindings_in_compilation_unit(sysroot, &[(source, file)], 0)
}

/// 在一个多文件 compilation-unit 中执行目标文件的 `const val` initializer。
///
/// 说明：
/// - `files` 仅包含当前编译单元的用户源文件；sysroot 由 `sysroot` 单独注入。
/// - `target_file_idx` 指定要导出哪一个文件的顶层 `const val` 结果。
pub fn eval_const_bindings_in_compilation_unit<'a>(
    sysroot: &'a crate::sysroot::Sysroot,
    files: &[(&'a SourceFile, &'a ast::File)],
    target_file_idx: usize,
) -> Result<Vec<ConstBinding>, ConstEvalError> {
    assert!(
        target_file_idx < files.len(),
        "target_file_idx must point at an input file"
    );

    #[derive(Clone)]
    struct PreparedConstEvalFile<'a> {
        source: &'a SourceFile,
        ast: ast::File,
    }

    #[derive(Clone)]
    struct OwnedPreparedConstEvalFile {
        source: SourceFile,
        ast: ast::File,
    }

    let mut prepared_sysroot: Vec<PreparedConstEvalFile<'a>> =
        Vec::with_capacity(sysroot.files.len());
    for f in &sysroot.files {
        let mut ast = f.ast.clone();
        trim_package_level_comptime_ifs(&f.source, &mut ast)?;
        prepared_sysroot.push(PreparedConstEvalFile {
            source: &f.source,
            ast,
        });
    }

    let stdlib_source_paths = load_stdlib_source_paths()?;
    let mut prepared_stdlib: Vec<OwnedPreparedConstEvalFile> =
        Vec::with_capacity(stdlib_source_paths.len());
    for path in stdlib_source_paths {
        let source = SourceFile::load(&path).map_err(|err| frontend_message(err.to_string()))?;
        let mut ast = crate::parser::parse_file(&source).map_err(frontend_diagnostic)?;
        trim_package_level_comptime_ifs(&source, &mut ast)?;
        prepared_stdlib.push(OwnedPreparedConstEvalFile { source, ast });
    }

    let mut compilable_sysroot_paths = sysroot.compilable_source_paths.clone();
    compilable_sysroot_paths.sort();

    let mut prepared_compilable_sysroot: Vec<OwnedPreparedConstEvalFile> =
        Vec::with_capacity(compilable_sysroot_paths.len());
    for path in compilable_sysroot_paths {
        let source = SourceFile::load(&path).map_err(|err| frontend_message(err.to_string()))?;
        let mut ast = crate::parser::parse_file(&source).map_err(frontend_diagnostic)?;
        trim_package_level_comptime_ifs(&source, &mut ast)?;
        prepared_compilable_sysroot.push(OwnedPreparedConstEvalFile { source, ast });
    }

    let mut prepared: Vec<PreparedConstEvalFile<'a>> = Vec::with_capacity(files.len());
    for (source, file) in files.iter().copied() {
        let mut ast = file.clone();
        trim_package_level_comptime_ifs(source, &mut ast)?;
        prepared.push(PreparedConstEvalFile { source, ast });
    }

    let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::with_capacity(
        prepared_sysroot.len()
            + prepared_stdlib.len()
            + prepared_compilable_sysroot.len()
            + prepared.len(),
    );
    for f in &prepared_sysroot {
        pairs.push((f.source, &f.ast));
    }
    for f in &prepared_stdlib {
        pairs.push((&f.source, &f.ast));
    }
    for f in &prepared_compilable_sysroot {
        pairs.push((&f.source, &f.ast));
    }
    for f in &prepared {
        pairs.push((f.source, &f.ast));
    }

    let index = crate::resolve::Index::build(&pairs).map_err(frontend_diagnostic)?;
    let mut env =
        crate::typecheck::TypeEnv::from_sysroot(sysroot, &index).map_err(frontend_diagnostic)?;
    for f in &prepared_stdlib {
        env.extend_from_file(&f.source, &f.ast, &index)
            .map_err(frontend_diagnostic)?;
    }
    for f in &prepared_compilable_sysroot {
        env.extend_from_file(&f.source, &f.ast, &index)
            .map_err(frontend_diagnostic)?;
    }
    for f in &prepared {
        env.extend_from_file(f.source, &f.ast, &index)
            .map_err(frontend_diagnostic)?;
    }

    let mut types = crate::ty::TypeStore::new();
    let builtins = types.intern_builtins();

    let mut run_frontend_pipeline =
        |source: &SourceFile, ast: &mut ast::File| -> Result<(), ConstEvalError> {
            crate::typecheck::check_file_struct_decls(source, ast).map_err(frontend_diagnostic)?;

            let headers = crate::resolve::check_file_headers(source, ast, &index)
                .map_err(frontend_diagnostic)?;
            crate::resolve::check_file_bodies(source, ast, &index, &headers)
                .map_err(frontend_diagnostic)?;

            crate::typecheck::check_file_annotations(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(frontend_diagnostic)?;
            crate::typecheck::check_file_properties(source, ast, &index, &env)
                .map_err(|err| frontend_boxed_diagnostic(err))?;
            crate::typecheck::check_file_inheritance(source, ast, &index)
                .map_err(frontend_diagnostic)?;
            crate::typecheck::check_file_interfaces(source, ast, &index, &env)
                .map_err(frontend_diagnostic)?;
            crate::typecheck::check_file_override_effects(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(|err| frontend_boxed_diagnostic(err))?;
            crate::typecheck::check_file_type_refs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(frontend_diagnostic)?;
            crate::typecheck::check_file_where_clauses(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(frontend_diagnostic)?;
            crate::typecheck::check_file_overload_conflicts(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(frontend_diagnostic)?;
            crate::typecheck::check_file_exprs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .map_err(frontend_diagnostic)?;
            Ok(())
        };

    for f in &mut prepared_sysroot {
        run_frontend_pipeline(f.source, &mut f.ast)?;
    }
    for f in &mut prepared_stdlib {
        run_frontend_pipeline(&f.source, &mut f.ast)?;
    }
    for f in &mut prepared_compilable_sysroot {
        run_frontend_pipeline(&f.source, &mut f.ast)?;
    }
    for f in &mut prepared {
        run_frontend_pipeline(f.source, &mut f.ast)?;
    }

    let target = &prepared[target_file_idx];
    let mut interp = ConstInterpreter::with_types(
        ConstEvalCtx::new(target.source),
        &target.ast,
        ConstEvalOptions::default(),
        types,
    );
    for f in &prepared_sysroot {
        interp.register_file(f.source, &f.ast);
    }
    for f in &prepared_stdlib {
        interp.register_file(&f.source, &f.ast);
    }
    for f in &prepared_compilable_sysroot {
        interp.register_file(&f.source, &f.ast);
    }
    for f in &prepared {
        interp.register_file(f.source, &f.ast);
    }

    interp.eval_const_bindings_for_file(target.source, &target.ast)
}

/// 在 resolver/index 之前裁剪 package-level `comptime if`（TODO T1220b）。
///
/// 语义（v0）：
/// - 对顶层 `comptime if (<cond>) { <items> } else ...` 的 `<cond>` 做编译期求值（Bool）；
/// - 只保留被选中的分支块内的顶层 items；
/// - 未选中分支完全被忽略：不会进入 index/resolve/typecheck/codegen，也不会触发错误；
/// - `else if (...)` 以 `else comptime if (...)` 的语法糖形式在 AST 中表现为 `ComptimeIfItemElse::If` 链。
///
/// 说明：
/// - 该裁剪步骤发生在“AST 阶段”，因此这里的求值仍属于 const/comptime 解释器 v0 的能力边界；
/// - 早期阶段只要求 `<cond>` 是可在编译期求值的 Bool；否则返回结构化诊断（稳定错误码）。
pub fn trim_package_level_comptime_ifs(
    source: &SourceFile,
    file: &mut ast::File,
) -> Result<(), ConstEvalError> {
    if !file
        .items
        .iter()
        .any(|it| matches!(it, ast::Item::ComptimeIf(_)))
    {
        return Ok(());
    }

    let trimmed = {
        // 这里借用 `file` 进行裁剪，但直到最终替换 `file.items` 之前都不写回，
        // 以避免 “解释器内部持有对 AST 节点的引用” 与 “移动 items” 之间的冲突。
        let file_ref: &ast::File = &*file;

        let mut interp = ConstInterpreter::with_options(
            ConstEvalCtx::new(source),
            file_ref,
            ConstEvalOptions::default(),
        );
        interp.register_file(source, file_ref);

        let mut out: Vec<ast::Item> = Vec::new();
        trim_package_level_items(&mut interp, &file_ref.items, &mut out, PreRegisterDecls::No)?;
        out
    };

    file.items = trimmed;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreRegisterDecls {
    No,
    Yes,
}

#[derive(Clone, Copy)]
struct RegisteredFun<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    fun: &'a ast::FunDecl,
}

#[derive(Clone, Copy)]
struct RegisteredType<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    decl: &'a ast::TypeDecl,
}

#[derive(Clone)]
struct ReflectionTypeTarget<'a> {
    full_name: String,
    simple_name: String,
    span: Span,
    decl: Option<RegisteredType<'a>>,
}

fn trim_package_level_items<'a>(
    interp: &mut ConstInterpreter<'a>,
    items: &'a [ast::Item],
    out: &mut Vec<ast::Item>,
    pre_register: PreRegisterDecls,
) -> Result<(), ConstEvalError> {
    // 在处理一个“被选中的分支块”之前，先把该块内直接出现的 type/fun 声明预注册到环境，
    // 以支持 const/comptime 里“先用后声明”的常见模式。
    if pre_register == PreRegisterDecls::Yes {
        interp.register_item_decls(items);
    }

    for item in items {
        match item {
            ast::Item::ComptimeIf(ci) => {
                if let Some(block) = interp.select_comptime_if_item_branch(ci)? {
                    trim_package_level_items(interp, &block.items, out, PreRegisterDecls::Yes)?;
                }
            }
            other => {
                // 顶层 const val 可以参与后续分支条件求值，因此按顺序执行并写入环境。
                interp.maybe_eval_top_level_const_val(other)?;
                out.push(other.clone());
            }
        }
    }

    Ok(())
}

fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    package
        .map(|pkg| {
            pkg.path
                .iter()
                .map(|id| id.text(source))
                .collect::<Vec<_>>()
                .join(".")
        })
        .unwrap_or_default()
}

fn top_level_fqn(pkg_prefix: &str, local: &str) -> String {
    if pkg_prefix.is_empty() {
        local.to_string()
    } else {
        format!("{pkg_prefix}.{local}")
    }
}

fn frontend_diagnostic<E>(error: E) -> ConstEvalError
where
    E: miette::Diagnostic + Send + Sync + 'static,
{
    ConstEvalError::Frontend(Box::new(error))
}

fn frontend_boxed_diagnostic(error: Box<dyn miette::Diagnostic + Send + Sync>) -> ConstEvalError {
    ConstEvalError::Frontend(error)
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("{message}")]
struct FrontendMessageDiagnostic {
    message: String,
}

fn frontend_message(message: String) -> ConstEvalError {
    frontend_boxed_diagnostic(Box::new(FrontendMessageDiagnostic { message }))
}

fn load_stdlib_source_paths() -> Result<Vec<std::path::PathBuf>, ConstEvalError> {
    let root = default_stdlib_path()
        .canonicalize()
        .map_err(|err| frontend_message(format!("无法定位 stdlib 目录：{err}")))?;
    let mut paths = Vec::new();
    collect_scoop_files_recursively(&root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn default_stdlib_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib")
}

fn collect_scoop_files_recursively(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) -> Result<(), ConstEvalError> {
    let entries = std::fs::read_dir(dir)
        .map_err(|err| frontend_message(format!("无法读取目录：{}: {err}", dir.display())))?;
    for entry in entries {
        let entry = entry
            .map_err(|err| frontend_message(format!("无法读取目录项：{}: {err}", dir.display())))?;
        let path = entry.path();
        let ty = entry.file_type().map_err(|err| {
            frontend_message(format!("无法读取文件类型：{}: {err}", path.display()))
        })?;
        if ty.is_dir() {
            collect_scoop_files_recursively(&path, out)?;
            continue;
        }
        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

/// `const fun` 调用与局部求值的解释器状态。
struct ConstInterpreter<'a> {
    default_int_ty: ConstIntTy,
    options: ConstEvalOptions,
    call_depth: usize,
    /// 作用域栈（后进先出）：局部 val/参数/顶层 const val 都放在这里。
    scopes: Vec<HashMap<String, ConstValue>>,
    /// generic `const fun` 的活动类型实参绑定（后进先出）。
    type_param_scopes: Vec<HashMap<String, TypeId>>,
    /// 当前求值栈上的 source/file 上下文。
    current_sources: Vec<&'a SourceFile>,
    current_files: Vec<&'a ast::File>,
    /// compilation-unit typecheck 产出的类型表；generic `const fun` 的 type args 与反射
    /// 都要复用它，避免再按 AST 文本猜测实例化结果。
    types: TypeStore,
    /// 顶层函数注册表（simple name → 声明集合）。
    funs_by_name: HashMap<String, Vec<RegisteredFun<'a>>>,
    /// 顶层函数注册表（FQN → overload set）。
    funs_by_fqn: HashMap<String, Vec<RegisteredFun<'a>>>,
    /// 顶层类型声明（按 simple name 聚合；用于反射 intrinsics v0）。
    types_by_name: HashMap<String, Vec<RegisteredType<'a>>>,
    /// 顶层类型声明（按 FQN 精确索引；generic reflection 需要绕过 simple-name 歧义）。
    types_by_fqn: HashMap<String, RegisteredType<'a>>,
}

impl<'a> ConstInterpreter<'a> {
    fn with_options(ctx: ConstEvalCtx<'a>, file: &'a ast::File, options: ConstEvalOptions) -> Self {
        Self::with_types(ctx, file, options, TypeStore::new())
    }

    fn with_types(
        ctx: ConstEvalCtx<'a>,
        file: &'a ast::File,
        options: ConstEvalOptions,
        types: TypeStore,
    ) -> Self {
        Self {
            default_int_ty: ctx.default_int_ty,
            options,
            call_depth: 0,
            scopes: vec![HashMap::new()],
            type_param_scopes: Vec::new(),
            current_sources: vec![ctx.source],
            current_files: vec![file],
            types,
            funs_by_name: HashMap::new(),
            funs_by_fqn: HashMap::new(),
            types_by_name: HashMap::new(),
            types_by_fqn: HashMap::new(),
        }
    }

    fn current_source(&self) -> &'a SourceFile {
        self.current_sources
            .last()
            .copied()
            .expect("const interpreter must always have a current source")
    }

    fn current_file(&self) -> &'a ast::File {
        self.current_files
            .last()
            .copied()
            .expect("const interpreter must always have a current file")
    }

    fn current_ctx(&self) -> ConstEvalCtx<'a> {
        ConstEvalCtx {
            source: self.current_source(),
            default_int_ty: self.default_int_ty,
        }
    }

    fn push_eval_frame(&mut self, source: &'a SourceFile, file: &'a ast::File) {
        self.current_sources.push(source);
        self.current_files.push(file);
    }

    fn pop_eval_frame(&mut self) {
        let _ = self.current_sources.pop();
        let _ = self.current_files.pop();
    }

    fn register_file(&mut self, source: &'a SourceFile, file: &'a ast::File) {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Fun(fun) => {
                    let local = fun.name.text(source).to_string();
                    let entry = RegisteredFun { source, file, fun };
                    self.funs_by_name
                        .entry(local.clone())
                        .or_default()
                        .push(entry);
                    self.funs_by_fqn
                        .entry(top_level_fqn(&pkg_prefix, &local))
                        .or_default()
                        .push(entry);
                }
                ast::Item::Type(ty) => {
                    let name = ty.name.text(source).to_string();
                    let entry = RegisteredType {
                        source,
                        file,
                        decl: ty,
                    };
                    self.types_by_name.entry(name).or_default().push(entry);
                    self.types_by_fqn
                        .insert(top_level_fqn(&pkg_prefix, ty.name.text(source)), entry);
                }
                ast::Item::TypeAlias(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Object(_)
                | ast::Item::Val(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }

    fn register_item_decls(&mut self, items: &'a [ast::Item]) {
        let source = self.current_source();
        let file = self.current_file();
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in items {
            match item {
                ast::Item::Fun(fun) => {
                    let local = fun.name.text(source).to_string();
                    let entry = RegisteredFun { source, file, fun };
                    self.funs_by_name
                        .entry(local.clone())
                        .or_default()
                        .push(entry);
                    self.funs_by_fqn
                        .entry(top_level_fqn(&pkg_prefix, &local))
                        .or_default()
                        .push(entry);
                }
                ast::Item::Type(ty) => {
                    let name = ty.name.text(source).to_string();
                    let entry = RegisteredType {
                        source,
                        file,
                        decl: ty,
                    };
                    self.types_by_name.entry(name).or_default().push(entry);
                    self.types_by_fqn
                        .insert(top_level_fqn(&pkg_prefix, ty.name.text(source)), entry);
                }
                // 只预注册“直接出现的”声明；`comptime if` 的分支选择应发生在裁剪时，
                // 因此这里刻意跳过它，避免把未选中分支的声明引入环境。
                ast::Item::ComptimeIf(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Object(_)
                | ast::Item::Val(_) => {}
            }
        }
    }

    fn eval_const_bindings_for_file(
        &mut self,
        source: &'a SourceFile,
        file: &'a ast::File,
    ) -> Result<Vec<ConstBinding>, ConstEvalError> {
        self.push_eval_frame(source, file);
        let result = self.eval_const_bindings(file);
        self.pop_eval_frame();
        result
    }

    fn maybe_eval_top_level_const_val(&mut self, item: &ast::Item) -> Result<(), ConstEvalError> {
        let ast::Item::Val(v) = item else {
            return Ok(());
        };
        if !v.modifiers.contains(&ast::Modifier::Const) {
            return Ok(());
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

        let ctx = self.current_ctx();
        let source = ctx.source;
        let name = name_ident.text(source).to_string();
        let init_value = eval_const_expr_with_host(ctx, self, init)?;
        let value = self.coerce_value_to_declared_type(source, init_value, v.ty.as_ref());

        self.define_local(name, value);
        Ok(())
    }

    fn select_comptime_if_item_branch<'b>(
        &mut self,
        ci: &'b ast::ComptimeIfItem,
    ) -> Result<Option<&'b ast::ItemBlock>, ConstEvalError> {
        let cond_v = eval_const_expr_with_host(self.current_ctx(), self, &ci.cond)?;
        let ConstValue::Bool(cond_b) = cond_v else {
            return Err(ConstEvalError::OperandTypeMismatch {
                expected: "Bool",
                found: value_kind(&cond_v),
                span: ci.cond.span.into(),
            });
        };

        if cond_b {
            return Ok(Some(&ci.then_branch));
        }

        match &ci.else_branch {
            None => Ok(None),
            Some(else_branch) => match &**else_branch {
                ast::ComptimeIfItemElse::Block(b) => Ok(Some(b)),
                ast::ComptimeIfItemElse::If(next) => self.select_comptime_if_item_branch(next),
            },
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

            let ctx = self.current_ctx();
            let source = ctx.source;
            let name = name_ident.text(source).to_string();
            let init_value = eval_const_expr_with_host(ctx, self, init)?;
            let value = self.coerce_value_to_declared_type(source, init_value, v.ty.as_ref());

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

    fn push_type_bindings(&mut self, bindings: impl IntoIterator<Item = (String, TypeId)>) {
        let mut frame = HashMap::new();
        for (name, ty) in bindings {
            frame.insert(name, ty);
        }
        self.type_param_scopes.push(frame);
    }

    fn pop_type_bindings(&mut self) {
        let _ = self.type_param_scopes.pop();
    }

    fn lookup_type_binding(&self, name: &str) -> Option<TypeId> {
        self.type_param_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn active_type_bindings_map(&self) -> HashMap<String, TypeId> {
        let mut bindings = HashMap::new();
        for scope in &self.type_param_scopes {
            for (name, ty) in scope {
                bindings.insert(name.clone(), *ty);
            }
        }
        bindings
    }

    fn apply_active_type_bindings(&mut self, ty: TypeId) -> TypeId {
        if self.type_param_scopes.is_empty() {
            return ty;
        }

        let bindings = self.active_type_bindings_map();
        if bindings.is_empty() {
            ty
        } else {
            substitute_type_params_in_store(&mut self.types, ty, &bindings)
        }
    }

    fn find_registered_fun_by_binding(
        &self,
        binding: &ast::TopLevelFunCallBinding,
    ) -> Option<RegisteredFun<'a>> {
        self.funs_by_fqn.get(&binding.fqn).and_then(|candidates| {
            candidates.iter().copied().find(|entry| {
                entry.source.path() == binding.decl_file.as_path()
                    && entry.fun.name.span == binding.decl_span
            })
        })
    }

    fn call_bound_const_fun(
        &mut self,
        call_span: Span,
        binding: &ast::TopLevelFunCallBinding,
        fallback_name: &str,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        let Some(fun) = self.find_registered_fun_by_binding(binding) else {
            return Err(ConstEvalError::UnknownConstFun {
                name: binding.fqn.clone(),
                span: call_span.into(),
            });
        };

        if !fun.fun.modifiers.contains(&ast::Modifier::Const) {
            return Err(ConstEvalError::CalleeNotConstFun {
                name: fallback_name.to_string(),
                span: call_span.into(),
            });
        }

        let type_args = binding
            .type_args
            .iter()
            .copied()
            .map(|ty| self.apply_active_type_bindings(ty))
            .collect();
        self.eval_fun_call(call_span, fun, type_args, args)
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
            .filter(|f| f.fun.modifiers.contains(&ast::Modifier::Const))
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
            .filter(|f| f.fun.params.len() == arity)
            .collect::<Vec<_>>();

        let fun = match arity_matches.as_slice() {
            [] => {
                // 早期阶段只按 arity 匹配；默认参数/命名参数/重载决议留给后续阶段。
                let expected = candidates
                    .iter()
                    .filter(|f| f.fun.modifiers.contains(&ast::Modifier::Const))
                    .map(|f| f.fun.params.len())
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

        self.eval_fun_call(call_span, fun, Vec::new(), args)
    }

    fn call_fun_or_intrinsic(
        &mut self,
        call_span: Span,
        callee_name: &str,
        type_args: Vec<ast::TypeRef>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        let selected_binding = self.current_file().top_level_fun_call_binding(call_span);

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

        if let Some(binding) = selected_binding
            && !binding.is_intrinsic
        {
            return self.call_bound_const_fun(call_span, &binding, callee_name, args);
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

        let target = self.resolve_reflection_type_target(&type_args[0])?;
        let full_name = target.full_name.clone();
        let simple_name = target.simple_name.clone();
        let ty_span = target.span;

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
                    self.default_int_ty,
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
                    self.default_int_ty,
                    align as u128,
                )))
            }
            "fieldsOf" => {
                let decl = target
                    .decl
                    .ok_or_else(|| ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    })?;
                let decl_source = decl.source;
                let decl_file = decl.file;
                let decl = decl.decl;
                if decl.kind != ast::TypeKind::Struct && decl.kind != ast::TypeKind::Class {
                    return Err(ConstEvalError::ReflectionUnsupportedTarget {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    });
                }

                self.push_eval_frame(decl_source, decl_file);
                let result = (|| {
                    let mut fields: Vec<ConstValue> = Vec::new();
                    let mut seen: std::collections::BTreeSet<String> =
                        std::collections::BTreeSet::new();

                    // 1) 主构造 `val/var` 参数声明的字段
                    if let Some(ctor) = decl.primary_ctor.as_ref() {
                        for p in &ctor.params {
                            if p.kind.is_none() {
                                continue;
                            }
                            let fname = p.name.text(decl_source).to_string();
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

                            let fname = p.name.text(decl_source).to_string();
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
                })();
                self.pop_eval_frame();
                result
            }
            "variantsOf" => {
                let decl = target
                    .decl
                    .ok_or_else(|| ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    })?;
                let decl_source = decl.source;
                let decl_file = decl.file;
                let decl = decl.decl;
                if decl.kind != ast::TypeKind::Enum {
                    return Err(ConstEvalError::ReflectionVariantsOfUnsupportedTarget {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    });
                }

                self.push_eval_frame(decl_source, decl_file);
                let result = (|| {
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
                })();
                self.pop_eval_frame();
                result
            }
            "superTypesOf" => {
                let decl = target
                    .decl
                    .ok_or_else(|| ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    })?;
                self.push_eval_frame(decl.source, decl.file);
                let result = (|| {
                    let decl = decl.decl;
                    let mut supers: Vec<ConstValue> = Vec::with_capacity(decl.supertypes.len());
                    for st in &decl.supertypes {
                        supers.push(self.mk_type_meta(Some(&st.ty))?);
                    }
                    Ok(ConstValue::Tuple(supers))
                })();
                self.pop_eval_frame();
                result
            }
            "annotationsOf" => {
                let decl = target
                    .decl
                    .ok_or_else(|| ConstEvalError::ReflectionUnknownType {
                        name: full_name.clone(),
                        span: ty_span.into(),
                    })?;
                self.push_eval_frame(decl.source, decl.file);
                let result = (|| {
                    let decl = decl.decl;
                    let mut anns: Vec<ConstValue> = Vec::new();
                    for a in &decl.annotations {
                        // `annotationsOf<T>()` 只返回“类型本身”的注解：忽略 use-site target。
                        if a.use_site_target.is_some() {
                            continue;
                        }
                        anns.push(self.mk_annotation_meta(a)?);
                    }
                    Ok(ConstValue::Tuple(anns))
                })();
                self.pop_eval_frame();
                result
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

        self.push_eval_frame(fun.source, fun.file);
        let mut params: Vec<ConstValue> = Vec::with_capacity(fun.fun.params.len());
        for (idx, p) in fun.fun.params.iter().enumerate() {
            params.push(self.mk_param_meta(p, idx)?);
        }
        self.pop_eval_frame();
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

    fn lookup_unique_type_decl(&self, simple_name: &str) -> Option<RegisteredType<'a>> {
        let decls = self.types_by_name.get(simple_name)?;
        match decls.as_slice() {
            [one] => Some(*one),
            _ => None,
        }
    }

    fn lookup_type_decl_for_type_id(&self, ty: TypeId) -> Option<RegisteredType<'a>> {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
                self.types_by_fqn
                    .get(&n.fqn)
                    .copied()
                    .or_else(|| self.lookup_unique_type_decl(&self.type_id_simple_name(ty)))
            }
            _ => self.lookup_unique_type_decl(&self.type_id_simple_name(ty)),
        }
    }

    fn type_id_simple_name(&self, ty: TypeId) -> String {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Any) => "Any".to_string(),
            TypeKind::Ref(RefTypeKind::String) => "String".to_string(),
            TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
                self.types_by_fqn
                    .get(&n.fqn)
                    .map(|decl| decl.decl.name.text(decl.source).to_string())
                    .unwrap_or_else(|| {
                        n.fqn
                            .rsplit('.')
                            .next()
                            .unwrap_or(n.fqn.as_str())
                            .to_string()
                    })
            }
            TypeKind::Ref(RefTypeKind::Function(_)) => "Function".to_string(),
            TypeKind::Ref(RefTypeKind::Union(_)) => self.type_id_to_stable_name(ty),
            TypeKind::StarProjection(_) => "*".to_string(),
            TypeKind::Value(ValueTypeKind::Unit) => "Unit".to_string(),
            TypeKind::Value(ValueTypeKind::Nothing) => "Nothing".to_string(),
            TypeKind::Value(ValueTypeKind::Bool) => "Bool".to_string(),
            TypeKind::Value(ValueTypeKind::Char) => "Char".to_string(),
            TypeKind::Value(ValueTypeKind::Float64) => "Float64".to_string(),
            TypeKind::Value(ValueTypeKind::Float32) => "Float32".to_string(),
            TypeKind::Value(ValueTypeKind::Int) => "Int".to_string(),
            TypeKind::Value(ValueTypeKind::UInt) => "UInt".to_string(),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => format!("Int{bits}"),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => format!("UInt{bits}"),
            TypeKind::Value(ValueTypeKind::Option(_)) => "Option".to_string(),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                if elements.is_empty() {
                    "Unit".to_string()
                } else {
                    self.type_id_to_stable_name(ty)
                }
            }
            TypeKind::Param(p) => p.name.clone(),
        }
    }

    fn type_id_to_stable_name(&self, ty: TypeId) -> String {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Any) => "Any".to_string(),
            TypeKind::Ref(RefTypeKind::String) => "String".to_string(),
            TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
                let mut out = self.type_id_simple_name(ty);
                if !n.args.is_empty() || n.eff.is_some() {
                    let mut args: Vec<String> = n
                        .args
                        .iter()
                        .copied()
                        .map(|arg| self.type_id_to_stable_name(arg))
                        .collect();
                    if let Some(eff) = &n.eff {
                        args.push(format!("eff {}", self.effect_row_to_stable_name(eff)));
                    }
                    out.push('<');
                    out.push_str(&args.join(", "));
                    out.push('>');
                }
                out
            }
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                let mut out = String::new();
                if let Some(receiver) = fun.receiver {
                    out.push_str(&self.type_id_to_stable_name(receiver));
                    out.push('.');
                }
                out.push('(');
                out.push_str(
                    &fun.params
                        .iter()
                        .copied()
                        .map(|param| self.type_id_to_stable_name(param))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                out.push_str(") -> ");
                out.push_str(&self.type_id_to_stable_name(fun.return_ty));
                out.push_str(" / ");
                out.push_str(&self.effect_row_to_stable_name(&fun.effects));
                if fun.effects_closed {
                    out.push('!');
                }
                out
            }
            TypeKind::Ref(RefTypeKind::Union(union)) => union
                .variants
                .iter()
                .copied()
                .map(|variant| self.type_id_to_stable_name(variant))
                .collect::<Vec<_>>()
                .join(" | "),
            TypeKind::StarProjection(_) => "*".to_string(),
            TypeKind::Value(ValueTypeKind::Unit) => "Unit".to_string(),
            TypeKind::Value(ValueTypeKind::Nothing) => "Nothing".to_string(),
            TypeKind::Value(ValueTypeKind::Bool) => "Bool".to_string(),
            TypeKind::Value(ValueTypeKind::Char) => "Char".to_string(),
            TypeKind::Value(ValueTypeKind::Float64) => "Float64".to_string(),
            TypeKind::Value(ValueTypeKind::Float32) => "Float32".to_string(),
            TypeKind::Value(ValueTypeKind::Int) => "Int".to_string(),
            TypeKind::Value(ValueTypeKind::UInt) => "UInt".to_string(),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => format!("Int{bits}"),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => format!("UInt{bits}"),
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                format!("Option<{}>", self.type_id_to_stable_name(*inner))
            }
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                if elements.is_empty() {
                    return "Unit".to_string();
                }
                let mut out = String::from("(");
                out.push_str(
                    &elements
                        .iter()
                        .copied()
                        .map(|element| self.type_id_to_stable_name(element))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                if elements.len() == 1 {
                    out.push(',');
                }
                out.push(')');
                out
            }
            TypeKind::Param(p) => p.name.clone(),
        }
    }

    fn effect_row_to_stable_name(&self, row: &EffectRow) -> String {
        if row.is_pure() {
            return "Pure".to_string();
        }
        if row.terms.len() == 1 {
            return self.type_id_to_stable_name(row.terms[0]);
        }
        row.terms
            .iter()
            .copied()
            .map(|term| self.type_id_to_stable_name(term))
            .collect::<Vec<_>>()
            .join(" + ")
    }

    fn try_resolve_bound_type_ref(&self, ty: &ast::TypeRef) -> Option<TypeId> {
        let ast::TypeRef::Path(path) = ty else {
            return None;
        };
        if path.segments.len() != 1 || !path.args.is_empty() {
            return None;
        }
        self.lookup_type_binding(path.segments[0].text(self.current_source()))
    }

    fn resolve_reflection_type_target(
        &self,
        ty: &ast::TypeRef,
    ) -> Result<ReflectionTypeTarget<'a>, ConstEvalError> {
        let ast::TypeRef::Path(path) = ty else {
            return Err(ConstEvalError::ReflectionTypeArgNotSupported {
                found: "non-path type",
                span: ty.span().into(),
            });
        };

        if let Some(bound_ty) = self.try_resolve_bound_type_ref(ty) {
            return Ok(ReflectionTypeTarget {
                full_name: self.type_id_to_stable_name(bound_ty),
                simple_name: self.type_id_simple_name(bound_ty),
                span: path.span,
                decl: self.lookup_type_decl_for_type_id(bound_ty),
            });
        }

        let full_name =
            self.type_ref_to_string(ty)
                .ok_or(ConstEvalError::ReflectionTypeArgNotSupported {
                    found: "non-path type",
                    span: ty.span().into(),
                })?;
        let simple_name = path
            .segments
            .last()
            .map(|s| s.text(self.current_source()).to_string())
            .unwrap_or_default();
        let decl = match self.types_by_name.get(&simple_name) {
            Some(decls) => match decls.as_slice() {
                [one] => Some(*one),
                _ => {
                    return Err(ConstEvalError::ReflectionAmbiguousType {
                        name: full_name.clone(),
                        span: path.span.into(),
                    });
                }
            },
            None => None,
        };
        Ok(ReflectionTypeTarget {
            full_name,
            simple_name,
            span: path.span,
            decl,
        })
    }

    /// 把一个类型引用“降级”为 TypeMeta。
    ///
    /// 说明：
    /// - 对普通 AST type ref，仍保持“尽力而为”的轻量策略；
    /// - 若 type ref 是当前 generic `const fun` 的类型参数，会先按调用点实参解析为具体类型，
    ///   再构造与实例化结果一致的 `TypeMeta`。
    fn mk_type_meta(&mut self, ty: Option<&ast::TypeRef>) -> Result<ConstValue, ConstEvalError> {
        if let Some(ty) = ty
            && let Some(bound_ty) = self.try_resolve_bound_type_ref(ty)
        {
            return self.mk_type_meta_from_type_id(bound_ty);
        }

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
                    .map(|s| s.text(self.current_source()))
                    .unwrap_or("");
                if let Some(decl) = self.lookup_unique_type_decl(simple) {
                    self.push_eval_frame(decl.source, decl.file);
                    let kind = match decl.decl.kind {
                        ast::TypeKind::Struct => self.mk_type_kind("Struct"),
                        ast::TypeKind::Enum => self.mk_type_kind("Enum"),
                        ast::TypeKind::Class => self.mk_type_kind("Class"),
                        ast::TypeKind::Interface => self.mk_type_kind("Interface"),
                        ast::TypeKind::Effect => self.mk_type_kind("Effect"),
                    };
                    let annotations = self.mk_annotation_list(&decl.decl.annotations, true)?;
                    self.pop_eval_frame();
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

    fn mk_type_meta_from_type_id(&mut self, ty: TypeId) -> Result<ConstValue, ConstEvalError> {
        let name = self.type_id_to_stable_name(ty);
        let decl = self.lookup_type_decl_for_type_id(ty);

        let (kind, annotations) = match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Unit) => (self.mk_type_kind("Tuple"), Vec::new()),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) if elements.is_empty() => {
                (self.mk_type_kind("Tuple"), Vec::new())
            }
            _ => {
                if let Some(decl) = decl {
                    self.push_eval_frame(decl.source, decl.file);
                    let kind = match decl.decl.kind {
                        ast::TypeKind::Struct => self.mk_type_kind("Struct"),
                        ast::TypeKind::Enum => self.mk_type_kind("Enum"),
                        ast::TypeKind::Class => self.mk_type_kind("Class"),
                        ast::TypeKind::Interface => self.mk_type_kind("Interface"),
                        ast::TypeKind::Effect => self.mk_type_kind("Effect"),
                    };
                    let annotations = self.mk_annotation_list(&decl.decl.annotations, true)?;
                    self.pop_eval_frame();
                    (kind, annotations)
                } else {
                    (self.mk_type_kind("Primitive"), Vec::new())
                }
            }
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
        &mut self,
        name: String,
        ty: Option<&ast::TypeRef>,
        index: usize,
        annotations: &[ast::AnnotationUse],
    ) -> Result<ConstValue, ConstEvalError> {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "index".to_string(),
            ConstValue::Int(super::ConstInt::new(self.default_int_ty, index as u128)),
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

    fn mk_param_meta(
        &mut self,
        p: &ast::Param,
        index: usize,
    ) -> Result<ConstValue, ConstEvalError> {
        let pname = p.name.text(self.current_source()).to_string();

        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "index".to_string(),
            ConstValue::Int(super::ConstInt::new(self.default_int_ty, index as u128)),
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
        &mut self,
        v: &ast::EnumVariantDecl,
        index: usize,
    ) -> Result<ConstValue, ConstEvalError> {
        let vname = v.name.text(self.current_source()).to_string();

        let mut field_metas: Vec<ConstValue> = Vec::with_capacity(v.params.len());
        for (idx, p) in v.params.iter().enumerate() {
            let fname = p.name.text(self.current_source()).to_string();
            field_metas.push(self.mk_field_meta(fname, p.ty.as_ref(), idx, &p.annotations)?);
        }

        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "index".to_string(),
            ConstValue::Int(super::ConstInt::new(self.default_int_ty, index as u128)),
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
            .map(|id| id.text(self.current_source()))
            .collect::<Vec<_>>()
            .join(".");
        let simple = a
            .path
            .last()
            .map(|id| id.text(self.current_source()).to_string())
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
                Some(id) => id.text(self.current_source()).to_string(),
                None => {
                    let name = ctor_params
                        .and_then(|ps| ps.get(positional_index))
                        .map(|p| p.name.text(self.current_source()).to_string())
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
            let value = eval_const_expr(self.current_ctx(), &arg.value)?;
            provided_by_name.insert(arg_name.clone(), value.clone());
            provided_order.push((arg_name, value));
        }

        let args: Vec<ConstValue> = match ctor_params {
            Some(params) => {
                let mut remaining = provided_by_name;
                let mut out: Vec<ConstValue> = Vec::new();

                // 1) 按 ctor 参数顺序输出（含默认值）。
                for p in params {
                    let pname = p.name.text(self.current_source()).to_string();
                    if let Some(v) = remaining.remove(&pname) {
                        out.push(self.mk_annotation_arg_meta_value(pname, v));
                        continue;
                    }
                    if let Some(default_value) = p.default_value.as_ref() {
                        let v = eval_const_expr(self.current_ctx(), default_value)?;
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
        if !decl.decl.modifiers.contains(&ast::Modifier::Annotation) {
            return None;
        }
        let ctor = decl.decl.primary_ctor.as_ref()?;
        Some(ctor.params.as_slice())
    }

    /// 把 `TypeRef` 格式化为稳定的字符串（用于 `TypeMeta.name`）。
    ///
    /// 说明：
    /// - 默认保持“语法层面”的名字（基于 AST），并不保证是全限定名；
    /// - 若当前处于 generic `const fun` 实例化环境，会先把 `T/U/...` 替换为调用点选定的
    ///   具体类型，再继续做稳定字符串格式化。
    fn type_ref_to_string(&self, ty: &ast::TypeRef) -> Option<String> {
        match ty {
            ast::TypeRef::Path(p) => {
                if let Some(bound_ty) = self.try_resolve_bound_type_ref(ty) {
                    return Some(self.type_id_to_stable_name(bound_ty));
                }
                let mut out = p
                    .segments
                    .iter()
                    .map(|id| id.text(self.current_source()))
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

    fn eval_fun_call(
        &mut self,
        call_span: Span,
        fun: RegisteredFun<'a>,
        type_args: Vec<TypeId>,
        args: Vec<ConstValue>,
    ) -> Result<ConstValue, ConstEvalError> {
        let decl = fun.fun;
        let decl_source = fun.source;
        let decl_file = fun.file;
        let fun_name = decl.name.text(decl_source).to_string();
        if self.call_depth >= self.options.recursion_limit {
            return Err(ConstEvalError::RecursionLimitExceeded {
                name: fun_name,
                limit: self.options.recursion_limit,
                span: call_span.into(),
            });
        }

        // 解释器入口做一次“最小签名门禁”，避免把复杂语义带入 v0。
        if decl.receiver.is_some() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "extension receiver",
                span: decl.span.into(),
            });
        }
        if decl.type_params.len() != type_args.len() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "generic type args",
                span: decl.span.into(),
            });
        }
        if decl.eff_param.is_some() {
            return Err(ConstEvalError::UnsupportedConstFunSignature {
                reason: "effect row param",
                span: decl.span.into(),
            });
        }
        if decl.params.len() != args.len() {
            return Err(ConstEvalError::ConstFunArityMismatch {
                name: decl.name.text(decl_source).to_string(),
                expected: decl.params.len(),
                found: args.len(),
                span: call_span.into(),
            });
        }

        self.call_depth += 1;
        self.push_eval_frame(decl_source, decl_file);
        self.push_scope();
        if !type_args.is_empty() {
            let bindings = decl
                .type_params
                .iter()
                .zip(type_args.iter().copied())
                .map(|(param, ty)| (param.name.text(decl_source).to_string(), ty));
            self.push_type_bindings(bindings);
        }

        let result = (|| {
            // 参数绑定写入当前 frame scope。
            for (param, arg) in decl.params.iter().zip(args) {
                let name = param.name.text(decl_source).to_string();
                let value = self.coerce_value_to_declared_type(decl_source, arg, param.ty.as_ref());
                self.define_local(name, value);
            }

            let ret = match &decl.body {
                ast::FunBody::Block(b) => match self.eval_block(b)? {
                    ControlFlow::Break(v) | ControlFlow::Continue(v) => v,
                },
                ast::FunBody::Missing => {
                    return Err(ConstEvalError::UnsupportedConstFunSignature {
                        reason: "missing body",
                        span: decl.span.into(),
                    });
                }
            };
            Ok(self.coerce_value_to_declared_type(decl_source, ret, decl.return_ty.as_ref()))
        })();

        if !type_args.is_empty() {
            self.pop_type_bindings();
        }
        self.pop_scope();
        self.pop_eval_frame();
        self.call_depth -= 1;
        result
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
                let v = eval_const_expr_with_host(self.current_ctx(), self, e)?;
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

                let ctx = self.current_ctx();
                let init_value = eval_const_expr_with_host(ctx, self, init)?;
                let value =
                    self.coerce_value_to_declared_type(ctx.source, init_value, v.ty.as_ref());
                self.define_local(name.text(ctx.source).to_string(), value);
                Ok(ControlFlow::Continue(None))
            }
            ast::StmtKind::Return { value, .. } => {
                let v = match value {
                    Some(expr) => eval_const_expr_with_host(self.current_ctx(), self, expr)?,
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
        let cond_v = eval_const_expr_with_host(self.current_ctx(), self, &ci.cond)?;
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
        let binder_name = cf.binder.text(self.current_source()).to_string();

        // 1) 整数范围：`a..b`
        if let ast::ExprKind::Binary {
            lhs,
            op: ast::BinaryOp::RangeInclusive,
            rhs,
            ..
        } = &cf.iter.kind
        {
            let lv = eval_const_expr_with_host(self.current_ctx(), self, lhs)?;
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

            let rv = eval_const_expr_with_host(self.current_ctx(), self, rhs)?;
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
        let iter_v = eval_const_expr_with_host(self.current_ctx(), self, &cf.iter)?;
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

    fn builtin_float_ty_from_type_id(&self, ty: TypeId) -> Option<ConstFloatTy> {
        match self.types.kind(ty) {
            TypeKind::Value(ValueTypeKind::Float64) => Some(ConstFloatTy::Float64),
            TypeKind::Value(ValueTypeKind::Float32) => Some(ConstFloatTy::Float32),
            _ => None,
        }
    }

    fn builtin_float_ty_from_type_ref(
        &self,
        source: &SourceFile,
        ty: &ast::TypeRef,
    ) -> Option<ConstFloatTy> {
        if let Some(bound_ty) = self.try_resolve_bound_type_ref(ty) {
            return self.builtin_float_ty_from_type_id(bound_ty);
        }

        match ty {
            ast::TypeRef::Path(path) => {
                let name = path
                    .segments
                    .iter()
                    .map(|seg| seg.text(source))
                    .collect::<Vec<_>>()
                    .join(".");
                match name.as_str() {
                    "Float64" | "Double" | "scoop.core.Float64" => Some(ConstFloatTy::Float64),
                    "Float32" | "scoop.core.Float32" => Some(ConstFloatTy::Float32),
                    _ => None,
                }
            }
            ast::TypeRef::Nullable { inner, .. } => {
                self.builtin_float_ty_from_type_ref(source, inner)
            }
            ast::TypeRef::Tuple(_)
            | ast::TypeRef::Star { .. }
            | ast::TypeRef::EffectRowArg { .. }
            | ast::TypeRef::Function(_) => None,
        }
    }

    fn coerce_value_to_declared_type(
        &self,
        source: &SourceFile,
        value: ConstValue,
        ty: Option<&ast::TypeRef>,
    ) -> ConstValue {
        let Some(target_ty) = ty.and_then(|t| self.builtin_float_ty_from_type_ref(source, t))
        else {
            return value;
        };

        match value {
            ConstValue::Float(f) => ConstValue::Float(f.cast(target_ty)),
            other => other,
        }
    }
}

fn substitute_type_params_in_store(
    types: &mut TypeStore,
    ty: TypeId,
    param_map: &HashMap<String, TypeId>,
) -> TypeId {
    match types.kind(ty).clone() {
        TypeKind::Param(p) => param_map.get(&p.name).copied().unwrap_or(ty),
        TypeKind::StarProjection(star) => {
            let read_ty = substitute_type_params_in_store(types, star.read_ty, param_map);
            if read_ty == star.read_ty {
                ty
            } else {
                types.ty_star_projection(read_ty)
            }
        }
        TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String)
        | TypeKind::Value(ValueTypeKind::Unit)
        | TypeKind::Value(ValueTypeKind::Nothing)
        | TypeKind::Value(ValueTypeKind::Bool)
        | TypeKind::Value(ValueTypeKind::Char)
        | TypeKind::Value(ValueTypeKind::Float64)
        | TypeKind::Value(ValueTypeKind::Float32)
        | TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::IntN(_))
        | TypeKind::Value(ValueTypeKind::UIntN(_)) => ty,
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let new_inner = substitute_type_params_in_store(types, inner, param_map);
            if new_inner == inner {
                ty
            } else {
                types.ty_option(new_inner)
            }
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let mut changed = false;
            let new_elements: Vec<TypeId> = elements
                .into_iter()
                .map(|element| {
                    let new_element = substitute_type_params_in_store(types, element, param_map);
                    if new_element != element {
                        changed = true;
                    }
                    new_element
                })
                .collect();
            if changed {
                types.ty_tuple(new_elements)
            } else {
                ty
            }
        }
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            let mut changed = false;
            let args: Vec<TypeId> = nominal
                .args
                .into_iter()
                .map(|arg| {
                    let new_arg = substitute_type_params_in_store(types, arg, param_map);
                    if new_arg != arg {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            let eff = nominal.eff.map(|row| {
                let new_row = substitute_type_params_in_effect_row(types, &row, param_map);
                if new_row != row {
                    changed = true;
                }
                new_row
            });
            if changed {
                types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                    fqn: nominal.fqn,
                    args,
                    eff,
                })))
            } else {
                ty
            }
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let mut changed = false;
            let args: Vec<TypeId> = nominal
                .args
                .into_iter()
                .map(|arg| {
                    let new_arg = substitute_type_params_in_store(types, arg, param_map);
                    if new_arg != arg {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            let eff = nominal.eff.map(|row| {
                let new_row = substitute_type_params_in_effect_row(types, &row, param_map);
                if new_row != row {
                    changed = true;
                }
                new_row
            });
            if changed {
                types.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                    fqn: nominal.fqn,
                    args,
                    eff,
                })))
            } else {
                ty
            }
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let mut changed = false;
            let receiver = fun.receiver.map(|receiver| {
                let new_receiver = substitute_type_params_in_store(types, receiver, param_map);
                if new_receiver != receiver {
                    changed = true;
                }
                new_receiver
            });
            let params: Vec<TypeId> = fun
                .params
                .into_iter()
                .map(|param| {
                    let new_param = substitute_type_params_in_store(types, param, param_map);
                    if new_param != param {
                        changed = true;
                    }
                    new_param
                })
                .collect();
            let return_ty = substitute_type_params_in_store(types, fun.return_ty, param_map);
            if return_ty != fun.return_ty {
                changed = true;
            }
            let effects = substitute_type_params_in_effect_row(types, &fun.effects, param_map);
            if effects != fun.effects {
                changed = true;
            }
            if changed {
                types.ty_function(receiver, params, return_ty, effects, fun.effects_closed)
            } else {
                ty
            }
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            let mut changed = false;
            let variants: Vec<TypeId> = union
                .variants
                .into_iter()
                .map(|variant| {
                    let new_variant = substitute_type_params_in_store(types, variant, param_map);
                    if new_variant != variant {
                        changed = true;
                    }
                    new_variant
                })
                .collect();
            if changed {
                types.ty_union(variants)
            } else {
                ty
            }
        }
    }
}

fn substitute_type_params_in_effect_row(
    types: &mut TypeStore,
    row: &EffectRow,
    param_map: &HashMap<String, TypeId>,
) -> EffectRow {
    let mut changed = false;
    let terms: Vec<TypeId> = row
        .terms
        .iter()
        .copied()
        .map(|term| {
            let new_term = substitute_type_params_in_store(types, term, param_map);
            if new_term != term {
                changed = true;
            }
            new_term
        })
        .collect();
    if changed {
        EffectRow::new(terms)
    } else {
        EffectRow { terms }
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
