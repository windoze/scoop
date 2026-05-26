//! `.cone` 归档中的 pre-specialize 元数据（v0，T1108/T1109）。
//!
//! 目标（当前阶段）：
//! - 支持 Cone.toml 的 `[pre-specialize].functions` 声明；
//! - 支持 Cone.toml 的 `[pre-specialize].types` 声明；
//! - 在打包 `.cone` 时把“预编译（预生成）实例”的索引写入归档（JSON）；
//! - 下游读取时加载该索引，供后续 monomorph/codegen 复用（当前仅用于回归与计数验证）。
//!
//! 说明：
//! - 当前阶段的产物仍以“可回归/可验证”为目标：
//!   - 函数实例：导出 MIR debug 文本占位；
//!   - 类型实例：导出稳定 key（用于下游命中/缺失计数），不生成真实 codegen 产物。

use std::collections::{HashMap, HashSet};

use miette::{Context as _, Diagnostic, IntoDiagnostic as _, Result, miette};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use scoop_project_model::{ConeId, ConeManifest};
use scoopc_ast as ast;
use scoopc_hir::hir;
use scoopc_hir::resolve::{Index, IndexedFile};
use scoopc_hir::session::Session;
use scoopc_hir::stable_id::StableConeKey;
use scoopc_mir::mir;
use scoopc_source::SourceFile;
use scoopc_types::{
    BuiltinTypes, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
};

/// `.cone` 内的 pre-specialize 元数据文件名（v0 约定）。
pub const CONE_PRE_SPECIALIZE_FILE_NAME: &str = "PRE_SPECIALIZE.json";

pub const CONE_PRE_SPECIALIZE_SCHEMA_NAME: &str = "scoop.cone.pre_specialize";
pub const CONE_PRE_SPECIALIZE_SCHEMA_VERSION: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConePreSpecializeSchema {
    pub name: String,
    pub version: u32,
}

/// 一个“预编译函数实例”的稳定键（v0）。
///
/// 说明：
/// - key 不包含 `decl_file/span`，因为 `.cone` 的消费侧只有 public API，并不持有源级定位信息；
/// - v0 仅支持 `type_args`（effect row args 后续任务补齐）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PreSpecializedFunKey {
    /// 泛型函数的 FQN（例如 `my.pkg.id`）。
    pub fqn: String,
    /// 类型实参（canonical 文本，需与 `TypeStore::display` 对齐，例如 `Int` / `a.b.Token`）。
    pub type_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreSpecializedFunInstance {
    pub key: PreSpecializedFunKey,
    /// 单态化实例的内部符号名（当前阶段沿用 dump-ir 的命名约定：`fqn::<T...>`）。
    pub instance_fqn: String,
    /// 预编译产物（当前阶段先用 MIR Debug 文本占位，便于回归与人工排查）。
    pub mir_debug: String,
}

/// 一个“预生成类型实例”的稳定键（v0）。
///
/// 说明：
/// - `fqn` 指向泛型名义类型的 FQN（例如 `my.pkg.Box`）；
/// - `type_args` 使用 canonical 文本，与 `TypeStore::display` 对齐（例如 `Int` / `a.b.Token`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PreSpecializedTypeKey {
    /// 泛型名义类型的 FQN（例如 `my.pkg.Box`）。
    pub fqn: String,
    /// 类型实参（canonical 文本）。
    pub type_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreSpecializedTypeInstance {
    pub key: PreSpecializedTypeKey,
    /// 单态化实例的内部符号名（当前阶段沿用 `::<...>` 命名约定）。
    pub instance_fqn: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConePreSpecializeFile {
    pub schema: ConePreSpecializeSchema,
    #[serde(default)]
    pub funs: Vec<PreSpecializedFunInstance>,
    #[serde(default)]
    pub types: Vec<PreSpecializedTypeInstance>,
}

impl ConePreSpecializeFile {
    pub fn new_v0(
        mut funs: Vec<PreSpecializedFunInstance>,
        mut types: Vec<PreSpecializedTypeInstance>,
    ) -> Self {
        funs.sort_by(|a, b| {
            a.key
                .fqn
                .cmp(&b.key.fqn)
                .then(a.instance_fqn.cmp(&b.instance_fqn))
        });
        funs.dedup_by(|a, b| a.key == b.key);

        types.sort_by(|a, b| {
            a.key
                .fqn
                .cmp(&b.key.fqn)
                .then(a.instance_fqn.cmp(&b.instance_fqn))
        });
        types.dedup_by(|a, b| a.key == b.key);

        Self {
            schema: ConePreSpecializeSchema {
                name: CONE_PRE_SPECIALIZE_SCHEMA_NAME.to_string(),
                version: CONE_PRE_SPECIALIZE_SCHEMA_VERSION,
            },
            funs,
            types,
        }
    }

    pub fn fun_key_set(&self) -> HashSet<PreSpecializedFunKey> {
        self.funs.iter().map(|f| f.key.clone()).collect()
    }

    pub fn type_key_set(&self) -> HashSet<PreSpecializedTypeKey> {
        self.types.iter().map(|t| t.key.clone()).collect()
    }
}

#[derive(Debug, Error, Diagnostic)]
pub enum PreSpecializeError {
    #[error("`[pre-specialize].functions` 的条目必须形如 `fqn<TypeArgs...>`，但得到：{spec}")]
    #[diagnostic(code(scoop::cone::pre_specialize_invalid_fun_spec))]
    InvalidFunSpec { spec: String },

    #[error("`[pre-specialize].types` 的条目必须形如 `TypeFqn<TypeArgs...>`，但得到：{spec}")]
    #[diagnostic(code(scoop::cone::pre_specialize_invalid_type_spec))]
    InvalidTypeSpec { spec: String },

    #[error("pre-specialize 找不到函数声明：{fqn}")]
    #[diagnostic(code(scoop::cone::pre_specialize_fun_not_found))]
    FunNotFound { fqn: String },

    #[error("pre-specialize 目标函数存在多个 overload：{fqn}（当前阶段不支持消歧）")]
    #[diagnostic(code(scoop::cone::pre_specialize_fun_overloaded))]
    FunOverloaded { fqn: String },

    #[error("pre-specialize 的 type args 数量不匹配：{fqn} 期望 {expected} 个，但得到 {found} 个")]
    #[diagnostic(code(scoop::cone::pre_specialize_type_arg_arity_mismatch))]
    TypeArgArityMismatch {
        fqn: String,
        expected: usize,
        found: usize,
    },

    #[error("pre-specialize 找不到类型：{fqn}")]
    #[diagnostic(code(scoop::cone::pre_specialize_type_not_found))]
    TypeNotFound { fqn: String },

    #[error("pre-specialize 找不到类型声明：{fqn}")]
    #[diagnostic(code(scoop::cone::pre_specialize_type_decl_not_found))]
    TypeDeclNotFound { fqn: String },

    #[error("pre-specialize 目标类型存在重复声明：{fqn}")]
    #[diagnostic(code(scoop::cone::pre_specialize_type_decl_duplicated))]
    TypeDeclDuplicated { fqn: String },

    #[error("pre-specialize 只支持名义类型（path）类型实参：{text}")]
    #[diagnostic(code(scoop::cone::pre_specialize_unsupported_type_syntax))]
    UnsupportedTypeSyntax { text: String },

    #[error("pre-specialize 解析类型失败：{text}")]
    #[diagnostic(code(scoop::cone::pre_specialize_parse_type_failed))]
    ParseTypeFailed { text: String },
}

/// 从 `.cone` 归档条目内容解析 pre-specialize 文件（v0 JSON）。
pub fn parse_pre_specialize_file(bytes: &[u8]) -> Result<ConePreSpecializeFile> {
    let file: ConePreSpecializeFile = serde_json::from_slice(bytes)
        .into_diagnostic()
        .wrap_err("解析 PRE_SPECIALIZE.json 失败")?;

    if file.schema.name != CONE_PRE_SPECIALIZE_SCHEMA_NAME {
        return Err(miette!(
            "PRE_SPECIALIZE schema.name 不匹配：期望 `{}`，但得到 `{}`",
            CONE_PRE_SPECIALIZE_SCHEMA_NAME,
            file.schema.name
        ));
    }
    if file.schema.version > CONE_PRE_SPECIALIZE_SCHEMA_VERSION {
        return Err(miette!(
            "PRE_SPECIALIZE schema.version 不支持：当前最多支持 v{}，但得到 v{}",
            CONE_PRE_SPECIALIZE_SCHEMA_VERSION,
            file.schema.version
        ));
    }

    Ok(file)
}

/// 为一个 cone 的 sources 生成 pre-specialize 文件（若未声明则返回 `Ok(None)`）。
pub fn build_pre_specialize_file_for_cone_sources(
    session: &Session,
    sources: &[SourceFile],
    manifest: &ConeManifest,
) -> Result<Option<ConePreSpecializeFile>> {
    if manifest.pre_specialize_functions.is_empty() && manifest.pre_specialize_types.is_empty() {
        return Ok(None);
    }

    // 1) parse sources → AST（resolver 会写回绑定结果，因此用可变 Vec 承载）。
    let mut asts = Vec::with_capacity(sources.len());
    for source in sources {
        let ast = scoopc_ast::parser::parse_file(source).map_err(miette::Report::from)?;
        asts.push(ast);
    }
    // 2) index：sysroot cone=0，当前 cone=1（与 build/scoopir 导出保持一致）。
    let mut indexed: Vec<IndexedFile<'_>> = Vec::new();
    for f in session.sysroot().index_files() {
        indexed.push(IndexedFile {
            cone: ConeId::new(0),
            cone_kind: if f.source.is_trusted_syslib() {
                scoop_project_model::ConeKind::Syslib
            } else {
                scoop_project_model::ConeKind::Lib
            },
            source: &f.source,
            file: &f.ast,
        });
    }
    for (source, ast) in sources.iter().zip(asts.iter()) {
        indexed.push(IndexedFile {
            cone: ConeId::new(1),
            cone_kind: manifest.cone.kind,
            source,
            file: ast,
        });
    }
    let index = Index::build_with_cones(&indexed).map_err(miette::Report::from)?;

    // 3) resolver：headers + bodies（写回 resolved 信息，供 HIR lowering 使用）。
    let mut headers = Vec::with_capacity(sources.len());
    for (source, ast) in sources.iter().zip(asts.iter()) {
        headers.push(
            scoopc_hir::resolve::check_file_headers(source, ast, &index)
                .map_err(miette::Report::from)?,
        );
    }
    for ((source, ast), h) in sources.iter().zip(asts.iter_mut()).zip(headers.iter()) {
        scoopc_hir::resolve::check_file_bodies(source, ast, &index, h)
            .map_err(miette::Report::from)?;
    }

    // 4) type_kinds：用于把名义类型区分为 value/ref nominal（struct/enum vs class/interface/effect）。
    let mut compilation_unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in session.sysroot().index_files() {
        compilation_unit.push((&f.source, &f.ast));
    }
    for (source, ast) in sources.iter().zip(asts.iter()) {
        compilation_unit.push((source, ast));
    }
    let type_kinds = collect_type_decl_kinds(&compilation_unit);
    let class_vtables = scoopc_hir::vtable::collect_class_vtables(&compilation_unit, &index)
        .map_err(miette::Report::from)?;
    let (_interfaces, _class_itables) = scoopc_hir::itable::collect_interfaces_and_class_itables(
        &compilation_unit,
        &index,
        &class_vtables,
    )
    .map_err(miette::Report::from)?;

    // 5) 扫描顶层 fun decls：FQN → (decl_source_idx, decl_ptr)。
    let fun_decl_index = index_compilation_unit_fun_decls(sources, &asts);
    let type_decl_index = index_compilation_unit_type_decls(sources, &asts);

    // 6) 逐条生成实例（并写出 v0 JSON）。
    let mut out_funs: Vec<PreSpecializedFunInstance> = Vec::new();
    for spec in &manifest.pre_specialize_functions {
        let (fqn, raw_type_args) = parse_fun_instance_spec(spec)
            .map_err(|_| PreSpecializeError::InvalidFunSpec { spec: spec.clone() })?;

        let decl = fun_decl_index
            .get(&fqn)
            .ok_or_else(|| PreSpecializeError::FunNotFound { fqn: fqn.clone() })?;
        if decl.len() != 1 {
            return Err(PreSpecializeError::FunOverloaded { fqn }.into());
        }
        let (source, file, fun) = decl[0];

        let expected = fun.type_params.len();
        let found = raw_type_args.len();
        if expected != found {
            return Err(PreSpecializeError::TypeArgArityMismatch {
                fqn,
                expected,
                found,
            }
            .into());
        }

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        let mut type_args: Vec<TypeId> = Vec::with_capacity(raw_type_args.len());
        let mut type_args_text: Vec<String> = Vec::with_capacity(raw_type_args.len());
        for t in &raw_type_args {
            let id = intern_type_arg_from_text(&mut types, builtins, &type_kinds, t)?;
            type_args_text.push(types.display(id).to_string());
            type_args.push(id);
        }

        // 建立 `T -> TypeId` 的绑定。
        let mut bindings: Vec<(String, TypeId)> = Vec::with_capacity(fun.type_params.len());
        for (idx, p) in fun.type_params.iter().enumerate() {
            let name = p.name.text(source).to_string();
            bindings.push((name, type_args[idx]));
        }

        let compilation_unit = [(source, file)];
        let stable_cone_key = StableConeKey::from_manifest(manifest);
        let generic_template_symbol_suffixes =
            hir::generic_template_symbol_suffixes_for_compilation_unit(
                &stable_cone_key,
                &index,
                &compilation_unit,
            );
        let lowered_fun = hir::lower_fun_with_type_bindings_and_mir_facts(
            hir::LoweringInputs {
                source,
                file,
                index: &index,
                type_kinds: &type_kinds,
                typecheck_types: None,
                compilation_unit: &compilation_unit,
                types: &mut types,
                builtins,
                generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                materialize_direct_call_targets: false,
            },
            fun,
            bindings,
        );
        let mut hir_fun = lowered_fun.fun;

        let instance_fqn = monomorph_instance_fqn(&hir_fun.fqn, &type_args, &types);
        hir_fun.fqn = instance_fqn.clone();

        let hir_file = hir::File {
            decls: Vec::new(),
            items: vec![hir::Item::Fun(hir_fun)],
        };
        let hir_fact_scaffold = hir::LoweredHir {
            file: hir_file.clone(),
            stable_cone_key: stable_cone_key.clone(),
            source_cones: HashMap::new(),
            source_cone_order: HashMap::from([(stable_cone_key.clone(), 0)]),
            stable_type_param_keys: HashMap::new(),
            member_funs: Vec::new(),
            types: types.clone(),
            struct_layouts: HashMap::new(),
            enum_layouts: HashMap::new(),
            extern_funs: HashMap::new(),
            native_callable_funs: HashMap::new(),
            extern_globals: HashMap::new(),
            extern_libs: Vec::new(),
            top_level_vars: HashMap::new(),
            top_level_immutable_values: HashMap::new(),
            top_level_fun_call_sites: lowered_fun.top_level_fun_call_sites,
            call_arg_bindings: HashMap::new(),
            with_update_contracts: HashMap::new(),
            assign_place_contracts: HashMap::new(),
            object_inits: HashMap::new(),
            generic_class_decls: HashMap::new(),
            class_inits: HashMap::new(),
            class_vtables: HashMap::new(),
            interfaces: HashMap::new(),
            class_itables: HashMap::new(),
            ctor_call_sites: HashMap::new(),
            dispatch_call_sites: lowered_fun.dispatch_call_sites,
            effect_op_call_sites: lowered_fun.effect_op_call_sites,
            handle_payload_tuple_tys: HashMap::new(),
            continuation_resume_call_sites: file
                .continuation_resume_call_sites()
                .into_iter()
                .map(|span| hir::CallSite::new(source.path().to_path_buf(), span))
                .collect(),
            non_pure_continuation_resume_call_sites: file
                .non_pure_continuation_resume_call_sites()
                .into_iter()
                .map(|span| hir::CallSite::new(source.path().to_path_buf(), span))
                .collect(),
            when_pat_binding_tys: lowered_fun.when_pat_binding_tys,
            nominal_kinds: HashMap::new(),
            nominal_variances: HashMap::new(),
            direct_supertypes: HashMap::new(),
            builtins,
        };
        let hir_facts = scoopc_hir::stage::build_hir_declaration_facts_from_lowered_hir(
            &hir_fact_scaffold,
            source.path(),
        )
        .map_err(miette::Report::from)?;
        let mir_facts = mir::MirLoweringFacts::from_hir_facts(&hir_fact_scaffold, &hir_facts);
        let mir_file = mir::lower_hir_file_for_dump_with_facts(
            builtins,
            &mut types,
            &hir_file,
            &[],
            &mir_facts,
        );

        out_funs.push(PreSpecializedFunInstance {
            key: PreSpecializedFunKey {
                fqn: fqn.clone(),
                type_args: type_args_text,
            },
            instance_fqn,
            mir_debug: format!("{mir_file:#?}"),
        });
    }

    let mut out_types: Vec<PreSpecializedTypeInstance> = Vec::new();
    for spec in &manifest.pre_specialize_types {
        let path = parse_type_path(spec)
            .map_err(|_| PreSpecializeError::InvalidTypeSpec { spec: spec.clone() })?;

        let base_fqn = path.fqn.clone();
        let decl =
            type_decl_index
                .get(&base_fqn)
                .ok_or_else(|| PreSpecializeError::TypeDeclNotFound {
                    fqn: base_fqn.clone(),
                })?;
        if decl.len() != 1 {
            return Err(PreSpecializeError::TypeDeclDuplicated { fqn: base_fqn }.into());
        }
        let (_source, _file, decl) = decl[0];

        let expected = decl.type_params.len();
        let found = path.args.len();
        if expected != found {
            return Err(PreSpecializeError::TypeArgArityMismatch {
                fqn: base_fqn,
                expected,
                found,
            }
            .into());
        }

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        let mut type_args: Vec<TypeId> = Vec::with_capacity(path.args.len());
        let mut type_args_text: Vec<String> = Vec::with_capacity(path.args.len());
        for a in &path.args {
            let id = intern_type_path(&mut types, builtins, &type_kinds, a)?;
            type_args_text.push(types.display(id).to_string());
            type_args.push(id);
        }

        let instance_fqn = monomorph_instance_fqn(&path.fqn, &type_args, &types);
        out_types.push(PreSpecializedTypeInstance {
            key: PreSpecializedTypeKey {
                fqn: path.fqn,
                type_args: type_args_text,
            },
            instance_fqn,
        });
    }

    Ok(Some(ConePreSpecializeFile::new_v0(out_funs, out_types)))
}

fn parse_fun_instance_spec(spec: &str) -> std::result::Result<(String, Vec<String>), ()> {
    let spec = spec.trim();
    if !spec.ends_with('>') {
        return Err(());
    }
    let bytes = spec.as_bytes();
    let mut depth: i32 = 0;
    let mut lt_pos: Option<usize> = None;
    for (idx, ch) in bytes.iter().enumerate().rev() {
        match *ch as char {
            '>' => depth += 1,
            '<' => {
                depth -= 1;
                if depth == 0 {
                    lt_pos = Some(idx);
                    break;
                }
                if depth < 0 {
                    return Err(());
                }
            }
            _ => {}
        }
    }
    let Some(lt) = lt_pos else {
        return Err(());
    };
    let base = spec[..lt].trim();
    let inner = &spec[lt + 1..spec.len() - 1];
    if base.is_empty() {
        return Err(());
    }

    let mut args = Vec::new();
    let mut start = 0usize;
    let mut nest: i32 = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '<' => nest += 1,
            '>' => nest -= 1,
            ',' if nest == 0 => {
                let part = inner[start..idx].trim();
                if part.is_empty() {
                    return Err(());
                }
                args.push(part.to_string());
                start = idx + 1;
            }
            _ => {}
        }
        if nest < 0 {
            return Err(());
        }
    }
    let tail = inner[start..].trim();
    if tail.is_empty() {
        return Err(());
    }
    args.push(tail.to_string());

    Ok((base.to_string(), args))
}

/// 收集编译单元中的顶层 `fun` 声明：FQN → (source, file, fun) 列表。
fn index_compilation_unit_fun_decls<'a>(
    sources: &'a [SourceFile],
    asts: &'a [ast::File],
) -> HashMap<String, Vec<(&'a SourceFile, &'a ast::File, &'a ast::FunDecl)>> {
    let mut out: HashMap<String, Vec<(&SourceFile, &ast::File, &ast::FunDecl)>> = HashMap::new();

    for (source, file) in sources.iter().zip(asts.iter()) {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Fun(fun) = item else {
                continue;
            };
            let local = source.slice(fun.name.span);
            let fqn = if pkg_prefix.is_empty() {
                local.to_string()
            } else {
                format!("{pkg_prefix}.{local}")
            };
            out.entry(fqn).or_default().push((source, file, fun));
        }
    }

    out
}

/// 收集编译单元中的顶层 `type` 声明：FQN → (source, file, type_decl) 列表。
fn index_compilation_unit_type_decls<'a>(
    sources: &'a [SourceFile],
    asts: &'a [ast::File],
) -> HashMap<String, Vec<(&'a SourceFile, &'a ast::File, &'a ast::TypeDecl)>> {
    let mut out: HashMap<String, Vec<(&SourceFile, &ast::File, &ast::TypeDecl)>> = HashMap::new();

    for (source, file) in sources.iter().zip(asts.iter()) {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            let local = ty.name.text(source);
            let fqn = if pkg_prefix.is_empty() {
                local.to_string()
            } else {
                format!("{pkg_prefix}.{local}")
            };
            out.entry(fqn).or_default().push((source, file, ty));
        }
    }

    out
}

fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(p) = package else {
        return String::new();
    };
    p.path
        .iter()
        .map(|seg| seg.text(source))
        .collect::<Vec<_>>()
        .join(".")
}

fn collect_type_decl_kinds(pairs: &[(&SourceFile, &ast::File)]) -> HashMap<String, ast::TypeKind> {
    let mut out: HashMap<String, ast::TypeKind> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };
            out.insert(fqn, ty.kind);
        }
    }
    out
}

fn monomorph_instance_fqn(base: &str, type_args: &[TypeId], types: &TypeStore) -> String {
    if type_args.is_empty() {
        return base.to_string();
    }
    let args = type_args
        .iter()
        .copied()
        .map(|id| types.display(id).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("{base}::<{args}>")
}

/// 解析并 intern 一个 type arg（v0：只支持 path/nominal + `<...>`）。
fn intern_type_arg_from_text(
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    type_kinds: &HashMap<String, ast::TypeKind>,
    text: &str,
) -> Result<TypeId> {
    let ty = parse_type_path(text).map_err(|_| PreSpecializeError::ParseTypeFailed {
        text: text.to_string(),
    })?;
    intern_type_path(types, builtins, type_kinds, &ty)
}

#[derive(Debug, Clone)]
struct TypePathSpec {
    fqn: String,
    args: Vec<TypePathSpec>,
}

fn parse_type_path(text: &str) -> std::result::Result<TypePathSpec, ()> {
    let text = text.trim();
    if text.is_empty() {
        return Err(());
    }

    // v0：只支持 `a.b.C<...>`；其它形式（tuple/function/nullable/union）暂不支持。
    if text.starts_with('(') || text.contains("->") || text.contains('|') || text.ends_with('?') {
        return Err(());
    }

    let (head, args) = if text.ends_with('>') && text.contains('<') {
        let bytes = text.as_bytes();
        let mut depth: i32 = 0;
        let mut lt_pos: Option<usize> = None;
        for (idx, ch) in bytes.iter().enumerate().rev() {
            match *ch as char {
                '>' => depth += 1,
                '<' => {
                    depth -= 1;
                    if depth == 0 {
                        lt_pos = Some(idx);
                        break;
                    }
                    if depth < 0 {
                        return Err(());
                    }
                }
                _ => {}
            }
        }
        let Some(lt) = lt_pos else {
            return Err(());
        };
        let head = text[..lt].trim();
        let inner = &text[lt + 1..text.len() - 1];
        let args = split_top_level_commas(inner)?;
        (head.to_string(), args)
    } else {
        (text.to_string(), Vec::new())
    };

    if head.is_empty() {
        return Err(());
    }
    if head.split('.').any(|seg| seg.trim().is_empty()) {
        return Err(());
    }

    let mut parsed_args = Vec::with_capacity(args.len());
    for a in args {
        parsed_args.push(parse_type_path(&a)?);
    }

    Ok(TypePathSpec {
        fqn: head.to_string(),
        args: parsed_args,
    })
}

fn split_top_level_commas(s: &str) -> std::result::Result<Vec<String>, ()> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut nest: i32 = 0;
    for (idx, ch) in s.char_indices() {
        match ch {
            '<' => nest += 1,
            '>' => nest -= 1,
            ',' if nest == 0 => {
                let part = s[start..idx].trim();
                if part.is_empty() {
                    return Err(());
                }
                out.push(part.to_string());
                start = idx + 1;
            }
            _ => {}
        }
        if nest < 0 {
            return Err(());
        }
    }
    let tail = s[start..].trim();
    if tail.is_empty() {
        return Err(());
    }
    out.push(tail.to_string());
    Ok(out)
}

fn intern_type_path(
    types: &mut TypeStore,
    builtins: BuiltinTypes,
    type_kinds: &HashMap<String, ast::TypeKind>,
    path: &TypePathSpec,
) -> Result<TypeId> {
    // builtin 类型：同时允许 `Int` 与 `scoop.core.Int` 两种写法。
    // 与 typecheck::implicit_builtin_type_fqn 保持一致。
    match path.fqn.as_str() {
        "Any" | "scoop.core.Any" => return Ok(builtins.any),
        "String" | "scoop.core.String" => return Ok(builtins.string),
        "Unit" | "scoop.core.Unit" => return Ok(builtins.unit),
        "Nothing" | "scoop.core.Nothing" => return Ok(builtins.nothing),
        "Bool" | "scoop.core.Bool" => return Ok(builtins.bool_),
        "Char" | "scoop.core.Char" => return Ok(builtins.char_),
        "Float64" | "scoop.core.Float64" => return Ok(builtins.float64),
        "Float32" | "scoop.core.Float32" => return Ok(builtins.float32),
        "Int" | "scoop.core.Int" => return Ok(builtins.int),
        "UInt" | "scoop.core.UInt" => return Ok(builtins.uint),
        "Option" | "scoop.core.Option" => {
            if path.args.len() != 1 {
                return Err(PreSpecializeError::TypeArgArityMismatch {
                    fqn: "Option".to_string(),
                    expected: 1,
                    found: path.args.len(),
                }
                .into());
            }
            let inner = intern_type_path(types, builtins, type_kinds, &path.args[0])?;
            return Ok(types.ty_option(inner));
        }
        _ => {}
    }

    // v0：只支持声明表中可见的名义类型（sysroot + 当前 cone sources）。
    let Some(kind) = type_kinds.get(&path.fqn).copied() else {
        return Err(PreSpecializeError::TypeNotFound {
            fqn: path.fqn.clone(),
        }
        .into());
    };

    let mut args: Vec<TypeId> = Vec::with_capacity(path.args.len());
    for a in &path.args {
        args.push(intern_type_path(types, builtins, type_kinds, a)?);
    }

    // v0：pre-specialize type key 目前只编码普通 type args；use-site effect row
    // 仍不写入该 JSON key，因此这里保持 `eff: None`。
    let nominal = NominalType {
        fqn: path.fqn.clone(),
        args,
        eff: None,
    };

    let id = match kind {
        ast::TypeKind::Struct | ast::TypeKind::Enum => {
            types.intern(TypeKind::Value(ValueTypeKind::Nominal(nominal)))
        }
        ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => {
            types.intern(TypeKind::Ref(RefTypeKind::Nominal(nominal)))
        }
    };

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_json_surface_stays_semantic_and_path_free(json: &str) {
        for forbidden in [
            "TypeId(",
            "ClosureId(",
            "SourceId(",
            "ConeId(",
            "SymbolId(",
            "BasicBlockId(",
            "LocalId(",
            "SiteId(",
            "StepSchemaId(",
            "ContinuationSchemaId(",
            "CaseTag(",
            "source_path",
            "decl_span",
            "/Users/",
            env!("CARGO_MANIFEST_DIR"),
        ] {
            assert!(
                !json.contains(forbidden),
                "healthy PRE_SPECIALIZE schema surface 不应泄漏 dense id/path 文本: {forbidden}\n{json}"
            );
        }
    }

    #[test]
    fn parse_fun_instance_spec_supports_nested_generics() {
        let (fqn, args) = parse_fun_instance_spec("a.b.f<Map<String, Int>>").unwrap();
        assert_eq!(fqn, "a.b.f");
        assert_eq!(args, vec!["Map<String, Int>"]);
    }

    #[test]
    fn pre_specialize_json_baseline_stays_semantic_and_path_free() {
        let file = ConePreSpecializeFile::new_v0(
            vec![PreSpecializedFunInstance {
                key: PreSpecializedFunKey {
                    fqn: "a.id".to_string(),
                    type_args: vec!["a.Token".to_string()],
                },
                instance_fqn: "a.id::<a.Token>".to_string(),
                mir_debug: "FunDecl(name = a.id::<a.Token>)".to_string(),
            }],
            vec![PreSpecializedTypeInstance {
                key: PreSpecializedTypeKey {
                    fqn: "a.Box".to_string(),
                    type_args: vec!["a.Token".to_string()],
                },
                instance_fqn: "a.Box::<a.Token>".to_string(),
            }],
        );

        let json = serde_json::to_string_pretty(&file).unwrap();
        assert!(json.contains("\"fqn\": \"a.id\""));
        assert!(json.contains("\"instance_fqn\": \"a.Box::<a.Token>\""));
        assert_json_surface_stays_semantic_and_path_free(&json);
    }
}
