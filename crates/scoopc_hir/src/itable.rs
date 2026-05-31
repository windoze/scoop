//! interface dispatch table（itable）布局（T1507c3 / T1508c）。
//!
//! 目标：
//! - 为 interface 分配稳定 `interface_id`（共享 RTTI helper）并生成 method slot 表（声明顺序）。
//! - 为每个 class 生成 itable entries：`interface_id -> slot -> impl_member_fqn`。
//!
//! 说明（v0 简化）：
//! - slot key 仍以“最小形状信息”为主：`name + params_len + has_receiver`；
//! - 若 interface method 有 body（默认方法），且 class 未实现，则 itable slot 指向 interface 自身的默认实现；
//! - 若 interface method 无 body（抽象方法）且无法解析实现，则返回错误（应在 typecheck 阶段先被门禁）。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::resolve::{ImportTable, Index};
use crate::source::SourceFile;
use crate::stable_id::{
    NoTypeParamResolver, canonical_nominal_type_key, stable_rtti_interface_id, stable_rtti_type_id,
    stable_rtti_type_id_for_type,
};
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};
use crate::typecheck::is_type_assignable;
use crate::typecheck::{TypeEnv, TypeEnvError, TypeLowerError, TypeLowering, TypeSymbolKind};
use crate::vtable::ClassVtableIndex;

/// interface method 的“最小形状 key”（用于 slot/实现匹配）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MethodShapeKey {
    name: String,
    params_len: u32,
    has_receiver: bool,
}

/// interface method slot 信息（v0：按声明顺序分配 slot）。
#[derive(Debug, Clone)]
pub struct InterfaceMethodSlot {
    pub slot: u32,
    pub name: String,
    pub member_fqn: String,
    pub decl_span: crate::span::Span,
    pub params_len: u32,
    pub has_receiver: bool,
    /// interface 默认方法（有 body）为 true；抽象方法为 false。
    pub has_body: bool,
}

/// interface 元数据（codegen-friendly）。
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub fqn: String,
    pub interface_id: u64,
    pub super_interfaces: Vec<String>,
    pub method_slots: Vec<InterfaceMethodSlot>,
}

/// interface FQN -> interface info。
pub type InterfaceIndex = HashMap<String, InterfaceInfo>;

/// class itable entry：一个 interface 的 slot → impl 映射。
#[derive(Debug, Clone)]
pub struct ClassItableEntry {
    pub interface_fqn: String,
    pub interface_id: u64,
    /// 该 entry 对应的“具体 interface 实例”可读名字，例如 `foo.Readable<String>`。
    pub interface_type_name: String,
    pub interface_type_id: u64,
    /// 该具体 interface 实例在运行期可匹配的 target 集可读名字（按前端 assignable 规则预计算）。
    pub runtime_match_type_names: Vec<String>,
    pub runtime_match_type_ids: Vec<u64>,
    pub method_impl_fqns: Vec<String>,
    /// slot 对应 receiver 的 authoritative ABI 形状：
    /// - `0` 表示 receiver 继续按 ref/object ptr 传递；
    /// - 非 `0` 表示 receiver 来自某个 concrete value nominal，需要按该 type id 对应的
    ///   method signature 从 value-box payload 里重建并 marshal。
    pub method_receiver_type_ids: Vec<u64>,
}

/// class FQN -> itable entries（按 interface_id 稳定排序）。
pub type ClassItableIndex = HashMap<String, Vec<ClassItableEntry>>;

#[derive(Debug, Error, Diagnostic)]
pub enum ItableLayoutError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeEnv(#[from] TypeEnvError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TypeLower(#[from] TypeLowerError),

    #[error("interface 继承链存在循环：{fqn}")]
    #[diagnostic(code(scoop::itable::inheritance_cycle))]
    InheritanceCycle { fqn: String },

    #[error("interface method slot 形状不唯一：{interface_fqn}.{member}")]
    #[diagnostic(code(scoop::itable::ambiguous_interface_method_slot))]
    AmbiguousInterfaceMethodSlot {
        interface_fqn: String,
        member: String,
    },

    #[error("无法为 itable metadata 找到文件类型上下文：{path}")]
    #[diagnostic(code(scoop::itable::missing_file_type_context))]
    MissingFileTypeContext { path: String },

    #[error("无法为 itable metadata 找到源文件内容：{path}")]
    #[diagnostic(code(scoop::itable::missing_source_file))]
    MissingSourceFile { path: String },

    #[error("无法为 itable runtime metadata 计算 stable type id：{message}")]
    #[diagnostic(code(scoop::itable::stable_type_id))]
    StableTypeId { message: String },
}

#[derive(Debug, Clone)]
struct ClassDeclInfo {
    fqn: String,
    is_value_type: bool,
    super_class_fqn: Option<String>,
    direct_interfaces: Vec<String>,
    methods: Vec<ClassMethodInfo>,
}

#[derive(Debug, Clone)]
struct ClassMethodInfo {
    name: String,
    params_len: u32,
    has_receiver: bool,
}

#[derive(Debug, Clone)]
struct ConcreteClassTarget {
    class_key: String,
    base_fqn: String,
    ty: TypeId,
}

pub const ITABLE_RECEIVER_REF_TYPE_ID: u64 = 0;

pub fn collect_interfaces_and_class_itables(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
    class_vtables: &ClassVtableIndex,
) -> Result<(InterfaceIndex, ClassItableIndex), ItableLayoutError> {
    let mut interfaces: InterfaceIndex = HashMap::new();
    let mut classes: HashMap<String, ClassDeclInfo> = HashMap::new();

    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_interfaces_in_type_decl(
                        source,
                        file,
                        &pkg_prefix,
                        ty,
                        index,
                        &mut interfaces,
                    );
                    collect_classes_in_type_decl(
                        source,
                        file,
                        &pkg_prefix,
                        ty,
                        index,
                        &mut classes,
                    );
                }
                ast::Item::Object(obj) => {
                    collect_interfaces_in_object_decl(
                        source,
                        file,
                        &pkg_prefix,
                        obj,
                        index,
                        &mut interfaces,
                    );
                    collect_classes_in_object_decl(
                        source,
                        file,
                        &pkg_prefix,
                        obj,
                        index,
                        &mut classes,
                    );
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_) => {}
            }
        }
    }

    let class_itables = build_base_class_itables(&classes, &interfaces, class_vtables)?;
    Ok((interfaces, class_itables))
}

/// 运行期精确 itable metadata：
/// - 保留 base interface 的 dispatch slot 布局；
/// - 额外为每个具体 interface 实例预计算可匹配的 target 集，供 `is/as/as?` 使用。
pub fn collect_runtime_interfaces_and_class_itables_with_env(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
    class_vtables: &ClassVtableIndex,
    env: &TypeEnv,
    typecheck_types: &TypeStore,
) -> Result<(InterfaceIndex, ClassItableIndex), ItableLayoutError> {
    let (interfaces, mut class_itables) =
        collect_interfaces_and_class_itables(compilation_unit, index, class_vtables)?;

    let mut classes: HashMap<String, ClassDeclInfo> = HashMap::new();
    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => {
                    collect_classes_in_type_decl(
                        source,
                        file,
                        &pkg_prefix,
                        ty,
                        index,
                        &mut classes,
                    );
                }
                ast::Item::Object(obj) => {
                    collect_classes_in_object_decl(
                        source,
                        file,
                        &pkg_prefix,
                        obj,
                        index,
                        &mut classes,
                    );
                }
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_) => {}
            }
        }
    }

    let mut runtime_types = typecheck_types.clone();
    let builtins = runtime_types.intern_builtins();
    let concrete_classes = collect_concrete_class_targets(&runtime_types, env);
    let concrete_interface_targets = collect_concrete_interface_targets(&runtime_types, env);
    let generic_template_symbol_suffixes =
        crate::hir::collect_generic_template_symbol_suffixes(index, compilation_unit);
    if concrete_classes.is_empty() {
        return Ok((interfaces, class_itables));
    }

    let (ctx_source, pkg_prefix, imports) = runtime_type_lowering_context(compilation_unit, env)?;
    let mut lower = TypeLowering::new_with_ctx(
        ctx_source,
        index,
        env,
        &mut runtime_types,
        builtins,
        pkg_prefix,
        imports,
    );

    for concrete_class in concrete_classes {
        let entries = build_precise_class_itable_entries(
            &concrete_class,
            &classes,
            &interfaces,
            class_vtables,
            index,
            &generic_template_symbol_suffixes,
            &concrete_interface_targets,
            &mut lower,
            builtins,
        )?;
        if !entries.is_empty() {
            class_itables.insert(concrete_class.class_key, entries);
        }
    }

    Ok((interfaces, class_itables))
}

/// 在缺少外部 `TypeEnv` 注入时，从当前 compilation unit 重建一个最小 env。
///
/// 说明：
/// - 该入口适合测试、`dump-rtti` 等仅依赖“sysroot + 当前源文件 / 当前 cone 文件集”的场景；
/// - 若调用方已经拥有带依赖 cone 注入的完整 `TypeEnv`，应优先使用
///   `collect_runtime_interfaces_and_class_itables_with_env`，避免丢失外部 API 头信息。
pub fn collect_runtime_interfaces_and_class_itables(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
    class_vtables: &ClassVtableIndex,
    typecheck_types: &TypeStore,
) -> Result<(InterfaceIndex, ClassItableIndex), ItableLayoutError> {
    let mut env = TypeEnv::default();
    for (source, file) in compilation_unit {
        env.extend_from_file(source, file, index)?;
    }
    collect_runtime_interfaces_and_class_itables_with_env(
        compilation_unit,
        index,
        class_vtables,
        &env,
        typecheck_types,
    )
}

fn build_base_class_itables(
    classes: &HashMap<String, ClassDeclInfo>,
    interfaces: &InterfaceIndex,
    class_vtables: &ClassVtableIndex,
) -> Result<ClassItableIndex, ItableLayoutError> {
    let mut memo_iface_sets: HashMap<String, HashSet<String>> = HashMap::new();
    let mut visiting_classes: HashSet<String> = HashSet::new();

    let mut class_fqns: Vec<&str> = classes.keys().map(|k| k.as_str()).collect();
    class_fqns.sort();

    let mut out: ClassItableIndex = HashMap::new();
    for class_fqn in class_fqns {
        let ifaces = compute_class_interface_closure(
            class_fqn,
            classes,
            interfaces,
            &mut visiting_classes,
            &mut memo_iface_sets,
        )?;

        // vtable impl map：shape -> 真实实现成员（已考虑 override 覆盖）。
        let mut vtable_impls: HashMap<MethodShapeKey, String> = HashMap::new();
        if let Some(vslots) = class_vtables.get(class_fqn) {
            for s in vslots {
                vtable_impls.insert(
                    MethodShapeKey {
                        name: s.name.clone(),
                        params_len: s.params_len,
                        has_receiver: s.has_receiver,
                    },
                    s.impl_member_fqn.clone(),
                );
            }
        }

        let mut entries: Vec<ClassItableEntry> = Vec::new();
        for iface_fqn in ifaces {
            let (interface_id, method_slots) = match interfaces.get(&iface_fqn) {
                Some(info) => (info.interface_id, info.method_slots.clone()),
                None => (stable_rtti_interface_id(&iface_fqn), Vec::new()),
            };

            // slot -> impl_member_fqn：保持与 slot index 对齐。
            let mut impls: Vec<String> = vec![String::new(); method_slots.len()];
            let mut receiver_type_ids: Vec<u64> =
                vec![ITABLE_RECEIVER_REF_TYPE_ID; method_slots.len()];

            // v0：slot key（name+params_len+has_receiver）在单个 interface 内必须是唯一的，
            // 否则 lowering/codegen 无法在调用点稳定选中正确 slot。
            let mut seen_shapes: HashSet<MethodShapeKey> = HashSet::new();

            for slot in &method_slots {
                let key = MethodShapeKey {
                    name: slot.name.clone(),
                    params_len: slot.params_len,
                    has_receiver: slot.has_receiver,
                };

                if !seen_shapes.insert(key.clone()) {
                    return Err(ItableLayoutError::AmbiguousInterfaceMethodSlot {
                        interface_fqn: iface_fqn.clone(),
                        member: slot.name.clone(),
                    });
                }

                let impl_member_fqn = if let Some(found) = vtable_impls.get(&key).cloned() {
                    found
                } else if let Some((in_fqn, member_fqn)) =
                    resolve_method_in_class_hierarchy(class_fqn, &key, classes, &mut HashSet::new())
                {
                    let _ = in_fqn;
                    member_fqn
                } else if slot.has_body {
                    // 默认方法：允许回退到 interface 自身的实现。
                    format!("{iface_fqn}.{}", slot.name)
                } else {
                    // 抽象方法：原则上应由 typecheck（`missing_interface_member`）提前门禁。
                    // 为避免“typecheck 未覆盖的 inherited interface”导致后端直接失败，这里用空占位，
                    // 由 codegen 将该 slot 填充为 NULL（若运行期触发调用，则属于未定义行为的防线兜底）。
                    String::new()
                };

                let idx = slot.slot as usize;
                if idx >= impls.len() {
                    // slot 不是连续 0..N 的情况在 v0 不支持（slot 分配即声明顺序）。
                    continue;
                }
                impls[idx] = impl_member_fqn;
                receiver_type_ids[idx] = if classes
                    .get(class_fqn)
                    .is_some_and(|info| info.is_value_type)
                    && !impls[idx].is_empty()
                    && !impls[idx].starts_with(&format!("{iface_fqn}."))
                {
                    let type_key = canonical_nominal_type_key(class_fqn);
                    stable_rtti_type_id(type_key.as_str())
                } else {
                    ITABLE_RECEIVER_REF_TYPE_ID
                };
            }

            let interface_type_name = iface_fqn.clone();
            let interface_type_key = canonical_nominal_type_key(&interface_type_name);
            let interface_type_id = stable_rtti_type_id(interface_type_key.as_str());
            entries.push(ClassItableEntry {
                interface_fqn: iface_fqn,
                interface_id,
                interface_type_name: interface_type_name.clone(),
                interface_type_id,
                runtime_match_type_names: vec![interface_type_name],
                runtime_match_type_ids: vec![interface_type_id],
                method_impl_fqns: impls,
                method_receiver_type_ids: receiver_type_ids,
            });
        }

        entries.sort_by(|a, b| {
            a.interface_id
                .cmp(&b.interface_id)
                .then_with(|| a.interface_type_id.cmp(&b.interface_type_id))
        });
        out.insert(class_fqn.to_string(), entries);
    }

    Ok(out)
}

fn runtime_type_lowering_context<'a>(
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    env: &TypeEnv,
) -> Result<(&'a SourceFile, String, ImportTable), ItableLayoutError> {
    let Some((source, _file)) = compilation_unit.first().copied() else {
        return Err(ItableLayoutError::MissingSourceFile {
            path: "<empty compilation unit>".to_string(),
        });
    };
    let file_ctx = env
        .file_type_context(source.path())
        .cloned()
        .ok_or_else(|| ItableLayoutError::MissingFileTypeContext {
            path: source.path().display().to_string(),
        })?;
    Ok((source, file_ctx.pkg_prefix, file_ctx.imports))
}

fn collect_concrete_class_targets(types: &TypeStore, env: &TypeEnv) -> Vec<ConcreteClassTarget> {
    let mut out: HashMap<String, ConcreteClassTarget> = HashMap::new();

    for id in types.iter_ids() {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(id) else {
            continue;
        };
        let Some(sym) = env.type_symbol(&nominal.fqn) else {
            continue;
        };
        if !matches!(sym.kind, TypeSymbolKind::Nominal(ast::TypeKind::Class)) {
            continue;
        }
        // Runtime itable metadata is only meaningful for ground instances;
        // generic templates such as `Box<T>` cannot have stable RTTI ids.
        if type_contains_param(types, id)
            || (!sym.type_param_names.is_empty()
                && nominal.args.len() != sym.type_param_names.len())
        {
            continue;
        }

        let class_key = if nominal.args.is_empty() {
            nominal.fqn.clone()
        } else {
            crate::hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, types)
        };

        out.entry(class_key.clone())
            .or_insert_with(|| ConcreteClassTarget {
                class_key,
                base_fqn: nominal.fqn.clone(),
                ty: id,
            });
    }

    let mut concrete = out.into_values().collect::<Vec<_>>();
    concrete.sort_by(|a, b| a.class_key.cmp(&b.class_key));
    concrete
}

fn collect_concrete_interface_targets(types: &TypeStore, env: &TypeEnv) -> Vec<TypeId> {
    let mut out: Vec<TypeId> = types
        .iter_ids()
        .filter(|id| {
            let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(*id) else {
                return false;
            };
            let Some(sym) = env.type_symbol(&nominal.fqn) else {
                return false;
            };
            matches!(sym.kind, TypeSymbolKind::Nominal(ast::TypeKind::Interface))
                && !type_contains_param(types, *id)
                && (sym.type_param_names.is_empty()
                    || nominal.args.len() == sym.type_param_names.len())
        })
        .collect();

    out.sort_by(|lhs, rhs| {
        types
            .display(*lhs)
            .to_string()
            .cmp(&types.display(*rhs).to_string())
            .then_with(|| lhs.cmp(rhs))
    });
    out.dedup();
    out
}

fn collect_concrete_interface_closure(
    class_ty: TypeId,
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<TypeId>, ItableLayoutError> {
    let mut out: Vec<TypeId> = Vec::new();
    let mut stack: Vec<TypeId> = vec![class_ty];
    let mut seen: HashSet<TypeId> = HashSet::new();

    while let Some(cur) = stack.pop() {
        if !seen.insert(cur) {
            continue;
        }

        for super_ty in lower.instantiated_direct_supertypes(cur)? {
            stack.push(super_ty);
            let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = lower.type_kind(super_ty) else {
                continue;
            };
            if matches!(
                lower.env().type_symbol(&nominal.fqn).map(|sym| sym.kind),
                Some(TypeSymbolKind::Nominal(ast::TypeKind::Interface))
            ) && !out.contains(&super_ty)
            {
                out.push(super_ty);
            }
        }
    }

    out.sort_by(|lhs, rhs| {
        lower
            .fmt_type(*lhs)
            .cmp(&lower.fmt_type(*rhs))
            .then_with(|| lhs.cmp(rhs))
    });
    Ok(out)
}

fn stable_runtime_type_id_for_lower(
    lower: &TypeLowering<'_>,
    ty: TypeId,
    context: &str,
) -> Result<u64, ItableLayoutError> {
    if type_contains_param(lower.types(), ty) {
        return Err(ItableLayoutError::StableTypeId {
            message: format!(
                "{context}: non-ground runtime type `{}` still contains type parameters",
                lower.fmt_type(ty)
            ),
        });
    }
    stable_rtti_type_id_for_type(lower.types(), ty, &NoTypeParamResolver).map_err(|err| {
        ItableLayoutError::StableTypeId {
            message: format!("{context}: {err}"),
        }
    })
}

/// Returns whether `ty` still contains a declaration-site type parameter.
fn type_contains_param(types: &TypeStore, ty: TypeId) -> bool {
    let mut stack = vec![ty];
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
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
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                if let Some(receiver) = fun.receiver {
                    stack.push(receiver);
                }
                stack.extend(fun.params.iter().copied());
                stack.push(fun.return_ty);
                stack.extend(fun.effects.terms.iter().copied());
            }
            TypeKind::Ref(RefTypeKind::Union(union)) => {
                stack.extend(union.variants.iter().copied());
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => stack.push(*inner),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                stack.extend(elements.iter().copied());
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

/// Resolved owner nominal instantiation: `(owner_fqn, type_args, owner_effect_row)`.
type MemberOwnerNominalInstantiation = (String, Vec<TypeId>, Option<EffectRow>);

fn find_member_owner_nominal_instantiation(
    receiver_ty: TypeId,
    member_fqn: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<MemberOwnerNominalInstantiation>, ItableLayoutError> {
    let Some((member_owner_fqn, _)) = member_fqn.rsplit_once('.') else {
        return Ok(None);
    };

    let mut stack = vec![receiver_ty];
    let mut visited: HashSet<TypeId> = HashSet::new();
    while let Some(cur) = stack.pop() {
        if !visited.insert(cur) {
            continue;
        }

        let (nominal_fqn, nominal_args, nominal_eff) = match lower.type_kind(cur) {
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
            | TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                (nominal.fqn, nominal.args, nominal.eff)
            }
            _ => continue,
        };
        if nominal_fqn == member_owner_fqn {
            return Ok(Some((nominal_fqn, nominal_args, nominal_eff)));
        }

        stack.extend(lower.instantiated_direct_supertypes(cur)?);
    }

    Ok(None)
}

fn materialize_member_impl_fqn_for_owner(
    impl_member_fqn: &str,
    owner_ty: TypeId,
    index: &Index,
    generic_template_symbol_suffixes: &crate::hir::GenericTemplateSymbolSuffixIndex,
    lower: &mut TypeLowering<'_>,
) -> Result<String, ItableLayoutError> {
    let Some((_owner_fqn, owner_args, owner_eff)) =
        find_member_owner_nominal_instantiation(owner_ty, impl_member_fqn, lower)?
    else {
        return Ok(impl_member_fqn.to_string());
    };
    if owner_args.is_empty() && owner_eff.is_none() {
        return Ok(impl_member_fqn.to_string());
    }

    let Some(overload) = index
        .by_fqn
        .get(impl_member_fqn)
        .and_then(|syms| syms.fun.first())
    else {
        return Ok(impl_member_fqn.to_string());
    };
    let template = scoopc_ids::TemplateKey {
        fqn: impl_member_fqn.to_string(),
        source_path: overload.symbol.decl_file.clone(),
        decl_span: overload.symbol.span,
    };
    Ok(crate::hir::stable_instance_fqn(
        lower.types(),
        &template,
        &owner_args,
        &owner_eff.into_iter().collect::<Vec<_>>(),
        generic_template_symbol_suffixes
            .get(&template)
            .map(String::as_str)
            .unwrap_or(""),
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_precise_class_itable_entries(
    concrete_class: &ConcreteClassTarget,
    classes: &HashMap<String, ClassDeclInfo>,
    interfaces: &InterfaceIndex,
    class_vtables: &ClassVtableIndex,
    index: &Index,
    generic_template_symbol_suffixes: &crate::hir::GenericTemplateSymbolSuffixIndex,
    concrete_interface_targets: &[TypeId],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<Vec<ClassItableEntry>, ItableLayoutError> {
    let mut vtable_impls: HashMap<MethodShapeKey, String> = HashMap::new();
    if let Some(vslots) = class_vtables
        .get(&concrete_class.class_key)
        .or_else(|| class_vtables.get(&concrete_class.base_fqn))
    {
        for s in vslots {
            vtable_impls.insert(
                MethodShapeKey {
                    name: s.name.clone(),
                    params_len: s.params_len,
                    has_receiver: s.has_receiver,
                },
                s.impl_member_fqn.clone(),
            );
        }
    }

    let concrete_ifaces = collect_concrete_interface_closure(concrete_class.ty, lower)?;
    let mut entries: Vec<ClassItableEntry> = Vec::with_capacity(concrete_ifaces.len());

    for iface_ty in concrete_ifaces {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = lower.type_kind(iface_ty) else {
            continue;
        };
        let (interface_id, method_slots) = match interfaces.get(&nominal.fqn) {
            Some(info) => (info.interface_id, info.method_slots.clone()),
            None => (stable_rtti_interface_id(&nominal.fqn), Vec::new()),
        };

        let mut impls: Vec<String> = vec![String::new(); method_slots.len()];
        let mut receiver_type_ids: Vec<u64> = vec![ITABLE_RECEIVER_REF_TYPE_ID; method_slots.len()];
        let mut seen_shapes: HashSet<MethodShapeKey> = HashSet::new();

        for slot in &method_slots {
            let key = MethodShapeKey {
                name: slot.name.clone(),
                params_len: slot.params_len,
                has_receiver: slot.has_receiver,
            };

            if !seen_shapes.insert(key.clone()) {
                return Err(ItableLayoutError::AmbiguousInterfaceMethodSlot {
                    interface_fqn: nominal.fqn.clone(),
                    member: slot.name.clone(),
                });
            }

            let impl_member_fqn = if let Some(found) = vtable_impls.get(&key).cloned() {
                materialize_member_impl_fqn_for_owner(
                    &found,
                    concrete_class.ty,
                    index,
                    generic_template_symbol_suffixes,
                    lower,
                )?
            } else if let Some((_in_fqn, member_fqn)) = resolve_method_in_class_hierarchy(
                &concrete_class.base_fqn,
                &key,
                classes,
                &mut HashSet::new(),
            ) {
                materialize_member_impl_fqn_for_owner(
                    &member_fqn,
                    concrete_class.ty,
                    index,
                    generic_template_symbol_suffixes,
                    lower,
                )?
            } else if slot.has_body {
                format!("{}.{}", nominal.fqn, slot.name)
            } else {
                String::new()
            };

            let idx = slot.slot as usize;
            if idx < impls.len() {
                impls[idx] = impl_member_fqn;
                receiver_type_ids[idx] = ITABLE_RECEIVER_REF_TYPE_ID;
            }
        }

        let interface_type_name = lower.fmt_type(iface_ty);
        let interface_type_id =
            stable_runtime_type_id_for_lower(lower, iface_ty, "具体 interface 实例")?;

        let mut runtime_matches = concrete_interface_targets
            .iter()
            .copied()
            .filter(|target| is_type_assignable(iface_ty, *target, lower, builtins))
            .map(|target| {
                Ok((
                    lower.fmt_type(target),
                    stable_runtime_type_id_for_lower(lower, target, "runtime-match target")?,
                ))
            })
            .collect::<Result<Vec<_>, ItableLayoutError>>()?;
        if runtime_matches.is_empty() {
            runtime_matches.push((interface_type_name.clone(), interface_type_id));
        }
        runtime_matches.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0).then_with(|| lhs.1.cmp(&rhs.1)));
        runtime_matches.dedup();
        let (runtime_match_type_names, runtime_match_type_ids): (Vec<_>, Vec<_>) =
            runtime_matches.into_iter().unzip();

        entries.push(ClassItableEntry {
            interface_fqn: nominal.fqn,
            interface_id,
            interface_type_name,
            interface_type_id,
            runtime_match_type_names,
            runtime_match_type_ids,
            method_impl_fqns: impls,
            method_receiver_type_ids: receiver_type_ids,
        });
    }

    entries.sort_by(|a, b| {
        a.interface_id
            .cmp(&b.interface_id)
            .then_with(|| a.interface_type_id.cmp(&b.interface_type_id))
    });
    Ok(entries)
}

fn compute_class_interface_closure(
    class_fqn: &str,
    classes: &HashMap<String, ClassDeclInfo>,
    interfaces: &InterfaceIndex,
    visiting: &mut HashSet<String>,
    memo: &mut HashMap<String, HashSet<String>>,
) -> Result<HashSet<String>, ItableLayoutError> {
    if let Some(found) = memo.get(class_fqn) {
        return Ok(found.clone());
    }

    if !visiting.insert(class_fqn.to_string()) {
        return Err(ItableLayoutError::InheritanceCycle {
            fqn: class_fqn.to_string(),
        });
    }

    let mut out: HashSet<String> = HashSet::new();
    if let Some(info) = classes.get(class_fqn) {
        for iface in &info.direct_interfaces {
            collect_interface_and_supers(iface, interfaces, &mut out, &mut HashSet::new());
        }

        if let Some(super_fqn) = info.super_class_fqn.as_deref()
            && classes.contains_key(super_fqn)
        {
            let super_ifaces =
                compute_class_interface_closure(super_fqn, classes, interfaces, visiting, memo)?;
            out.extend(super_ifaces);
        }
    }

    let _ = visiting.remove(class_fqn);
    memo.insert(class_fqn.to_string(), out.clone());
    Ok(out)
}

fn collect_interface_and_supers(
    iface_fqn: &str,
    interfaces: &InterfaceIndex,
    out: &mut HashSet<String>,
    visiting: &mut HashSet<String>,
) {
    if !visiting.insert(iface_fqn.to_string()) {
        return;
    }

    if !out.insert(iface_fqn.to_string()) {
        let _ = visiting.remove(iface_fqn);
        return;
    }

    if let Some(iface) = interfaces.get(iface_fqn) {
        for sup in &iface.super_interfaces {
            collect_interface_and_supers(sup, interfaces, out, visiting);
        }
    }

    let _ = visiting.remove(iface_fqn);
}

fn resolve_method_in_class_hierarchy(
    class_fqn: &str,
    key: &MethodShapeKey,
    classes: &HashMap<String, ClassDeclInfo>,
    visiting: &mut HashSet<String>,
) -> Option<(String, String)> {
    if !visiting.insert(class_fqn.to_string()) {
        return None;
    }

    let info = classes.get(class_fqn)?;
    for m in &info.methods {
        if m.name == key.name
            && m.params_len == key.params_len
            && m.has_receiver == key.has_receiver
        {
            let member = format!("{}.{}", info.fqn, m.name);
            let _ = visiting.remove(class_fqn);
            return Some((info.fqn.clone(), member));
        }
    }

    if let Some(super_fqn) = info.super_class_fqn.as_deref()
        && classes.contains_key(super_fqn)
    {
        let resolved = resolve_method_in_class_hierarchy(super_fqn, key, classes, visiting);
        let _ = visiting.remove(class_fqn);
        return resolved;
    }

    let _ = visiting.remove(class_fqn);
    None
}

fn collect_interfaces_in_type_decl(
    source: &SourceFile,
    file: &ast::File,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    index: &Index,
    out: &mut InterfaceIndex,
) {
    let name = decl.name.text(source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);

    if matches!(decl.kind, ast::TypeKind::Interface)
        && !decl.modifiers.contains(&ast::Modifier::Sealed)
    {
        let super_interfaces = decl
            .supertypes
            .iter()
            .filter(|st| st.ctor_args_span.is_none())
            .filter_map(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty))
            .collect::<Vec<_>>();

        let mut method_slots: Vec<InterfaceMethodSlot> = Vec::new();
        if let Some(body) = &decl.body {
            let mut slot = 0u32;
            for member in &body.members {
                let ast::TypeMember::Fun(fun) = member else {
                    continue;
                };
                method_slots.push(InterfaceMethodSlot {
                    slot,
                    name: fun.name.text(source).to_string(),
                    member_fqn: format!("{}.{}", type_fqn, fun.name.text(source)),
                    decl_span: fun.span,
                    params_len: fun.params.len() as u32,
                    has_receiver: fun.receiver.is_some(),
                    has_body: matches!(fun.body, ast::FunBody::Block(_)),
                });
                slot = slot.saturating_add(1);
            }
        }

        out.insert(
            type_fqn.clone(),
            InterfaceInfo {
                fqn: type_fqn.clone(),
                interface_id: stable_rtti_interface_id(&type_fqn),
                super_interfaces,
                method_slots,
            },
        );
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_interfaces_in_type_decl(source, file, &type_fqn, nested, index, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_interfaces_in_object_decl(source, file, &type_fqn, obj, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_interfaces_in_object_decl(
    source: &SourceFile,
    file: &ast::File,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    index: &Index,
    out: &mut InterfaceIndex,
) {
    let Some(name) = object_decl_name(source, obj) else {
        return;
    };
    let obj_fqn = join_prefix(owner_prefix, &name);

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_interfaces_in_type_decl(source, file, &obj_fqn, nested, index, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_interfaces_in_object_decl(source, file, &obj_fqn, nested, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_classes_in_type_decl(
    source: &SourceFile,
    file: &ast::File,
    owner_prefix: &str,
    decl: &ast::TypeDecl,
    index: &Index,
    out: &mut HashMap<String, ClassDeclInfo>,
) {
    let name = decl.name.text(source).to_string();
    let type_fqn = join_prefix(owner_prefix, &name);

    if matches!(decl.kind, ast::TypeKind::Class | ast::TypeKind::Struct) {
        let super_class_fqn = decl
            .supertypes
            .iter()
            .filter(|st| st.ctor_args_span.is_some())
            .find_map(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty));

        let direct_interfaces = decl
            .supertypes
            .iter()
            .filter(|st| st.ctor_args_span.is_none())
            .filter_map(|st| index.type_ref_to_fqn_in_file(source, file, &st.ty))
            .collect::<Vec<_>>();

        let mut methods: Vec<ClassMethodInfo> = Vec::new();
        if let Some(body) = &decl.body {
            for member in &body.members {
                let ast::TypeMember::Fun(fun) = member else {
                    continue;
                };

                methods.push(ClassMethodInfo {
                    name: fun.name.text(source).to_string(),
                    params_len: fun.params.len() as u32,
                    has_receiver: fun.receiver.is_some(),
                });
            }
        }

        out.insert(
            type_fqn.clone(),
            ClassDeclInfo {
                fqn: type_fqn.clone(),
                is_value_type: matches!(decl.kind, ast::TypeKind::Struct),
                super_class_fqn,
                direct_interfaces,
                methods,
            },
        );
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(source, file, &type_fqn, nested, index, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_classes_in_object_decl(source, file, &type_fqn, obj, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_classes_in_object_decl(
    source: &SourceFile,
    file: &ast::File,
    owner_prefix: &str,
    obj: &ast::ObjectDecl,
    index: &Index,
    out: &mut HashMap<String, ClassDeclInfo>,
) {
    let Some(name) = object_decl_name(source, obj) else {
        return;
    };
    let obj_fqn = join_prefix(owner_prefix, &name);

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_classes_in_type_decl(source, file, &obj_fqn, nested, index, out);
            }
            ast::TypeMember::Object(nested) => {
                collect_classes_in_object_decl(source, file, &obj_fqn, nested, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn object_decl_name(source: &SourceFile, obj: &ast::ObjectDecl) -> Option<String> {
    match obj.name.as_ref() {
        Some(name) => Some(name.text(source).to_string()),
        None => match obj.kind {
            ast::ObjectKind::Companion => Some("Companion".to_string()),
            ast::ObjectKind::Object => None,
        },
    }
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}
