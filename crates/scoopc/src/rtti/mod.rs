//! RTTI（Runtime Type Info）v0：为类型生成运行期描述符（TODO T1206）。
//!
//! 目标：
//! - 先给“可静态确定布局”的类型生成最小 RTTI：type id + size/align +（struct）字段布局；
//! - 该信息主要用于调试/后续 GC 与后端集成；不承诺提供完整运行期反射能力（spec §6.6）。
//!
//! 约束（v0）：
//! - 仅覆盖 **非泛型** `struct` 的字段布局（primary ctor params + 有 backing field 的属性）；
//! - 其它类型只提供 size/align（或按指针大小占位）；
//! - 目标平台布局暂用 host pointer size/align（与 typecheck/layout 一致，T0803 再替换为 target machine）。

pub mod type_desc;

use std::collections::{BTreeMap, HashMap, HashSet};

use miette::Diagnostic;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::ast;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{ImportTable, Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
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

    #[error("暂不支持生成 RTTI：泛型/eff 参数化类型：{name}")]
    #[diagnostic(code(scoop::rtti::unsupported_generic_type))]
    UnsupportedGenericType { name: String },

    #[error("struct RTTI 生成缺少声明：{fqn}")]
    #[diagnostic(code(scoop::rtti::missing_struct_decl))]
    MissingStructDecl { fqn: String },
}

/// 计算输入文件内“可生成 RTTI 的类型表”（当前仅 non-generic struct），并返回稳定输出结构。
pub fn dump_file_rtti(session: &Session, source: &SourceFile) -> Result<RttiDump, RttiError> {
    let mut cx = RttiContext::build(session, source)?;
    let mut out: Vec<TypeRtti> = Vec::new();

    let mut names: Vec<String> = cx.structs.keys().cloned().collect();
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
/// - 支持 input file 内 non-generic struct 的 FQN 或 simple name（simple name 必须唯一）。
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
    fields: Vec<StructFieldInfo>,
}

struct StructFieldInfo {
    name: String,
    ty: TypeId,
}

struct RttiContext {
    env: TypeEnv,
    types: TypeStore,
    builtins: BuiltinTypes,
    target: TargetLayout,
    structs: HashMap<String, StructInfo>,
    layout_cache: HashMap<TypeId, TypeLayout>,
    in_progress: HashSet<TypeId>,
}

impl RttiContext {
    fn build(session: &Session, source: &SourceFile) -> Result<Self, RttiError> {
        // 1) parse + 最小声明检查（不依赖 index/resolver）。
        let mut file = parse_file(source)?;
        crate::comptime::trim_package_level_comptime_ifs(source, &mut file)?;
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
        let structs = collect_struct_infos(
            source,
            &file,
            &index,
            &resolved_headers.imports,
            &env,
            &mut types,
            builtins,
        )?;

        Ok(Self {
            env,
            types,
            builtins,
            target: TargetLayout::host(),
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

        // 2) 允许 FQN 或 simple name 查找 input file 内的 struct。
        if self.structs.contains_key(name) {
            return Ok(self.struct_type_id(name));
        }

        // simple name：取最后一段做匹配（保证输出稳定，用 BTreeMap 排序）。
        let mut by_simple: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for fqn in self.structs.keys() {
            let simple = fqn.rsplit('.').next().unwrap_or(fqn).to_string();
            by_simple.entry(simple).or_default().push(fqn.clone());
        }

        if let Some(cands) = by_simple.get(name) {
            if cands.len() == 1 {
                let fqn = &cands[0];
                return Ok(self.struct_type_id(fqn));
            }
            return Err(RttiError::AmbiguousType {
                name: name.to_string(),
                candidates: cands.join(", "),
            });
        }

        Err(RttiError::UnknownType {
            name: name.to_string(),
        })
    }

    fn builtin_type_id(&mut self, name: &str) -> Option<TypeId> {
        match name {
            "Any" => Some(self.builtins.any),
            "String" => Some(self.builtins.string),
            "Unit" => Some(self.builtins.unit),
            "Bool" => Some(self.builtins.bool_),
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
        let type_id = stable_hash64(&name);
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
            TypeKind::Value(vk) => match vk {
                ValueTypeKind::Unit
                | ValueTypeKind::Nothing
                | ValueTypeKind::Bool
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
                                let fields = Some(self.struct_fields_rtti(&nominal.fqn)?);
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

    fn struct_fields_rtti(&mut self, fqn: &str) -> Result<Vec<FieldRtti>, RttiError> {
        let Some(info) = self.structs.get(fqn) else {
            return Err(RttiError::MissingStructDecl {
                fqn: fqn.to_string(),
            });
        };
        let fields_snapshot: Vec<(String, TypeId)> =
            info.fields.iter().map(|f| (f.name.clone(), f.ty)).collect();

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
            TypeKind::Value(vk) => match vk {
                ValueTypeKind::Unit | ValueTypeKind::Nothing => TypeLayout::new(0, 1),
                ValueTypeKind::Bool => self.bool_layout(),
                ValueTypeKind::Int | ValueTypeKind::UInt => self.word_layout(),
                ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => {
                    let size = ((bits as u64) + 7) / 8;
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
        if !nominal.args.is_empty() || nominal.eff.is_some() {
            return Err(RttiError::UnsupportedGenericType {
                name: nominal.fqn.clone(),
            });
        }

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
                ast::TypeKind::Struct => self.struct_layout(&nominal.fqn),
            },
        }
    }

    fn struct_layout(&mut self, fqn: &str) -> Result<TypeLayout, RttiError> {
        let Some(info) = self.structs.get(fqn) else {
            return Err(RttiError::MissingStructDecl {
                fqn: fqn.to_string(),
            });
        };
        let fields_snapshot: Vec<TypeId> = info.fields.iter().map(|f| f.ty).collect();
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
        if let Some(mut domain) = inner_layout.niche {
            if domain.take_one().is_some() {
                return Ok(
                    TypeLayout::new(inner_layout.size, inner_layout.align).with_niche(domain)
                );
            }
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
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    imports: &ImportTable,
    env: &TypeEnv,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Result<HashMap<String, StructInfo>, RttiError> {
    let pkg_prefix = package_prefix(source, file.package.as_ref());

    let mut lower = TypeLowering::new(source, file, index, imports, env, types, builtins);
    let mut out: HashMap<String, StructInfo> = HashMap::new();
    for item in &file.items {
        let ast::Item::Type(ty) = item else {
            continue;
        };
        collect_struct_infos_in_type_decl(source, ty, &pkg_prefix, &mut lower, &mut out)?;
    }
    Ok(out)
}

fn collect_struct_infos_in_type_decl(
    source: &SourceFile,
    decl: &ast::TypeDecl,
    prefix: &str,
    lower: &mut TypeLowering<'_>,
    out: &mut HashMap<String, StructInfo>,
) -> Result<(), RttiError> {
    let local_name = decl.name.text(source).to_string();
    let type_fqn = if prefix.is_empty() {
        local_name
    } else {
        format!("{prefix}.{local_name}")
    };

    if matches!(decl.kind, ast::TypeKind::Struct) {
        // v0：跳过泛型/eff 参数化 struct（布局需要单态化）。
        if !decl.type_params.is_empty() || decl.eff_param.is_some() {
            // 仅当后续按名字查询到该类型时才报错；这里先跳过。
        } else if !out.contains_key(&type_fqn) {
            let mut fields: Vec<StructFieldInfo> = Vec::new();

            // 1) primary ctor params
            if let Some(primary_ctor) = &decl.primary_ctor {
                for p in &primary_ctor.params {
                    let Some(ty_ref) = &p.ty else { continue };
                    let field_name = p.name.text(source).to_string();
                    let field_ty = lower.lower_type_ref(ty_ref)?;
                    fields.push(StructFieldInfo {
                        name: field_name,
                        ty: field_ty,
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
                    let field_ty = lower.lower_type_ref(ty_ref)?;
                    fields.push(StructFieldInfo {
                        name: field_name,
                        ty: field_ty,
                    });
                }
            }

            out.insert(type_fqn.clone(), StructInfo { fields });
        }
    }

    // 递归 nested types（可能存在 nested struct）。
    if let Some(body) = &decl.body {
        for member in &body.members {
            match member {
                ast::TypeMember::Type(nested) => {
                    collect_struct_infos_in_type_decl(source, nested, &type_fqn, lower, out)?;
                }
                ast::TypeMember::Object(obj) => {
                    collect_struct_infos_in_object_decl(source, obj, &type_fqn, lower, out)?;
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
    lower: &mut TypeLowering<'_>,
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
                collect_struct_infos_in_type_decl(source, nested, &obj_fqn, lower, out)?;
            }
            ast::TypeMember::Object(nested) => {
                collect_struct_infos_in_object_decl(source, nested, &obj_fqn, lower, out)?;
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

fn stable_hash64(text: &str) -> u64 {
    let digest = Sha256::digest(text.as_bytes());
    let bytes: [u8; 8] = digest[0..8].try_into().expect("sha256 output is 32 bytes");
    u64::from_le_bytes(bytes)
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
}
