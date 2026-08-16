//! generic → monomorphic 单态化。
//!
//! 从 generic 模板 Module 出发，自 entry（`main`）起 BFS 收集实例化请求，对每个实例做
//! 类型替换（用 `scoop2_hir::ty::Subst` + `TypeStore::apply_subst`），产出 monomorphic 的
//! [`MaterializedMir`]。
//!
//! 关键设计（与参考实现 `scoopc_mir/src/mir/materialize/` 对齐）：
//! - `build_subst` 从 `FunDecl.type_params`（类型参数名序列）按声明顺序绑定到实例化 type_args；
//! - `subst_body` 覆盖**全部** body：local decls / rvalues / statements / terminators / metadata；
//! - 泛型检测按 `type_params.len() > 0`（不再误判 effect_row）；
//! - 可达性扫描递归进实例化后的 body；
//! - `MaterializedMir` 携带 backend contracts（class/enum/interface layout 发布）。

#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};

use scoop2_hir::ty::{Subst, TypeId, TypeKind, TypeStore};

use crate::diagnostics::MonomorphError;
use crate::mir::{Body, CallKind, FunDecl, Item, Module, Rvalue, StatementKind, TerminatorKind};

/// 单态化实例化键：模板 FQN + overload signature + 类型实参。
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct InstanceKey {
    pub template_fqn: String,
    /// 重载签名 canonical 文本（区分同名重载；空表示非重载/无法解析）。
    pub overload_sig: String,
    pub type_args: Vec<TypeId>,
}

/// 语言级 backend contract 发布（不含 LLVM-specific 信息）。
///
/// 携带 per-type 的方法槽位映射和成员布局信息，供后端生成 vtable/itable/layout。
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct BackendContracts {
    /// class vtable 契约：class FQN → 该 class 上的虚方法槽位列表。
    pub class_vtables: Vec<ClassVtableContract>,
    /// interface 契约：interface FQN → 该 interface 的方法签名列表。
    pub interfaces: Vec<InterfaceContract>,
    /// class→interface itable 契约：哪些 class 实现了哪些 interface。
    pub class_itables: Vec<ClassItableContract>,
    /// enum layout 契约：enum FQN → variant 列表 + payload 类型。
    pub enum_layouts: Vec<EnumLayoutContract>,
    /// struct layout 契约：struct FQN → 字段名 + 类型列表。
    pub struct_layouts: Vec<StructLayoutContract>,
    /// class init 契约：class FQN → 初始化顺序。
    pub class_inits: Vec<ClassInitContract>,
    /// ctor call site 契约：构造器调用位点列表。
    pub ctor_call_sites: Vec<CtorCallSiteContract>,
}

/// class vtable 契约：class 上的虚方法（按声明顺序）。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClassVtableContract {
    pub class_fqn: String,
    /// 虚方法：(方法名, owner FQN, overload signature canonical)。
    pub virtual_methods: Vec<(String, String, String)>,
}

/// interface 契约：interface 的方法签名列表。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct InterfaceContract {
    pub interface_fqn: String,
    /// (方法名, overload signature canonical)。
    pub methods: Vec<(String, String)>,
}

/// class→interface itable 契约。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClassItableContract {
    pub class_fqn: String,
    pub interface_fqns: Vec<String>,
}

/// enum layout 契约。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EnumLayoutContract {
    pub enum_fqn: String,
    /// variant 名列表。
    pub variants: Vec<String>,
}

/// struct layout 契约。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct StructLayoutContract {
    pub struct_fqn: String,
    /// 字段名 + 类型 FQN。
    pub fields: Vec<(String, String)>,
}

/// class init 契约。
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ClassInitContract {
    pub class_fqn: String,
}

/// ctor call site 契约。
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CtorCallSiteContract {
    pub type_fqn: String,
    pub ordered_param_count: usize,
}

/// 单态化产物容器。
#[derive(Clone, Debug)]
pub struct MaterializedMir {
    pub module: Module,
    /// 所有实例化键（按发现顺序）。
    pub instance_keys: Vec<InstanceKey>,
    /// 语言级 backend contracts。
    pub backend_contracts: BackendContracts,
}

/// 单态化结果。
pub type MaterializeResult<T> = Result<T, MonomorphError>;

/// 从 generic Module 单态化：自 entry（`main`）起 BFS 收集实例，类型替换。
pub fn materialize(
    generic: Module,
    entry_fqn: Option<&str>,
    hir: &scoop2_hir::hir::TypedHir,
) -> MaterializeResult<MaterializedMir> {
    let templates = collect_templates(&generic);
    let generic_types = generic.types.clone();
    let mut work: Materializer = Materializer {
        store: generic_types.clone(),
        templates,
        instances: HashMap::new(),
        order: Vec::new(),
        queue: VecDeque::new(),
        seen: HashMap::new(),
        backend_contracts: BackendContracts::default(),
        interner: hir.interner.clone(),
    };
    // 种子：entry 函数（无类型实参）或所有非泛型函数。
    if let Some(entry) = entry_fqn {
        let key = InstanceKey {
            template_fqn: entry.to_string(),
            overload_sig: String::new(),
            type_args: Vec::new(),
        };
        work.enqueue(key);
    } else {
        let seeds: Vec<InstanceKey> = work
            .templates
            .keys()
            .filter(|fqn| !is_generic_template_by_fqn(fqn, &work.templates))
            .map(|fqn| InstanceKey {
                template_fqn: fqn.clone(),
                overload_sig: String::new(),
                type_args: Vec::new(),
            })
            .collect();
        for s in seeds {
            work.enqueue(s);
        }
    }
    // 先收集 backend_contracts（从 metadata items），供 itable 方法实例化使用。
    collect_metadata_contracts(&mut work, &generic.items, hir, &generic_types);
    work.run()?;
    // 强制实例化 class_itables 中引用的所有方法（它们通过 itable 间接调用，scan_calls 检测不到）。
    let itable_method_fqns: Vec<String> = work
        .backend_contracts
        .class_itables
        .iter()
        .flat_map(|ci| {
            let class = &ci.class_fqn;
            ci.interface_fqns
                .iter()
                .flat_map(|iface_fqn| {
                    hir.interner
                        .get(iface_fqn)
                        .map(|sym| {
                            // 成员函数模板 FQN = owner.method（与 lower_fun_decl_inner 的 owner-qualified FQN 一致）。
                            // 迭代走声明序侧表，保证实例化顺序逐次构建一致。
                            hir.ordered_member_fun_names(&sym)
                                .into_iter()
                                .map(move |m| format!("{}.{}", class, hir.interner.resolve(m)))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
        })
        .collect();
    for method_fqn in &itable_method_fqns {
        // 成员函数 FQN = owner.method。若该 class 没有自己的实现（继承自超类型），
        // 沿超类型链查找拥有该方法的 owner。
        let template_fqn = if work.templates.contains_key(method_fqn) {
            method_fqn.clone()
        } else {
            // 沿超类型链查找：method_fqn = "scoop.core.String.hash"
            // 拆为 class_fqn + method_name，沿 supertypes 查找。
            let dot = method_fqn.rfind('.');
            if let Some(dot) = dot {
                let class_fqn_text = &method_fqn[..dot];
                let method_name = &method_fqn[dot + 1..];
                let class_sym = hir.interner.get(class_fqn_text);
                if let Some(class_sym) = class_sym {
                    let mut found = None;
                    let mut queue = std::collections::VecDeque::new();
                    queue.push_back(class_sym);
                    let mut visited = std::collections::HashSet::new();
                    while let Some(sym) = queue.pop_front() {
                        if !visited.insert(sym) {
                            continue;
                        }
                        let fqn_text = hir.interner.resolve(sym);
                        let candidate = format!("{}.{}", fqn_text, method_name);
                        if work.templates.contains_key(&candidate) {
                            found = Some(candidate);
                            break;
                        }
                        if let Some(supers) = hir.supertypes.get(&sym) {
                            for &sup in supers {
                                queue.push_back(sup);
                            }
                        }
                    }
                    found.unwrap_or_else(|| method_fqn.clone())
                } else {
                    method_fqn.clone()
                }
            } else {
                method_fqn.clone()
            }
        };
        work.enqueue(InstanceKey {
            template_fqn,
            overload_sig: String::new(),
            type_args: Vec::new(),
        });
    }
    work.run()?;

    // 构造 materialized module。
    let mut items: Vec<Item> = Vec::new();
    for key in &work.order {
        if let Some(fds) = work.instances.get(key) {
            // 对带类型实参的实例，计算唯一符号名（含 type args 哈希），
            // 确保同 FQN 不同实参的实例（如 println<Int> / println<String>）
            // 符号不冲突。非泛型实例（type_args 空）不设置（走默认 mangle）。
            let instance_symbol = if key.type_args.is_empty() {
                None
            } else {
                Some(compute_instance_symbol(
                    &key.template_fqn,
                    &key.type_args,
                    &work.store,
                    &hir.interner,
                ))
            };
            for fd in fds {
                let mut fd = fd.clone();
                fd.instance_symbol = instance_symbol.clone();
                items.push(Item::Fun(fd));
            }
        }
    }
    // 非 callable items（Initializer / ExternGlobal / Metadata）直接保留 + 收集 backend contracts。
    for it in &generic.items {
        match it {
            Item::Fun(_) => {}
            Item::Metadata(m) => {
                // 按 kind 收集 backend contracts（携带真实数据）。
                match m.kind {
                    crate::mir::MetadataKind::Class | crate::mir::MetadataKind::Struct => {
                        // class vtable 契约：从 HIR member_funs 收集虚方法（含 overload signature）。
                        // 继承感知 + 声明序：超类链方法占前面的 slot，子类 override 保留
                        // slot 位置；迭代走 member_fun_order，避免 HashMap 迭代序不确定。
                        let virtual_methods: Vec<(String, String, String)> =
                            collect_class_virtual_methods(hir, &generic_types, &m.fqn);
                        work.backend_contracts
                            .class_vtables
                            .push(ClassVtableContract {
                                class_fqn: m.fqn.clone(),
                                virtual_methods,
                            });
                        work.backend_contracts.class_inits.push(ClassInitContract {
                            class_fqn: m.fqn.clone(),
                        });
                        // class × interface itable 契约：该 class 实现的所有 interface（来自超类型链）。
                        let interface_fqns: Vec<String> = hir_fqn_for_metadata(hir, &m.fqn)
                            .and_then(|fqn_sym| hir.supertypes.get(&fqn_sym))
                            .map(|supers| {
                                supers
                                    .iter()
                                    .map(|&s| hir.interner.resolve(s).to_string())
                                    .filter(|fqn| {
                                        hir.interner
                                            .get(fqn)
                                            .is_some_and(|sym| hir.interface_fqns.contains(&sym))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if !interface_fqns.is_empty() {
                            // 避免重复（已在 collect_metadata_contracts 中收集）。
                            let already = work
                                .backend_contracts
                                .class_itables
                                .iter()
                                .any(|ci| ci.class_fqn == m.fqn);
                            if !already {
                                work.backend_contracts
                                    .class_itables
                                    .push(ClassItableContract {
                                        class_fqn: m.fqn.clone(),
                                        interface_fqns,
                                    });
                            }
                        }
                    }
                    crate::mir::MetadataKind::Interface => {
                        let store_ref = &generic_types;
                        let interner_ref = &hir.interner;
                        // itable slot 同样按声明序（member_fun_order）分配，
                        // 避免 HashMap 迭代序不确定导致逐次构建不一致。
                        let methods: Vec<(String, String)> = hir_fqn_for_metadata(hir, &m.fqn)
                            .map(|fqn_sym| {
                                hir.ordered_member_fun_names(&fqn_sym)
                                    .into_iter()
                                    .flat_map(|name_sym| {
                                        let mname = hir.interner.resolve(name_sym).to_string();
                                        let sigs = hir
                                            .member_funs
                                            .get(&fqn_sym)
                                            .and_then(|mf| mf.get(&name_sym))
                                            .into_iter()
                                            .flatten();
                                        sigs.map(move |sig| {
                                            let sig_canonical =
                                                crate::mir::stable_id::build_overload_sig(
                                                    store_ref,
                                                    interner_ref,
                                                    &sig.param_types,
                                                );
                                            (mname.clone(), sig_canonical)
                                        })
                                        .collect::<Vec<_>>()
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        work.backend_contracts.interfaces.push(InterfaceContract {
                            interface_fqn: m.fqn.clone(),
                            methods,
                        });
                    }
                    crate::mir::MetadataKind::Enum => {
                        let variants: Vec<String> = hir_fqn_for_metadata(hir, &m.fqn)
                            .and_then(|fqn_sym| hir.enum_variants.get(&fqn_sym))
                            .map(|vs| {
                                vs.iter()
                                    .map(|v| hir.interner.resolve(*v).to_string())
                                    .collect()
                            })
                            .unwrap_or_default();
                        work.backend_contracts
                            .enum_layouts
                            .push(EnumLayoutContract {
                                enum_fqn: m.fqn.clone(),
                                variants,
                            });
                    }
                    crate::mir::MetadataKind::Struct => {
                        let fields: Vec<(String, String)> = hir_fqn_for_metadata(hir, &m.fqn)
                            .and_then(|fqn_sym| hir.members.get(&fqn_sym))
                            .map(|ms| {
                                ms.iter()
                                    .map(|(name, ty)| {
                                        let ty_text = crate::mir::stable_id::canonical_type_text(
                                            &generic_types,
                                            &hir.interner,
                                            *ty,
                                        );
                                        (hir.interner.resolve(*name).to_string(), ty_text)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        work.backend_contracts
                            .struct_layouts
                            .push(StructLayoutContract {
                                struct_fqn: m.fqn.clone(),
                                fields,
                            });
                    }
                    _ => {}
                }
                items.push(it.clone());
            }
            _ => items.push(it.clone()),
        }
    }
    let mut result_module = Module {
        items,
        types: work.store.clone(),
    };
    // 去虚化 pass：final/单候选接收者的 Virtual/Interface 调用改写为 Direct。
    let devirt_ctx = crate::mir::devirtualize::DevirtContext {
        interner: &hir.interner,
        extensible_class_fqns: &hir.extensible_class_fqns,
        interface_fqns: &hir.interface_fqns,
        direct_subtypes: &hir.direct_subtypes,
    };
    crate::mir::devirtualize::devirtualize_module(&mut result_module, &devirt_ctx);
    crate::mir::inline::inline_module(
        &mut result_module,
        crate::mir::inline::InlineConfig::default(),
        &hir.interner,
    );
    // effect lowering pass：把 Perform/Handle/Resume 消除为本地 dispatch / 状态机。
    // 在 inline 之后运行：inline 消除 effect-transparent HOF 后，effect 边界更少。
    crate::mir::effect_lower::lower_effects(&mut result_module, &hir.interner);
    // 为所有顶层函数计算 stable template key（供分离编译）。
    crate::mir::stable_id::compute_public_stable_keys(&mut result_module, &hir.interner);
    // mangling 定稿（M3-2）：非泛型实例补 `mangle(fqn, stable_template_key)`；
    // 泛型实例已带 `compute_instance_symbol`。Initializer 符号一并定稿——
    // LIR/codegen 从此纯读（archive 携带定稿值）。
    for item in &mut result_module.items {
        match item {
            crate::mir::Item::Fun(fd) => {
                if fd.instance_symbol.is_none() {
                    fd.instance_symbol = Some(crate::mir::stable_id::mangle_symbol(
                        &fd.fqn,
                        &fd.stable_template_key,
                    ));
                }
            }
            crate::mir::Item::Initializer(ir) => {
                if ir.symbol.is_empty() {
                    ir.symbol = crate::mir::stable_id::mangle_symbol(&ir.fqn, &None);
                }
            }
            _ => {}
        }
    }
    // 无泛型出口 gate（M3-4，ICE 级）：实例体内不得残留 TypeParam——违反即
    // materialize 类型替换不完备（编译器 bug，bug 通道而非用户诊断，C5）。
    no_generics_gate(&result_module, &work.store);
    Ok(MaterializedMir {
        module: result_module,
        instance_keys: work.order,
        backend_contracts: work.backend_contracts,
    })
}

/// 无泛型出口 gate：扫描单态化模块的全部类型位点（签名/局部/语句/终结符
/// 内嵌类型），发现 `TypeKind::Param` 残留即 panic（ICE）。
fn no_generics_gate(module: &Module, store: &TypeStore) {
    let is_param = |ty: scoop2_hir::ty::TypeId| {
        matches!(store.kind(ty), scoop2_hir::ty::TypeKind::Param(_))
    };
    // debug 断言形态的 ICE（C9-4：verify 降级为 debug 断言——不进生产控制流）。
    let mut check = |ty: scoop2_hir::ty::TypeId, where_: &str| {
        debug_assert!(
            !is_param(ty),
            "ICE[no-generics-gate]: 单态化输出残留 TypeParam @ {where_}"
        );
    };
    for item in &module.items {
        if let Item::Fun(fd) = item {
            debug_assert!(
                fd.type_params.is_empty(),
                "ICE[no-generics-gate]: {} type_params 非空",
                fd.fqn
            );
            check(fd.ty, &format!("{}:fn_ty", fd.fqn));
            check(fd.return_ty, &format!("{}:return", fd.fqn));
            for p in &fd.params {
                check(p.ty, &format!("{}:param {}", fd.fqn, p.name));
            }
            let Some(body) = &fd.body else { continue };
            for (i, decl) in body.locals.iter().enumerate() {
                check(
                    decl.ty,
                    &format!("{}:local[{i}]={:?}", fd.fqn, decl.name),
                );
            }
            for block in &body.blocks {
                for stmt in &block.stmts {
                    if let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind {
                        gate_rvalue_types(value, &mut check);
                    }
                }
            }
        }
    }
}

/// 收集 rvalue 内嵌类型位点并逐个检查（TupleIndex/EnumVariant/Cast/TypeTest/
/// MakeArray/MemberAccess 等携带 TypeId 的变体）。
fn gate_rvalue_types(
    rv: &crate::mir::Rvalue,
    check: &mut impl FnMut(scoop2_hir::ty::TypeId, &str),
) {
    use crate::mir::Rvalue;
    match rv {
        Rvalue::TupleIndex { element_ty, .. } => check(*element_ty, "rvalue:tuple_index"),
        Rvalue::IndexAccess {
            element_ty,
            receiver_ty,
            ..
        } => {
            check(*element_ty, "rvalue:index");
            check(*receiver_ty, "rvalue:index_recv");
        }
        Rvalue::EnumVariant {
            enum_ty, payload, ..
        } => {
            check(*enum_ty, "rvalue:enum");
            check(payload.aggregate_ty, "rvalue:enum_payload");
        }
        Rvalue::TypeTest { metadata, .. } => {
            check(metadata.descriptor.ty, "rvalue:type_test_desc");
            check(metadata.target_ty, "rvalue:type_test_target");
            check(metadata.source_ty, "rvalue:type_test_source");
        }
        Rvalue::Cast { metadata, .. } => {
            check(metadata.test.source_ty, "rvalue:cast_src");
            check(metadata.test.target_ty, "rvalue:cast_target");
        }
        Rvalue::MemberAccess { member, .. } => {
            check(member.receiver_ty, "rvalue:member_recv");
        }
        Rvalue::MakeArray { result_ty, .. } => check(*result_ty, "rvalue:make_array"),
        Rvalue::MakeTuple { transport, .. } => {
            check(transport.aggregate_ty, "rvalue:make_tuple");
        }
        Rvalue::StructLit { transport, .. } => {
            check(transport.aggregate_ty, "rvalue:struct_lit");
        }
        Rvalue::MakeClosure { env_contract, .. } => {
            check(env_contract.env_ty, "rvalue:closure_env");
            for cap in &env_contract.captures {
                check(cap.transport.source_ty, "rvalue:closure_capture");
            }
        }
        _ => {}
    }
}

/// 从 generic.items 的 Class metadata 收集 class_itables contracts。
/// 提前于 materialize 的 run()，使 itable 方法实例化可用。
fn collect_metadata_contracts(
    work: &mut Materializer,
    items: &[Item],
    hir: &scoop2_hir::hir::TypedHir,
    _types: &scoop2_hir::ty::TypeStore,
) {
    for it in items {
        if let Item::Metadata(m) = it {
            if matches!(
                m.kind,
                crate::mir::MetadataKind::Class | crate::mir::MetadataKind::Struct
            ) {
                let interface_fqns: Vec<String> = hir_fqn_for_metadata(hir, &m.fqn)
                    .and_then(|fqn_sym| hir.supertypes.get(&fqn_sym))
                    .map(|supers| {
                        supers
                            .iter()
                            .map(|&s| hir.interner.resolve(s).to_string())
                            .filter(|fqn| {
                                hir.interner
                                    .get(fqn)
                                    .is_some_and(|sym| hir.interface_fqns.contains(&sym))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if !interface_fqns.is_empty() {
                    work.backend_contracts
                        .class_itables
                        .push(ClassItableContract {
                            class_fqn: m.fqn.clone(),
                            interface_fqns,
                        });
                }
            }
        }
    }
}

struct Materializer {
    store: TypeStore,
    templates: HashMap<String, Vec<FunDecl>>,
    instances: HashMap<InstanceKey, Vec<FunDecl>>,
    order: Vec<InstanceKey>,
    queue: VecDeque<InstanceKey>,
    seen: HashMap<InstanceKey, bool>,
    backend_contracts: BackendContracts,
    /// interner（解析 ClassCtor.type_fqn Symbol 为 FQN 文本，供 $init 实例化入队）。
    interner: scoop2_base::Interner,
}

impl Materializer {
    fn enqueue(&mut self, key: InstanceKey) {
        if self.seen.contains_key(&key) || self.instances.contains_key(&key) {
            return;
        }
        self.seen.insert(key.clone(), true);
        self.queue.push_back(key);
    }

    fn run(&mut self) -> MaterializeResult<()> {
        while let Some(key) = self.queue.pop_front() {
            if self.instances.contains_key(&key) {
                continue;
            }
            let family = self.materialize_instance(&key)?;
            // 递归扫描 family 中的调用，发现新实例化请求。
            let discovered = self.scan_calls(&family);
            self.instances.insert(key.clone(), family);
            self.order.push(key);
            for req in discovered {
                self.enqueue(req);
            }
        }
        Ok(())
    }

    fn materialize_instance(&mut self, key: &InstanceKey) -> MaterializeResult<Vec<FunDecl>> {
        let templates = self.templates.get(&key.template_fqn).cloned();
        let Some(templates) = templates else {
            return Err(MonomorphError::no_template(
                scoop2_base::Span::default(),
                &key.template_fqn,
            ));
        };
        // 构造 Subst：从首个模板的 type_params 按声明顺序绑定到 key.type_args。
        let subst = build_subst(&templates, &key.type_args)?;
        let family: Vec<FunDecl> = templates
            .into_iter()
            .map(|fd| subst_fun_decl(fd, &subst, &mut self.store))
            .collect();
        Ok(family)
    }

    fn scan_calls(&self, family: &[FunDecl]) -> Vec<InstanceKey> {
        let mut reqs = Vec::new();
        for fd in family {
            if let Some(body) = &fd.body {
                let mut raw_reqs = Vec::new();
                scan_body_calls(body, &mut raw_reqs, &self.interner);
                for r in raw_reqs {
                    if self.templates.contains_key(&r.template_fqn) {
                        reqs.push(r);
                    }
                }
            }
        }
        reqs
    }
}

/// 计算单态化实例的唯一符号名。
///
/// 格式：`mangle(fqn) + "_" + hash(canonical(type_args))`。
/// 与 `mangle_symbol(fqn, stable_template_key)`（只含模板信息）互补：
/// 本函数额外编码具体类型实参，确保同模板不同实参的实例符号唯一。
///
/// 公开供 LIR 调用点（`map_rvalue`）按相同公式解析目标实例符号。
pub fn compute_instance_symbol(
    fqn: &str,
    type_args: &[TypeId],
    store: &TypeStore,
    interner: &scoop2_base::Interner,
) -> String {
    use crate::mir::stable_id::{StableHashScope, canonical_type_text, stable_hash};
    let base = fqn.replace('.', "_");
    let args_canonical: Vec<String> = type_args
        .iter()
        .map(|&ty| canonical_type_text(store, interner, ty))
        .collect();
    let instance_text = format!("inst({fqn};T[{}])", args_canonical.join(","));
    let hash = stable_hash(StableHashScope::Abi, &instance_text);
    format!("{base}_I{hash}")
}

// ---------------------------------------------------------------------------
// 模板收集 / 泛型检测
// ---------------------------------------------------------------------------

fn collect_templates(module: &Module) -> HashMap<String, Vec<FunDecl>> {
    let mut map: HashMap<String, Vec<FunDecl>> = HashMap::new();
    for item in &module.items {
        if let Item::Fun(fd) = item {
            // 闭包（`<enclosing>$closure<N>`）并入**外层函数的 family**：其
            // 体内的 Param 类型属于外层模板的参数空间——只有随外层同一
            // subst 替换才完备（独立实例化 + 空实参会让 Param 残留——
            // no-generics gate 抓到的真身）。
            let key = if let Some(pos) = fd.fqn.find("$closure") {
                fd.fqn[..pos].to_string()
            } else {
                fd.fqn.clone()
            };
            map.entry(key).or_default().push(fd.clone());
        }
    }
    map
}

/// 泛型检测：按 type_params.len() > 0（不再误判 effect_row）。
fn is_generic_template_by_fqn(fqn: &str, templates: &HashMap<String, Vec<FunDecl>>) -> bool {
    templates
        .get(fqn)
        .map(|fds| fds.iter().any(|fd| !fd.type_params.is_empty()))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Subst 构造（真实绑定类型参数名 → 实参）
// ---------------------------------------------------------------------------

/// 从模板的 type_params（类型参数 id 序列）按声明顺序绑定到 type_args。
fn build_subst(templates: &[FunDecl], type_args: &[TypeId]) -> MaterializeResult<Subst> {
    let mut subst = Subst::new();
    if let Some(first) = templates.first() {
        let params_count = first.type_params.len();
        let args_count = type_args.len();
        if args_count < params_count {
            return Err(MonomorphError::error(
                scoop2_base::Span::default(),
                format!(
                    "单态化失败：泛型实参数量不足（需要 {} 个，实际 {} 个）",
                    params_count, args_count
                ),
            ));
        }
        for (i, &tp_id) in first.type_params.iter().enumerate() {
            if let Some(&arg_ty) = type_args.get(i) {
                subst.insert(tp_id, arg_ty);
            }
        }
    }
    Ok(subst)
}

// ---------------------------------------------------------------------------
// 类型替换（覆盖全部 body：rvalue / statement / terminator / metadata）
// ---------------------------------------------------------------------------

fn subst_fun_decl(mut fd: FunDecl, subst: &Subst, store: &mut TypeStore) -> FunDecl {
    fd.ty = store.apply_subst(fd.ty, subst);
    fd.return_ty = store.apply_subst(fd.return_ty, subst);
    fd.effect_row = store.apply_subst_row(fd.effect_row, subst);
    for p in &mut fd.params {
        p.ty = store.apply_subst(p.ty, subst);
    }
    if let Some(body) = fd.body.take() {
        fd.body = Some(subst_body(body, subst, store));
    }
    // 实例是具体化的：模板参数清空（无泛型出口 gate 的契约）。
    fd.type_params = Vec::new();
    fd
}

fn subst_body(mut body: Body, subst: &Subst, store: &mut TypeStore) -> Body {
    // 替换所有 local 类型。
    for decl in &mut body.locals {
        decl.ty = store.apply_subst(decl.ty, subst);
    }
    // 替换所有 statement / terminator 中的 TypeId。
    for block in &mut body.blocks {
        for stmt in &mut block.stmts {
            subst_statement(stmt, subst, store);
        }
        subst_terminator(&mut block.terminator, subst, store);
    }
    body
}

fn subst_statement(stmt: &mut crate::mir::Statement, subst: &Subst, store: &mut TypeStore) {
    match &mut stmt.kind {
        StatementKind::Assign { value, .. } => subst_rvalue(value, subst, store),
        StatementKind::StoreMember {
            member, value_ty, ..
        } => {
            subst_member_access_metadata(member, subst, store);
            *value_ty = store.apply_subst(*value_ty, subst);
        }
        StatementKind::StoreTupleIndex { value_ty, .. }
        | StatementKind::StoreTopLevelVar { value_ty, .. } => {
            *value_ty = store.apply_subst(*value_ty, subst);
        }
        _ => {}
    }
}

fn subst_rvalue(rv: &mut Rvalue, subst: &Subst, store: &mut TypeStore) {
    use crate::mir::transport::*;
    match rv {
        Rvalue::Use(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::ClassLit { .. }
        | Rvalue::MakeContinuation { .. }
        | Rvalue::MakeChainLink { .. } => {}
        Rvalue::TopLevelRef(tl) => {
            subst_type_ids(store, &mut tl.generic_type_args, subst);
            tl.hidden_effects = store.apply_subst_row(tl.hidden_effects.clone(), subst);
            subst_effect_rows(store, &mut tl.generic_eff_args, subst);
        }
        Rvalue::TypeTest { metadata, .. } => {
            subst_type_test_metadata(metadata, subst, store);
        }
        Rvalue::Cast { metadata, .. } => {
            subst_cast_metadata(metadata, subst, store);
        }
        Rvalue::MemberAccess { member, .. } => {
            subst_member_access_metadata(member, subst, store);
        }
        Rvalue::TupleIndex { element_ty, .. } => {
            *element_ty = store.apply_subst(*element_ty, subst);
        }
        Rvalue::IndexAccess {
            element_ty,
            receiver_ty,
            ..
        } => {
            *element_ty = store.apply_subst(*element_ty, subst);
            *receiver_ty = store.apply_subst(*receiver_ty, subst);
        }
        Rvalue::EnumVariant {
            enum_ty,
            payload,
            args,
            ..
        } => {
            *enum_ty = store.apply_subst(*enum_ty, subst);
            subst_aggregate_transport(payload, subst, store);
            for a in args.iter_mut() {
                a.value_ty = store.apply_subst(a.value_ty, subst);
            }
        }
        Rvalue::ClassCtor {
            hidden_effects,
            args,
            ..
        } => {
            for a in args.iter_mut() {
                a.value_ty = store.apply_subst(a.value_ty, subst);
            }
            *hidden_effects = store.apply_subst_row(hidden_effects.clone(), subst);
        }
        Rvalue::Call {
            kind,
            args,
            transport,
            ..
        } => {
            subst_call_kind(kind, subst, store);
            for a in args.iter_mut() {
                a.value_ty = store.apply_subst(a.value_ty, subst);
            }
            subst_value_transport(&mut transport.result, subst, store);
            if let Some(ar) = &mut transport.aggregate_return {
                subst_value_transport(ar, subst, store);
            }
            if let Some(arr) = &mut transport.array {
                subst_array_element_transport(arr, subst, store);
            }
            if let Some(gc) = &mut transport.gc {
                subst_gc_intrinsic_transport(gc, subst, store);
            }
        }
        Rvalue::MakeTuple { transport, .. } => {
            subst_aggregate_transport(transport, subst, store);
        }
        Rvalue::StructLit {
            transport, fields, ..
        } => {
            subst_aggregate_transport(transport, subst, store);
            for f in fields.iter_mut() {
                f.value_ty = store.apply_subst(f.value_ty, subst);
            }
        }
        Rvalue::MakeArray {
            result_ty,
            elements,
            ..
        } => {
            *result_ty = store.apply_subst(*result_ty, subst);
            let _ = elements; // elements 是 Operand（无 TypeId）
        }
        Rvalue::WithUpdate {
            base: _,
            updates,
            result_ty,
        } => {
            *result_ty = store.apply_subst(*result_ty, subst);
            for u in updates.iter_mut() {
                u.value_ty = store.apply_subst(u.value_ty, subst);
            }
        }
        Rvalue::MakeClosure { env_contract, .. } => {
            env_contract.env_ty = store.apply_subst(env_contract.env_ty, subst);
            for cap in &mut env_contract.captures {
                subst_value_transport(&mut cap.transport, subst, store);
            }
        }
        Rvalue::InterpolatedString { .. } => {}
        Rvalue::PerformResult { result_ty, .. } => {
            *result_ty = store.apply_subst(*result_ty, subst);
        }
        Rvalue::TakeChainLink { result_ty } | Rvalue::ResumeChainLink { result_ty, .. } => {
            *result_ty = store.apply_subst(*result_ty, subst);
        }
        Rvalue::PatternMatch {
            subject: _,
            pattern,
        } => {
            subst_pattern(pattern, subst, store);
        }
        Rvalue::PatternExtract {
            subject: _,
            result_ty,
            ..
        } => {
            *result_ty = store.apply_subst(*result_ty, subst);
        }
        Rvalue::IntEq { .. } => {}
    }
}

fn subst_call_kind(kind: &mut CallKind, subst: &Subst, store: &mut TypeStore) {
    match kind {
        CallKind::Direct {
            type_args,
            generic_type_args,
            generic_eff_args,
            ..
        } => {
            subst_type_ids(store, type_args, subst);
            subst_type_ids(store, generic_type_args, subst);
            subst_effect_rows(store, generic_eff_args, subst);
        }
        CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. } => {
            dispatch.receiver_ty = store.apply_subst(dispatch.receiver_ty, subst);
            subst_type_ids(store, &mut dispatch.generic_type_args, subst);
            subst_effect_rows(store, &mut dispatch.generic_eff_args, subst);
        }
        CallKind::Closure { .. } | CallKind::FunValue { .. } | CallKind::Resume { .. } => {}
    }
}

fn subst_terminator(term: &mut crate::mir::Terminator, subst: &Subst, store: &mut TypeStore) {
    match &mut term.kind {
        TerminatorKind::Return { value: Some(op) } => {
            // Operand 不含 TypeId；无需替换（local 的类型已替换）。
            let _ = op;
        }
        TerminatorKind::Perform { metadata, args, .. } => {
            metadata.effect_ty = store.apply_subst(metadata.effect_ty, subst);
            metadata.result_ty = store.apply_subst(metadata.result_ty, subst);
            subst_type_ids(store, &mut metadata.op_type_args, subst);
            subst_optional_type_id(store, &mut metadata.payload_tuple_ty, subst);
            for t in &mut metadata.payload_component_tys {
                *t = store.apply_subst(*t, subst);
            }
            for vt in &mut metadata.payload_transport {
                subst_value_transport(vt, subst, store);
            }
            for a in args.iter_mut() {
                a.value_ty = store.apply_subst(a.value_ty, subst);
            }
        }
        TerminatorKind::Handle { metadata, arms, .. } => {
            metadata.result_ty = store.apply_subst(metadata.result_ty, subst);
            metadata.body_result_ty = store.apply_subst(metadata.body_result_ty, subst);
            if let Some(ref mut f) = metadata.finally_result_ty {
                *f = store.apply_subst(*f, subst);
            }
            for arm in arms.iter_mut() {
                arm.handled_effect_ty = store.apply_subst(arm.handled_effect_ty, subst);
                arm.body_ty = store.apply_subst(arm.body_ty, subst);
                subst_type_ids(store, &mut arm.op_type_args, subst);
                subst_optional_type_id(store, &mut arm.payload_tuple_ty, subst);
                for t in &mut arm.payload_component_tys {
                    *t = store.apply_subst(*t, subst);
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// metadata 替换 helper
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 通用替换 helper：Vec<TypeId> / Vec<EffectRow> / Option<TypeId>
// ---------------------------------------------------------------------------

fn subst_type_ids(store: &mut TypeStore, tys: &mut Vec<scoop2_hir::ty::TypeId>, subst: &Subst) {
    for t in tys.iter_mut() {
        *t = store.apply_subst(*t, subst);
    }
}

fn subst_effect_rows(
    store: &mut TypeStore,
    rows: &mut Vec<scoop2_hir::ty::EffectRow>,
    subst: &Subst,
) {
    for r in rows.iter_mut() {
        *r = store.apply_subst_row(r.clone(), subst);
    }
}

fn subst_optional_type_id(
    store: &mut TypeStore,
    ty: &mut Option<scoop2_hir::ty::TypeId>,
    subst: &Subst,
) {
    if let Some(t) = ty {
        *t = store.apply_subst(*t, subst);
    }
}

fn subst_value_transport(
    vt: &mut crate::mir::ValueTransportMetadata,
    subst: &Subst,
    store: &mut TypeStore,
) {
    vt.source_ty = store.apply_subst(vt.source_ty, subst);
    if let Some(b) = &mut vt.boxing {
        b.source_ty = store.apply_subst(b.source_ty, subst);
        if let Some(t) = b.target_ty {
            b.target_ty = Some(store.apply_subst(t, subst));
        }
    }
}

fn subst_aggregate_transport(
    at: &mut crate::mir::AggregateTransportMetadata,
    subst: &Subst,
    store: &mut TypeStore,
) {
    at.aggregate_ty = store.apply_subst(at.aggregate_ty, subst);
    for f in &mut at.fields {
        f.ty = store.apply_subst(f.ty, subst);
        subst_value_transport(&mut f.transport, subst, store);
    }
}

fn subst_array_element_transport(
    aet: &mut crate::mir::ArrayElementTransportMetadata,
    subst: &Subst,
    store: &mut TypeStore,
) {
    aet.array_ty = store.apply_subst(aet.array_ty, subst);
    aet.element_ty = store.apply_subst(aet.element_ty, subst);
    subst_value_transport(&mut aet.element, subst, store);
}

fn subst_gc_intrinsic_transport(
    gc: &mut crate::mir::GcIntrinsicTransportMetadata,
    subst: &Subst,
    store: &mut TypeStore,
) {
    gc.subject_ty = store.apply_subst(gc.subject_ty, subst);
    if let Some(t) = gc.token_ty {
        gc.token_ty = Some(store.apply_subst(t, subst));
    }
    subst_value_transport(&mut gc.subject, subst, store);
}

fn subst_type_test_metadata(
    m: &mut crate::mir::RuntimeTypeTestMetadata,
    subst: &Subst,
    store: &mut TypeStore,
) {
    m.source_ty = store.apply_subst(m.source_ty, subst);
    m.target_ty = store.apply_subst(m.target_ty, subst);
    m.descriptor.ty = store.apply_subst(m.descriptor.ty, subst);
    subst_runtime_type_parameterized(&mut m.parameterized, subst, store);
}

fn subst_runtime_type_parameterized(
    p: &mut crate::mir::transport::RuntimeTypeParameterizedMatch,
    subst: &Subst,
    store: &mut TypeStore,
) {
    use crate::mir::transport::RuntimeTypeParameterizedMatch as P;
    match p {
        P::None => {}
        P::Nominal {
            type_args,
            effect_arg,
        } => {
            subst_type_ids(store, type_args, subst);
            if let Some(ea) = effect_arg {
                *ea = store.apply_subst_row(ea.clone(), subst);
            }
        }
        P::Function {
            receiver,
            params,
            return_ty,
            effects,
            ..
        } => {
            if let Some(r) = receiver {
                *r = store.apply_subst(*r, subst);
            }
            subst_type_ids(store, params, subst);
            *return_ty = store.apply_subst(*return_ty, subst);
            *effects = store.apply_subst_row(effects.clone(), subst);
        }
        P::Option { payload_ty } => {
            *payload_ty = store.apply_subst(*payload_ty, subst);
        }
        P::Tuple { element_tys } => {
            subst_type_ids(store, element_tys, subst);
        }
        P::Union { variants } => {
            subst_type_ids(store, variants, subst);
        }
        P::StarProjection { read_ty } => {
            *read_ty = store.apply_subst(*read_ty, subst);
        }
    }
}

fn subst_cast_metadata(
    m: &mut crate::mir::RuntimeCastMetadata,
    subst: &Subst,
    store: &mut TypeStore,
) {
    subst_type_test_metadata(&mut m.test, subst, store);
    use crate::mir::transport::RuntimeCastResult as R;
    match &mut m.result {
        R::Target { ty } => {
            *ty = store.apply_subst(*ty, subst);
        }
        R::Option { option_ty, some_ty } => {
            *option_ty = store.apply_subst(*option_ty, subst);
            *some_ty = store.apply_subst(*some_ty, subst);
        }
    }
}

fn subst_member_access_metadata(
    m: &mut crate::mir::MemberAccessMetadata,
    subst: &Subst,
    store: &mut TypeStore,
) {
    m.receiver_ty = store.apply_subst(m.receiver_ty, subst);
    m.hidden_effects = store.apply_subst_row(m.hidden_effects.clone(), subst);
}

/// 替换 Pattern 中的所有 TypeId。
fn subst_pattern(pat: &mut crate::mir::Pattern, subst: &Subst, store: &mut TypeStore) {
    use crate::mir::Pattern;
    match pat {
        Pattern::Wildcard
        | Pattern::IntLit(_)
        | Pattern::CharLit(_)
        | Pattern::StringLit(_)
        | Pattern::BoolLit(_) => {}
        Pattern::Bind { ty, .. } => {
            *ty = store.apply_subst(*ty, subst);
        }
        Pattern::Is { ty, .. } => {
            *ty = store.apply_subst(*ty, subst);
        }
        Pattern::Tuple { elements } => {
            for e in elements.iter_mut() {
                subst_pattern(e, subst, store);
            }
        }
        Pattern::Struct { fields, .. } => {
            for f in fields.iter_mut() {
                subst_pattern(&mut f.pattern, subst, store);
            }
        }
        Pattern::Variant { args, .. } => {
            for a in args.iter_mut() {
                subst_pattern(a, subst, store);
            }
        }
        Pattern::Or { patterns } => {
            for p in patterns.iter_mut() {
                subst_pattern(p, subst, store);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 可达性扫描（递归进实例化后的 body）
// ---------------------------------------------------------------------------

fn scan_body_calls(body: &Body, reqs: &mut Vec<InstanceKey>, interner: &scoop2_base::Interner) {
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let StatementKind::Assign { value, .. } = &stmt.kind {
                scan_rvalue_calls(value, reqs, interner);
            }
        }
        scan_terminator_calls(&block.terminator.kind, reqs);
    }
}

fn scan_rvalue_calls(rv: &Rvalue, reqs: &mut Vec<InstanceKey>, interner: &scoop2_base::Interner) {
    match rv {
        Rvalue::Call { kind, args, .. } => {
            scan_call_kind(kind, reqs);
            // 递归进 CallArg（嵌套调用）。
            let _ = args;
        }
        // 闭包构造：闭包并入外层 family（collect_templates）——随外层实例
        // 一同替换/发射，不独立入队（其 Param 属外层参数空间）。
        Rvalue::MakeClosure { .. } => {}
        // class 构造：强制实例化该类的初始化 callable。
        // primary ctor → `<Class>.$init`；secondary ctor → `<Class>.$ctor.s<span_start>`。
        // 两者都 push（$ctor 存在时实例化，不存在则 scan_calls 过滤）。
        Rvalue::ClassCtor { type_fqn, ctor, .. } => {
            let class_fqn = interner.resolve(*type_fqn);
            // secondary ctor callable（若选中 secondary）。
            if let Some(span) = ctor.selected_ctor_span {
                reqs.push(InstanceKey {
                    template_fqn: format!("{}.$ctor.s{}", class_fqn, span.start),
                    overload_sig: String::new(),
                    type_args: Vec::new(),
                });
            }
            // primary $init（secondary ctor 内部也会调它，需实例化）。
            reqs.push(InstanceKey {
                template_fqn: format!("{}.$init", class_fqn),
                overload_sig: String::new(),
                type_args: Vec::new(),
            });
        }
        _ => {}
    }
}

fn scan_call_kind(kind: &CallKind, reqs: &mut Vec<InstanceKey>) {
    match kind {
        CallKind::Direct {
            callee_fqn,
            generic_type_args,
            stable_template_key,
            ..
        } => {
            // 从 stable_template_key 提取 overload_sig（canonical 文本中的 sig= 部分）。
            let overload_sig = stable_template_key
                .as_ref()
                .map(|stk| extract_overload_sig(&stk.canonical))
                .unwrap_or_default();
            reqs.push(InstanceKey {
                template_fqn: callee_fqn.clone(),
                overload_sig,
                type_args: generic_type_args.clone(),
            });
        }
        CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. } => {
            let overload_sig = dispatch
                .stable_template_key
                .as_ref()
                .map(|stk| extract_overload_sig(&stk.canonical))
                .unwrap_or_default();
            reqs.push(InstanceKey {
                template_fqn: dispatch.member_fqn.clone(),
                overload_sig,
                type_args: dispatch.generic_type_args.clone(),
            });
        }
        // 闭包调用：同 MakeClosure——闭包随外层 family 实例化，不独立入队。
        CallKind::Closure { .. } => {
        }
        // FunValue 调用：callee 是函数值 local，无静态 FQN 可扫描。
        // 若该 local 绑定到一个已知闭包，其 invoke_fqn 已在 MakeClosure 处入队。
        CallKind::FunValue { .. } => {}
        // Resume 调用：continuation 是 continuation 对象，不产生新实例。
        CallKind::Resume { .. } => {}
    }
}

/// 从 canonical 文本 `template(fqn;[...];sig=...)` 中提取 overload_sig 部分。
fn extract_overload_sig(canonical: &str) -> String {
    if let Some(pos) = canonical.find("sig=") {
        canonical[pos + 4..].to_string()
    } else {
        String::new()
    }
}

fn scan_terminator_calls(kind: &TerminatorKind, reqs: &mut Vec<InstanceKey>) {
    let _ = kind;
    // Perform / Handle 终结符不产生新 callable 实例（它们是 effect 终结符，不是函数调用）。
}

// ---------------------------------------------------------------------------
// 泛型参数残留检测
// ---------------------------------------------------------------------------

/// 检查 materialized body 中是否还有残留的 TypeKind::Param（泛型参数未替换）。
pub fn check_no_generic_param_residue(
    store: &TypeStore,
    body: &Body,
) -> Result<(), MonomorphError> {
    for decl in &body.locals {
        if matches!(store.kind(decl.ty), TypeKind::Param(_)) {
            return Err(MonomorphError::error(
                scoop2_base::Span::default(),
                format!(
                    "单态化后仍有泛型参数残留：local {:?} 的类型 {:?}",
                    decl.name, decl.ty
                ),
            ));
        }
    }
    Ok(())
}

/// 从模块 MetadataRoot 的 FQN 文本查找 HIR 中的 Symbol。
fn hir_fqn_for_metadata(
    hir: &scoop2_hir::hir::TypedHir,
    fqn_text: &str,
) -> Option<scoop2_base::Symbol> {
    // 用 interner 查找 FQN 文本对应的 Symbol。
    hir.interner.get(fqn_text)
}

/// 收集 class 的虚方法槽（继承感知 + 确定性顺序）：
/// - 超类链（仅 class，自顶向下）先声明的方法占前面的 slot；
/// - 子类 override 同 (方法名, overload sig) 的方法时保留原 slot 位置、
///   target owner 换成本级实现（保证子类 vtable 与超类 vtable 前缀布局一致）；
/// - 子类新增方法 / 新 overload 按声明序追加在后。
///
/// 方法名迭代走 HIR `ordered_member_fun_names`（声明序侧表），避免
/// `member_funs` HashMap 迭代序不确定导致 vtable slot 逐次构建不一致。
fn collect_class_virtual_methods(
    hir: &scoop2_hir::hir::TypedHir,
    store: &TypeStore,
    class_fqn_text: &str,
) -> Vec<(String, String, String)> {
    let Some(class_sym) = hir.interner.get(class_fqn_text) else {
        return Vec::new();
    };
    // 超类链（仅 class）：自顶向下排列（链首 = 最顶层超类，链尾 = 本 class）。
    let mut chain: Vec<scoop2_base::Symbol> = Vec::new();
    let mut cur = class_sym;
    let mut visited = std::collections::HashSet::new();
    while visited.insert(cur) {
        let next = hir
            .supertypes
            .get(&cur)
            .and_then(|supers| supers.iter().find(|s| hir.class_fqns.contains(s)).copied());
        match next {
            Some(sup) => {
                chain.push(sup);
                cur = sup;
            }
            None => break,
        }
    }
    let mut owners: Vec<scoop2_base::Symbol> = chain.iter().rev().copied().collect();
    owners.push(class_sym);
    let mut slots: Vec<(String, String, String)> = Vec::new();
    for owner_sym in owners {
        let owner_text = hir.interner.resolve(owner_sym).to_string();
        for name_sym in hir.ordered_member_fun_names(&owner_sym) {
            let Some(sigs) = hir
                .member_funs
                .get(&owner_sym)
                .and_then(|m| m.get(&name_sym))
            else {
                continue;
            };
            let mname = hir.interner.resolve(name_sym).to_string();
            for sig in sigs {
                let sig_canonical = crate::mir::stable_id::build_overload_sig(
                    store,
                    &hir.interner,
                    &sig.param_types,
                );
                if let Some(existing) = slots
                    .iter_mut()
                    .find(|(n, _, s)| *n == mname && *s == sig_canonical)
                {
                    // override：保留超类 slot 位置，target 换成本级实现。
                    existing.1 = owner_text.clone();
                } else {
                    slots.push((mname.clone(), owner_text.clone(), sig_canonical));
                }
            }
        }
    }
    slots
}
