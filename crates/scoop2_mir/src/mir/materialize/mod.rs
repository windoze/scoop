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

use std::collections::{HashMap, VecDeque};

use scoop2_hir::ty::{Subst, TypeId, TypeKind, TypeParamType, TypeStore};

use crate::diagnostics::MonomorphError;
use crate::mir::{Body, CallKind, FunDecl, Item, Module, Rvalue, StatementKind, TerminatorKind};

/// 单态化实例化键：模板 FQN + 类型实参。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub template_fqn: String,
    pub type_args: Vec<TypeId>,
}

/// 语言级 backend contract 发布（不含 LLVM-specific 信息）。
///
/// 携带 per-type 的方法槽位映射和成员布局信息，供后端生成 vtable/itable/layout。
#[derive(Clone, Debug, Default)]
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
#[derive(Clone, Debug)]
pub struct ClassVtableContract {
    pub class_fqn: String,
    /// 虚方法：方法名 + owner FQN。
    pub virtual_methods: Vec<(String, String)>,
}

/// interface 契约：interface 的方法签名列表。
#[derive(Clone, Debug)]
pub struct InterfaceContract {
    pub interface_fqn: String,
    /// 方法名列表。
    pub methods: Vec<String>,
}

/// class→interface itable 契约。
#[derive(Clone, Debug)]
pub struct ClassItableContract {
    pub class_fqn: String,
    pub interface_fqns: Vec<String>,
}

/// enum layout 契约。
#[derive(Clone, Debug)]
pub struct EnumLayoutContract {
    pub enum_fqn: String,
    /// variant 名列表。
    pub variants: Vec<String>,
}

/// struct layout 契约。
#[derive(Clone, Debug)]
pub struct StructLayoutContract {
    pub struct_fqn: String,
    /// 字段名 + 类型 FQN。
    pub fields: Vec<(String, String)>,
}

/// class init 契约。
#[derive(Clone, Debug)]
pub struct ClassInitContract {
    pub class_fqn: String,
}

/// ctor call site 契约。
#[derive(Clone, Debug)]
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
    let mut work: Materializer = Materializer {
        store: generic.types.clone(),
        templates,
        instances: HashMap::new(),
        order: Vec::new(),
        queue: VecDeque::new(),
        seen: HashMap::new(),
        backend_contracts: BackendContracts::default(),
    };
    // 种子：entry 函数（无类型实参）或所有非泛型函数。
    if let Some(entry) = entry_fqn {
        let key = InstanceKey {
            template_fqn: entry.to_string(),
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
                type_args: Vec::new(),
            })
            .collect();
        for s in seeds {
            work.enqueue(s);
        }
    }
    work.run()?;
    // 构造 materialized module。
    let mut items: Vec<Item> = Vec::new();
    for key in &work.order {
        if let Some(fds) = work.instances.get(key) {
            for fd in fds {
                items.push(Item::Fun(fd.clone()));
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
                    crate::mir::MetadataKind::Class => {
                        // class vtable 契约：从 HIR member_funs 收集虚方法。
                        let virtual_methods: Vec<(String, String)> = hir_fqn_for_metadata(hir, &m.fqn)
                            .and_then(|fqn_sym| hir.member_funs.get(&fqn_sym))
                            .map(|methods| {
                                methods.iter()
                                    .flat_map(|(method_name, sigs)| {
                                        sigs.iter().filter(|s| {
                                            // open/abstract/override 方法才是虚方法。
                                            // 当前 MIR 不携带修饰符信息；保守收集全部。
                                            true
                                        }).map(move |_| {
                                            (hir.interner.resolve(*method_name).to_string(), m.fqn.clone())
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        work.backend_contracts.class_vtables.push(ClassVtableContract {
                            class_fqn: m.fqn.clone(),
                            virtual_methods,
                        });
                        work.backend_contracts.class_inits.push(ClassInitContract {
                            class_fqn: m.fqn.clone(),
                        });
                    }
                    crate::mir::MetadataKind::Interface => {
                        let methods: Vec<String> = hir_fqn_for_metadata(hir, &m.fqn)
                            .and_then(|fqn_sym| hir.member_funs.get(&fqn_sym))
                            .map(|mf| mf.keys().map(|k| hir.interner.resolve(*k).to_string()).collect())
                            .unwrap_or_default();
                        work.backend_contracts.interfaces.push(InterfaceContract {
                            interface_fqn: m.fqn.clone(),
                            methods,
                        });
                    }
                    crate::mir::MetadataKind::Enum => {
                        let variants: Vec<String> = hir_fqn_for_metadata(hir, &m.fqn)
                            .and_then(|fqn_sym| hir.enum_variants.get(&fqn_sym))
                            .map(|vs| vs.iter().map(|v| hir.interner.resolve(*v).to_string()).collect())
                            .unwrap_or_default();
                        work.backend_contracts.enum_layouts.push(EnumLayoutContract {
                            enum_fqn: m.fqn.clone(),
                            variants,
                        });
                    }
                    crate::mir::MetadataKind::Struct => {
                        let fields: Vec<(String, String)> = hir_fqn_for_metadata(hir, &m.fqn)
                            .and_then(|fqn_sym| hir.members.get(&fqn_sym))
                            .map(|ms| ms.iter()
                                .map(|(name, ty)| {
                                    let ty_text = crate::mir::stable_id::canonical_type_text(
                                        &work.store, *ty,
                                    );
                                    (hir.interner.resolve(*name).to_string(), ty_text)
                                })
                                .collect())
                            .unwrap_or_default();
                        work.backend_contracts.struct_layouts.push(StructLayoutContract {
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
        types: work.store,
    };
    // 去虚化 pass：final 接收者的 Virtual 调用改写为 Direct。
    crate::mir::devirtualize::devirtualize_module(&mut result_module);
    Ok(MaterializedMir {
        module: result_module,
        instance_keys: work.order,
        backend_contracts: work.backend_contracts,
    })
}

struct Materializer {
    store: TypeStore,
    templates: HashMap<String, Vec<FunDecl>>,
    instances: HashMap<InstanceKey, Vec<FunDecl>>,
    order: Vec<InstanceKey>,
    queue: VecDeque<InstanceKey>,
    seen: HashMap<InstanceKey, bool>,
    backend_contracts: BackendContracts,
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
        let subst = build_subst(&templates, &key.type_args);
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
                scan_body_calls(body, &mut raw_reqs);
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

// ---------------------------------------------------------------------------
// 模板收集 / 泛型检测
// ---------------------------------------------------------------------------

fn collect_templates(module: &Module) -> HashMap<String, Vec<FunDecl>> {
    let mut map: HashMap<String, Vec<FunDecl>> = HashMap::new();
    for item in &module.items {
        if let Item::Fun(fd) = item {
            map.entry(fd.fqn.clone()).or_default().push(fd.clone());
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

/// 从模板的 type_params（类型参数名序列）按声明顺序绑定到 type_args。
fn build_subst(templates: &[FunDecl], type_args: &[TypeId]) -> Subst {
    let mut subst = Subst::new();
    if let Some(first) = templates.first() {
        for (i, &tp_sym) in first.type_params.iter().enumerate() {
            if let Some(&arg_ty) = type_args.get(i) {
                // TypeParamType 需要 name + file + span；file/span 用占位（不影响替换逻辑）。
                let tp = TypeParamType {
                    name: tp_sym,
                    file: scoop2_base::FileId(0),
                    span: scoop2_base::Span::default(),
                };
                subst.insert(tp, arg_ty);
            }
        }
    }
    subst
}

// ---------------------------------------------------------------------------
// 类型替换（覆盖全部 body：rvalue / statement / terminator / metadata）
// ---------------------------------------------------------------------------

fn subst_fun_decl(mut fd: FunDecl, subst: &Subst, store: &mut TypeStore) -> FunDecl {
    fd.ty = store.apply_subst(fd.ty, subst);
    fd.return_ty = store.apply_subst(fd.return_ty, subst);
    for p in &mut fd.params {
        p.ty = store.apply_subst(p.ty, subst);
    }
    if let Some(body) = fd.body.take() {
        fd.body = Some(subst_body(body, subst, store));
    }
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
        StatementKind::StoreMember { member, value_ty, .. } => {
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
        Rvalue::Use(_) | Rvalue::UnresolvedName { .. } | Rvalue::ClassLit { .. } => {}
        Rvalue::TopLevelRef(tl) => {
            for t in &mut tl.generic_type_args {
                *t = store.apply_subst(*t, subst);
            }
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
        Rvalue::TupleIndex { element_ty, .. } | Rvalue::IndexAccess { element_ty, .. } => {
            *element_ty = store.apply_subst(*element_ty, subst);
        }
        Rvalue::EnumVariant { enum_ty, payload, .. } => {
            *enum_ty = store.apply_subst(*enum_ty, subst);
            subst_aggregate_transport(payload, subst, store);
        }
        Rvalue::ClassCtor { ctor, hidden_effects, args, .. } => {
            for a in args.iter_mut() {
                a.value_ty = store.apply_subst(a.value_ty, subst);
            }
            let _ = (ctor, hidden_effects);
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
        }
        Rvalue::MakeTuple { transport, .. } | Rvalue::StructLit { transport, .. } => {
            subst_aggregate_transport(transport, subst, store);
        }
        Rvalue::MakeArray { result_ty, .. } | Rvalue::WithUpdate { result_ty, .. } => {
            *result_ty = store.apply_subst(*result_ty, subst);
        }
        Rvalue::MakeClosure { env_contract, .. } => {
            env_contract.env_ty = store.apply_subst(env_contract.env_ty, subst);
            for cap in &mut env_contract.captures {
                subst_value_transport(&mut cap.transport, subst, store);
            }
        }
        Rvalue::InterpolatedString { .. } | Rvalue::PerformResult { .. } | Rvalue::PatternMatch { .. } | Rvalue::PatternExtract { .. } => {}
    }
}

fn subst_call_kind(kind: &mut CallKind, subst: &Subst, store: &mut TypeStore) {
    match kind {
        CallKind::Direct {
            generic_type_args, ..
        } => {
            for t in generic_type_args {
                *t = store.apply_subst(*t, subst);
            }
        }
        CallKind::Virtual { dispatch, .. } => {
            dispatch.receiver_ty = store.apply_subst(dispatch.receiver_ty, subst);
            for t in &mut dispatch.generic_type_args {
                *t = store.apply_subst(*t, subst);
            }
        }
        CallKind::Closure { .. } | CallKind::FunValue { .. } => {}
    }
}

fn subst_terminator(term: &mut crate::mir::Terminator, subst: &Subst, store: &mut TypeStore) {
    match &mut term.kind {
        TerminatorKind::Return { value: Some(op) } => {
            // Operand 不含 TypeId；无需替换（local 的类型已替换）。
            let _ = op;
        }
        TerminatorKind::Perform {
            metadata, args, ..
        } => {
            metadata.effect_ty = store.apply_subst(metadata.effect_ty, subst);
            metadata.result_ty = store.apply_subst(metadata.result_ty, subst);
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

fn subst_value_transport(vt: &mut crate::mir::ValueTransportMetadata, subst: &Subst, store: &mut TypeStore) {
    vt.source_ty = store.apply_subst(vt.source_ty, subst);
    if let Some(b) = &mut vt.boxing {
        b.source_ty = store.apply_subst(b.source_ty, subst);
        if let Some(t) = b.target_ty {
            b.target_ty = Some(store.apply_subst(t, subst));
        }
    }
}

fn subst_aggregate_transport(at: &mut crate::mir::AggregateTransportMetadata, subst: &Subst, store: &mut TypeStore) {
    at.aggregate_ty = store.apply_subst(at.aggregate_ty, subst);
    for f in &mut at.fields {
        f.ty = store.apply_subst(f.ty, subst);
        subst_value_transport(&mut f.transport, subst, store);
    }
}

fn subst_type_test_metadata(m: &mut crate::mir::RuntimeTypeTestMetadata, subst: &Subst, store: &mut TypeStore) {
    m.source_ty = store.apply_subst(m.source_ty, subst);
    m.target_ty = store.apply_subst(m.target_ty, subst);
    m.descriptor.ty = store.apply_subst(m.descriptor.ty, subst);
}

fn subst_cast_metadata(m: &mut crate::mir::RuntimeCastMetadata, subst: &Subst, store: &mut TypeStore) {
    subst_type_test_metadata(&mut m.test, subst, store);
}

fn subst_member_access_metadata(m: &mut crate::mir::MemberAccessMetadata, subst: &Subst, store: &mut TypeStore) {
    m.receiver_ty = store.apply_subst(m.receiver_ty, subst);
}

// ---------------------------------------------------------------------------
// 可达性扫描（递归进实例化后的 body）
// ---------------------------------------------------------------------------

fn scan_body_calls(body: &Body, reqs: &mut Vec<InstanceKey>) {
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let StatementKind::Assign { value, .. } = &stmt.kind {
                scan_rvalue_calls(value, reqs);
            }
        }
        scan_terminator_calls(&block.terminator.kind, reqs);
    }
}

fn scan_rvalue_calls(rv: &Rvalue, reqs: &mut Vec<InstanceKey>) {
    match rv {
        Rvalue::Call { kind, args, .. } => {
            scan_call_kind(kind, reqs);
            // 递归进 CallArg（嵌套调用）。
            let _ = args;
        }
        _ => {}
    }
}

fn scan_call_kind(kind: &CallKind, reqs: &mut Vec<InstanceKey>) {
    match kind {
        CallKind::Direct {
            callee_fqn,
            generic_type_args,
            ..
        } => {
            reqs.push(InstanceKey {
                template_fqn: callee_fqn.clone(),
                type_args: generic_type_args.clone(),
            });
        }
        CallKind::Virtual { dispatch, .. } => {
            // virtual 调用的目标在运行期决定；但 owner_fqn.member_name 可作为候选。
            reqs.push(InstanceKey {
                template_fqn: dispatch.member_fqn.clone(),
                type_args: dispatch.generic_type_args.clone(),
            });
        }
        _ => {}
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
