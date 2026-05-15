use std::collections::BTreeMap;

use crate::mir::{
    ExternGlobalRoot as MirExternGlobalRoot, File as MirFile, FunDecl as MirFunDecl,
    InitializerRoot as MirInitializerRoot, Item as MirItem, LoweredMir, MaterializedMir,
    MetadataRoot as MirMetadataRoot, MirLowerError, MirLoweringFacts,
    lower_hir_file_for_dump_with_facts,
};
use crate::ty::{TypeId, TypeStore};

use super::{TypedHirEffectContracts, TypedHirStageOutput};

/// direct-style MIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P3/P4 及后续阶段直接消费：
/// - `lowered_mir` 仍是 direct-style MIR，而不是 late-lowered `Step` IR；
/// - 当前所有 effect-sensitive site 继续通过 MIR 节点上的 `SiteId` 锚定；
/// - `effect_contracts` 保留这次 lowering 消费过的 P2 typed HIR handoff，便于测试/审计；
///   canonical 的 site-level contract 现已下沉到 MIR 节点 metadata；P4 可以把它用于审计，
///   但不得把它当成重新解释 `Call / Perform / Resume / Handle` 语义的 source of truth；
/// - `callable_body_indices` 与可选的 `materialized_mir` 把 P4 会消费的 canonical MIR
///   handoff 显式挂在 stage 输出上，而不是继续藏在 `LoweredHir` 私有字段或 dump helper 里。
/// - P4 的 authoritative 输入是这份 stage 输出上的 callable body 身份、可选
///   `materialized_mir` 快照，以及 MIR 节点上的 `SiteId` / metadata；P4 不得回看 P2 原始
///   HIR side tables 重新猜测 site contract。
/// - 本 stage 仍未提供 `StepSchema`、`ContinuationSchema` 或 `MaterializedEffectFacts`；这些属于
///   P4/P5 的职责，而不是 P3 dump / stage 输出应提前伪造的内容。
#[derive(Debug)]
pub struct MirStageOutput {
    lowered_mir: LoweredMir,
    effect_contracts: TypedHirEffectContracts,
    callable_body_indices: BTreeMap<String, usize>,
    initializer_root_indices: BTreeMap<String, usize>,
    global_root_indices: BTreeMap<String, usize>,
    metadata_root_indices: BTreeMap<String, usize>,
    materialized_mir: Option<MaterializedMir>,
}

impl MirStageOutput {
    pub(crate) fn new(
        lowered_mir: LoweredMir,
        effect_contracts: TypedHirEffectContracts,
        materialized_mir: Option<MaterializedMir>,
    ) -> Self {
        let callable_body_indices = collect_callable_body_indices(&lowered_mir.file);
        let initializer_root_indices = collect_initializer_root_indices(&lowered_mir.file);
        let global_root_indices = collect_global_root_indices(&lowered_mir.file);
        let metadata_root_indices = collect_metadata_root_indices(&lowered_mir.file);
        Self {
            callable_body_indices,
            initializer_root_indices,
            global_root_indices,
            metadata_root_indices,
            lowered_mir,
            effect_contracts,
            materialized_mir,
        }
    }

    pub fn file(&self) -> &MirFile {
        &self.lowered_mir.file
    }

    pub fn types(&self) -> &TypeStore {
        &self.lowered_mir.types
    }

    pub fn effect_contracts(&self) -> &TypedHirEffectContracts {
        &self.effect_contracts
    }

    /// 返回当前 stage 输出上显式挂住的 canonical materialized MIR 快照（若存在）。
    pub fn materialized_mir(&self) -> Option<&MaterializedMir> {
        self.materialized_mir.as_ref()
    }

    pub(crate) fn materialized_mir_mut(&mut self) -> Option<&mut MaterializedMir> {
        self.materialized_mir.as_mut()
    }

    pub(crate) fn with_materialized_mir(mut self, materialized_mir: MaterializedMir) -> Self {
        self.materialized_mir = Some(materialized_mir);
        self
    }

    /// 以稳定顺序枚举当前 direct-style MIR 中可查询的 callable body 身份。
    pub fn callable_body_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.callable_body_indices.keys().map(String::as_str)
    }

    /// 按 callable body 身份查询 canonical direct-style MIR body。
    pub fn callable_body(&self, fqn: &str) -> Option<&MirFunDecl> {
        let item_index = *self.callable_body_indices.get(fqn)?;
        match self.file().items.get(item_index)? {
            MirItem::Fun(fun) if fun.body.is_some() => Some(fun),
            _ => None,
        }
    }

    /// 以稳定顺序枚举 MIR-owned top-level initializer/value roots。
    pub fn initializer_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.initializer_root_indices.keys().map(String::as_str)
    }

    /// 按 root FQN 查询 top-level initializer/value root。
    pub fn initializer_root(&self, fqn: &str) -> Option<&MirInitializerRoot> {
        let item_index = *self.initializer_root_indices.get(fqn)?;
        match self.file().items.get(item_index)? {
            MirItem::InitializerRoot(root) => Some(root),
            _ => None,
        }
    }

    /// 以稳定顺序枚举 MIR-owned global/extern roots。
    pub fn global_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.global_root_indices.keys().map(String::as_str)
    }

    /// 按 FQN 查询 `@Extern` global root contract。
    pub fn extern_global_root(&self, fqn: &str) -> Option<&MirExternGlobalRoot> {
        let item_index = *self.global_root_indices.get(fqn)?;
        match self.file().items.get(item_index)? {
            MirItem::ExternGlobal(root) => Some(root),
            _ => None,
        }
    }

    /// 以稳定顺序枚举 MIR-owned type/object/typealias metadata roots。
    pub fn metadata_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.metadata_root_indices.keys().map(String::as_str)
    }

    /// 按 FQN 查询 MIR declaration metadata root。
    pub fn metadata_root(&self, fqn: &str) -> Option<&MirMetadataRoot> {
        let item_index = *self.metadata_root_indices.get(fqn)?;
        match self.file().items.get(item_index)? {
            MirItem::Metadata(root) => Some(root),
            _ => None,
        }
    }

    /// refactor `dump-mir` / `mir_refactor` fixtures / 定向单测共用的稳定文本 surface。
    ///
    /// P3-T04 起，这个 formatter 就是 refactor direct-style MIR 的 snapshot/golden 基线：
    /// - 必须稳定暴露 direct-style MIR body / CFG；
    /// - 必须保留 `SiteId`、cleanup/finally target，以及 `Call / Perform / Resume / Handle`
    ///   的关键 metadata；
    /// - 不能在 CLI、fixture runner、或单测之间各自拼接不同文本。
    pub fn stable_dump(&self) -> String {
        let mut out = crate::mir::stable_dump_file(self.file(), self.types());
        out.push('\n');
        out
    }

    pub fn into_lowered_mir(self) -> LoweredMir {
        self.lowered_mir
    }
}

fn collect_callable_body_indices(file: &MirFile) -> BTreeMap<String, usize> {
    let mut indices = BTreeMap::new();
    for (item_index, item) in file.items.iter().enumerate() {
        let MirItem::Fun(fun) = item else {
            continue;
        };
        if fun.body.is_none() {
            continue;
        }
        indices.entry(fun.fqn.clone()).or_insert(item_index);
    }
    indices
}

fn collect_initializer_root_indices(file: &MirFile) -> BTreeMap<String, usize> {
    let mut indices = BTreeMap::new();
    for (item_index, item) in file.items.iter().enumerate() {
        let MirItem::InitializerRoot(root) = item else {
            continue;
        };
        indices.entry(root.fqn.clone()).or_insert(item_index);
    }
    indices
}

fn collect_global_root_indices(file: &MirFile) -> BTreeMap<String, usize> {
    let mut indices = BTreeMap::new();
    for (item_index, item) in file.items.iter().enumerate() {
        let MirItem::ExternGlobal(root) = item else {
            continue;
        };
        indices.entry(root.fqn.clone()).or_insert(item_index);
    }
    indices
}

fn collect_metadata_root_indices(file: &MirFile) -> BTreeMap<String, usize> {
    let mut indices = BTreeMap::new();
    for (item_index, item) in file.items.iter().enumerate() {
        let MirItem::Metadata(root) = item else {
            continue;
        };
        indices.entry(root.fqn().to_string()).or_insert(item_index);
    }
    indices
}

fn validate_refactor_bodies(file: &MirFile, unit_ty: TypeId) -> Result<(), MirLowerError> {
    file.validate_refactor_production(unit_ty)
        .map_err(|error| MirLowerError::InvalidRefactorMir {
            fqn: error.refactor_body_fqn().unwrap_or("<file>").to_string(),
            error,
        })
}

fn lower_refactor_mir_stage_unvalidated(
    typed_hir_output: TypedHirStageOutput,
) -> (MirStageOutput, TypeId) {
    let facts = MirLoweringFacts::from_refactor_typed_handoff(
        typed_hir_output.lowered_hir(),
        typed_hir_output.effect_contracts(),
    );
    let effect_contracts = typed_hir_output.effect_contracts().clone();
    let mut lowered_hir = typed_hir_output.into_lowered_hir();
    let builtins = lowered_hir.types.intern_builtins();
    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let types = std::mem::replace(&mut lowered_hir.types, TypeStore::new());
    let materialized_mir = lowered_hir.into_materialized_mir();

    (
        MirStageOutput::new(
            LoweredMir { file, types },
            effect_contracts,
            materialized_mir,
        ),
        builtins.unit,
    )
}

pub(crate) fn run(typed_hir_output: TypedHirStageOutput) -> Result<MirStageOutput, MirLowerError> {
    let (output, unit_ty) = lower_refactor_mir_stage_unvalidated(typed_hir_output);
    validate_refactor_bodies(output.file(), unit_ty)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::MirStageOutput;
    use crate::ast;
    use crate::mir::{
        AggregateTransportKind, ArrayTransportOperation, CallKind, GcIntrinsicOperation,
        GcIntrinsicPairing, GcRootLifetime, HandlerArmKind, InitializerDependencyKind,
        InitializerRootKind, Item, MemberTarget, MetadataRoot, MirBoxingReason, MirCallableAbiKind,
        MirCallableImplPlan, MirLowerError, MirLoweringFacts, MirSiteMetadataKind,
        MirTransportKind, MirValidationError, Operand, Pattern, RuntimeCastFailure,
        RuntimeCastResult, RuntimePatternTypeTestKind, RuntimeTypeDescriptorKind,
        RuntimeTypeParameterizedMatch, RuntimeTypeStaticFold, Rvalue, StatementKind,
        TerminatorKind, UnwindAction, ValueTransportMetadata, lower_hir_file_for_dump_with_facts,
    };
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::ty::TypeStore;
    use std::path::PathBuf;

    use super::super::TypedHirEffectContracts;

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    fn run_fixture(phase: &str, name: &str) -> MirStageOutput {
        let session = refactor_session();
        let source = load_fixture(phase, name);
        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap();
        super::run(typed_hir_output).expect("fixture 应可通过 refactor MIR stage")
    }

    fn callable_body<'a>(output: &'a MirStageOutput, fqn: &str) -> &'a crate::mir::Body {
        output
            .callable_body(fqn)
            .and_then(|fun| fun.body.as_ref())
            .unwrap_or_else(|| panic!("应找到 callable body: {fqn}"))
    }

    fn validated_callable_body<'a>(output: &'a MirStageOutput, fqn: &str) -> &'a crate::mir::Body {
        let body = callable_body(output, fqn);
        body.validate_refactor_direct_style()
            .unwrap_or_else(|err| panic!("refactor MIR body `{fqn}` 应通过验证器: {err}"));
        body
    }

    fn unit_operand_is_visible_in_body(
        output: &MirStageOutput,
        body: &crate::mir::Body,
        operand: &Operand,
    ) -> bool {
        match operand {
            Operand::Const(crate::mir::ConstValue::Unit) => true,
            Operand::Local(local) => {
                output
                    .types()
                    .display(body.locals[local.as_u32() as usize].ty)
                    .to_string()
                    == "Unit"
            }
            Operand::Const(_) => false,
        }
    }

    #[test]
    fn refactor_direct_mir_stage_output_is_constructible() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        );

        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap();
        let output = super::run(typed_hir_output).unwrap();

        assert_eq!(output.file().items.len(), 2);
        assert!(output.callable_body("sample.helper").is_some());
        assert!(output.callable_body("sample.main").is_some());
        assert_eq!(output.effect_contracts().function_effects().len(), 2);
        assert!(output.stable_dump().contains("FunDecl"));
    }

    #[test]
    fn refactor_mir_stable_dump_normalizes_workspace_source_paths() {
        let session = refactor_session();
        let source = load_fixture("mir_refactor", "top_level_roots.scoop");

        let output = super::super::load_direct_style_mir_stage_output_for_dump(&session, &source)
            .expect("top-level roots fixture should produce strict refactor MIR");
        let dump = output.stable_dump();

        assert!(
            dump.contains("source_path: \"tests/fixtures/mir_refactor/top_level_roots.scoop\""),
            "stable dump should use workspace-relative source paths: {dump}"
        );
        assert!(
            !dump.contains(env!("CARGO_MANIFEST_DIR")),
            "stable dump must not embed machine-local manifest paths: {dump}"
        );
    }

    #[test]
    fn refactor_mir_stable_dump_uses_semantic_types_and_stable_labels() {
        let session = refactor_session();
        let source = load_fixture("mir", "direct_zero_arg_call.scoop");

        let output = super::super::load_direct_style_mir_stage_output_for_dump(&session, &source)
            .expect("direct zero-arg fixture should produce MIR");
        let dump = output.stable_dump();

        for forbidden in ["TypeId(", "bb0", "site0", "l0"] {
            assert!(
                !dump.contains(forbidden),
                "stable dump must not leak allocator-derived token `{forbidden}`: {dump}"
            );
        }
        assert!(
            dump.contains("site_id: site#h"),
            "stable dump should render stable site labels: {dump}"
        );
        assert!(
            dump.contains("ty: Int"),
            "stable dump should render semantic type text: {dump}"
        );
    }

    #[test]
    fn refactor_mir_item_graph_publishes_top_level_roots() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_mir_item_graph.scoop",
            r#"package sample
import scoop.core.*

typealias Alias = Int
struct Point(val x: Int)

const val Base: Int = 1
val Runtime: Int = Base + 1

@Global
var Counter: Int = Runtime

@Extern(name = "native_counter")
var NativeCounter: Int

object Registry {
    val count: Int = Runtime
    fun touch(): Int { return 0 }
}

fun main() {}
"#,
        );

        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap();
        let output = super::run(typed_hir_output).unwrap();

        assert!(
            output
                .file()
                .items
                .iter()
                .all(|item| !matches!(item, Item::Todo { .. })),
            "refactor MIR item graph must not contain top-level declaration Todo: {:#?}",
            output.file()
        );

        let initializer_fqns = output.initializer_root_fqns().collect::<Vec<_>>();
        for expected in [
            "sample.Base",
            "sample.Counter",
            "sample.Runtime",
            "sample.Registry",
        ] {
            assert!(
                initializer_fqns.contains(&expected),
                "missing initializer root `{expected}` in {initializer_fqns:?}"
            );
        }

        let runtime = output.initializer_root("sample.Runtime").unwrap();
        assert_eq!(runtime.kind, InitializerRootKind::RuntimeImmutableVal);
        assert!(runtime.dependencies.iter().any(|dependency| {
            dependency.fqn == "sample.Base"
                && dependency.kind == InitializerDependencyKind::TopLevelValue
        }));

        let registry = output.initializer_root("sample.Registry").unwrap();
        assert_eq!(registry.kind, InitializerRootKind::ObjectSingleton);
        assert!(registry.dependencies.iter().any(|dependency| {
            dependency.fqn == "sample.Runtime"
                && dependency.kind == InitializerDependencyKind::TopLevelValue
        }));

        let native = output.extern_global_root("sample.NativeCounter").unwrap();
        assert_eq!(native.symbol, "native_counter");
        assert!(native.initializer_absent);
        assert!(
            output
                .global_root_fqns()
                .any(|fqn| fqn == "sample.NativeCounter")
        );

        assert!(matches!(
            output.metadata_root("sample.Alias"),
            Some(MetadataRoot::TypeAlias(alias)) if alias.name == "Alias"
        ));
        assert!(matches!(
            output.metadata_root("sample.Point"),
            Some(MetadataRoot::Nominal(nominal)) if nominal.name == "Point"
        ));
        assert!(matches!(
            output.metadata_root("sample.Registry"),
            Some(MetadataRoot::Object(object)) if object.initializer_root == "sample.Registry"
        ));
        assert!(
            output
                .metadata_root_fqns()
                .any(|fqn| fqn == "sample.Registry")
        );
        assert!(output.callable_body("sample.Registry.touch").is_some());
        assert!(output.callable_body("sample.main").is_some());
    }

    #[test]
    fn refactor_mir_place_contract_lowers_assignment_places() {
        let output = run_fixture("mir_refactor", "assignment_places.scoop");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("Todo"),
            "refactor MIR assignment place lowering must not leak Todo placeholders: {dump}"
        );

        let native = output
            .extern_global_root("mir_refactor.assignment_places.NativeCounter")
            .expect("extern global root should be published");
        assert!(native.unsafe_required);

        let body = validated_callable_body(&output, "mir_refactor.assignment_places.use");
        let mut saw_global_store = false;
        let mut saw_extern_store = false;
        let mut saw_capture_box_new = false;
        let mut saw_capture_box_set = false;
        let mut box_value_store_count = 0usize;

        for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
            match &stmt.kind {
                StatementKind::StoreTopLevelVar { fqn, .. }
                    if fqn == "mir_refactor.assignment_places.G" =>
                {
                    saw_global_store = true;
                }
                StatementKind::StoreTopLevelVar { fqn, .. }
                    if fqn == "mir_refactor.assignment_places.NativeCounter" =>
                {
                    saw_extern_store = true;
                }
                StatementKind::Assign {
                    value: Rvalue::CaptureBoxNew { .. },
                    ..
                } => {
                    saw_capture_box_new = true;
                }
                StatementKind::Assign {
                    value: Rvalue::CaptureBoxSet { .. },
                    ..
                } => {
                    saw_capture_box_set = true;
                }
                StatementKind::StoreMember { member, .. }
                    if matches!(
                        member.resolved.as_ref(),
                        Some(MemberTarget::Value { fqn })
                            if fqn == "mir_refactor.assignment_places.Box.value"
                    ) =>
                {
                    box_value_store_count += 1;
                }
                _ => {}
            }
        }

        assert!(saw_global_store, "top-level var store missing: {dump}");
        assert!(saw_extern_store, "extern global store missing: {dump}");
        assert!(
            saw_capture_box_new && saw_capture_box_set,
            "boxed mutable local should lower to explicit capture-box new/set: {dump}"
        );
        assert!(
            box_value_store_count >= 2,
            "direct and nested member stores should target Box.value: {dump}"
        );
    }

    #[test]
    fn refactor_mir_place_contract_rejects_invalid_inputs_before_mir() {
        let session = refactor_session();
        for (name, source, expected) in [
            (
                "local_missing_initializer",
                "package sample\nimport scoop.core.*\nfun main() { var x: Int }\n",
                "局部 val/var（缺少 initializer）",
            ),
            (
                "assignment_call_lhs",
                "package sample\nimport scoop.core.*\nfun make(): Int { return 0 }\nfun main() { make() = 1 }\n",
                "可赋值的左值（标识符或成员访问）",
            ),
            (
                "break_not_in_loop",
                "package sample\nimport scoop.core.*\nfun main() { break }\n",
                "BreakNotInLoop",
            ),
            (
                "continue_not_in_loop",
                "package sample\nimport scoop.core.*\nfun main() { continue }\n",
                "ContinueNotInLoop",
            ),
        ] {
            let source = SourceFile::new_virtual(format!("<mem>/{name}.scoop"), source);
            let err =
                super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap_err();
            let report = format!("{err:?}");
            assert!(
                report.contains(expected),
                "expected diagnostic `{expected}` for {name}, got: {report}"
            );
        }
    }

    #[test]
    fn refactor_mir_call_contract_lowers_typed_call_sites() {
        let output = run_fixture("mir_refactor", "call_contracts.scoop");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("Todo"),
            "refactor MIR call lowering must not leak Todo placeholders: {dump}"
        );

        let main = validated_callable_body(&output, "mir_refactor.call_contracts.main");
        let apply = validated_callable_body(&output, "mir_refactor.call_contracts.apply");
        let mut direct_fqns = Vec::new();
        let mut saw_get_platform = false;
        let mut saw_class_ctor = false;
        let mut saw_size_of = false;
        let mut saw_name_of_metadata = false;
        let mut saw_closure_call = false;
        let mut saw_named_default_call = false;
        let mut saw_extension_default_call = false;

        for stmt in main.blocks.iter().flat_map(|block| block.stmts.iter()) {
            match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                    ..
                } => {
                    assert!(
                        args.iter().all(|arg| arg.name.is_none()),
                        "named/default args should be canonical positional MIR args: {stmt:#?}"
                    );
                    if callee_fqn == "scoop.core.getPlatform" {
                        saw_get_platform = true;
                    }
                    if callee_fqn == "mir_refactor.call_contracts.namedDefault" {
                        assert_eq!(
                            args.len(),
                            2,
                            "default args should be canonicalized before MIR direct call lowering: {stmt:#?}"
                        );
                        saw_named_default_call = true;
                    }
                    if callee_fqn == "mir_refactor.call_contracts.ext" {
                        assert_eq!(
                            args.len(),
                            2,
                            "extension default args should include receiver + defaulted slot before MIR lowering: {stmt:#?}"
                        );
                        saw_extension_default_call = true;
                    }
                    direct_fqns.push(callee_fqn.as_str());
                }
                StatementKind::Assign {
                    value:
                        Rvalue::ClassCtor {
                            class_fqn,
                            ctor,
                            args,
                            ..
                        },
                    ..
                } if class_fqn == "mir_refactor.call_contracts.Box" && args.len() == 1 => {
                    assert_eq!(ctor.ordered_param_count, 1);
                    assert!(ctor.selected_ctor_span.is_some());
                    saw_class_ctor = true;
                }
                StatementKind::Assign {
                    value: Rvalue::SizeOf { value_ty },
                    ..
                } if output.types().display(*value_ty).to_string()
                    == "mir_refactor.call_contracts.Box" =>
                {
                    saw_size_of = true;
                }
                StatementKind::Assign {
                    value: Rvalue::TypeMetadataLiteral(metadata),
                    ..
                } if metadata.source_fqn.as_deref() == Some("mir_refactor.call_contracts.Box") => {
                    saw_name_of_metadata = true;
                }
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Closure { .. },
                            ..
                        },
                    ..
                } => {
                    saw_closure_call = true;
                }
                _ => {}
            }
        }

        for expected in [
            "mir_refactor.call_contracts.direct",
            "mir_refactor.call_contracts.generic",
            "mir_refactor.call_contracts.namedDefault",
            "mir_refactor.call_contracts.ext",
            "mir_refactor.call_contracts.Singleton.get",
            "mir_refactor.call_contracts.apply",
        ] {
            assert!(
                direct_fqns.contains(&expected),
                "missing direct call `{expected}` in {direct_fqns:?}\n{dump}"
            );
        }

        assert!(
            saw_get_platform,
            "getPlatform intrinsic call missing: {dump}"
        );
        assert!(saw_class_ctor, "class ctor contract missing: {dump}");
        assert!(
            saw_named_default_call,
            "top-level default-arg call should lower with full ordered args: {dump}"
        );
        assert!(
            saw_extension_default_call,
            "extension default-arg call should lower with full ordered args: {dump}"
        );
        assert!(saw_size_of, "sizeOf<T>() MIR primitive missing: {dump}");
        assert!(
            saw_name_of_metadata,
            "nameOf<T>() type metadata primitive missing: {dump}"
        );
        assert!(saw_closure_call, "immediate closure call missing: {dump}");
        assert!(
            apply
                .blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Call {
                            kind: CallKind::FunValue { .. },
                            ..
                        },
                        ..
                    }
                ))
        );
    }

    #[test]
    fn refactor_mir_funptr_calls_lower_to_explicit_funptr_kind() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_mir_funptr.scoop",
            r#"package sample

import scoop.core.*
import scoop.unsafe.*

@Extern("native_get_funptr")
fun getFunPtr(): FunPtr<(Int) -> Int>

fun use(): Int {
    val fp: FunPtr<(Int) -> Int> = @Unsafe do { getFunPtr() }
    return @Unsafe do { fp(41) }
}
"#,
        );

        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap();
        let output = super::run(typed_hir_output).expect("FunPtr source should lower to MIR");
        let body = validated_callable_body(&output, "sample.use");

        assert!(
            body.blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Call {
                            kind: CallKind::FunPtr { .. },
                            ..
                        },
                        ..
                    }
                )),
            "FunPtr call should lower to explicit CallKind::FunPtr"
        );
    }

    #[test]
    fn refactor_mir_ctor_default_args_lower_to_ordered_class_ctor() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_mir_ctor_default_args.scoop",
            r#"package sample

class Pair(val first: Int = 7, val second: Int)

fun main(): Int {
    val pair: Pair = Pair(second = 6)
    return pair.first + pair.second
}
"#,
        );

        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source).unwrap();
        let output = super::run(typed_hir_output).expect("ctor default args should lower to MIR");
        let body = validated_callable_body(&output, "sample.main");

        let (ctor, args) = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::ClassCtor {
                            class_fqn,
                            ctor,
                            args,
                            ..
                        },
                    ..
                } if class_fqn == "sample.Pair" => Some((ctor, args)),
                _ => None,
            })
            .expect("Pair class ctor should lower through ordered class ctor contract");

        assert!(
            args.iter().all(|arg| arg.name.is_none()),
            "class ctor default args should be canonical positional MIR args: {args:#?}"
        );
        assert_eq!(args.len(), 2);
        assert_eq!(ctor.ordered_param_count, 2);
        assert!(
            ctor.selected_ctor_span.is_some(),
            "class ctor contract must keep the selected ctor identity"
        );
    }

    #[test]
    fn refactor_mir_value_primitives_record_typecheck_and_cast_metadata() {
        let output = run_fixture("mir_refactor", "runtime_typecheck_cast.scoop");
        let body = validated_callable_body(&output, "mir_refactor.runtime_typecheck_cast.inspect");
        let mut saw_iface_is = false;
        let mut saw_other_not_is = false;
        let mut saw_parameterized_holder_is = false;
        let mut saw_as_raise = false;
        let mut saw_asq_none = false;

        for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
            let StatementKind::Assign { value, .. } = &stmt.kind else {
                continue;
            };
            match value {
                Rvalue::TypeCheck {
                    op,
                    test_ty,
                    metadata,
                    ..
                } => {
                    assert_eq!(metadata.target_ty, *test_ty);
                    assert_eq!(metadata.descriptor.ty, *test_ty);
                    assert_eq!(metadata.static_fold, RuntimeTypeStaticFold::Dynamic);
                    match (&metadata.descriptor.kind, op) {
                        (
                            RuntimeTypeDescriptorKind::Nominal {
                                fqn,
                                kind: Some(ast::TypeKind::Interface),
                            },
                            ast::TypeCheckOp::Is,
                        ) if fqn == "mir_refactor.runtime_typecheck_cast.IFace" => {
                            saw_iface_is = true;
                        }
                        (
                            RuntimeTypeDescriptorKind::Nominal {
                                fqn,
                                kind: Some(ast::TypeKind::Class),
                            },
                            ast::TypeCheckOp::NotIs,
                        ) if fqn == "mir_refactor.runtime_typecheck_cast.Other" => {
                            saw_other_not_is = true;
                        }
                        (
                            RuntimeTypeDescriptorKind::Nominal {
                                fqn,
                                kind: Some(ast::TypeKind::Class),
                            },
                            ast::TypeCheckOp::Is,
                        ) if fqn == "mir_refactor.runtime_typecheck_cast.Holder" => {
                            assert!(matches!(
                                &metadata.parameterized,
                                RuntimeTypeParameterizedMatch::Nominal { type_args, .. }
                                    if type_args.len() == 1
                            ));
                            saw_parameterized_holder_is = true;
                        }
                        _ => {}
                    }
                }
                Rvalue::Cast {
                    op,
                    target_ty,
                    metadata,
                    ..
                } => {
                    assert_eq!(metadata.test.target_ty, *target_ty);
                    assert_eq!(metadata.test.descriptor.ty, *target_ty);
                    match (op, &metadata.failure, &metadata.result) {
                        (
                            ast::CastOp::As,
                            RuntimeCastFailure::Raise {
                                effect_ty,
                                error_fqn,
                            },
                            RuntimeCastResult::Target { ty },
                        ) => {
                            assert_eq!(*ty, *target_ty);
                            assert!(effect_ty.is_some());
                            assert_eq!(error_fqn, "scoop.core.RuntimeError.ClassCastFailed");
                            saw_as_raise = true;
                        }
                        (
                            ast::CastOp::AsQ,
                            RuntimeCastFailure::ReturnNone,
                            RuntimeCastResult::Option { option_ty, some_ty },
                        ) => {
                            assert_eq!(*some_ty, *target_ty);
                            assert_ne!(*option_ty, *target_ty);
                            assert!(matches!(
                                &metadata.test.parameterized,
                                RuntimeTypeParameterizedMatch::Nominal { type_args, .. }
                                    if type_args.len() == 1
                            ));
                            saw_asq_none = true;
                        }
                        other => panic!("unexpected cast metadata: {other:?}"),
                    }
                }
                _ => {}
            }
        }

        assert!(saw_iface_is, "missing interface `is` metadata");
        assert!(saw_other_not_is, "missing class `!is` metadata");
        assert!(
            saw_parameterized_holder_is,
            "missing parameterized typecheck metadata"
        );
        assert!(saw_as_raise, "missing `as` failure raise metadata");
        assert!(saw_asq_none, "missing `as?` none-result metadata");
    }

    #[test]
    fn refactor_mir_value_primitives_not_null_assert_is_explicit_match_and_raise() {
        let output = run_fixture("mir_refactor", "not_null_assert.scoop");
        let body = validated_callable_body(&output, "mir_refactor.not_null_assert.unwrap");
        let mut saw_some_match = false;
        let mut saw_none_match = false;
        let mut saw_extract = false;
        let mut saw_raise = false;

        for block in &body.blocks {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StatementKind::Assign {
                        value: Rvalue::PatternMatch { pattern, .. },
                        ..
                    } => {
                        saw_some_match |= pattern_contains_variant(pattern, "Some");
                        saw_none_match |= pattern_contains_variant(pattern, "None");
                    }
                    StatementKind::Assign {
                        value: Rvalue::PatternExtract { .. },
                        ..
                    } => saw_extract = true,
                    _ => {}
                }
            }
            if let TerminatorKind::Perform { metadata, .. } = &block.terminator.kind {
                saw_raise |= output.types().display(metadata.effect_ty).to_string()
                    == "scoop.core.Raise<scoop.core.RuntimeError>"
                    && output.types().display(metadata.result_ty).to_string() == "Nothing";
            }
        }

        assert!(saw_some_match, "`!!` success arm should test Some payload");
        assert!(saw_none_match, "`!!` failure arm should test None");
        assert!(saw_extract, "`!!` success arm should extract payload");
        assert!(
            saw_raise,
            "`!!` failure arm should perform RuntimeError raise"
        );
    }

    #[test]
    fn refactor_mir_value_primitives_pattern_is_type_metadata_is_classified() {
        let output = run_fixture("mir_refactor", "pattern_is_type.scoop");
        let body = validated_callable_body(&output, "mir_refactor.pattern_is_type.classify");
        let mut saw_string = false;
        let mut saw_box = false;

        for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
            let StatementKind::Assign {
                value: Rvalue::PatternMatch { pattern, .. },
                ..
            } = &stmt.kind
            else {
                continue;
            };
            collect_pattern_is_metadata(pattern, &mut |metadata| {
                assert_eq!(
                    output.types().display(metadata.subject_ty).to_string(),
                    "Any"
                );
                match &metadata.descriptor.kind {
                    RuntimeTypeDescriptorKind::String => {
                        assert_eq!(metadata.match_kind, RuntimePatternTypeTestKind::RuntimeRef);
                        saw_string = true;
                    }
                    RuntimeTypeDescriptorKind::Nominal {
                        fqn,
                        kind: Some(ast::TypeKind::Class),
                    } if fqn == "mir_refactor.pattern_is_type.Box" => {
                        assert_eq!(
                            metadata.match_kind,
                            RuntimePatternTypeTestKind::RuntimeClass
                        );
                        saw_box = true;
                    }
                    _ => {}
                }
            });
        }

        assert!(saw_string, "missing `is String` pattern metadata");
        assert!(saw_box, "missing `is Box` pattern metadata");
    }

    #[test]
    fn refactor_mir_value_primitives_reject_unsupported_function_type_cast_before_mir() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/unsupported_function_type_cast.scoop",
            r#"package sample
import scoop.core.*

fun bad() {
    val f: () -> Int / Pure! = { 1 }
    val a: Any = f
    val g: (() -> Int / Pure!)? = a as? (() -> Int / Pure!)
    val _ = g
}
"#,
        );
        let err = super::super::load_typed_hir_stage_output_for_dump(&session, &source)
            .expect_err("function type runtime cast must be rejected before MIR");
        let report = format!("{err:?}");
        assert!(
            report.contains("FunctionTypeCastNotSupported")
                || report.contains("function_type_cast_not_supported"),
            "expected function-type cast diagnostic, got: {report}"
        );
    }

    #[test]
    fn refactor_mir_aggregate_transport_records_composite_contracts() {
        let output = run_fixture("mir_refactor", "aggregate_transport.scoop");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("Todo"),
            "aggregate transport fixture must not leak MIR Todo: {dump}"
        );

        let mut saw_tuple = false;
        let mut saw_struct = false;
        let mut saw_enum_unit = false;
        let mut saw_enum_nested = false;
        let mut saw_enum_wide = false;
        let mut saw_closure_env = false;
        let mut saw_capture_box = false;
        let mut saw_array_build = false;
        let mut saw_array_get = false;
        let mut saw_array_set = false;
        let mut saw_array_element_boxing = false;
        let mut saw_effect_payload = false;
        let mut saw_effect_payload_boxing = false;
        let mut saw_fun_value_abi = false;
        let mut saw_aggregate_return = false;

        for fun in output.file().items.iter().filter_map(|item| match item {
            Item::Fun(fun) => Some(fun),
            _ => None,
        }) {
            let Some(body) = &fun.body else {
                continue;
            };
            body.validate_refactor_direct_style()
                .unwrap_or_else(|err| panic!("{} should validate: {err}", fun.fqn));
            for block in &body.blocks {
                for stmt in &block.stmts {
                    let StatementKind::Assign { value, .. } = &stmt.kind else {
                        continue;
                    };
                    match value {
                        Rvalue::MakeTuple {
                            elements,
                            transport,
                        } => {
                            match transport.kind {
                                AggregateTransportKind::Tuple => saw_tuple = true,
                                AggregateTransportKind::ClosureEnv => saw_closure_env = true,
                                other => panic!("unexpected MakeTuple transport kind: {other:?}"),
                            }
                            assert_eq!(transport.fields.len(), elements.len());
                            assert_transport_fields_are_consistent(&transport.fields);
                        }
                        Rvalue::StructLit { fields, transport } => {
                            saw_struct = true;
                            assert_eq!(transport.kind, AggregateTransportKind::Struct);
                            assert_eq!(transport.fields.len(), fields.len());
                            assert_transport_fields_are_consistent(&transport.fields);
                        }
                        Rvalue::EnumVariant {
                            variant_name,
                            args,
                            payload,
                            ..
                        } => {
                            assert_eq!(payload.kind, AggregateTransportKind::EnumPayload);
                            assert_eq!(payload.fields.len(), args.len());
                            assert_transport_fields_are_consistent(&payload.fields);
                            match variant_name.as_str() {
                                "Empty" if payload.fields.is_empty() => saw_enum_unit = true,
                                "Nested" if payload.fields.len() == 2 => saw_enum_nested = true,
                                "Wide" if payload.fields.len() == 1 => saw_enum_wide = true,
                                _ => {}
                            }
                        }
                        Rvalue::CaptureBoxNew { contract, .. }
                        | Rvalue::CaptureBoxGet { contract, .. }
                        | Rvalue::CaptureBoxSet { contract, .. } => {
                            saw_capture_box = true;
                            assert_ne!(contract.box_ty, contract.value.source_ty);
                        }
                        Rvalue::MakeClosure { env_contract, .. }
                            if !env_contract.captures.is_empty() =>
                        {
                            saw_closure_env = true;
                            assert!(env_contract.captures.iter().any(|capture| {
                                capture.mutable
                                    && capture.transport.kind == MirTransportKind::CaptureBox
                            }));
                            assert!(env_contract.captures.iter().any(|capture| {
                                value_transport_has_boxing(
                                    &capture.transport,
                                    MirBoxingReason::ClosureCapture,
                                )
                            }));
                        }
                        Rvalue::Call {
                            kind, transport, ..
                        } => {
                            if transport.aggregate_return.is_some() {
                                saw_aggregate_return = true;
                            }
                            if let Some(array) = &transport.array {
                                match array.operation {
                                    ArrayTransportOperation::BuilderBuildArray
                                    | ArrayTransportOperation::BuilderBuildMutableArray => {
                                        saw_array_build = true;
                                    }
                                    ArrayTransportOperation::Get => saw_array_get = true,
                                    ArrayTransportOperation::Set => saw_array_set = true,
                                    ArrayTransportOperation::BuilderNew
                                    | ArrayTransportOperation::BuilderPush => {}
                                }
                                saw_array_element_boxing |= value_transport_has_boxing(
                                    &array.element,
                                    MirBoxingReason::ArrayElement,
                                );
                            }
                            if matches!(kind, CallKind::FunValue { .. }) {
                                saw_fun_value_abi = true;
                                assert_eq!(
                                    transport.abi.callable_abi_kind,
                                    MirCallableAbiKind::DeferredToEffectFacts
                                );
                                assert_eq!(
                                    transport.abi.impl_plan,
                                    MirCallableImplPlan::DeferredToEffectFacts
                                );
                            }
                        }
                        _ => {}
                    }
                }
                if let TerminatorKind::Perform { metadata, args, .. } = &block.terminator.kind {
                    saw_effect_payload = true;
                    assert_eq!(metadata.payload_transport.len(), args.len());
                    for transport in &metadata.payload_transport {
                        assert_eq!(transport.kind, MirTransportKind::EffectPayload);
                        saw_effect_payload_boxing |=
                            value_transport_has_boxing(transport, MirBoxingReason::EffectPayload);
                    }
                }
            }
        }

        assert!(saw_tuple, "tuple aggregate transport missing: {dump}");
        assert!(saw_struct, "struct aggregate transport missing: {dump}");
        assert!(saw_enum_unit, "unit enum payload schema missing: {dump}");
        assert!(
            saw_enum_nested,
            "nested enum payload schema missing: {dump}"
        );
        assert!(saw_enum_wide, "wide enum payload schema missing: {dump}");
        assert!(saw_closure_env, "closure env transport missing: {dump}");
        assert!(
            saw_capture_box,
            "mutable capture box transport missing: {dump}"
        );
        assert!(saw_array_build, "array build transport missing: {dump}");
        assert!(saw_array_get, "array get transport missing: {dump}");
        assert!(saw_array_set, "array set transport missing: {dump}");
        assert!(
            saw_array_element_boxing,
            "array composite element boxing intent missing: {dump}"
        );
        assert!(
            saw_effect_payload,
            "effect payload transport missing: {dump}"
        );
        assert!(
            saw_effect_payload_boxing,
            "effect composite payload boxing intent missing: {dump}"
        );
        assert!(
            saw_fun_value_abi,
            "function-value ABI handoff missing: {dump}"
        );
        assert!(
            saw_aggregate_return,
            "aggregate return transport missing: {dump}"
        );
    }

    #[test]
    fn refactor_mir_composite_transport_metadata_contracts() {
        refactor_mir_aggregate_transport_records_composite_contracts();
    }

    #[test]
    fn refactor_mir_value_boxing_transport_contract() {
        let output = run_fixture("mir_refactor", "value_boxing_transport.scoop");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("Todo"),
            "value boxing transport fixture must not leak MIR Todo: {dump}"
        );

        let top = output
            .initializer_root("mir_refactor.value_boxing_transport.topAny")
            .expect("top-level Any initializer root must be published");
        assert_value_erasure_transport(
            top.initializer_transport.as_ref(),
            MirBoxingReason::AnyErasure,
            "top-level initializer",
            &dump,
        );

        let mut any_erasure_count = 0usize;
        let mut ref_erasure_count = 0usize;
        let mut saw_struct_any = false;
        let mut saw_tuple_any = false;
        let mut saw_enum_any = false;
        let mut saw_struct_ref = false;

        for stmt in output
            .file()
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => fun.body.as_ref(),
                _ => None,
            })
            .flat_map(|body| body.blocks.iter())
            .flat_map(|block| block.stmts.iter())
        {
            let StatementKind::Assign {
                value: Rvalue::Transport { transport, .. },
                ..
            } = &stmt.kind
            else {
                continue;
            };
            let boxing = transport
                .boxing
                .as_ref()
                .expect("value erasure transport must publish boxing intent");
            assert_eq!(boxing.source_ty, transport.source_ty);
            assert!(boxing.target_ty.is_some());
            assert!(transport.requirements.copy);
            match boxing.reason {
                MirBoxingReason::AnyErasure => {
                    any_erasure_count += 1;
                    saw_struct_any |= transport.kind == MirTransportKind::Struct;
                    saw_tuple_any |= transport.kind == MirTransportKind::Tuple;
                    saw_enum_any |= transport.kind == MirTransportKind::EnumPayload;
                }
                MirBoxingReason::RefErasure => {
                    ref_erasure_count += 1;
                    saw_struct_ref |= transport.kind == MirTransportKind::Struct;
                }
                other => panic!("unexpected value erasure boxing reason: {other:?}"),
            }
        }

        assert!(
            any_erasure_count >= 6,
            "initializer/assignment/return/call-arg Any erasure transports missing: {dump}"
        );
        assert!(
            ref_erasure_count >= 2,
            "local/call-arg Ref erasure transports missing: {dump}"
        );
        assert!(
            saw_struct_any,
            "struct -> Any boxing transport missing: {dump}"
        );
        assert!(
            saw_tuple_any,
            "tuple -> Any boxing transport missing: {dump}"
        );
        assert!(
            saw_enum_any,
            "payload-bearing enum -> Any boxing transport metadata missing: {dump}"
        );
        assert!(
            saw_struct_ref,
            "struct -> Ref/interface boxing transport missing: {dump}"
        );
    }

    fn pattern_contains_variant(pattern: &Pattern, expected: &str) -> bool {
        match pattern {
            Pattern::Variant { name, .. } => name == expected,
            Pattern::Or { pats } => pats
                .iter()
                .any(|pat| pattern_contains_variant(pat, expected)),
            Pattern::Tuple { elements } => elements
                .iter()
                .any(|pat| pattern_contains_variant(pat, expected)),
            Pattern::Else
            | Pattern::Wildcard
            | Pattern::Rest
            | Pattern::Is { .. }
            | Pattern::Bind { .. }
            | Pattern::IntLit { .. }
            | Pattern::CharLit { .. }
            | Pattern::StringLit { .. }
            | Pattern::BoolLit { .. } => false,
        }
    }

    fn assert_transport_fields_are_consistent(fields: &[crate::mir::AggregateTransportField]) {
        for (index, field) in fields.iter().enumerate() {
            assert_eq!(field.index, index);
            assert_eq!(field.ty, field.transport.source_ty);
        }
    }

    fn value_transport_has_boxing(
        transport: &ValueTransportMetadata,
        reason: MirBoxingReason,
    ) -> bool {
        matches!(transport.boxing.as_ref(), Some(boxing) if boxing.reason == reason)
    }

    fn assert_value_erasure_transport(
        transport: Option<&ValueTransportMetadata>,
        reason: MirBoxingReason,
        surface: &str,
        dump: &str,
    ) {
        let transport = transport.unwrap_or_else(|| panic!("{surface} transport missing: {dump}"));
        let boxing = transport
            .boxing
            .as_ref()
            .unwrap_or_else(|| panic!("{surface} boxing intent missing: {dump}"));
        assert_eq!(boxing.source_ty, transport.source_ty);
        assert_eq!(boxing.reason, reason);
        assert!(boxing.target_ty.is_some());
        assert!(transport.requirements.copy);
    }

    fn collect_pattern_is_metadata(
        pattern: &Pattern,
        visit: &mut impl FnMut(&crate::mir::RuntimePatternTypeTestMetadata),
    ) {
        match pattern {
            Pattern::Is { metadata, .. } => visit(metadata),
            Pattern::Or { pats } => {
                for pat in pats {
                    collect_pattern_is_metadata(pat, visit);
                }
            }
            Pattern::Tuple { elements } => {
                for pat in elements {
                    collect_pattern_is_metadata(pat, visit);
                }
            }
            Pattern::Variant { args, .. } => {
                for pat in args {
                    collect_pattern_is_metadata(pat, visit);
                }
            }
            Pattern::Else
            | Pattern::Wildcard
            | Pattern::Rest
            | Pattern::Bind { .. }
            | Pattern::IntLit { .. }
            | Pattern::CharLit { .. }
            | Pattern::StringLit { .. }
            | Pattern::BoolLit { .. } => {}
        }
    }

    #[test]
    fn refactor_mir_no_todo_stage_validator_rejects_item_todo() {
        const SYNTHETIC_ITEM_TODO_REASON: &str = "synthetic item todo";

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = crate::mir::File {
            items: vec![crate::mir::Item::Todo {
                span: crate::span::Span::new(0, 1),
                kind: SYNTHETIC_ITEM_TODO_REASON,
            }],
        };

        let err = super::validate_refactor_bodies(&file, builtins.unit)
            .expect_err("production stage validator should reject item Todo");
        let rendered = err.to_string();
        assert!(rendered.contains("<file>"));
        assert!(rendered.contains(SYNTHETIC_ITEM_TODO_REASON));
    }

    #[test]
    fn refactor_direct_mir_stage_keeps_callable_body_query_surface_stable() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let output = MirStageOutput::new(
            crate::mir::LoweredMir {
                file: crate::mir::File {
                    items: vec![crate::mir::Item::Fun(crate::mir::FunDecl {
                        span: crate::span::Span::new(0, 1),
                        fqn: "sample.main".to_string(),
                        name: "main".to_string(),
                        ty: builtins.unit,
                        params: Vec::new(),
                        return_ty: builtins.unit,
                        body: Some(crate::mir::Body::new_empty()),
                    })],
                },
                types,
            },
            TypedHirEffectContracts::default(),
            None,
        );

        assert_eq!(
            output.callable_body_fqns().collect::<Vec<_>>(),
            vec!["sample.main"]
        );
        assert!(output.callable_body("sample.main").is_some());
    }

    #[test]
    fn refactor_mir_effect_site_contract_keeps_dispatch_and_resume_sites_explicit() {
        let direct_output = run_fixture("mir", "direct_and_fun_value_call.scoop");
        let main_body = callable_body(&direct_output, "a.main");
        let main_calls = main_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value: Rvalue::Call { kind, .. },
                    ..
                } => Some(kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            main_calls.as_slice(),
            [CallKind::Direct { callee_fqn }, CallKind::Direct { callee_fqn: callee_fqn_2 }]
                if callee_fqn == "a.id" && callee_fqn_2 == "a.apply"
        ));

        let apply_body = callable_body(&direct_output, "a.apply");
        assert!(
            apply_body
                .blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| {
                    matches!(
                        &stmt.kind,
                        StatementKind::Assign {
                            value: Rvalue::Call {
                                kind: CallKind::FunValue { .. },
                                ..
                            },
                            ..
                        }
                    )
                })
        );

        let dispatch_output = run_fixture("mir", "dispatch_and_resume_call.scoop");
        let virtual_body = callable_body(&dispatch_output, "fixtures.mir.callVirtual");
        assert!(
            virtual_body
                .blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| {
                    matches!(
                        &stmt.kind,
                        StatementKind::Assign {
                            value: Rvalue::Call {
                                kind: CallKind::Virtual { .. },
                                ..
                            },
                            ..
                        }
                    )
                })
        );
        let interface_body = callable_body(&dispatch_output, "fixtures.mir.callInterface");
        assert!(
            interface_body
                .blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| {
                    matches!(
                        &stmt.kind,
                        StatementKind::Assign {
                            value: Rvalue::Call {
                                kind: CallKind::Interface { .. },
                                ..
                            },
                            ..
                        }
                    )
                })
        );

        let resume_once_body = callable_body(&dispatch_output, "fixtures.mir.resumeOnce");
        let resume_once = resume_once_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Resume { resume, .. },
                            args,
                            ..
                        },
                    ..
                } => Some((resume, args)),
                _ => None,
            })
            .expect("resumeOnce 应 lower 成显式 Resume call");
        assert_eq!(resume_once.1.len(), 1);
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_once.0.resume_ty)
                .to_string(),
            "Int"
        );
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_once.0.answer_ty)
                .to_string(),
            "Unit"
        );
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_once.0.return_ty)
                .to_string(),
            "Unit"
        );
        assert!(resume_once.0.out_effects.is_pure());
        assert!(!resume_once.0.suspends_outward);
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_once.0.runtime_error_effect_ty.unwrap())
                .to_string(),
            "scoop.core.Raise<scoop.core.RuntimeError>"
        );

        let resume_boom_body = callable_body(&dispatch_output, "fixtures.mir.resumeBoom");
        let resume_boom = resume_boom_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Resume { resume, .. },
                            ..
                        },
                    ..
                } => Some(resume),
                _ => None,
            })
            .expect("resumeBoom 应 lower 成显式 Resume call");
        assert!(resume_boom.suspends_outward);
        assert_eq!(resume_boom.out_effects.terms.len(), 1);
        assert_eq!(
            dispatch_output
                .types()
                .display(resume_boom.out_effects.terms[0])
                .to_string(),
            "fixtures.mir.Boom"
        );
        assert!(
            !dispatch_output.stable_dump().contains("Todo"),
            "dispatch/resume fixture should not leak Todo placeholders"
        );
    }

    #[test]
    fn refactor_mir_effect_site_contract_records_perform_and_handle_metadata() {
        let output = run_fixture("mir", "handle_perform.scoop");
        let body = callable_body(&output, "a.main");
        let entry = &body.blocks[body.start.as_u32() as usize].terminator.kind;
        let (handle_metadata, arms) = match entry {
            TerminatorKind::Handle { metadata, arms, .. } => (metadata, arms),
            other => panic!("expected handle terminator, got {other:?}"),
        };
        assert_eq!(
            output
                .types()
                .display(handle_metadata.result_ty)
                .to_string(),
            "Int"
        );
        assert_eq!(
            output
                .types()
                .display(handle_metadata.body_result_ty)
                .to_string(),
            "Int"
        );
        assert!(handle_metadata.finally_result_ty.is_none());
        assert_eq!(arms.len(), 1);
        let arm = &arms[0];
        assert_eq!(arm.op_fqn, "scoop.core.Raise.raise");
        assert_eq!(arm.kind, HandlerArmKind::NonResuming);
        assert_eq!(
            output.types().display(arm.handled_effect_ty).to_string(),
            "scoop.core.Raise<Int>"
        );
        assert_eq!(arm.payload_component_tys.len(), 1);
        assert_eq!(
            output
                .types()
                .display(arm.payload_component_tys[0])
                .to_string(),
            "Int"
        );
        assert_eq!(output.types().display(arm.body_ty).to_string(), "Int");

        let (perform_metadata, perform_args) = body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator.kind {
                TerminatorKind::Perform { metadata, args, .. } => Some((metadata, args)),
                _ => None,
            })
            .expect("handle_perform 应包含显式 Perform terminator");
        assert_eq!(
            output
                .types()
                .display(perform_metadata.effect_ty)
                .to_string(),
            "scoop.core.Raise<Int>"
        );
        assert_eq!(
            output
                .types()
                .display(perform_metadata.result_ty)
                .to_string(),
            "Nothing"
        );
        assert_eq!(perform_metadata.arg_mapping, vec![0]);
        assert_eq!(perform_metadata.payload_component_tys.len(), 1);
        assert_eq!(
            output
                .types()
                .display(perform_metadata.payload_component_tys[0])
                .to_string(),
            "Int"
        );
        assert_eq!(perform_args.len(), 1);
        assert_eq!(perform_args[0].source_arg_index, 0);
    }

    #[test]
    fn refactor_mir_effect_site_contract_missing_perform_contract_is_stage_error() {
        let source = SourceFile::new_virtual(
            "<mem>/missing_perform_contract.scoop",
            r#"package sample
import scoop.core.Raise

fun entry(): Int / Raise<Int> {
    Raise.raise(1)
    return 0
}
"#,
        );
        let (file, unit_ty) = lower_with_empty_refactor_contracts(&source);
        let dump = format!("{file:#?}");
        let old_reason = ["refactor perform", " contract missing"].concat();
        assert!(
            !dump.contains(&old_reason) && !dump.contains("Todo"),
            "missing typed perform contract should be an invalid site metadata diagnostic, not a Todo: {dump}"
        );

        assert_site_metadata_error(
            super::validate_refactor_bodies(&file, unit_ty)
                .expect_err("missing perform contract should fail stage validation"),
            MirSiteMetadataKind::Perform,
        );
    }

    #[test]
    fn refactor_mir_effect_site_contract_missing_handle_contract_is_stage_error() {
        let source = load_fixture("mir_refactor", "handle_perform.scoop");
        let (file, unit_ty) = lower_with_empty_refactor_contracts(&source);
        let dump = format!("{file:#?}");
        let old_reason = ["refactor handle", " contract missing"].concat();
        assert!(
            !dump.contains(&old_reason) && !dump.contains("Todo"),
            "missing typed handle contract should be an invalid site metadata diagnostic, not a Todo: {dump}"
        );

        assert_site_metadata_error(
            super::validate_refactor_bodies(&file, unit_ty)
                .expect_err("missing handle contract should fail stage validation"),
            MirSiteMetadataKind::Handle,
        );
    }

    #[test]
    fn refactor_mir_effect_site_contract_canonicalizes_resume_unit_sugar() {
        let output = run_fixture("mir_refactor", "continuation_resume_unit_sugar.scoop");
        let body = callable_body(&output, "fixtures.mir_refactor.resumeUnit");

        let resume_calls = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Resume { resume, .. },
                            args,
                            ..
                        },
                    ..
                } => Some((resume, args)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(resume_calls.len(), 2);
        for (resume, args) in resume_calls {
            assert_eq!(args.len(), 1);
            assert_eq!(output.types().display(resume.resume_ty).to_string(), "Unit");
            assert_eq!(output.types().display(resume.answer_ty).to_string(), "Unit");
            assert_eq!(output.types().display(resume.return_ty).to_string(), "Unit");
            assert!(unit_operand_is_visible_in_body(
                &output,
                body,
                &args[0].value
            ));
        }

        let direct_unit_calls = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                    ..
                } if callee_fqn == "fixtures.mir_refactor.takesUnit" => Some(args),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(direct_unit_calls.len(), 2);
        for args in direct_unit_calls {
            assert_eq!(args.len(), 1);
            assert!(unit_operand_is_visible_in_body(
                &output,
                body,
                &args[0].value
            ));
        }
        assert!(
            !output.stable_dump().contains("Todo"),
            "resume unit sugar fixture should not leak Todo placeholders"
        );
    }

    fn lower_with_empty_refactor_contracts(
        source: &SourceFile,
    ) -> (crate::mir::File, crate::ty::TypeId) {
        let session = refactor_session();
        let typed_hir_output = super::super::load_typed_hir_stage_output_for_dump(&session, source)
            .expect("typed HIR should pass before forged contract lowering");
        let facts = MirLoweringFacts::default()
            .with_refactor_typed_contracts(&TypedHirEffectContracts::default());
        let mut lowered_hir = typed_hir_output.into_lowered_hir();
        let builtins = lowered_hir.types.intern_builtins();
        let file = lower_hir_file_for_dump_with_facts(
            builtins,
            &mut lowered_hir.types,
            &lowered_hir.file,
            &lowered_hir.member_funs,
            &facts,
        );
        (file, builtins.unit)
    }

    fn assert_site_metadata_error(error: MirLowerError, expected_site: MirSiteMetadataKind) {
        let MirLowerError::InvalidRefactorMir { error, .. } = error else {
            panic!("expected refactor MIR validation error, got {error:?}");
        };
        let MirValidationError::RefactorProductionSiteMetadata { site, detail, .. } = error else {
            panic!("expected site metadata validation error, got {error:?}");
        };
        assert_eq!(site, expected_site);
        assert!(!detail.is_empty());
    }

    #[test]
    fn refactor_mir_cfg_existing_control_flow_samples_validate() {
        let while_output = run_fixture("mir", "while_break_continue.scoop");
        validated_callable_body(&while_output, "a.main");

        let if_when_output = run_fixture("mir", "if_when.scoop");
        validated_callable_body(&if_when_output, "a.main");
    }

    #[test]
    fn refactor_mir_cfg_handle_finally_boundary_is_explicit() {
        let output = run_fixture("mir_refactor", "handle_finally_boundary.scoop");

        let completes = validated_callable_body(&output, "fixtures.mir_refactor.body_completes");
        let completes_entry = &completes.blocks[completes.start.as_u32() as usize]
            .terminator
            .kind;
        let (body_target, arm_targets, finally_target, exit_target) = match completes_entry {
            TerminatorKind::Handle {
                has_finally,
                body_target,
                arm_targets,
                finally_target,
                exit_target,
                ..
            } => {
                assert!(*has_finally, "body_completes 应保留 finally boundary");
                (
                    *body_target,
                    arm_targets.clone(),
                    finally_target.expect("body_completes 应显式指向 finally cleanup block"),
                    *exit_target,
                )
            }
            other => panic!("body_completes 入口应为 Handle terminator，而不是 {other:?}"),
        };
        assert!(completes.blocks[finally_target.as_u32() as usize].is_cleanup);
        assert_eq!(arm_targets.len(), 1);
        assert!(matches!(
            completes.blocks[body_target.as_u32() as usize].terminator.kind,
            TerminatorKind::Goto { target } if target == finally_target
        ));
        assert!(matches!(
            completes.blocks[arm_targets[0].as_u32() as usize].terminator.kind,
            TerminatorKind::Goto { target } if target == finally_target
        ));
        assert!(matches!(
            completes.blocks[finally_target.as_u32() as usize].terminator.kind,
            TerminatorKind::Goto { target } if target == exit_target
        ));

        let raised = validated_callable_body(&output, "fixtures.mir_refactor.handled_raise");
        let raised_entry = &raised.blocks[raised.start.as_u32() as usize]
            .terminator
            .kind;
        let raised_finally = match raised_entry {
            TerminatorKind::Handle {
                has_finally,
                finally_target,
                ..
            } => {
                assert!(*has_finally, "handled_raise 应保留 finally boundary");
                finally_target.expect("handled_raise 应显式指向 finally cleanup block")
            }
            other => panic!("handled_raise 入口应为 Handle terminator，而不是 {other:?}"),
        };
        assert!(raised.blocks[raised_finally.as_u32() as usize].is_cleanup);
        let perform = raised
            .blocks
            .iter()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Perform { .. }))
            .expect("handled_raise 应包含显式 Perform terminator");
        assert!(matches!(
            perform.terminator.unwind,
            UnwindAction::Cleanup { target } if raised.blocks[target.as_u32() as usize].is_cleanup
        ));
    }

    #[test]
    fn refactor_mir_policy_gates_keep_resume_unwind_cleanup_contract() {
        let output = run_fixture("mir_refactor", "handle_finally_boundary.scoop");
        let raised = validated_callable_body(&output, "fixtures.mir_refactor.handled_raise");

        let perform = raised
            .blocks
            .iter()
            .find(|block| matches!(block.terminator.kind, TerminatorKind::Perform { .. }))
            .expect("policy fixture should contain a perform that can unwind through finally");
        let cleanup_target = match perform.terminator.unwind {
            UnwindAction::Cleanup { target } => target,
            ref other => panic!("perform should publish cleanup unwind action, got {other:?}"),
        };
        assert!(raised.blocks[cleanup_target.as_u32() as usize].is_cleanup);
        assert!(raised.blocks.iter().any(|block| {
            block.is_cleanup && matches!(block.terminator.kind, TerminatorKind::ResumeUnwind)
        }));
    }

    #[test]
    fn refactor_mir_policy_gates_publish_gc_pin_handle_intrinsic_contracts() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/gc_policy_gates.scoop",
            r#"package fixtures.mir_refactor

import scoop.core.*

class Box(val value: Int)

fun main(): Unit {
    @Unsafe do {
        val box: Box = Box(value = 1)
        val pinned: Pinned = GC.pin(box)
        GC.unpin(pinned)
        val gcHandle: GcHandle = GC.handleNew(box)
        val anyRef: Any = GC.handleGet(gcHandle)
        GC.handleDrop(gcHandle)
    }
}
"#,
        );
        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source)
                .expect("GC policy fixture should typecheck before MIR");
        let output = super::run(typed_hir_output).expect("GC policy fixture should lower to MIR");
        let body = callable_body(&output, "fixtures.mir_refactor.main");
        let call_contracts = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value: Rvalue::Call { transport, .. },
                    ..
                } => transport
                    .gc
                    .as_ref()
                    .map(|gc| (gc.callee_fqn.as_str(), Some(gc))),
                _ => None,
            })
            .collect::<Vec<_>>();

        let find_gc = |callee: &str| {
            call_contracts
                .iter()
                .find_map(|(found, gc)| {
                    (*found == callee).then_some(gc.expect("GC call must publish metadata"))
                })
                .unwrap_or_else(|| {
                    panic!("missing GC intrinsic contract for {callee}: {call_contracts:?}")
                })
        };

        let pin = find_gc("scoop.core.GC.pin");
        assert_eq!(pin.operation, GcIntrinsicOperation::Pin);
        assert_eq!(pin.root_lifetime, GcRootLifetime::PinnedUntilUnpin);
        assert_eq!(pin.pairing, GcIntrinsicPairing::PinMustPairUnpin);
        assert!(pin.unsafe_required);
        assert!(pin.subject.requirements.trace);

        let unpin = find_gc("scoop.core.GC.unpin");
        assert_eq!(unpin.operation, GcIntrinsicOperation::Unpin);
        assert_eq!(unpin.root_lifetime, GcRootLifetime::EndsPinnedRoot);
        assert_eq!(unpin.pairing, GcIntrinsicPairing::UnpinMatchesPin);

        let handle_new = find_gc("scoop.core.GC.handleNew");
        assert_eq!(handle_new.operation, GcIntrinsicOperation::HandleNew);
        assert_eq!(
            handle_new.root_lifetime,
            GcRootLifetime::StableHandleUntilDrop
        );
        assert_eq!(
            handle_new.pairing,
            GcIntrinsicPairing::HandleNewMustPairDrop
        );
        assert!(handle_new.subject.requirements.trace);

        let handle_get = find_gc("scoop.core.GC.handleGet");
        assert_eq!(handle_get.operation, GcIntrinsicOperation::HandleGet);
        assert_eq!(
            handle_get.root_lifetime,
            GcRootLifetime::BorrowedFromStableHandle
        );
        assert_eq!(
            handle_get.pairing,
            GcIntrinsicPairing::HandleGetRequiresLiveHandle
        );

        let handle_drop = find_gc("scoop.core.GC.handleDrop");
        assert_eq!(handle_drop.operation, GcIntrinsicOperation::HandleDrop);
        assert_eq!(handle_drop.root_lifetime, GcRootLifetime::EndsStableHandle);
        assert_eq!(
            handle_drop.pairing,
            GcIntrinsicPairing::HandleDropMatchesHandleNew
        );
    }

    #[test]
    fn refactor_mir_gc_handle_raw_uintptr_token_stays_scalar() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/gc_handle_uintptr_policy.scoop",
            r#"package fixtures.mir_refactor

import scoop.core.*

@Extern("scoop_test_handle_token_slot_store")
fun handleTokenSlotStore(raw: UIntPtr): Unit

@Extern("scoop_test_handle_token_slot_take")
fun handleTokenSlotTake(): UIntPtr

fun main(): Unit {
    val raw: UIntPtr = @Unsafe do { handleTokenSlotTake() }
    val returned: GcHandle = GcHandle { raw: raw }
    @Unsafe do { handleTokenSlotStore(returned.raw) }
}
"#,
        );
        let typed_hir_output =
            super::super::load_typed_hir_stage_output_for_dump(&session, &source)
                .expect("GC handle raw UIntPtr fixture should typecheck before MIR");
        let output = super::run(typed_hir_output)
            .expect("GC handle raw UIntPtr fixture should lower to MIR");
        let body = callable_body(&output, "fixtures.mir_refactor.main");

        let take_transport = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            transport,
                            ..
                        },
                    ..
                } if callee_fqn == "fixtures.mir_refactor.handleTokenSlotTake" => Some(transport),
                _ => None,
            })
            .expect("extern take-handle-token call should be present");
        assert_eq!(take_transport.result.kind, MirTransportKind::Scalar);
        assert!(
            !take_transport.result.requirements.trace,
            "UIntPtr token return should stay scalar rather than GC-tracked"
        );

        let raw_field_transport = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value: Rvalue::StructLit { transport, .. },
                    ..
                } => transport
                    .fields
                    .iter()
                    .find(|field| field.name.as_deref() == Some("raw"))
                    .map(|field| &field.transport),
                _ => None,
            })
            .expect("GcHandle { raw: ... } struct literal should be present");
        assert_eq!(raw_field_transport.kind, MirTransportKind::Scalar);
        assert!(
            !raw_field_transport.requirements.trace,
            "GcHandle.raw field transport should stay scalar rather than GC-tracked"
        );
    }

    #[test]
    fn refactor_mir_cfg_effect_boundary_inside_expr_context_uses_explicit_blocks() {
        let output = run_fixture("mir_refactor", "effect_boundary_inside_expr_context.scoop");
        let body = validated_callable_body(&output, "fixtures.mir_refactor.main");

        let handle_count = body
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.terminator.kind,
                    TerminatorKind::Handle {
                        has_finally: true,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(
            handle_count, 4,
            "local init / call arg / if 条件 / return expr 中的 boundary 都应显式落成独立 Handle block"
        );
        assert!(
            body.blocks.iter().filter(|block| block.is_cleanup).count() >= 4,
            "每个带 finally 的 boundary 都应生成 cleanup block"
        );
        assert!(body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value:
                            Rvalue::Call {
                                kind: CallKind::Direct { callee_fqn },
                                ..
                            },
                        ..
                    } if callee_fqn == "fixtures.mir_refactor.box_int"
                )
            })
        }));
        assert!(
            body.blocks
                .iter()
                .any(|block| { matches!(block.terminator.kind, TerminatorKind::CondBr { .. }) })
        );
    }

    #[test]
    fn refactor_mir_cfg_escape_continuation_finally_materializes_continuation_local() {
        let output = run_fixture(
            "run-pass",
            "effect_handle_return_from_function_finally.scoop",
        );
        let body = validated_callable_body(&output, "returnThroughFinally");
        let entry = &body.blocks[body.start.as_u32() as usize].terminator.kind;
        let arm = match entry {
            TerminatorKind::Handle { arms, .. } => arms
                .first()
                .expect("escape continuation fixture 应包含唯一的 handler arm"),
            other => panic!("returnThroughFinally 入口应为 Handle terminator，而不是 {other:?}"),
        };
        assert_eq!(arm.kind, HandlerArmKind::EscapeContinuation);
        assert!(
            arm.continuation_local.is_some(),
            "escape continuation arm 应显式 materialize continuation binder local"
        );
        assert!(
            !output.stable_dump().contains("unbound local ref"),
            "escape continuation arm 不应再回退成未绑定局部占位"
        );
    }
}
