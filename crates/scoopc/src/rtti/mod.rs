//! RTTI（Runtime Type Info）v0：为类型生成运行期描述符（TODO T1206）。
//!
//! 目标：
//! - 先给“可静态确定布局”的类型生成最小 RTTI：type id + size/align +（struct）字段布局；
//! - 该信息主要用于调试/后续 GC 与后端集成；不承诺提供完整运行期反射能力（spec §6.6）。
//!
//! 约束（v0）：
//! - `dump_file_rtti` 仍只枚举“当前输入文件内可直接命名的非参数化 struct”；
//! - `dump_type_rtti` 则允许查询参数化 nominal，并使用 `TypeStore::display` 产出的
//!   canonical name 计算稳定 `type_id`；
//! - 其它类型只提供 size/align（或按指针大小占位）；
//! - 目标平台布局暂用 host pointer size/align（与 typecheck/layout 一致，T0803 再替换为 target machine）。

pub mod type_desc;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use miette::Diagnostic;
use serde::Serialize;
use thiserror::Error;

use crate::ast;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{ImportTable, Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
use crate::stable_id::stable_rtti_type_id;
use crate::ty::layout::{NicheDomain, NicheStorage, TargetLayout, TypeLayout};
use crate::ty::{
    BuiltinTypes, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
};
use crate::typecheck::{
    StructDeclError, TypeEnv, TypeEnvError, TypeHeaderError, TypeLowerError, TypeLowering,
};

#[derive(Debug, Clone, Serialize)]
pub struct TargetLayoutInfo {
    pub pointer_size: u64,
    pub pointer_align: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RttiDump {
    pub target: TargetLayoutInfo,
    /// 按 `name` 排序的类型描述符列表（输出稳定）。
    pub types: Vec<TypeRtti>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RttiKind {
    Builtin,
    Ref,
    Struct,
    Enum,
    Tuple,
    Option,
    Opaque,
}

#[derive(Debug, Clone, Serialize)]
pub struct TypeRtti {
    /// 稳定的可读名字（当前使用 `TypeStore::display` 产出的 canonical name）。
    pub name: String,
    /// 稳定 type id（v0：sha256(name) 的前 8 字节，小端解释）。
    pub type_id: u64,
    pub kind: RttiKind,
    pub size: u64,
    pub align: u64,
    /// 仅对 struct 有字段布局信息。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<FieldRtti>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldRtti {
    pub name: String,
    pub ty: String,
    pub offset: u64,
    pub size: u64,
    pub align: u64,
    /// 是否为“直接引用类型”字段（用于后续 GC trace bitmap 生成）。
    pub is_ref: bool,
}

#[derive(Debug, Error, Diagnostic)]
pub enum RttiError {
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

    #[error("未知类型：{name}")]
    #[diagnostic(code(scoop::rtti::unknown_type))]
    UnknownType { name: String },

    #[error("类型名不唯一：{name}（候选：{candidates}）")]
    #[diagnostic(code(scoop::rtti::ambiguous_type))]
    AmbiguousType { name: String, candidates: String },

    #[error("struct RTTI 生成缺少声明：{fqn}")]
    #[diagnostic(code(scoop::rtti::missing_struct_decl))]
    MissingStructDecl { fqn: String },
}

/// 计算输入文件内“可生成 RTTI 的类型表”，并返回稳定输出结构。
///
/// 当前阶段：
/// - 列表模式仍只导出当前输入文件里“声明本身已是 concrete 的 struct”；
/// - 参数化 nominal 的查询能力由 `dump_type_rtti` 提供。
pub fn dump_file_rtti(session: &Session, source: &SourceFile) -> Result<RttiDump, RttiError> {
    let mut cx = RttiContext::build(session, source)?;
    let mut out: Vec<TypeRtti> = Vec::new();

    let mut names = cx.dumpable_structs.clone();
    names.sort();
    for fqn in names {
        let ty = cx.struct_type_id(&fqn);
        out.push(cx.type_rtti(ty)?);
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(RttiDump {
        target: TargetLayoutInfo {
            pointer_size: cx.target.pointer_size,
            pointer_align: cx.target.pointer_align,
        },
        types: out,
    })
}

/// 按名字查询并生成单个类型的 RTTI（用于 `scoop dump-rtti --type ...`）。
///
/// 说明（v0）：
/// - 支持 builtin：`Any/String/Unit/Bool/Int/UInt/IntN/UIntN`（最小集合）；
/// - 支持当前文件语境下可解析的 type query，包括带 type args / `eff` row 的 nominal。
pub fn dump_type_rtti(
    session: &Session,
    source: &SourceFile,
    name: &str,
) -> Result<TypeRtti, RttiError> {
    let mut cx = RttiContext::build(session, source)?;
    let ty = cx.resolve_query_type_id(name)?;
    cx.type_rtti(ty)
}

struct StructInfo {
    decl_file: PathBuf,
    fields: Vec<StructFieldInfo>,
}

struct StructFieldInfo {
    name: String,
    ty: ast::TypeRef,
}

struct RttiContext {
    index: Index,
    env: TypeEnv,
    types: TypeStore,
    builtins: BuiltinTypes,
    target: TargetLayout,
    query_pkg_prefix: String,
    query_imports: ImportTable,
    dumpable_structs: Vec<String>,
    structs: HashMap<String, StructInfo>,
    layout_cache: HashMap<TypeId, TypeLayout>,
    in_progress: HashSet<TypeId>,
}

impl RttiContext {
    fn build(session: &Session, source: &SourceFile) -> Result<Self, RttiError> {
        // 1) parse + 最小声明检查（不依赖 index/resolver）。
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
        crate::typecheck::check_file_headers(source, &file)?;
        crate::typecheck::check_file_struct_decls(source, &file)?;

        // 2) build index（sysroot + 当前文件）。
        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for f in &session.sysroot().files {
                pairs.push((&f.source, &f.ast));
            }
            pairs.push((source, &file));
            Index::build(&pairs)?
        };

        // 3) resolver（写回 binding 信息；同时生成 imports 表）。
        let resolved_headers = crate::resolve::check_file_headers(source, &file, &index)?;
        crate::resolve::check_file_bodies(source, &mut file, &index, &resolved_headers)?;

        // 4) type env（sysroot + 当前文件）。
        let mut env = TypeEnv::from_sysroot(session.sysroot(), &index)?;
        env.extend_from_file(source, &file, &index)?;

        // 5) 建立 struct 字段类型索引（会在 TypeStore 中 intern 字段类型与 struct 自身类型）。
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        let mut all_pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &session.sysroot().files {
            all_pairs.push((&f.source, &f.ast));
        }
        all_pairs.push((source, &file));
        let (dumpable_structs, structs) = collect_struct_infos(&all_pairs, source.path())?;

        Ok(Self {
            index,
            env,
            types,
            builtins,
            target: TargetLayout::host(),
            query_pkg_prefix: pkg_prefix,
            query_imports: resolved_headers.imports.clone(),
            dumpable_structs,
            structs,
            layout_cache: HashMap::new(),
            in_progress: HashSet::new(),
        })
    }

    fn resolve_query_type_id(&mut self, name: &str) -> Result<TypeId, RttiError> {
        // 1) builtin names（最小集合）。
        if let Some(id) = self.builtin_type_id(name) {
            return Ok(id);
        }

        let (query_source, query_ty) = parse_type_query(name)?;
        let mut lower = TypeLowering::new_with_ctx(
            &query_source,
            &self.index,
            &self.env,
            &mut self.types,
            self.builtins,
            self.query_pkg_prefix.clone(),
            self.query_imports.clone(),
        );
        match lower.lower_type_ref(&query_ty) {
            Ok(ty) => Ok(ty),
            Err(
                TypeLowerError::UnresolvedType { .. }
                | TypeLowerError::MissingTypeSymbolInEnv { .. },
            ) => Err(RttiError::UnknownType {
                name: name.to_string(),
            }),
            Err(err) => Err(err.into()),
        }
    }

    fn builtin_type_id(&mut self, name: &str) -> Option<TypeId> {
        match name {
            "Any" => Some(self.builtins.any),
            "String" => Some(self.builtins.string),
            "Unit" => Some(self.builtins.unit),
            "Bool" => Some(self.builtins.bool_),
            "Char" => Some(self.builtins.char_),
            "Float64" => Some(self.builtins.float64),
            "Float32" => Some(self.builtins.float32),
            "Int" => Some(self.builtins.int),
            "UInt" => Some(self.builtins.uint),
            _ => parse_int_width_suffix(name).map(|(signed, bits)| {
                if signed {
                    self.types.ty_int_n(bits)
                } else {
                    self.types.ty_uint_n(bits)
                }
            }),
        }
    }

    fn struct_type_id(&mut self, fqn: &str) -> TypeId {
        self.types
            .intern(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                fqn: fqn.to_string(),
                args: Vec::new(),
                eff: None,
            })))
    }

    fn type_rtti(&mut self, ty: TypeId) -> Result<TypeRtti, RttiError> {
        let name = self.types.display(ty).to_string();
        let type_id = stable_rtti_type_id(&name);
        let layout = self.type_layout(ty)?;

        let kind_snapshot = self.types.kind(ty).clone();
        let (kind, fields) = match kind_snapshot {
            TypeKind::Ref(rk) => (
                match rk {
                    RefTypeKind::Any
                    | RefTypeKind::String
                    | RefTypeKind::Function(_)
                    | RefTypeKind::Union(_)
                    | RefTypeKind::Nominal(_) => RttiKind::Ref,
                },
                None,
            ),
            TypeKind::Param(_) => (RttiKind::Opaque, None),
            TypeKind::StarProjection(_) => (RttiKind::Ref, None),
            TypeKind::Value(vk) => match vk {
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
                | ValueTypeKind::Char
                | ValueTypeKind::Float64
                | ValueTypeKind::Float32
                | ValueTypeKind::Int
                | ValueTypeKind::UInt
                | ValueTypeKind::IntN(_)
                | ValueTypeKind::UIntN(_) => (RttiKind::Builtin, None),
                ValueTypeKind::Tuple(_) => (RttiKind::Tuple, None),
                ValueTypeKind::Option(_) => (RttiKind::Option, None),
                ValueTypeKind::Nominal(nominal) => {
                    let Some(sym) = self.env.type_symbol(&nominal.fqn) else {
                        return Ok(TypeRtti {
                            name,
                            type_id,
                            kind: RttiKind::Opaque,
                            size: layout.size,
                            align: layout.align,
                            fields: None,
                        });
                    };
                    match sym.kind {
                        crate::typecheck::TypeSymbolKind::TypeAlias => (RttiKind::Opaque, None),
                        crate::typecheck::TypeSymbolKind::Nominal(kind) => match kind {
                            ast::TypeKind::Struct => {
                                let fields = Some(self.struct_fields_rtti(&nominal)?);
                                (RttiKind::Struct, fields)
                            }
                            ast::TypeKind::Enum => (RttiKind::Enum, None),
                            ast::TypeKind::Class
                            | ast::TypeKind::Interface
                            | ast::TypeKind::Effect => (RttiKind::Ref, None),
                        },
                    }
                }
            },
        };

        Ok(TypeRtti {
            name,
            type_id,
            kind,
            size: layout.size,
            align: layout.align,
            fields,
        })
    }

    fn struct_fields_rtti(&mut self, nominal: &NominalType) -> Result<Vec<FieldRtti>, RttiError> {
        let fields_snapshot = self.lower_struct_field_types(nominal)?;

        let mut out: Vec<FieldRtti> = Vec::with_capacity(fields_snapshot.len());
        let mut offset = 0u64;
        for (field_name, field_ty) in fields_snapshot {
            let layout = self.type_layout(field_ty)?;
            offset = align_to(offset, layout.align);
            out.push(FieldRtti {
                name: field_name,
                ty: self.types.display(field_ty).to_string(),
                offset,
                size: layout.size,
                align: layout.align,
                is_ref: self.types.is_ref(field_ty),
            });
            offset = offset.saturating_add(layout.size);
        }
        Ok(out)
    }

    fn lower_struct_field_types(
        &mut self,
        nominal: &NominalType,
    ) -> Result<Vec<(String, TypeId)>, RttiError> {
        let Some(info) = self.structs.get(&nominal.fqn) else {
            return Err(RttiError::MissingStructDecl {
                fqn: nominal.fqn.clone(),
            });
        };
        let Some(sym) = self.env.type_symbol(&nominal.fqn) else {
            return Err(RttiError::MissingStructDecl {
                fqn: nominal.fqn.clone(),
            });
        };
        let decl_source =
            self.env
                .source(&info.decl_file)
                .ok_or_else(|| RttiError::MissingStructDecl {
                    fqn: nominal.fqn.clone(),
                })?;
        let (pkg_prefix, imports) = self
            .env
            .file_type_context(&info.decl_file)
            .map(|ctx| (ctx.pkg_prefix.clone(), ctx.imports.clone()))
            .unwrap_or_else(|| (self.query_pkg_prefix.clone(), self.query_imports.clone()));

        let mut lower = TypeLowering::new_with_ctx(
            decl_source,
            &self.index,
            &self.env,
            &mut self.types,
            self.builtins,
            pkg_prefix,
            imports,
        );

        let mut out: Vec<(String, TypeId)> = Vec::with_capacity(info.fields.len());
        for field in &info.fields {
            let type_bindings = sym
                .type_param_names
                .iter()
                .cloned()
                .zip(nominal.args.iter().copied())
                .collect::<Vec<_>>();
            let eff_bindings = match (&sym.eff_param, nominal.eff.clone()) {
                (Some(eff_param), Some(row)) => vec![(eff_param.name.clone(), row)],
                _ => Vec::new(),
            };
            let field_ty = lower.lower_type_ref_in_decl_file_with_scopes(
                &info.decl_file,
                type_bindings,
                eff_bindings,
                &field.ty,
            )?;
            out.push((field.name.clone(), field_ty));
        }
        Ok(out)
    }

    fn type_layout(&mut self, id: TypeId) -> Result<TypeLayout, RttiError> {
        if let Some(layout) = self.layout_cache.get(&id).copied() {
            return Ok(layout);
        }

        // 防御性：避免递归 value type 布局计算导致的无限递归（例如自引用 struct）。
        if !self.in_progress.insert(id) {
            let layout = self.pointer_layout().without_niche();
            self.layout_cache.insert(id, layout);
            return Ok(layout);
        }

        let kind = self.types.kind(id).clone();
        let layout = match kind {
            TypeKind::Ref(_) => self.pointer_layout(),
            TypeKind::Param(_) => self.pointer_layout().without_niche(),
            TypeKind::StarProjection(star) => self.type_layout(star.read_ty)?,
            TypeKind::Value(vk) => match vk {
                ValueTypeKind::Unit | ValueTypeKind::Nothing => TypeLayout::new(0, 1),
                ValueTypeKind::Bool => self.bool_layout(),
                ValueTypeKind::Char => TypeLayout::new(4, 4),
                ValueTypeKind::Float64 => TypeLayout::new(8, 8),
                ValueTypeKind::Float32 => TypeLayout::new(4, 4),
                ValueTypeKind::Int | ValueTypeKind::UInt => self.word_layout(),
                ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => {
                    let size = (bits as u64).div_ceil(8);
                    let align = size.clamp(1, self.target.pointer_align.max(1));
                    TypeLayout::new(size, align)
                }
                ValueTypeKind::Tuple(elements) => self.aggregate_fields_layout(&elements)?,
                ValueTypeKind::Option(inner) => self.option_layout(inner)?,
                ValueTypeKind::Nominal(nominal) => self.nominal_layout(&nominal)?,
            },
        };

        self.in_progress.remove(&id);
        self.layout_cache.insert(id, layout);
        Ok(layout)
    }

    fn nominal_layout(&mut self, nominal: &NominalType) -> Result<TypeLayout, RttiError> {
        let Some(sym) = self.env.type_symbol(&nominal.fqn) else {
            return Ok(self.pointer_layout().without_niche());
        };

        match sym.kind {
            crate::typecheck::TypeSymbolKind::TypeAlias => {
                Ok(self.pointer_layout().without_niche())
            }
            crate::typecheck::TypeSymbolKind::Nominal(kind) => match kind {
                ast::TypeKind::Class | ast::TypeKind::Interface | ast::TypeKind::Effect => {
                    Ok(self.pointer_layout())
                }
                ast::TypeKind::Enum => {
                    // v0：暂不把 rich enum 的精确布局语义绑死到 RTTI；后续可复用 typecheck/layout 的规则。
                    Ok(self.word_layout())
                }
                ast::TypeKind::Struct => self.struct_layout(nominal),
            },
        }
    }

    fn struct_layout(&mut self, nominal: &NominalType) -> Result<TypeLayout, RttiError> {
        let fields_snapshot: Vec<TypeId> = self
            .lower_struct_field_types(nominal)?
            .into_iter()
            .map(|(_, ty)| ty)
            .collect();
        let mut size = 0u64;
        let mut align = 1u64;
        for field_ty in fields_snapshot {
            let layout = self.type_layout(field_ty)?;
            size = align_to(size, layout.align);
            size = size.saturating_add(layout.size);
            align = align.max(layout.align);
        }
        size = align_to(size, align);
        Ok(TypeLayout::new(size, align))
    }

    fn aggregate_fields_layout(&mut self, fields: &[TypeId]) -> Result<TypeLayout, RttiError> {
        let mut size = 0u64;
        let mut align = 1u64;
        for &field in fields {
            let layout = self.type_layout(field)?;
            size = align_to(size, layout.align);
            size = size.saturating_add(layout.size);
            align = align.max(layout.align);
        }
        size = align_to(size, align);
        Ok(TypeLayout::new(size, align))
    }

    fn option_layout(&mut self, inner: TypeId) -> Result<TypeLayout, RttiError> {
        let inner_layout = self.type_layout(inner)?;

        // niche path：inner 提供可用 niche 值时，Option 与 inner 共享 layout（对 size/align 很重要）。
        if let Some(mut domain) = inner_layout.niche
            && domain.take_one().is_some()
        {
            return Ok(TypeLayout::new(inner_layout.size, inner_layout.align).with_niche(domain));
        }

        // tagged union fallback：`tag(u8) + payload`（v0：仅保证 size/align）。
        let tag_layout = TypeLayout::new(1, 1);
        let payload = inner_layout.without_niche();

        let payload_offset = align_to(tag_layout.size, payload.align);
        let align = payload.align.max(tag_layout.align);
        let size = align_to(payload_offset + payload.size, align);
        Ok(TypeLayout::new(size, align))
    }

    fn pointer_layout(&self) -> TypeLayout {
        TypeLayout::new(self.target.pointer_size, self.target.pointer_align).with_niche(
            NicheDomain {
                storage: NicheStorage::Pointer,
                next: 0,
                end: self.target.pointer_align.max(1),
            },
        )
    }

    fn word_layout(&self) -> TypeLayout {
        TypeLayout::new(self.target.pointer_size, self.target.pointer_align)
    }

    fn bool_layout(&self) -> TypeLayout {
        TypeLayout::new(1, 1).with_niche(NicheDomain {
            storage: NicheStorage::U8,
            next: 2,
            end: 256,
        })
    }
}

trait WithoutNiche {
    fn without_niche(self) -> Self;
}

impl WithoutNiche for TypeLayout {
    fn without_niche(mut self) -> Self {
        self.niche = None;
        self
    }
}

fn collect_struct_infos(
    pairs: &[(&SourceFile, &ast::File)],
    main_source_path: &std::path::Path,
) -> Result<(Vec<String>, HashMap<String, StructInfo>), RttiError> {
    let mut dumpable: Vec<String> = Vec::new();
    let mut out: HashMap<String, StructInfo> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            collect_struct_infos_in_type_decl(
                source,
                ty,
                &pkg_prefix,
                main_source_path,
                &mut dumpable,
                &mut out,
            )?;
        }
    }
    Ok((dumpable, out))
}

fn collect_struct_infos_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    main_source_path: &std::path::Path,
    dumpable: &mut Vec<String>,
    out: &mut HashMap<String, StructInfo>,
) -> Result<(), RttiError> {
    let local_name = decl.name.text(source).to_string();
    let type_fqn = if prefix.is_empty() {
        local_name
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) && !out.contains_key(&type_fqn) {
        let mut fields: Vec<StructFieldInfo> = Vec::new();

        // 1) primary ctor params
        if let Some(primary_ctor) = &decl.primary_ctor {
            for p in &primary_ctor.params {
                let Some(ty_ref) = &p.ty else { continue };
                let field_name = p.name.text(source).to_string();
                fields.push(StructFieldInfo {
                    name: field_name,
                    ty: ty_ref.clone(),
                });
            }
        }

        // 2) type body properties with backing field（v0：无 delegate/getter/setter）
        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Property(p) = member else {
                    continue;
                };
                if p.delegate.is_some() || p.getter.is_some() || p.setter.is_some() {
                    continue;
                }
                let Some(ty_ref) = &p.ty else { continue };
                let field_name = p.name.text(source).to_string();
                fields.push(StructFieldInfo {
                    name: field_name,
                    ty: ty_ref.clone(),
                });
            }
        }

        if source.path() == main_source_path
            && decl.type_params.is_empty()
            && decl.eff_param.is_none()
        {
            dumpable.push(type_fqn.clone());
        }

        out.insert(
            type_fqn.clone(),
            StructInfo {
                decl_file: source.path().to_path_buf(),
                fields,
            },
        );
    }

    // 递归 nested types（可能存在 nested struct）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    collect_struct_infos_in_type_decl(
                        source,
                        nested,
                        &type_fqn,
                        main_source_path,
                        dumpable,
                        out,
                    )?;
                }
                ast::TypeMember::Object(obj) => {
                    collect_struct_infos_in_object_decl(
                        source,
                        obj,
                        &type_fqn,
                        main_source_path,
                        dumpable,
                        out,
                    )?;
                }
                ast::TypeMember::EnumVariant(_)
                | ast::TypeMember::Property(_)
                | ast::TypeMember::InitBlock(_)
                | ast::TypeMember::SecondaryCtor(_)
                | ast::TypeMember::Fun(_) => {}
            }
        }
    }

    Ok(())
}

fn collect_struct_infos_in_object_decl(
    source: &SourceFile,
    obj: &ast::ObjectDecl,
    prefix: &str,
    main_source_path: &std::path::Path,
    dumpable: &mut Vec<String>,
    out: &mut HashMap<String, StructInfo>,
) -> Result<(), RttiError> {
    let obj_name = match &obj.name {
        Some(name) => name.text(source).to_string(),
        None => match obj.kind {
            ast::ObjectKind::Companion => "Companion".to_string(),
            ast::ObjectKind::Object => return Ok(()),
        },
    };

    let obj_fqn = if prefix.is_empty() {
        obj_name
    } else {
        format!("{prefix}.{obj_name}")
    };

    let Some(body) = &obj.body else {
        return Ok(());
    };

    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_struct_infos_in_type_decl(
                    source,
                    nested,
                    &obj_fqn,
                    main_source_path,
                    dumpable,
                    out,
                )?;
            }
            ast::TypeMember::Object(nested) => {
                collect_struct_infos_in_object_decl(
                    source,
                    nested,
                    &obj_fqn,
                    main_source_path,
                    dumpable,
                    out,
                )?;
            }
            ast::TypeMember::Property(_)
            | ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }

    Ok(())
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| id.text(source))
        .collect::<Vec<_>>()
        .join(".")
}

fn parse_type_query(name: &str) -> Result<(SourceFile, ast::TypeRef), ParseError> {
    let source =
        SourceFile::new_virtual("<rtti-query>", format!("typealias __RttiQuery = {name}\n"));
    let file = parse_file(&source)?;
    let ast::Item::TypeAlias(alias) = file
        .items
        .first()
        .expect("synthetic RTTI query file should contain one typealias item")
    else {
        unreachable!("synthetic RTTI query file should parse as typealias")
    };
    Ok((source, alias.ty.clone()))
}

fn align_to(value: u64, align: u64) -> u64 {
    if align <= 1 {
        return value;
    }
    let mask = align - 1;
    (value + mask) & !mask
}

fn parse_int_width_suffix(name: &str) -> Option<(bool, u16)> {
    // `Int32` / `UInt64`
    let (signed, rest) = if let Some(rest) = name.strip_prefix("Int") {
        (true, rest)
    } else if let Some(rest) = name.strip_prefix("UInt") {
        (false, rest)
    } else {
        return None;
    };

    let bits: u16 = rest.parse().ok()?;
    Some((signed, bits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtti::type_desc;

    #[test]
    fn rtti_struct_field_offsets_basic() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            r#"
package a

struct Point(val x: Int, val y: Int)
"#,
        );

        let rtti = dump_type_rtti(&sess, &src, "Point").unwrap();
        assert_eq!(rtti.kind, RttiKind::Struct);
        assert_eq!(rtti.name, "a.Point");

        let ptr = std::mem::size_of::<usize>() as u64;
        assert_eq!(rtti.align, ptr);
        assert_eq!(rtti.size, ptr * 2);

        let fields = rtti.fields.expect("struct has fields");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "x");
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].name, "y");
        assert_eq!(fields[1].offset, ptr);
    }

    #[test]
    fn rtti_parameterized_struct_query_instantiates_field_types() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            r#"
package rtti

struct Pair<T>(val first: T, val second: T)
"#,
        );

        let rtti = dump_type_rtti(&sess, &src, "Pair<Int>").unwrap();
        assert_eq!(rtti.kind, RttiKind::Struct);
        assert_eq!(rtti.name, "rtti.Pair<Int>");
        assert_eq!(rtti.type_id, stable_rtti_type_id("rtti.Pair<Int>"));

        let ptr = std::mem::size_of::<usize>() as u64;
        assert_eq!(rtti.align, ptr);
        assert_eq!(rtti.size, ptr * 2);

        let fields = rtti
            .fields
            .expect("parameterized struct should expose fields");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "first");
        assert_eq!(fields[0].ty, "Int");
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].name, "second");
        assert_eq!(fields[1].ty, "Int");
        assert_eq!(fields[1].offset, ptr);
    }

    #[test]
    fn rtti_parameterized_nominal_query_matches_type_desc_metadata() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual(
            "<mem>",
            r#"
package rtti

import scoop.core.*

interface Readable<out T> {
  fun get(): T
}

class StringReadable : Readable<String> {
  fun get(): String { return "hello" }
}

interface Disposable<eff E = Pure> {
  fun dispose(): Unit / E
}

class RaiseManaged : Disposable<eff Raise<RuntimeError>> {
  fun dispose(): Unit { return }
}
"#,
        );

        let readable = dump_type_rtti(&sess, &src, "Readable<String>").unwrap();
        assert_eq!(readable.kind, RttiKind::Ref);
        assert_eq!(readable.name, "rtti.Readable<String>");
        assert_eq!(
            readable.type_id,
            stable_rtti_type_id("rtti.Readable<String>")
        );

        let disposable_raise =
            dump_type_rtti(&sess, &src, "Disposable<eff Raise<RuntimeError>>").unwrap();
        assert_eq!(disposable_raise.kind, RttiKind::Ref);
        assert!(disposable_raise.name.contains("Disposable<eff"));
        assert!(disposable_raise.name.contains("Raise"));
        assert!(disposable_raise.name.contains("RuntimeError"));
        assert_eq!(
            disposable_raise.type_id,
            stable_rtti_type_id(&disposable_raise.name)
        );

        let dump = type_desc::dump_file_type_desc(&sess, &src).unwrap();
        let string_readable = dump
            .types
            .iter()
            .find(|ty| ty.name == "rtti.StringReadable")
            .expect("type_desc should contain StringReadable");
        let readable_entry = string_readable
            .itable_entries
            .iter()
            .find(|entry| entry.interface_name == "rtti.Readable")
            .expect("StringReadable should expose Readable metadata");
        assert_eq!(readable.name, readable_entry.interface_type_name);
        assert_eq!(readable.type_id, readable_entry.interface_type_id);

        let raise_managed = dump
            .types
            .iter()
            .find(|ty| ty.name == "rtti.RaiseManaged")
            .expect("type_desc should contain RaiseManaged");
        let disposable_entry = raise_managed
            .itable_entries
            .iter()
            .find(|entry| entry.interface_name == "rtti.Disposable")
            .expect("RaiseManaged should expose Disposable metadata");
        assert_eq!(disposable_raise.name, disposable_entry.interface_type_name);
        assert_eq!(disposable_raise.type_id, disposable_entry.interface_type_id);
    }
}
