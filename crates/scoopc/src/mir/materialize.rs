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
use std::path::PathBuf;

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
}

type MaterializeResult<T> = Result<T, Box<MirMaterializeError>>;

fn materialize_err(error: MirMaterializeError) -> Box<MirMaterializeError> {
    Box::new(error)
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

/// 为 `dump-ir` / tests 生成 monomorphic MIR instances。
pub fn materialize_for_dump(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<MaterializedMir> {
    let (ast, typecheck_types, monomorph_keys) = collect_dump_monomorph_requests(session, source)?;
    let template_catalog = collect_generic_template_infos(source, &ast);
    let lowered_mir = super::lower_for_dump(session, source)?;
    let mut types = lowered_mir.types;
    let builtins = types.intern_builtins();

    materialize_generic_mir_for_dump(
        lowered_mir.file,
        types,
        builtins,
        &typecheck_types,
        &monomorph_keys,
        template_catalog,
    )
}

fn collect_dump_monomorph_requests(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<(ast::File, TypeStore, Vec<MonomorphKey>)> {
    let mut file = parse_file(source)?;
    {
        let sources = [source];
        let mut files = [&mut file];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )?;
    }
    typecheck::check_file_headers(source, &file)?;
    typecheck::check_file_struct_decls(source, &file)?;

    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &session.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((source, &file));
        Index::build(&pairs)?
    };

    let resolved_headers = crate::resolve::check_file_headers(source, &file, &index)?;
    crate::resolve::check_file_bodies(source, &mut file, &index, &resolved_headers)?;

    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index)?;
    env.extend_from_file(source, &file, &index)?;

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    typecheck::check_file_annotations(
        source,
        &file,
        &index,
        &resolved_headers.imports,
        &env,
        &mut types,
        builtins,
    )?;
    typecheck::check_file_type_refs(
        source,
        &file,
        &index,
        &resolved_headers.imports,
        &env,
        &mut types,
        builtins,
    )?;
    let monomorph_keys = typecheck::check_file_exprs_with_monomorph_keys(
        source,
        &file,
        &index,
        &resolved_headers.imports,
        &env,
        &mut types,
        builtins,
    )?;

    Ok((file, types, monomorph_keys))
}

#[derive(Clone)]
struct GenericTemplateInfo {
    request_lookup_key: (String, Span),
    template: TemplateKey,
    type_param_names: Vec<String>,
}

fn collect_generic_template_infos(
    source: &SourceFile,
    file: &ast::File,
) -> Vec<GenericTemplateInfo> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut out = Vec::new();
    for item in &file.items {
        let ast::Item::Fun(fun) = item else {
            continue;
        };
        if fun.type_params.is_empty() || matches!(fun.body, ast::FunBody::Missing) {
            continue;
        }
        let local_name = source.slice(fun.name.span);
        let fqn = if pkg_prefix.is_empty() {
            local_name.to_string()
        } else {
            format!("{pkg_prefix}.{local_name}")
        };
        out.push(GenericTemplateInfo {
            request_lookup_key: (fqn.clone(), fun.name.span),
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
        });
    }
    out
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
    typecheck_types: &TypeStore,
    monomorph_keys: &[MonomorphKey],
    template_infos: Vec<GenericTemplateInfo>,
) -> MaterializeResult<MaterializedMir> {
    let mut materializer =
        MirInstanceMaterializer::new(generic_file, types, builtins, template_infos)?;
    let initial_requests = materializer.seed_requests(typecheck_types, monomorph_keys)?;
    materializer.run(initial_requests)
}

#[derive(Clone)]
struct TemplateRootInfo {
    template: TemplateKey,
    request_decl_span: Span,
    type_param_names: Vec<String>,
    root_fun: FunDecl,
    family: Vec<FunDecl>,
}

struct MirInstanceMaterializer {
    types: TypeStore,
    builtins: BuiltinTypes,
    roots: HashMap<TemplateKey, TemplateRootInfo>,
    roots_by_fqn: HashMap<String, Vec<TemplateKey>>,
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
    ) -> MaterializeResult<Self> {
        let mut generic_funs = Vec::new();
        for item in &generic_file.items {
            if let Item::Fun(fun) = item {
                generic_funs.push(fun.clone());
            }
        }

        let mut info_by_request: HashMap<(String, Span), GenericTemplateInfo> = HashMap::new();
        for info in template_infos {
            info_by_request.insert(info.request_lookup_key.clone(), info);
        }

        let mut roots = HashMap::new();
        let mut roots_by_fqn: HashMap<String, Vec<TemplateKey>> = HashMap::new();
        for info in info_by_request.into_values() {
            let Some(root_fun) = generic_funs
                .iter()
                .find(|fun| fun.fqn == info.template.fqn && fun.span == info.template.decl_span)
                .cloned()
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingMirRootForTemplate {
                        fqn: info.template.fqn.clone(),
                        file: info.template.source_path.display().to_string(),
                        span: info.template.decl_span,
                    },
                ));
            };

            let family = generic_funs
                .iter()
                .filter(|fun| belongs_to_template_family(&fun.fqn, &info.template.fqn))
                .cloned()
                .collect::<Vec<_>>();
            let template = info.template.clone();
            roots_by_fqn
                .entry(template.fqn.clone())
                .or_default()
                .push(template.clone());
            roots.insert(
                template.clone(),
                TemplateRootInfo {
                    template,
                    request_decl_span: info.request_lookup_key.1,
                    type_param_names: info.type_param_names,
                    root_fun,
                    family,
                },
            );
        }

        Ok(Self {
            types,
            builtins,
            roots,
            roots_by_fqn,
            queued: HashSet::new(),
            queue: VecDeque::new(),
            materialized: HashMap::new(),
        })
    }

    fn seed_requests(
        &mut self,
        typecheck_types: &TypeStore,
        monomorph_keys: &[MonomorphKey],
    ) -> MaterializeResult<Vec<InstanceKey>> {
        let mut request_lookup: HashMap<(String, PathBuf, Span), TemplateKey> = HashMap::new();
        for root in self.roots.values() {
            request_lookup.insert(
                (
                    root.template.fqn.clone(),
                    root.template.source_path.clone(),
                    root.request_decl_span,
                ),
                root.template.clone(),
            );
        }

        let mut initial = Vec::new();
        for key in monomorph_keys {
            let Some(template) = request_lookup
                .get(&(
                    key.symbol.fqn.clone(),
                    key.symbol.decl_file.clone(),
                    key.symbol.decl_span,
                ))
                .cloned()
                .or_else(|| {
                    let matches = self
                        .roots
                        .keys()
                        .filter(|candidate| {
                            candidate.fqn == key.symbol.fqn
                                && candidate.source_path == key.symbol.decl_file
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    (matches.len() == 1).then(|| matches[0].clone())
                })
            else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: key.symbol.fqn.clone(),
                        file: key.symbol.decl_file.display().to_string(),
                        span: key.symbol.decl_span,
                    },
                ));
            };

            if key.type_args.is_empty() {
                continue;
            }
            let eff_args = key
                .eff_args
                .iter()
                .map(|row| re_intern_effect_row_from(&mut self.types, typecheck_types, row))
                .collect();
            initial.push(InstanceKey {
                template,
                type_args: key
                    .type_args
                    .iter()
                    .map(|&ty| self.types.re_intern_from(typecheck_types, ty))
                    .collect(),
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

        let instance_root_fqn = self.instance_fqn(instance);
        let param_map: HashMap<String, TypeId> = root
            .type_param_names
            .iter()
            .cloned()
            .zip(instance.type_args.iter().copied())
            .collect();

        let mut out = Vec::with_capacity(root.family.len());
        for template_fun in &root.family {
            let mut fun = template_fun.clone();
            fun.fqn = rewrite_family_symbol_name(&fun.fqn, &root.template.fqn, &instance_root_fqn)
                .unwrap_or_else(|| fun.fqn.clone());
            fun.ty = substitute_type_params(&mut self.types, fun.ty, &param_map);
            for param in &mut fun.params {
                param.ty = substitute_type_params(&mut self.types, param.ty, &param_map);
            }
            fun.return_ty = substitute_type_params(&mut self.types, fun.return_ty, &param_map);
            if let Some(body) = &mut fun.body {
                self.rewrite_body(body, &param_map, &root.template.fqn, &instance_root_fqn)?;
            }
            out.push(fun);
        }

        Ok(out)
    }

    fn rewrite_body(
        &mut self,
        body: &mut Body,
        param_map: &HashMap<String, TypeId>,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        for local in &mut body.locals {
            local.ty = substitute_type_params(&mut self.types, local.ty, param_map);
        }
        let locals = body.locals.clone();
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                self.rewrite_statement(
                    stmt,
                    &locals,
                    param_map,
                    template_root_fqn,
                    instance_root_fqn,
                )?;
            }
            self.rewrite_terminator(
                &mut block.terminator,
                &locals,
                param_map,
                template_root_fqn,
                instance_root_fqn,
            )?;
        }
        Ok(())
    }

    fn rewrite_statement(
        &mut self,
        stmt: &mut Statement,
        locals: &[LocalDecl],
        param_map: &HashMap<String, TypeId>,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        if let StatementKind::Assign { value, .. } = &mut stmt.kind {
            self.rewrite_rvalue(
                value,
                locals,
                param_map,
                template_root_fqn,
                instance_root_fqn,
            )?;
        }
        Ok(())
    }

    fn rewrite_terminator(
        &mut self,
        terminator: &mut Terminator,
        locals: &[LocalDecl],
        param_map: &HashMap<String, TypeId>,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        match &mut terminator.kind {
            TerminatorKind::Perform { metadata, args, .. } => {
                self.rewrite_perform_metadata(metadata, param_map);
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
        let _ = (locals, template_root_fqn, instance_root_fqn);
        Ok(())
    }

    fn rewrite_handler_arm(&mut self, _arm: &mut HandlerArm) {}

    fn rewrite_rvalue(
        &mut self,
        value: &mut Rvalue,
        locals: &[LocalDecl],
        param_map: &HashMap<String, TypeId>,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        match value {
            Rvalue::Use(operand) => *operand = self.rewrite_operand(operand.clone()),
            Rvalue::TopLevelRef(top) => {
                if let Some(rewritten) =
                    rewrite_family_symbol_name(&top.fqn, template_root_fqn, instance_root_fqn)
                {
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
                *test_ty = substitute_type_params(&mut self.types, *test_ty, param_map);
            }
            Rvalue::Cast {
                value, target_ty, ..
            } => {
                *value = self.rewrite_operand(value.clone());
                *target_ty = substitute_type_params(&mut self.types, *target_ty, param_map);
            }
            Rvalue::MemberAccess { receiver, member } => {
                *receiver = self.rewrite_operand(receiver.clone());
                self.rewrite_member_access_metadata(
                    member,
                    param_map,
                    template_root_fqn,
                    instance_root_fqn,
                );
            }
            Rvalue::Call { kind, args } => {
                for arg in args.iter_mut() {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
                self.rewrite_call_kind(
                    kind,
                    args,
                    locals,
                    param_map,
                    template_root_fqn,
                    instance_root_fqn,
                )?;
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
                self.rewrite_pattern(pattern, param_map);
            }
            Rvalue::PatternExtract { subject, path } => {
                *subject = self.rewrite_operand(subject.clone());
                let _ = path;
            }
            Rvalue::MakeClosure { env, fn_ptr } => {
                *env = self.rewrite_operand(env.clone());
                if let Some(rewritten) =
                    rewrite_family_symbol_name(fn_ptr, template_root_fqn, instance_root_fqn)
                {
                    *fn_ptr = rewritten;
                }
            }
            Rvalue::PerformResult { effect_ty, .. } => {
                *effect_ty = substitute_type_params(&mut self.types, *effect_ty, param_map);
            }
            Rvalue::Todo(_) => {}
        }
        Ok(())
    }

    fn rewrite_call_kind(
        &mut self,
        kind: &mut CallKind,
        args: &[CallArg],
        locals: &[LocalDecl],
        param_map: &HashMap<String, TypeId>,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) -> MaterializeResult<()> {
        match kind {
            CallKind::Direct { callee_fqn } => {
                if let Some(rewritten) =
                    rewrite_family_symbol_name(callee_fqn, template_root_fqn, instance_root_fqn)
                {
                    *callee_fqn = rewritten;
                    return Ok(());
                }
                if let Some(instance_key) =
                    self.infer_direct_call_instance(callee_fqn, args, locals)
                {
                    *callee_fqn = self.instance_fqn(&instance_key);
                    self.enqueue(instance_key);
                }
            }
            CallKind::Closure { callee, fn_ptr } => {
                *callee = self.rewrite_operand(callee.clone());
                if let Some(rewritten) =
                    rewrite_family_symbol_name(fn_ptr, template_root_fqn, instance_root_fqn)
                {
                    *fn_ptr = rewritten;
                }
            }
            CallKind::FunValue { callee } => *callee = self.rewrite_operand(callee.clone()),
            CallKind::Virtual { receiver, dispatch }
            | CallKind::Interface { receiver, dispatch } => {
                *receiver = self.rewrite_operand(receiver.clone());
                dispatch.receiver_ty =
                    substitute_type_params(&mut self.types, dispatch.receiver_ty, param_map);
            }
            CallKind::Resume {
                continuation,
                resume,
            } => {
                *continuation = self.rewrite_operand(continuation.clone());
                resume.continuation_ty =
                    substitute_type_params(&mut self.types, resume.continuation_ty, param_map);
            }
        }
        Ok(())
    }

    fn infer_direct_call_instance(
        &mut self,
        callee_fqn: &str,
        args: &[CallArg],
        locals: &[LocalDecl],
    ) -> Option<InstanceKey> {
        let candidates = self.roots_by_fqn.get(callee_fqn)?;
        if candidates.len() != 1 {
            return None;
        }
        let root = self.roots.get(&candidates[0])?;
        if root.type_param_names.is_empty() {
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

    fn rewrite_member_access_metadata(
        &mut self,
        member: &mut MemberAccessMetadata,
        param_map: &HashMap<String, TypeId>,
        template_root_fqn: &str,
        instance_root_fqn: &str,
    ) {
        member.receiver_ty = substitute_type_params(&mut self.types, member.receiver_ty, param_map);
        if let Some(target) = &mut member.resolved {
            match target {
                MemberTarget::Fun { fqn } | MemberTarget::ExtensionFun { fqn } => {
                    if let Some(rewritten) =
                        rewrite_family_symbol_name(fqn, template_root_fqn, instance_root_fqn)
                    {
                        *fqn = rewritten;
                    }
                }
                MemberTarget::Value { .. } | MemberTarget::ExtensionValue { .. } => {}
            }
        }
    }

    fn rewrite_pattern(&mut self, pattern: &mut Pattern, param_map: &HashMap<String, TypeId>) {
        match pattern {
            Pattern::Is { ty } | Pattern::Bind { ty, .. } => {
                *ty = substitute_type_params(&mut self.types, *ty, param_map);
            }
            Pattern::Or { pats } => {
                for pat in pats {
                    self.rewrite_pattern(pat, param_map);
                }
            }
            Pattern::Tuple { elements } | Pattern::Variant { args: elements, .. } => {
                for pat in elements {
                    self.rewrite_pattern(pat, param_map);
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
        param_map: &HashMap<String, TypeId>,
    ) {
        metadata.effect_ty = substitute_type_params(&mut self.types, metadata.effect_ty, param_map);
        metadata.payload_tuple_ty = metadata
            .payload_tuple_ty
            .map(|ty| substitute_type_params(&mut self.types, ty, param_map));
    }

    fn rewrite_operand(&mut self, operand: Operand) -> Operand {
        operand
    }

    fn instance_fqn(&self, instance: &InstanceKey) -> String {
        if instance.type_args.is_empty() {
            return instance.template.fqn.clone();
        }
        let args = instance
            .type_args
            .iter()
            .map(|&ty| self.types.display(ty).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}::<{}>", instance.template.fqn, args)
    }
}

fn belongs_to_template_family(fun_fqn: &str, root_fqn: &str) -> bool {
    fun_fqn == root_fqn
        || fun_fqn
            .strip_prefix(root_fqn)
            .is_some_and(|suffix| suffix.starts_with(".$lambda"))
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

fn substitute_type_params(
    types: &mut TypeStore,
    ty: TypeId,
    param_map: &HashMap<String, TypeId>,
) -> TypeId {
    match types.kind(ty).clone() {
        TypeKind::Param(param) => param_map.get(&param.name).copied().unwrap_or(ty),
        TypeKind::StarProjection(star) => {
            let read_ty = substitute_type_params(types, star.read_ty, param_map);
            types.ty_star_projection(read_ty)
        }
        TypeKind::Ref(RefTypeKind::Any) | TypeKind::Ref(RefTypeKind::String) => ty,
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            let args = nominal
                .args
                .iter()
                .map(|&arg| substitute_type_params(types, arg, param_map))
                .collect();
            let eff = nominal
                .eff
                .as_ref()
                .map(|row| substitute_type_params_in_effect_row(types, row, param_map));
            types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: nominal.fqn,
                args,
                eff,
            })))
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let receiver = fun
                .receiver
                .map(|receiver| substitute_type_params(types, receiver, param_map));
            let params = fun
                .params
                .iter()
                .map(|&param| substitute_type_params(types, param, param_map))
                .collect();
            let return_ty = substitute_type_params(types, fun.return_ty, param_map);
            let effects = substitute_type_params_in_effect_row(types, &fun.effects, param_map);
            types.ty_function(receiver, params, return_ty, effects, fun.effects_closed)
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            let variants = union
                .variants
                .iter()
                .map(|&variant| substitute_type_params(types, variant, param_map))
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
            let inner = substitute_type_params(types, inner, param_map);
            types.ty_option(inner)
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let elements = elements
                .iter()
                .map(|&element| substitute_type_params(types, element, param_map))
                .collect();
            types.ty_tuple(elements)
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let args = nominal
                .args
                .iter()
                .map(|&arg| substitute_type_params(types, arg, param_map))
                .collect();
            let eff = nominal
                .eff
                .as_ref()
                .map(|row| substitute_type_params_in_effect_row(types, row, param_map));
            types.intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                fqn: nominal.fqn,
                args,
                eff,
            })))
        }
    }
}

fn substitute_type_params_in_effect_row(
    types: &mut TypeStore,
    row: &EffectRow,
    param_map: &HashMap<String, TypeId>,
) -> EffectRow {
    EffectRow::new(
        row.terms
            .iter()
            .map(|&term| substitute_type_params(types, term, param_map))
            .collect(),
    )
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
