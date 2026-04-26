//! generic MIR template -> monomorphic MIR instance materialization（当前先服务 dump-ir）。
//!
//! 当前阶段的目标边界：
//! - 在 MIR 层定义稳定的 `TemplateKey` / `InstanceKey`；
//! - 用 typecheck 收集到的“实例请求”作为初始种子；
//! - 基于 generic MIR template 做单态实例物化，而不是对每个实例重新回到 HIR lowering；
//! - 先覆盖 dump/调试路径需要的最小闭环：standalone direct-call fixed-point、nested closure family
//!   的 FQN/fn_ptr 重写，以及 per-instance cache。

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::monomorph::MonomorphKey;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
};
use crate::typecheck;
use crate::typecheck::{
    AnnotationError, ExprTypeError, StructDeclError, TypeEnv, TypeEnvError, TypeHeaderError,
    TypeLowerError,
};

use super::{
    Body, CallArg, CallKind, ConstValue, File, FunDecl, HandlerArm, Item, LocalDecl,
    MemberAccessMetadata, MemberTarget, Operand, Param, Pattern, PerformMetadata, Rvalue,
    Statement, StatementKind, Terminator, TerminatorKind,
};

/// 一个 generic MIR template 的稳定标识。
///
/// 说明：
/// - `fqn` 给出语言级声明身份；
/// - `source_path + decl_span` 用于区分同名 overload / 多文件重复 span；
/// - 后续编译单元主路径也应复用这一层语义，而不是退回 mangled symbol name。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TemplateKey {
    pub fqn: String,
    pub source_path: PathBuf,
    pub decl_span: Span,
}

impl fmt::Debug for TemplateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{}:{:?}",
            self.fqn,
            self.source_path.display(),
            self.decl_span
        )
    }
}

/// 一个 monomorphic MIR instance 的稳定身份。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub template: TemplateKey,
    pub type_args: Vec<TypeId>,
    pub eff_args: Vec<EffectRow>,
}

impl fmt::Debug for InstanceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstanceKey")
            .field("template", &self.template)
            .field("type_args", &TypeIdList(&self.type_args))
            .field("eff_args", &EffectRowList(&self.eff_args))
            .finish()
    }
}

struct TypeIdList<'a>(&'a [TypeId]);

impl fmt::Debug for TypeIdList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

struct TypeIdRepr(TypeId);

impl fmt::Debug for TypeIdRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0.as_u32())
    }
}

struct EffectRowList<'a>(&'a [EffectRow]);

impl fmt::Debug for EffectRowList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(EffectRowRepr))
            .finish()
    }
}

struct EffectRowRepr<'a>(&'a EffectRow);

impl fmt::Debug for EffectRowRepr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_pure() {
            return write!(f, "Pure");
        }
        f.debug_list()
            .entries(self.0.terms.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

/// `dump-ir` / tests 使用的 monomorphic MIR 输出。
#[derive(Debug)]
pub struct MaterializedMir {
    pub file: File,
    pub types: TypeStore,
    pub instance_keys: Vec<InstanceKey>,
}

/// MIR 实例化错误。
#[derive(Debug, Error, Diagnostic)]
pub enum MirMaterializeError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Hir(#[from] crate::hir::HirLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Comptime(#[from] crate::comptime::ConstEvalError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolve(#[from] ResolveError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeHeader(#[from] TypeHeaderError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    StructDecl(#[from] StructDeclError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeEnv(#[from] TypeEnvError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLowering(#[from] TypeLowerError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Annotation(#[from] AnnotationError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ExprType(#[from] ExprTypeError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    VtableLayout(#[from] crate::vtable::VtableLayoutError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ItableLayout(#[from] crate::itable::ItableLayoutError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    MirLower(#[from] super::MirLowerError),

    #[error("{message}")]
    Frontend { message: String },

    #[error("实例请求找不到对应的 generic template：{fqn}@{file}:{span:?}")]
    #[diagnostic(code(scoop::mir::materialize::missing_generic_template))]
    MissingGenericTemplate {
        fqn: String,
        file: String,
        span: Span,
    },

    #[error("generic template 没有匹配的 MIR 根函数：{fqn}@{file}:{span:?}")]
    #[diagnostic(code(scoop::mir::materialize::missing_mir_root_for_template))]
    MissingMirRootForTemplate {
        fqn: String,
        file: String,
        span: Span,
    },

    #[error("实例化的 type args 数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个")]
    #[diagnostic(code(scoop::mir::materialize::type_arg_arity_mismatch))]
    TypeArgArityMismatch {
        fqn: String,
        expected: usize,
        found: usize,
        #[label("模板声明在这里")]
        decl_span: miette::SourceSpan,
    },

    #[error("实例化的 effect args 数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个")]
    #[diagnostic(code(scoop::mir::materialize::effect_arg_arity_mismatch))]
    EffectArgArityMismatch {
        fqn: String,
        expected: usize,
        found: usize,
        #[label("模板声明在这里")]
        decl_span: miette::SourceSpan,
    },
}

type MaterializeResult<T> = Result<T, Box<MirMaterializeError>>;

fn materialize_err(error: MirMaterializeError) -> Box<MirMaterializeError> {
    Box::new(error)
}

impl From<crate::hir::HirLowerError> for Box<MirMaterializeError> {
    fn from(error: crate::hir::HirLowerError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<ParseError> for Box<MirMaterializeError> {
    fn from(error: ParseError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<crate::comptime::ConstEvalError> for Box<MirMaterializeError> {
    fn from(error: crate::comptime::ConstEvalError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<ResolveError> for Box<MirMaterializeError> {
    fn from(error: ResolveError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<TypeHeaderError> for Box<MirMaterializeError> {
    fn from(error: TypeHeaderError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<StructDeclError> for Box<MirMaterializeError> {
    fn from(error: StructDeclError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<TypeEnvError> for Box<MirMaterializeError> {
    fn from(error: TypeEnvError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<TypeLowerError> for Box<MirMaterializeError> {
    fn from(error: TypeLowerError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<AnnotationError> for Box<MirMaterializeError> {
    fn from(error: AnnotationError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<ExprTypeError> for Box<MirMaterializeError> {
    fn from(error: ExprTypeError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<crate::vtable::VtableLayoutError> for Box<MirMaterializeError> {
    fn from(error: crate::vtable::VtableLayoutError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<crate::itable::ItableLayoutError> for Box<MirMaterializeError> {
    fn from(error: crate::itable::ItableLayoutError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

impl From<super::MirLowerError> for Box<MirMaterializeError> {
    fn from(error: super::MirLowerError) -> Self {
        materialize_err(MirMaterializeError::from(error))
    }
}

fn frontend_err(message: impl Into<String>) -> Box<MirMaterializeError> {
    materialize_err(MirMaterializeError::Frontend {
        message: message.into(),
    })
}

/// 为 `dump-ir` / tests 生成 monomorphic MIR instances。
pub fn materialize_for_dump(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<MaterializedMir> {
    let DumpMaterializationInputs {
        prepared_files,
        index,
        env,
        typecheck_types,
        monomorph_keys,
        top_level_fun_value_refs,
        top_level_fun_call_bindings,
    } = collect_dump_materialization_inputs(session, source)?;
    let template_catalog = collect_generic_template_infos(&prepared_files);
    let compilation_unit = prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let mut lowered_hir = crate::hir::lower_for_compilation_unit_multi_files_with_type_env(
        &index,
        &compilation_unit,
        &compilation_unit,
        &[],
        Some(&env),
        &typecheck_types,
    )?;
    let builtins = lowered_hir.types.intern_builtins();
    let facts = super::MirLoweringFacts::from_lowered_hir(&lowered_hir);
    let generic_file = super::lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &facts,
    );
    let types = lowered_hir.types;

    materialize_generic_mir_for_dump(
        generic_file,
        types,
        builtins,
        DumpMaterializeRequestSet {
            typecheck_types: &typecheck_types,
            monomorph_keys: &monomorph_keys,
            template_infos: template_catalog,
            top_level_fun_value_refs,
            top_level_fun_call_bindings,
        },
    )
}

#[derive(Clone)]
struct PreparedDumpFile {
    source: SourceFile,
    ast: ast::File,
    extend_type_env: bool,
    collect_monomorph_keys: bool,
}

struct DumpMaterializationInputs {
    prepared_files: Vec<PreparedDumpFile>,
    index: Index,
    env: TypeEnv,
    typecheck_types: TypeStore,
    monomorph_keys: Vec<MonomorphKey>,
    top_level_fun_value_refs: HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
    top_level_fun_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
}

type SourceSiteKey = (PathBuf, Span);

struct DumpMaterializeRequestSet<'a> {
    typecheck_types: &'a TypeStore,
    monomorph_keys: &'a [MonomorphKey],
    template_infos: Vec<GenericTemplateInfo>,
    top_level_fun_value_refs: HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
    top_level_fun_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
}

fn collect_dump_materialization_inputs(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<DumpMaterializationInputs> {
    let mut prepared_files = Vec::with_capacity(session.sysroot().files.len() + 8);
    for file in &session.sysroot().files {
        prepared_files.push(PreparedDumpFile {
            source: file.source.clone(),
            ast: file.ast.clone(),
            extend_type_env: false,
            collect_monomorph_keys: false,
        });
    }

    for support_source in load_dump_support_sources(session)? {
        let ast = parse_file(&support_source)?;
        prepared_files.push(PreparedDumpFile {
            source: support_source,
            ast,
            extend_type_env: true,
            collect_monomorph_keys: false,
        });
    }

    let entry_source = source.clone();
    let entry_ast = parse_file(&entry_source)?;
    prepared_files.push(PreparedDumpFile {
        source: entry_source,
        ast: entry_ast,
        extend_type_env: true,
        collect_monomorph_keys: true,
    });

    {
        let trim_sources = prepared_files
            .iter()
            .filter(|file| file.extend_type_env)
            .map(|file| file.source.clone())
            .collect::<Vec<_>>();
        let sources = trim_sources.iter().collect::<Vec<_>>();
        let mut files = prepared_files
            .iter_mut()
            .filter(|file| file.extend_type_env)
            .map(|file| &mut file.ast)
            .collect::<Vec<_>>();
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )?;
    }

    for file in &prepared_files {
        typecheck::check_file_headers(&file.source, &file.ast)?;
        typecheck::check_file_struct_decls(&file.source, &file.ast)?;
    }

    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::with_capacity(prepared_files.len());
        for file in &prepared_files {
            pairs.push((&file.source, &file.ast));
        }
        Index::build(&pairs)?
    };

    let mut resolved_headers = Vec::with_capacity(prepared_files.len());
    for file in &prepared_files {
        resolved_headers.push(crate::resolve::check_file_headers(
            &file.source,
            &file.ast,
            &index,
        )?);
    }
    for (file, headers) in prepared_files.iter_mut().zip(resolved_headers.iter()) {
        crate::resolve::check_file_bodies(&file.source, &mut file.ast, &index, headers)?;
    }

    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index)?;
    for file in &prepared_files {
        if file.extend_type_env {
            env.extend_from_file(&file.source, &file.ast, &index)?;
        }
    }

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut monomorph_keys = Vec::new();
    for (file, headers) in prepared_files.iter().zip(resolved_headers.iter()) {
        typecheck::check_file_annotations(
            &file.source,
            &file.ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )?;
        typecheck::check_file_type_refs(
            &file.source,
            &file.ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )?;

        if file.collect_monomorph_keys {
            monomorph_keys.extend(typecheck::check_file_exprs_with_monomorph_keys(
                &file.source,
                &file.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )?);
        } else {
            typecheck::check_file_exprs(
                &file.source,
                &file.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )?;
        }
    }

    let mut top_level_fun_value_refs = HashMap::new();
    let mut top_level_fun_call_bindings = HashMap::new();
    for file in &prepared_files {
        let source_path = file.source.path().to_path_buf();
        for (span, binding) in file.ast.top_level_fun_value_refs() {
            top_level_fun_value_refs.insert((source_path.clone(), span), binding);
        }
        for (span, binding) in file.ast.top_level_fun_call_bindings() {
            top_level_fun_call_bindings.insert((source_path.clone(), span), binding);
        }
    }

    Ok(DumpMaterializationInputs {
        prepared_files,
        index,
        env,
        typecheck_types: types,
        monomorph_keys,
        top_level_fun_value_refs,
        top_level_fun_call_bindings,
    })
}

type RequestTemplateKey = (String, PathBuf, Span);

#[derive(Clone)]
struct GenericTemplateInfo {
    request_lookup_key: RequestTemplateKey,
    template: TemplateKey,
    type_param_names: Vec<String>,
    eff_param_name: Option<String>,
    signature_key: String,
    has_body: bool,
}

fn normalize_sig_piece(s: &str) -> String {
    s.split_whitespace().collect()
}

fn generic_template_signature_key(source: &SourceFile, fun: &ast::FunDecl) -> String {
    let mut out = String::new();
    out.push_str(match fun.kind {
        ast::FunDeclKind::Regular => "fun",
        ast::FunDeclKind::EffectOp => "effect-op",
    });
    out.push('|');
    for param in &fun.type_params {
        out.push_str(param.name.text(source));
        out.push(',');
    }
    out.push('|');
    if let Some(eff) = &fun.eff_param {
        out.push_str(&normalize_sig_piece(source.slice(eff.span)));
    }
    out.push('|');
    if let Some(receiver) = &fun.receiver {
        out.push_str(&normalize_sig_piece(source.slice(receiver.span())));
    }
    out.push('|');
    for param in &fun.params {
        if let Some(ty) = &param.ty {
            out.push_str(&normalize_sig_piece(source.slice(ty.span())));
        } else {
            out.push('_');
        }
        out.push(';');
    }
    out.push('|');
    match &fun.return_ty {
        Some(ret) => out.push_str(&normalize_sig_piece(source.slice(ret.span()))),
        None => out.push_str("Unit"),
    }
    out.push('|');
    if let Some(effects) = &fun.effects {
        out.push_str(&normalize_sig_piece(source.slice(effects.span)));
    }
    out
}

fn push_generic_template_info(
    out: &mut Vec<GenericTemplateInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    fun: &ast::FunDecl,
) {
    if fun.type_params.is_empty() && fun.eff_param.is_none() {
        return;
    }

    let local_name = source.slice(fun.name.span);
    let fqn = if owner_fqn.is_empty() {
        local_name.to_string()
    } else {
        format!("{owner_fqn}.{local_name}")
    };
    out.push(GenericTemplateInfo {
        request_lookup_key: (fqn.clone(), source.path().to_path_buf(), fun.name.span),
        template: TemplateKey {
            fqn,
            source_path: source.path().to_path_buf(),
            decl_span: fun.span,
        },
        type_param_names: fun
            .type_params
            .iter()
            .map(|param| param.name.text(source).to_string())
            .collect(),
        eff_param_name: fun
            .eff_param
            .as_ref()
            .map(|param| param.name.text(source).to_string()),
        signature_key: generic_template_signature_key(source, fun),
        has_body: matches!(fun.body, ast::FunBody::Block(_)),
    });
}

fn collect_generic_templates_from_type_body(
    out: &mut Vec<GenericTemplateInfo>,
    source: &SourceFile,
    owner_fqn: &str,
    body: Option<&ast::TypeBody>,
) {
    let Some(body) = body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Fun(fun) => push_generic_template_info(out, source, owner_fqn, fun),
            ast::TypeMember::Type(ty) => {
                let nested_owner = format!("{owner_fqn}.{}", ty.name.text(source));
                collect_generic_templates_from_type_body(
                    out,
                    source,
                    &nested_owner,
                    ty.body.as_ref(),
                );
            }
            ast::TypeMember::Object(obj) => {
                let object_name = obj
                    .name
                    .as_ref()
                    .map(|name| name.text(source).to_string())
                    .or_else(|| {
                        matches!(obj.kind, ast::ObjectKind::Companion)
                            .then(|| "Companion".to_string())
                    });
                let Some(object_name) = object_name else {
                    continue;
                };
                let nested_owner = format!("{owner_fqn}.{object_name}");
                collect_generic_templates_from_type_body(
                    out,
                    source,
                    &nested_owner,
                    obj.body.as_ref(),
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_) => {}
        }
    }
}

fn collect_generic_template_infos(prepared_files: &[PreparedDumpFile]) -> Vec<GenericTemplateInfo> {
    let mut out = Vec::new();
    for file in prepared_files {
        let pkg_prefix = package_prefix(&file.source, file.ast.package.as_ref());
        for item in &file.ast.items {
            match item {
                ast::Item::Fun(fun) => {
                    push_generic_template_info(&mut out, &file.source, &pkg_prefix, fun);
                }
                ast::Item::Type(ty) => {
                    let owner_fqn = if pkg_prefix.is_empty() {
                        ty.name.text(&file.source).to_string()
                    } else {
                        format!("{pkg_prefix}.{}", ty.name.text(&file.source))
                    };
                    collect_generic_templates_from_type_body(
                        &mut out,
                        &file.source,
                        &owner_fqn,
                        ty.body.as_ref(),
                    );
                }
                ast::Item::Object(obj) => {
                    let object_name = obj
                        .name
                        .as_ref()
                        .map(|name| name.text(&file.source).to_string())
                        .or_else(|| {
                            matches!(obj.kind, ast::ObjectKind::Companion)
                                .then(|| "Companion".to_string())
                        });
                    let Some(object_name) = object_name else {
                        continue;
                    };
                    let owner_fqn = if pkg_prefix.is_empty() {
                        object_name
                    } else {
                        format!("{pkg_prefix}.{object_name}")
                    };
                    collect_generic_templates_from_type_body(
                        &mut out,
                        &file.source,
                        &owner_fqn,
                        obj.body.as_ref(),
                    );
                }
                ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::Val(_) => {}
            }
        }
    }
    out
}

fn load_dump_support_sources(session: &Session) -> MaterializeResult<Vec<SourceFile>> {
    let stdlib_root = default_stdlib_path();
    let stdlib_root = stdlib_root.canonicalize().map_err(|error| {
        frontend_err(format!(
            "dump-ir 无法定位 stdlib 目录：{}: {error}",
            stdlib_root.display()
        ))
    })?;

    let mut paths = Vec::new();
    collect_scoop_files(&stdlib_root, &mut paths)?;
    paths.extend(session.sysroot().compilable_source_paths.iter().cloned());
    paths.sort();

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let source = SourceFile::load(&path).map_err(|error| {
            frontend_err(format!(
                "dump-ir 无法读取 sysroot support source：{}: {error}",
                path.display()
            ))
        })?;
        sources.push(source);
    }
    Ok(sources)
}

fn default_stdlib_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../stdlib")
}

fn collect_scoop_files(dir: &Path, out: &mut Vec<PathBuf>) -> MaterializeResult<()> {
    for entry in std::fs::read_dir(dir).map_err(|error| {
        frontend_err(format!("dump-ir 无法读取目录：{}: {error}", dir.display()))
    })? {
        let entry = entry.map_err(|error| frontend_err(error.to_string()))?;
        let path = entry.path();
        let ty = entry
            .file_type()
            .map_err(|error| frontend_err(error.to_string()))?;
        if ty.is_dir() {
            collect_scoop_files(&path, out)?;
            continue;
        }
        if ty.is_file() && path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
    Ok(())
}

fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(package) = package else {
        return String::new();
    };
    package
        .path
        .iter()
        .map(|seg| seg.text(source))
        .collect::<Vec<_>>()
        .join(".")
}

fn materialize_generic_mir_for_dump(
    generic_file: File,
    types: TypeStore,
    builtins: BuiltinTypes,
    requests: DumpMaterializeRequestSet<'_>,
) -> MaterializeResult<MaterializedMir> {
    let DumpMaterializeRequestSet {
        typecheck_types,
        monomorph_keys,
        template_infos,
        top_level_fun_value_refs,
        top_level_fun_call_bindings,
    } = requests;
    let mut materializer = MirInstanceMaterializer::new(
        generic_file,
        types,
        builtins,
        template_infos,
        typecheck_types,
        top_level_fun_value_refs,
        top_level_fun_call_bindings,
    )?;
    let initial_requests = materializer.seed_requests(typecheck_types, monomorph_keys)?;
    materializer.run(initial_requests)
}

#[derive(Clone)]
struct TemplateRootInfo {
    template: TemplateKey,
    type_param_names: Vec<String>,
    eff_param_name: Option<String>,
    root_fun: FunDecl,
    family: Vec<FunDecl>,
}

#[derive(Clone)]
struct TemplateRootCandidate {
    request_lookup_key: RequestTemplateKey,
    template: TemplateKey,
    type_param_names: Vec<String>,
    eff_param_name: Option<String>,
    signature_key: String,
    root_fun: FunDecl,
}

#[derive(Clone)]
struct SiteInstanceBinding {
    template: TemplateKey,
    type_args: Vec<TypeId>,
    eff_args: Vec<EffectRow>,
}

#[derive(Default)]
struct InstanceSubstitution {
    type_params: HashMap<String, TypeId>,
    effect_params: HashMap<String, EffectRow>,
}

struct RewriteContext<'a> {
    locals: &'a [LocalDecl],
    substitution: &'a InstanceSubstitution,
    template_source_path: &'a Path,
    template_root_fqn: &'a str,
    instance_root_fqn: &'a str,
}

struct MirInstanceMaterializer {
    types: TypeStore,
    builtins: BuiltinTypes,
    request_templates: HashMap<RequestTemplateKey, TemplateKey>,
    roots: HashMap<TemplateKey, TemplateRootInfo>,
    roots_by_fqn: HashMap<String, Vec<TemplateKey>>,
    call_bindings: HashMap<SourceSiteKey, SiteInstanceBinding>,
    value_ref_bindings: HashMap<SourceSiteKey, SiteInstanceBinding>,
    queued: HashSet<InstanceKey>,
    queue: VecDeque<InstanceKey>,
    materialized: HashMap<InstanceKey, Vec<FunDecl>>,
}

impl MirInstanceMaterializer {
    fn new(
        generic_file: File,
        types: TypeStore,
        builtins: BuiltinTypes,
        template_infos: Vec<GenericTemplateInfo>,
        typecheck_types: &TypeStore,
        top_level_fun_value_refs: HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
        top_level_fun_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    ) -> MaterializeResult<Self> {
        let mut generic_funs = Vec::new();
        for item in &generic_file.items {
            if let Item::Fun(fun) = item {
                generic_funs.push(fun.clone());
            }
        }

        let mut root_candidates = Vec::new();
        let mut deferred_request_templates = Vec::new();
        for info in template_infos {
            let root_fun = generic_funs
                .iter()
                .find(|fun| fun.fqn == info.template.fqn && fun.span == info.template.decl_span)
                .cloned();
            let Some(root_fun) = root_fun else {
                if !info.has_body {
                    deferred_request_templates.push((
                        info.request_lookup_key,
                        info.template.fqn,
                        info.signature_key,
                    ));
                    continue;
                }
                return Err(materialize_err(
                    MirMaterializeError::MissingMirRootForTemplate {
                        fqn: info.template.fqn.clone(),
                        file: info.template.source_path.display().to_string(),
                        span: info.template.decl_span,
                    },
                ));
            };

            root_candidates.push(TemplateRootCandidate {
                request_lookup_key: info.request_lookup_key,
                template: info.template,
                type_param_names: info.type_param_names,
                eff_param_name: info.eff_param_name,
                signature_key: info.signature_key,
                root_fun,
            });
        }

        let canonical_templates = canonical_template_map(&root_candidates);

        let mut request_templates = HashMap::new();
        let mut roots = HashMap::new();
        let mut roots_by_fqn: HashMap<String, Vec<TemplateKey>> = HashMap::new();
        for candidate in root_candidates {
            let group_key = (
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            );
            let canonical = canonical_templates
                .get(&group_key)
                .cloned()
                .expect("canonical template must exist for every root candidate");
            request_templates.insert(candidate.request_lookup_key, canonical.clone());

            if candidate.template != canonical {
                continue;
            }

            let family = generic_funs
                .iter()
                .filter(|fun| belongs_to_template_family(fun, &candidate.root_fun))
                .cloned()
                .collect::<Vec<_>>();
            roots_by_fqn
                .entry(canonical.fqn.clone())
                .or_default()
                .push(canonical.clone());
            roots.insert(
                canonical.clone(),
                TemplateRootInfo {
                    template: canonical,
                    type_param_names: candidate.type_param_names,
                    eff_param_name: candidate.eff_param_name,
                    root_fun: candidate.root_fun,
                    family,
                },
            );
        }

        for (request_lookup_key, fqn, signature_key) in deferred_request_templates {
            let Some(canonical) = canonical_templates.get(&(fqn, signature_key)).cloned() else {
                continue;
            };
            request_templates.insert(request_lookup_key, canonical);
        }

        let mut materializer = Self {
            types,
            builtins,
            request_templates,
            roots,
            roots_by_fqn,
            call_bindings: HashMap::new(),
            value_ref_bindings: HashMap::new(),
            queued: HashSet::new(),
            queue: VecDeque::new(),
            materialized: HashMap::new(),
        };
        materializer.load_site_instance_bindings(
            typecheck_types,
            top_level_fun_value_refs,
            top_level_fun_call_bindings,
        )?;
        Ok(materializer)
    }

    fn load_site_instance_bindings(
        &mut self,
        typecheck_types: &TypeStore,
        top_level_fun_value_refs: HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
        top_level_fun_call_bindings: HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    ) -> MaterializeResult<()> {
        for (site, binding) in top_level_fun_call_bindings {
            if binding.type_args.is_empty() && binding.eff_args.is_empty() {
                continue;
            }
            let Some(template) =
                self.resolve_request_template(&binding.fqn, &binding.decl_file, binding.decl_span)
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: binding.fqn,
                        file: binding.decl_file.display().to_string(),
                        span: binding.decl_span,
                    },
                ));
            };
            let type_args = binding
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect();
            let eff_args = binding
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect();
            self.call_bindings.insert(
                site,
                SiteInstanceBinding {
                    template,
                    type_args,
                    eff_args,
                },
            );
        }

        for (site, binding) in top_level_fun_value_refs {
            if binding.type_args.is_empty() && binding.eff_args.is_empty() {
                continue;
            }
            let Some(template) =
                self.resolve_request_template(&binding.fqn, &binding.decl_file, binding.decl_span)
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: binding.fqn,
                        file: binding.decl_file.display().to_string(),
                        span: binding.decl_span,
                    },
                ));
            };
            let type_args = binding
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect();
            let eff_args = binding
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect();
            self.value_ref_bindings.insert(
                site,
                SiteInstanceBinding {
                    template,
                    type_args,
                    eff_args,
                },
            );
        }

        Ok(())
    }

    fn resolve_request_template(
        &self,
        fqn: &str,
        decl_file: &Path,
        decl_span: Span,
    ) -> Option<TemplateKey> {
        self.request_templates
            .get(&(fqn.to_string(), decl_file.to_path_buf(), decl_span))
            .cloned()
            .or_else(|| {
                let matches = self
                    .request_templates
                    .iter()
                    .filter(|((candidate_fqn, candidate_file, _), _)| {
                        candidate_fqn == fqn && candidate_file == decl_file
                    })
                    .map(|(_, template)| template.clone())
                    .collect::<HashSet<_>>();
                (matches.len() == 1).then(|| matches.into_iter().next().unwrap())
            })
    }

    fn seed_requests(
        &mut self,
        typecheck_types: &TypeStore,
        monomorph_keys: &[MonomorphKey],
    ) -> MaterializeResult<Vec<InstanceKey>> {
        let mut initial = Vec::new();
        for key in monomorph_keys {
            let Some(template) = self.resolve_request_template(
                &key.symbol.fqn,
                &key.symbol.decl_file,
                key.symbol.decl_span,
            ) else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: key.symbol.fqn.clone(),
                        file: key.symbol.decl_file.display().to_string(),
                        span: key.symbol.decl_span,
                    },
                ));
            };

            if key.type_args.is_empty() && key.eff_args.is_empty() {
                continue;
            }
            let type_args = key
                .type_args
                .iter()
                .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                .collect::<Vec<_>>();
            let eff_args = key
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect::<Vec<_>>();
            if !instance_request_is_concrete(&self.types, &type_args, &eff_args) {
                continue;
            }
            initial.push(InstanceKey {
                template,
                type_args,
                eff_args,
            });
        }
        initial.sort_by_key(|a| self.instance_fqn(a));
        Ok(initial)
    }

    fn run(mut self, initial_requests: Vec<InstanceKey>) -> MaterializeResult<MaterializedMir> {
        for request in initial_requests {
            self.enqueue(request);
        }

        while let Some(instance) = self.queue.pop_front() {
            self.queued.remove(&instance);
            if self.materialized.contains_key(&instance) {
                continue;
            }
            let family = self.materialize_instance(&instance)?;
            self.materialized.insert(instance, family);
        }

        let mut instance_keys = self.materialized.keys().cloned().collect::<Vec<_>>();
        instance_keys.sort_by_key(|a| self.instance_fqn(a));

        let mut items = Vec::new();
        for key in &instance_keys {
            let mut family = self
                .materialized
                .get(key)
                .cloned()
                .expect("materialized instance should exist");
            family.sort_by(|a, b| {
                let a_root = a.fqn == self.instance_fqn(key);
                let b_root = b.fqn == self.instance_fqn(key);
                (!a_root).cmp(&!b_root).then_with(|| a.fqn.cmp(&b.fqn))
            });
            items.extend(family.into_iter().map(Item::Fun));
        }

        Ok(MaterializedMir {
            file: File { items },
            types: self.types,
            instance_keys,
        })
    }

    fn enqueue(&mut self, key: InstanceKey) {
        if self.materialized.contains_key(&key) || !self.queued.insert(key.clone()) {
            return;
        }
        self.queue.push_back(key);
    }

    fn materialize_instance(&mut self, instance: &InstanceKey) -> MaterializeResult<Vec<FunDecl>> {
        let Some(root) = self.roots.get(&instance.template).cloned() else {
            return Err(materialize_err(
                MirMaterializeError::MissingGenericTemplate {
                    fqn: instance.template.fqn.clone(),
                    file: instance.template.source_path.display().to_string(),
                    span: instance.template.decl_span,
                },
            ));
        };

        if root.type_param_names.len() != instance.type_args.len() {
            return Err(materialize_err(MirMaterializeError::TypeArgArityMismatch {
                fqn: root.template.fqn.clone(),
                expected: root.type_param_names.len(),
                found: instance.type_args.len(),
                decl_span: root.template.decl_span.into(),
            }));
        }

        let substitution = self.build_instance_substitution(&root, instance)?;
        let instance_root_fqn = self.instance_fqn(instance);

        let mut out = Vec::with_capacity(root.family.len());
        for template_fun in &root.family {
            let mut fun = template_fun.clone();
            fun.fqn = rewrite_family_symbol_name(&fun.fqn, &root.template.fqn, &instance_root_fqn)
                .unwrap_or_else(|| fun.fqn.clone());
            fun.ty = substitute_type_and_effect_params(&mut self.types, fun.ty, &substitution);
            for param in &mut fun.params {
                param.ty =
                    substitute_type_and_effect_params(&mut self.types, param.ty, &substitution);
            }
            fun.return_ty =
                substitute_type_and_effect_params(&mut self.types, fun.return_ty, &substitution);
            if let Some(body) = &mut fun.body {
                self.rewrite_body(
                    body,
                    &substitution,
                    &root.template.source_path,
                    &root.template.fqn,
                    &instance_root_fqn,
                )?;
            }
            out.push(fun);
        }

        Ok(out)
    }

    fn build_instance_substitution(
        &self,
        root: &TemplateRootInfo,
        instance: &InstanceKey,
    ) -> MaterializeResult<InstanceSubstitution> {
        let mut substitution = InstanceSubstitution {
            type_params: root
                .type_param_names
                .iter()
                .cloned()
                .zip(instance.type_args.iter().copied())
                .collect(),
            effect_params: HashMap::new(),
        };

        match (&root.eff_param_name, instance.eff_args.as_slice()) {
            (None, []) => {}
            (None, eff_args) => {
                return Err(materialize_err(
                    MirMaterializeError::EffectArgArityMismatch {
                        fqn: root.template.fqn.clone(),
                        expected: 0,
                        found: eff_args.len(),
                        decl_span: root.template.decl_span.into(),
                    },
                ));
            }
            (Some(name), [row]) => {
                substitution.effect_params.insert(name.clone(), row.clone());
            }
            (Some(_), eff_args) => {
                return Err(materialize_err(
                    MirMaterializeError::EffectArgArityMismatch {
                        fqn: root.template.fqn.clone(),
                        expected: 1,
                        found: eff_args.len(),
                        decl_span: root.template.decl_span.into(),
                    },
                ));
            }
        }

        Ok(substitution)
    }

    fn rewrite_body(
        &mut self,
        body: &mut Body,
        substitution: &InstanceSubstitution,
        template_source_path: &Path,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        for local in &mut body.locals {
            local.ty = substitute_type_and_effect_params(&mut self.types, local.ty, substitution);
        }
        let locals = body.locals.clone();
        let ctx = RewriteContext {
            locals: &locals,
            substitution,
            template_source_path,
            template_root_fqn,
            instance_root_fqn,
        };
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                self.rewrite_statement(stmt, &ctx)?;
            }
            self.rewrite_terminator(&mut block.terminator, &ctx)?;
        }
        Ok(())
    }

    fn rewrite_statement(
        &mut self,
        stmt: &mut Statement,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        if let StatementKind::Assign { value, .. } = &mut stmt.kind {
            self.rewrite_rvalue(stmt.span, value, ctx)?;
        }
        Ok(())
    }

    fn rewrite_terminator(
        &mut self,
        terminator: &mut Terminator,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match &mut terminator.kind {
            TerminatorKind::Perform { metadata, args, .. } => {
                self.rewrite_perform_metadata(metadata, ctx.substitution);
                for arg in args {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
            }
            TerminatorKind::Handle { arms, .. } => {
                for arm in arms {
                    self.rewrite_handler_arm(arm);
                }
            }
            TerminatorKind::CondBr { cond, .. } => {
                *cond = self.rewrite_operand(cond.clone());
            }
            TerminatorKind::Return
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }
        Ok(())
    }

    fn rewrite_handler_arm(&mut self, _arm: &mut HandlerArm) {}

    fn rewrite_rvalue(
        &mut self,
        stmt_span: Span,
        value: &mut Rvalue,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match value {
            Rvalue::Use(operand) => *operand = self.rewrite_operand(operand.clone()),
            Rvalue::TopLevelRef(top) => {
                if let Some(rewritten) = rewrite_family_symbol_name(
                    &top.fqn,
                    ctx.template_root_fqn,
                    ctx.instance_root_fqn,
                ) {
                    top.fqn = rewritten;
                }
            }
            Rvalue::UnresolvedName { .. } => {}
            Rvalue::Unary { operand, .. } => *operand = self.rewrite_operand(operand.clone()),
            Rvalue::Binary { lhs, rhs, .. } => {
                *lhs = self.rewrite_operand(lhs.clone());
                *rhs = self.rewrite_operand(rhs.clone());
            }
            Rvalue::TypeCheck { value, test_ty, .. } => {
                *value = self.rewrite_operand(value.clone());
                *test_ty =
                    substitute_type_and_effect_params(&mut self.types, *test_ty, ctx.substitution);
            }
            Rvalue::Cast {
                value, target_ty, ..
            } => {
                *value = self.rewrite_operand(value.clone());
                *target_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    *target_ty,
                    ctx.substitution,
                );
            }
            Rvalue::MemberAccess { receiver, member } => {
                *receiver = self.rewrite_operand(receiver.clone());
                self.rewrite_member_access_metadata(member, ctx);
            }
            Rvalue::Call { kind, args } => {
                for arg in args.iter_mut() {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
                self.rewrite_call_kind(stmt_span, kind, args, ctx)?;
            }
            Rvalue::MakeTuple { elements } => {
                for element in elements.iter_mut() {
                    *element = self.rewrite_operand(element.clone());
                }
            }
            Rvalue::TupleGet { tuple, .. } => *tuple = self.rewrite_operand(tuple.clone()),
            Rvalue::CaptureBoxNew { value } => *value = self.rewrite_operand(value.clone()),
            Rvalue::CaptureBoxGet { box_operand } => {
                *box_operand = self.rewrite_operand(box_operand.clone());
            }
            Rvalue::CaptureBoxSet { box_operand, value } => {
                *box_operand = self.rewrite_operand(box_operand.clone());
                *value = self.rewrite_operand(value.clone());
            }
            Rvalue::PatternMatch { subject, pattern } => {
                *subject = self.rewrite_operand(subject.clone());
                self.rewrite_pattern(pattern, ctx.substitution);
            }
            Rvalue::PatternExtract { subject, path } => {
                *subject = self.rewrite_operand(subject.clone());
                let _ = path;
            }
            Rvalue::MakeClosure { env, fn_ptr } => {
                *env = self.rewrite_operand(env.clone());
                if let Some(rewritten) =
                    rewrite_family_symbol_name(fn_ptr, ctx.template_root_fqn, ctx.instance_root_fqn)
                {
                    *fn_ptr = rewritten;
                }
            }
            Rvalue::PerformResult { effect_ty, .. } => {
                *effect_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    *effect_ty,
                    ctx.substitution,
                );
            }
            Rvalue::Todo(_) => {}
        }
        Ok(())
    }

    fn rewrite_call_kind(
        &mut self,
        call_span: Span,
        kind: &mut CallKind,
        args: &[CallArg],
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match kind {
            CallKind::Direct { callee_fqn } => {
                if let Some(rewritten) = rewrite_family_symbol_name(
                    callee_fqn,
                    ctx.template_root_fqn,
                    ctx.instance_root_fqn,
                ) {
                    *callee_fqn = rewritten;
                    return Ok(());
                }
                if let Some(instance_key) = self.infer_direct_call_instance(
                    ctx.template_source_path,
                    call_span,
                    callee_fqn,
                    args,
                    ctx.locals,
                    ctx.substitution,
                ) {
                    *callee_fqn = self.instance_fqn(&instance_key);
                    self.enqueue(instance_key);
                }
            }
            CallKind::Closure { callee, fn_ptr } => {
                *callee = self.rewrite_operand(callee.clone());
                if let Some(rewritten) =
                    rewrite_family_symbol_name(fn_ptr, ctx.template_root_fqn, ctx.instance_root_fqn)
                {
                    *fn_ptr = rewritten;
                }
            }
            CallKind::FunValue { callee } => *callee = self.rewrite_operand(callee.clone()),
            CallKind::Virtual { receiver, dispatch }
            | CallKind::Interface { receiver, dispatch } => {
                *receiver = self.rewrite_operand(receiver.clone());
                dispatch.receiver_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    dispatch.receiver_ty,
                    ctx.substitution,
                );
            }
            CallKind::Resume {
                continuation,
                resume,
            } => {
                *continuation = self.rewrite_operand(continuation.clone());
                resume.continuation_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.continuation_ty,
                    ctx.substitution,
                );
            }
        }
        Ok(())
    }

    fn infer_direct_call_instance(
        &mut self,
        template_source_path: &Path,
        call_span: Span,
        callee_fqn: &str,
        args: &[CallArg],
        locals: &[LocalDecl],
        substitution: &InstanceSubstitution,
    ) -> Option<InstanceKey> {
        if let Some(binding) = self
            .lookup_site_instance_binding(template_source_path, call_span)
            .cloned()
        {
            return self.instantiate_site_binding(&binding, substitution);
        }

        let candidates = self.roots_by_fqn.get(callee_fqn)?;
        if candidates.len() != 1 {
            return None;
        }
        let root = self.roots.get(&candidates[0])?;
        if root.type_param_names.is_empty() || root.eff_param_name.is_some() {
            return None;
        }

        let arg_to_param = map_call_args_to_params(&root.root_fun.params, args)?;
        let mut bindings = HashMap::new();
        for (arg_idx, param_idx) in arg_to_param.into_iter().enumerate() {
            let param = root.root_fun.params.get(param_idx)?;
            if !type_contains_param(&self.types, param.ty) {
                continue;
            }
            let arg = args.get(arg_idx)?;
            let concrete_ty = operand_type(&self.types, self.builtins, locals, &arg.value)?;
            collect_type_param_bindings(&self.types, param.ty, concrete_ty, &mut bindings);
        }

        let mut ordered = Vec::with_capacity(root.type_param_names.len());
        for name in &root.type_param_names {
            let ty = bindings.get(name).copied()?;
            if type_contains_param(&self.types, ty) {
                return None;
            }
            ordered.push(ty);
        }
        if ordered.is_empty() {
            return None;
        }

        Some(InstanceKey {
            template: root.template.clone(),
            type_args: ordered,
            eff_args: Vec::new(),
        })
    }

    fn lookup_site_instance_binding(
        &self,
        template_source_path: &Path,
        call_span: Span,
    ) -> Option<&SiteInstanceBinding> {
        let key = (template_source_path.to_path_buf(), call_span);
        self.call_bindings
            .get(&key)
            .or_else(|| self.value_ref_bindings.get(&key))
    }

    fn instantiate_site_binding(
        &mut self,
        binding: &SiteInstanceBinding,
        substitution: &InstanceSubstitution,
    ) -> Option<InstanceKey> {
        let type_args = binding
            .type_args
            .iter()
            .copied()
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution))
            .collect::<Vec<_>>();
        let eff_args = binding
            .eff_args
            .iter()
            .map(|row| {
                substitute_type_and_effect_params_in_effect_row(&mut self.types, row, substitution)
            })
            .collect::<Vec<_>>();
        if (type_args.is_empty() && eff_args.is_empty())
            || !instance_request_is_concrete(&self.types, &type_args, &eff_args)
        {
            return None;
        }
        Some(InstanceKey {
            template: binding.template.clone(),
            type_args,
            eff_args,
        })
    }

    fn rewrite_member_access_metadata(
        &mut self,
        member: &mut MemberAccessMetadata,
        ctx: &RewriteContext<'_>,
    ) {
        member.receiver_ty = substitute_type_and_effect_params(
            &mut self.types,
            member.receiver_ty,
            ctx.substitution,
        );
        if let Some(target) = &mut member.resolved {
            match target {
                MemberTarget::Fun { fqn } | MemberTarget::ExtensionFun { fqn } => {
                    if let Some(rewritten) = rewrite_family_symbol_name(
                        fqn,
                        ctx.template_root_fqn,
                        ctx.instance_root_fqn,
                    ) {
                        *fqn = rewritten;
                    }
                }
                MemberTarget::Value { .. } | MemberTarget::ExtensionValue { .. } => {}
            }
        }
    }

    fn rewrite_pattern(&mut self, pattern: &mut Pattern, substitution: &InstanceSubstitution) {
        match pattern {
            Pattern::Is { ty } | Pattern::Bind { ty, .. } => {
                *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
            }
            Pattern::Or { pats } => {
                for pat in pats {
                    self.rewrite_pattern(pat, substitution);
                }
            }
            Pattern::Tuple { elements } | Pattern::Variant { args: elements, .. } => {
                for pat in elements {
                    self.rewrite_pattern(pat, substitution);
                }
            }
            Pattern::Else
            | Pattern::Wildcard
            | Pattern::Rest
            | Pattern::IntLit { .. }
            | Pattern::CharLit { .. }
            | Pattern::StringLit { .. }
            | Pattern::BoolLit { .. } => {}
        }
    }

    fn rewrite_perform_metadata(
        &mut self,
        metadata: &mut PerformMetadata,
        substitution: &InstanceSubstitution,
    ) {
        metadata.effect_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.effect_ty, substitution);
        metadata.payload_tuple_ty = metadata
            .payload_tuple_ty
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
    }

    fn rewrite_operand(&mut self, operand: Operand) -> Operand {
        operand
    }

    fn instance_fqn(&self, instance: &InstanceKey) -> String {
        if instance.type_args.is_empty() && instance.eff_args.is_empty() {
            return instance.template.fqn.clone();
        }
        let mut args = instance
            .type_args
            .iter()
            .map(|&ty| self.types.display(ty).to_string())
            .collect::<Vec<_>>();
        args.extend(
            instance
                .eff_args
                .iter()
                .map(|row| format!("eff {}", self.format_effect_row_stable(row))),
        );
        format!("{}::<{}>", instance.template.fqn, args.join(", "))
    }

    fn format_effect_row_stable(&self, row: &EffectRow) -> String {
        if row.terms.is_empty() {
            return "Pure".to_string();
        }
        row.terms
            .iter()
            .map(|&ty| self.types.display(ty).to_string())
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

fn canonical_template_map(
    candidates: &[TemplateRootCandidate],
) -> HashMap<(String, String), TemplateKey> {
    let mut grouped: HashMap<(String, String), Vec<&TemplateRootCandidate>> = HashMap::new();
    for candidate in candidates {
        grouped
            .entry((
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            ))
            .or_default()
            .push(candidate);
    }

    let mut out = HashMap::new();
    for (key, group) in grouped {
        let chosen = choose_canonical_template(&group);
        out.insert(key, chosen.template.clone());
    }
    out
}

fn choose_canonical_template<'a>(
    candidates: &[&'a TemplateRootCandidate],
) -> &'a TemplateRootCandidate {
    let mut preferred = candidates
        .iter()
        .copied()
        .filter(|candidate| candidate.root_fun.body.is_some())
        .collect::<Vec<_>>();
    if preferred.is_empty() {
        preferred.extend(candidates.iter().copied());
    }
    preferred.sort_by(|a, b| {
        a.template
            .source_path
            .cmp(&b.template.source_path)
            .then_with(|| a.template.decl_span.start.cmp(&b.template.decl_span.start))
            .then_with(|| a.template.decl_span.end.cmp(&b.template.decl_span.end))
    });
    preferred
        .into_iter()
        .next()
        .expect("template candidate group must not be empty")
}

fn belongs_to_template_family(fun: &FunDecl, root_fun: &FunDecl) -> bool {
    if fun.fqn == root_fun.fqn {
        return fun.span == root_fun.span;
    }
    fun.fqn.strip_prefix(&root_fun.fqn).is_some_and(|suffix| {
        suffix.starts_with(".$lambda") && span_contains(root_fun.span, fun.span)
    })
}

fn span_contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn rewrite_family_symbol_name(
    symbol: &str,
    root_fqn: &str,
    instance_root_fqn: &str,
) -> Option<String> {
    if symbol == root_fqn {
        return Some(instance_root_fqn.to_string());
    }
    let suffix = symbol.strip_prefix(root_fqn)?;
    suffix
        .starts_with(".$lambda")
        .then(|| format!("{instance_root_fqn}{suffix}"))
}

fn re_intern_effect_row_from(
    types: &mut TypeStore,
    other: &TypeStore,
    row: &EffectRow,
) -> EffectRow {
    EffectRow::new(
        row.terms
            .iter()
            .map(|&term| types.re_intern_from(other, term))
            .collect(),
    )
}

fn substitute_type_and_effect_params(
    types: &mut TypeStore,
    ty: TypeId,
    substitution: &InstanceSubstitution,
) -> TypeId {
    match types.kind(ty).clone() {
        TypeKind::Param(param) => {
            if param.decl_file.as_os_str() == crate::hir::EFFECT_ROW_PARAM_DECL_FILE {
                ty
            } else {
                substitution
                    .type_params
                    .get(&param.name)
                    .copied()
                    .unwrap_or(ty)
            }
        }
        TypeKind::StarProjection(star) => {
            let read_ty = substitute_type_and_effect_params(types, star.read_ty, substitution);
            types.ty_star_projection(read_ty)
        }
        TypeKind::Ref(RefTypeKind::Any) | TypeKind::Ref(RefTypeKind::String) => ty,
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            let args = nominal
                .args
                .iter()
                .map(|&arg| substitute_type_and_effect_params(types, arg, substitution))
                .collect();
            let eff = nominal.eff.as_ref().map(|row| {
                substitute_type_and_effect_params_in_effect_row(types, row, substitution)
            });
            types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: nominal.fqn,
                args,
                eff,
            })))
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let receiver = fun
                .receiver
                .map(|receiver| substitute_type_and_effect_params(types, receiver, substitution));
            let params = fun
                .params
                .iter()
                .map(|&param| substitute_type_and_effect_params(types, param, substitution))
                .collect();
            let return_ty = substitute_type_and_effect_params(types, fun.return_ty, substitution);
            let effects =
                substitute_type_and_effect_params_in_effect_row(types, &fun.effects, substitution);
            types.ty_function(receiver, params, return_ty, effects, fun.effects_closed)
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            let variants = union
                .variants
                .iter()
                .map(|&variant| substitute_type_and_effect_params(types, variant, substitution))
                .collect();
            types.ty_union(variants)
        }
        TypeKind::Value(ValueTypeKind::Unit)
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
            let inner = substitute_type_and_effect_params(types, inner, substitution);
            types.ty_option(inner)
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let elements = elements
                .iter()
                .map(|&element| substitute_type_and_effect_params(types, element, substitution))
                .collect();
            types.ty_tuple(elements)
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let args = nominal
                .args
                .iter()
                .map(|&arg| substitute_type_and_effect_params(types, arg, substitution))
                .collect();
            let eff = nominal.eff.as_ref().map(|row| {
                substitute_type_and_effect_params_in_effect_row(types, row, substitution)
            });
            types.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                fqn: nominal.fqn,
                args,
                eff,
            })))
        }
    }
}

fn substitute_type_and_effect_params_in_effect_row(
    types: &mut TypeStore,
    row: &EffectRow,
    substitution: &InstanceSubstitution,
) -> EffectRow {
    let mut terms = Vec::new();
    for &term in &row.terms {
        if let Some(name) = effect_row_param_marker_name(types, term)
            && let Some(bound) = substitution.effect_params.get(&name)
        {
            terms.extend(bound.terms.iter().copied().map(|bound_term| {
                substitute_type_and_effect_params(types, bound_term, substitution)
            }));
            continue;
        }
        terms.push(substitute_type_and_effect_params(types, term, substitution));
    }
    EffectRow::new(terms)
}

fn effect_row_param_marker_name(types: &TypeStore, ty: TypeId) -> Option<String> {
    match types.kind(ty) {
        TypeKind::Param(param)
            if param.decl_file.as_os_str() == crate::hir::EFFECT_ROW_PARAM_DECL_FILE =>
        {
            Some(param.name.clone())
        }
        _ => None,
    }
}

fn map_call_args_to_params(params: &[Param], args: &[CallArg]) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg.name.as_deref() {
            Some(name) => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == name).then_some(idx))?,
            None => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    Some(out)
}

fn operand_type(
    types: &TypeStore,
    builtins: BuiltinTypes,
    locals: &[LocalDecl],
    operand: &Operand,
) -> Option<TypeId> {
    match operand {
        Operand::Local(local) => locals.get(local.as_u32() as usize).map(|decl| decl.ty),
        Operand::Const(ConstValue::Bool(_)) => Some(builtins.bool_),
        Operand::Const(ConstValue::Char) => Some(builtins.char_),
        Operand::Const(ConstValue::Unit) => Some(builtins.unit),
        Operand::Const(ConstValue::Int) => Some(builtins.int),
        Operand::Const(ConstValue::Float64) => Some(builtins.float64),
        Operand::Const(ConstValue::Float32) => Some(builtins.float32),
        Operand::Const(ConstValue::String) => Some(builtins.string),
    }
    .filter(|ty| {
        !type_contains_param(types, *ty)
            && !matches!(types.kind(*ty), TypeKind::Ref(RefTypeKind::Any))
    })
}

fn instance_request_is_concrete(
    types: &TypeStore,
    type_args: &[TypeId],
    eff_args: &[EffectRow],
) -> bool {
    type_args.iter().all(|&ty| !type_contains_param(types, ty))
        && eff_args
            .iter()
            .all(|row| !effect_row_contains_param(types, row))
}

fn effect_row_contains_param(types: &TypeStore, row: &EffectRow) -> bool {
    row.terms
        .iter()
        .copied()
        .any(|term| type_contains_param(types, term))
}

fn type_contains_param(types: &TypeStore, ty: TypeId) -> bool {
    let mut stack = vec![ty];
    while let Some(id) = stack.pop() {
        match types.kind(id) {
            TypeKind::Param(_) => return true,
            TypeKind::StarProjection(star) => stack.push(star.read_ty),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                stack.extend(nominal.args.iter().copied());
                if let Some(eff) = &nominal.eff {
                    stack.extend(eff.terms.iter().copied());
                }
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                stack.extend(elements.iter().copied())
            }
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                if let Some(receiver) = fun.receiver {
                    stack.push(receiver);
                }
                stack.extend(fun.params.iter().copied());
                stack.push(fun.return_ty);
                stack.extend(fun.effects.terms.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Union(union)) => {
                stack.extend(union.variants.iter().copied())
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
            | TypeKind::Value(ValueTypeKind::UIntN(_)) => {}
        }
    }
    false
}

fn collect_type_param_bindings(
    types: &TypeStore,
    declared_ty: TypeId,
    concrete_ty: TypeId,
    bindings: &mut HashMap<String, TypeId>,
) {
    match (types.kind(declared_ty), types.kind(concrete_ty)) {
        (TypeKind::Param(param), _) => match bindings.get(&param.name).copied() {
            Some(existing) if existing == concrete_ty => {}
            Some(_) => {}
            None => {
                bindings.insert(param.name.clone(), concrete_ty);
            }
        },
        (
            TypeKind::Ref(RefTypeKind::Nominal(declared)),
            TypeKind::Ref(RefTypeKind::Nominal(concrete)),
        )
        | (
            TypeKind::Value(ValueTypeKind::Nominal(declared)),
            TypeKind::Value(ValueTypeKind::Nominal(concrete)),
        ) => {
            if declared.fqn != concrete.fqn || declared.args.len() != concrete.args.len() {
                return;
            }
            for (decl_arg, concrete_arg) in declared.args.iter().zip(concrete.args.iter()) {
                collect_type_param_bindings(types, *decl_arg, *concrete_arg, bindings);
            }
        }
        (
            TypeKind::Value(ValueTypeKind::Option(declared_inner)),
            TypeKind::Value(ValueTypeKind::Option(concrete_inner)),
        ) => {
            collect_type_param_bindings(types, *declared_inner, *concrete_inner, bindings);
        }
        (
            TypeKind::Value(ValueTypeKind::Tuple(declared_elements)),
            TypeKind::Value(ValueTypeKind::Tuple(concrete_elements)),
        ) => {
            if declared_elements.len() != concrete_elements.len() {
                return;
            }
            for (decl_elem, concrete_elem) in declared_elements.iter().zip(concrete_elements.iter())
            {
                collect_type_param_bindings(types, *decl_elem, *concrete_elem, bindings);
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Function(declared_fun)),
            TypeKind::Ref(RefTypeKind::Function(concrete_fun)),
        ) => {
            match (declared_fun.receiver, concrete_fun.receiver) {
                (Some(declared_receiver), Some(concrete_receiver)) => collect_type_param_bindings(
                    types,
                    declared_receiver,
                    concrete_receiver,
                    bindings,
                ),
                (None, None) => {}
                _ => return,
            }
            if declared_fun.params.len() != concrete_fun.params.len() {
                return;
            }
            for (decl_param, concrete_param) in
                declared_fun.params.iter().zip(concrete_fun.params.iter())
            {
                collect_type_param_bindings(types, *decl_param, *concrete_param, bindings);
            }
            collect_type_param_bindings(
                types,
                declared_fun.return_ty,
                concrete_fun.return_ty,
                bindings,
            );
        }
        (
            TypeKind::Ref(RefTypeKind::Union(declared_union)),
            TypeKind::Ref(RefTypeKind::Union(concrete_union)),
        ) => {
            if declared_union.variants.len() != concrete_union.variants.len() {
                return;
            }
            for (decl_variant, concrete_variant) in declared_union
                .variants
                .iter()
                .zip(concrete_union.variants.iter())
            {
                collect_type_param_bindings(types, *decl_variant, *concrete_variant, bindings);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::source::SourceFile;

    #[test]
    fn dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/materialize_member_binding_effect.scoop",
            r#"
package fixtures.materialize

effect Boom {
    fun ping(): Unit
}

class Box() {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box): Int / E {
    return box.forward<eff E>()
}

fun entry(): Int / Boom {
    return wrap<eff Boom>(Box())
}
"#,
        );

        let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
        let bindings = inputs
            .top_level_fun_call_bindings
            .values()
            .filter(|binding| binding.fqn == "fixtures.materialize.Box.forward")
            .collect::<Vec<_>>();
        assert_eq!(bindings.len(), 1);
        let binding = bindings[0];
        assert_eq!(binding.decl_file, source.path().to_path_buf());
        assert!(binding.decl_span.start < binding.decl_span.end);
        assert!(binding.type_args.is_empty());
        assert_eq!(binding.eff_args.len(), 1);
        assert!(
            !binding.eff_args[0].is_pure(),
            "成员 direct-call 的 TopLevelFunCallBinding 不应退回 Pure"
        );

        let keys = inputs
            .monomorph_keys
            .iter()
            .filter(|key| key.symbol.fqn == "fixtures.materialize.Box.forward")
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].type_args.is_empty());
        assert_eq!(keys[0].eff_args.len(), 1);
        assert!(
            !keys[0].eff_args[0].is_pure(),
            "成员 direct-call 的 monomorph key 应保留非 Pure 的 eff_args"
        );
    }
}
