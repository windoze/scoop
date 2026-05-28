use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::btree_map::Entry;
use std::path::{Path, PathBuf};

use scoop_project_model::StableConeKey;
use scoopc_ids::{BodyVersionKey, CanonicalTextKey, StableCanonicalKey, StageArtifactKey};
use scoopc_mir_facts::MirFacts;
use scoopc_mir_facts::common::{FactIdentity, MirBodyReference};
use scoopc_mir_facts::families::{
    CallableFamilyFact, InstanceFamilyInventory, InstanceInventoryEntry,
};
use scoopc_mir_facts::metadata::{
    MirMetadataFacts, MirNominalOwnerKind, NominalDirectSupertypesFact,
};
use scoopc_mir_facts::pass_artifacts::{
    CallableBodyArtifact, EscapeFactsArtifact, PassArtifactMetadata, PassArtifactRevision,
    SummaryArtifact,
};
use scoopc_mir_facts::pipeline::{MirPassPipelineMetadata, MirPassRun};
use scoopc_mir_facts::roots::{
    MirGlobalStorageKind, MirInitializerDependencyFact,
    MirInitializerDependencyKind as FactInitializerDependencyKind,
    MirInitializerRootKind as FactInitializerRootKind, MirItemReference, MirMetadataRootKind,
    MirRootDetail, MirRootFact, MirRootKind, RootInventories,
};
use scoopc_mir_facts::snapshot::{MaterializedSnapshotBinding, SnapshotBindings};

use crate::dump_support::normalize_dump_path;
use crate::mir::{
    ExternGlobalRoot as MirExternGlobalRoot, File as MirFile, FunDecl as MirFunDecl,
    InitializerRoot as MirInitializerRoot, InstanceKey, Item as MirItem, LoweredMir,
    MaterializedMir, MaterializedMirPassView, MetadataRoot as MirMetadataRoot, MirLowerError,
    MirLoweringFacts, lower_hir_file_for_dump_with_facts,
};
use crate::ty::{TypeId, TypeStore};

use super::HirStageOutput;

const MIR_STAGE_LABEL: &str = "mir";
const DIRECT_STYLE_BODY_ROLE: &str = "direct_style_mir";
const MATERIALIZED_BODY_ROLE: &str = "canonical_materialized_mir";
const MATERIALIZED_INSTANCE_ROLE: &str = "materialized_instance";
const CALLABLE_FAMILY_ROLE: &str = "callable_family";
const CANONICAL_SNAPSHOT_ROLE: &str = "canonical_materialized_snapshot";
const PASS_ARTIFACT_ROLE: &str = "canonical_pass_artifacts";

/// direct-style MIR dump / validation helper output.
///
/// This shape is intentionally not P4-ready: it carries the direct-style MIR IR and
/// root facts, but it has no canonical materialized snapshot. Consumers that feed
/// effect facts or later stages must first convert it with `with_materialized_mir`.
#[derive(Debug)]
pub struct DirectStyleMirStageOutput {
    lowered_mir: LoweredMir,
    mir_facts: MirFacts,
}

/// P4-ready MIR stage handoff.
///
/// 本阶段固定如下 invariants，供 P3/P4 及后续阶段直接消费：
/// - `lowered_mir` 仍是 direct-style MIR，而不是 late-lowered `Step` IR；
/// - 当前所有 effect-sensitive site 继续通过 MIR 节点上的 `SiteId` 锚定；
/// - source-site contracts 只从 `HirFacts` 进入 MIR lowering；canonical 的 site-level
///   contract 现已下沉到 MIR 节点 metadata，P4 不得重新解释 HIR side table；
/// - MIR-owned root inventories 由 `mir_facts` 发布，stage 查询方法只委托 facts 定位
///   direct-style MIR item；
/// - P4 的 authoritative 输入是这份 stage 输出上的 root identity、必选
///   canonical `materialized_mir` 快照 / pass query surface，以及 MIR 节点上的 `SiteId` /
///   metadata；P4 不得回看 P2 原始 HIR side tables 重新猜测 site contract。
/// - 本 stage 仍未提供 `StepSchema`、`ContinuationSchema` 或 `MaterializedEffectFacts`；这些属于
///   P4/P5 的职责，而不是 P3 dump / stage 输出应提前伪造的内容。
#[derive(Debug)]
pub struct MirStageOutput {
    direct_style: DirectStyleMirStageOutput,
    materialized_mir: MaterializedMir,
}

impl DirectStyleMirStageOutput {
    pub(crate) fn new(
        lowered_mir: LoweredMir,
        stable_cone_key: StableConeKey,
        source_cones: &HashMap<PathBuf, crate::cone::SourceConeInfo>,
        source_cone_order: &HashMap<StableConeKey, u32>,
    ) -> Self {
        let mir_facts = build_direct_style_mir_facts(
            &lowered_mir.file,
            &stable_cone_key,
            source_cones,
            source_cone_order,
        );
        Self {
            lowered_mir,
            mir_facts,
        }
    }

    pub(crate) fn with_materialized_mir(self, materialized_mir: MaterializedMir) -> MirStageOutput {
        MirStageOutput::from_direct_style(self, materialized_mir)
    }

    pub fn file(&self) -> &MirFile {
        &self.lowered_mir.file
    }

    pub fn types(&self) -> &TypeStore {
        &self.lowered_mir.types
    }

    /// Return MIR-owned facts published by this stage.
    pub fn mir_facts(&self) -> &MirFacts {
        &self.mir_facts
    }

    /// 以稳定顺序枚举当前 direct-style MIR 中可查询的 callable body 身份。
    pub fn callable_body_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.mir_facts.roots.callable_body_fqns()
    }

    /// 按 callable body 身份查询 canonical direct-style MIR body。
    pub fn callable_body(&self, fqn: &str) -> Option<&MirFunDecl> {
        let item_index = self.mir_facts.roots.callable_body(fqn)?.item.index;
        match self.file().items.get(item_index)? {
            MirItem::Fun(fun) if fun.body.is_some() => Some(fun),
            _ => None,
        }
    }

    /// 以稳定顺序枚举 MIR-owned top-level initializer/value roots。
    pub fn initializer_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.mir_facts.roots.initializer_fqns()
    }

    /// 按 root FQN 查询 top-level initializer/value root。
    pub fn initializer_root(&self, fqn: &str) -> Option<&MirInitializerRoot> {
        let item_index = self.mir_facts.roots.initializer(fqn)?.item.index;
        match self.file().items.get(item_index)? {
            MirItem::InitializerRoot(root) => Some(root),
            _ => None,
        }
    }

    /// 以稳定顺序枚举 MIR-owned global/extern roots。
    pub fn global_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.mir_facts.roots.extern_global_fqns()
    }

    /// 按 FQN 查询 `@Extern` global root contract。
    pub fn extern_global_root(&self, fqn: &str) -> Option<&MirExternGlobalRoot> {
        let item_index = self.mir_facts.roots.extern_global(fqn)?.item.index;
        match self.file().items.get(item_index)? {
            MirItem::ExternGlobal(root) => Some(root),
            _ => None,
        }
    }

    /// 以稳定顺序枚举 MIR-owned type/object/typealias metadata roots。
    pub fn metadata_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.mir_facts.roots.metadata_root_fqns()
    }

    /// 按 FQN 查询 MIR declaration metadata root。
    pub fn metadata_root(&self, fqn: &str) -> Option<&MirMetadataRoot> {
        let item_index = self.mir_facts.roots.metadata_root(fqn)?.item.index;
        match self.file().items.get(item_index)? {
            MirItem::Metadata(root) => Some(root),
            _ => None,
        }
    }

    /// `dump-mir` / `mir_lowered` fixtures / 定向单测共用的稳定文本 surface。
    ///
    /// P3-T02 起，这个 formatter 就是 direct-style MIR + MIR facts 的 snapshot/golden 基线：
    /// - 必须稳定暴露 direct-style MIR body / CFG；
    /// - 必须保留 `SiteId`、cleanup/finally target，以及 `Call / Perform / Resume / Handle`
    ///   的关键 metadata；
    /// - 不能在 CLI、fixture runner、或单测之间各自拼接不同文本。
    pub fn stable_dump(&self) -> String {
        let mut out = crate::mir::stable_dump_file(self.file(), self.types());
        out.push('\n');
        out.push('\n');
        out.push_str(&self.mir_facts.dump());
        out.push('\n');
        out
    }

    pub fn into_lowered_mir(self) -> LoweredMir {
        self.lowered_mir
    }
}

impl MirStageOutput {
    pub(crate) fn from_direct_style(
        mut direct_style: DirectStyleMirStageOutput,
        materialized_mir: MaterializedMir,
    ) -> Self {
        publish_materialized_handoff_facts(&mut direct_style.mir_facts, &materialized_mir);
        direct_style
            .mir_facts
            .verify()
            .expect("P4-ready MIR stage output must publish structurally valid MIR facts");
        Self {
            direct_style,
            materialized_mir,
        }
    }

    pub fn file(&self) -> &MirFile {
        self.direct_style.file()
    }

    pub fn types(&self) -> &TypeStore {
        self.direct_style.types()
    }

    /// Return MIR-owned facts published by this P4-ready handoff.
    pub fn mir_facts(&self) -> &MirFacts {
        self.direct_style.mir_facts()
    }

    /// Return the mandatory canonical materialized MIR snapshot handed to P4.
    pub fn materialized_mir(&self) -> &MaterializedMir {
        &self.materialized_mir
    }

    pub fn materialized_pass_view(&self) -> MaterializedMirPassView<'_> {
        self.materialized_mir.pass_view()
    }

    /// 以稳定顺序枚举当前 direct-style MIR 中可查询的 callable body 身份。
    pub fn callable_body_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.direct_style.callable_body_fqns()
    }

    /// 按 callable body 身份查询 canonical direct-style MIR body。
    pub fn callable_body(&self, fqn: &str) -> Option<&MirFunDecl> {
        self.direct_style.callable_body(fqn)
    }

    /// 以稳定顺序枚举 MIR-owned top-level initializer/value roots。
    pub fn initializer_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.direct_style.initializer_root_fqns()
    }

    /// 按 root FQN 查询 top-level initializer/value root。
    pub fn initializer_root(&self, fqn: &str) -> Option<&MirInitializerRoot> {
        self.direct_style.initializer_root(fqn)
    }

    /// 以稳定顺序枚举 MIR-owned global/extern roots。
    pub fn global_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.direct_style.global_root_fqns()
    }

    /// 按 FQN 查询 `@Extern` global root contract。
    pub fn extern_global_root(&self, fqn: &str) -> Option<&MirExternGlobalRoot> {
        self.direct_style.extern_global_root(fqn)
    }

    /// 以稳定顺序枚举 MIR-owned type/object/typealias metadata roots。
    pub fn metadata_root_fqns(&self) -> impl Iterator<Item = &str> + '_ {
        self.direct_style.metadata_root_fqns()
    }

    /// 按 FQN 查询 MIR declaration metadata root。
    pub fn metadata_root(&self, fqn: &str) -> Option<&MirMetadataRoot> {
        self.direct_style.metadata_root(fqn)
    }

    /// Stable text surface for the P4-ready MIR handoff.
    pub fn stable_dump(&self) -> String {
        self.direct_style.stable_dump()
    }

    pub fn into_direct_style(self) -> DirectStyleMirStageOutput {
        self.direct_style
    }

    #[cfg_attr(not(feature = "llvm"), allow(dead_code))]
    pub(crate) fn into_parts(self) -> (DirectStyleMirStageOutput, MaterializedMir) {
        (self.direct_style, self.materialized_mir)
    }
}

fn build_direct_style_mir_facts(
    file: &MirFile,
    stable_cone_key: &StableConeKey,
    source_cones: &HashMap<PathBuf, crate::cone::SourceConeInfo>,
    source_cone_order: &HashMap<StableConeKey, u32>,
) -> MirFacts {
    let mut facts = MirFacts::new();
    facts.roots = collect_root_inventories(file, stable_cone_key, source_cones, source_cone_order);
    facts.metadata = collect_mir_metadata_facts(file, stable_cone_key);
    facts
        .verify()
        .expect("MIR stage must publish structurally valid MIR facts");
    facts
}

fn publish_materialized_handoff_facts(facts: &mut MirFacts, materialized: &MaterializedMir) {
    let snapshot_key = canonical_snapshot_key(materialized);
    let canonical_body_fqns = canonical_materialized_body_fqns(materialized);
    facts.snapshots = SnapshotBindings {
        canonical: Some(snapshot_key.clone()),
        snapshots: vec![
            MaterializedSnapshotBinding::new(
                snapshot_key.clone(),
                materialized.stable_cone_key().clone(),
                materialized.opt_level(),
                canonical_body_fqns.len(),
                0,
            )
            .with_canonical_body_fqns(canonical_body_fqns),
        ],
    };
    facts.families = collect_instance_family_inventory(materialized);
    facts.pass_artifacts = collect_pass_artifact_metadata(materialized, &snapshot_key);
    facts.pass_pipeline = collect_pass_pipeline_metadata(materialized, &snapshot_key);
}

fn canonical_snapshot_key(materialized: &MaterializedMir) -> StageArtifactKey {
    StageArtifactKey::new(
        MIR_STAGE_LABEL,
        materialized.stable_cone_key(),
        format!(
            "{CANONICAL_SNAPSHOT_ROLE}@{}",
            materialized.opt_level().as_str()
        ),
        0,
    )
}

fn canonical_materialized_body_fqns(materialized: &MaterializedMir) -> Vec<String> {
    let pass_view = materialized.pass_view();
    let mut fqns = BTreeSet::new();
    for family in pass_view.instances() {
        for fun in family.callable_bodies() {
            fqns.insert(fun.fqn.clone());
        }
    }
    fqns.into_iter().collect()
}

fn collect_instance_family_inventory(materialized: &MaterializedMir) -> InstanceFamilyInventory {
    let cone = materialized.stable_cone_key().clone();
    let pass_view = materialized.pass_view();
    let mut instances = Vec::new();
    let mut callable_families = Vec::new();

    for family in pass_view.instances() {
        let instance_artifact = materialized_instance_artifact(materialized, family.key());
        let root_fqn = family.root_fqn().to_string();
        let body = family
            .root_body()
            .map(|fun| materialized_body_reference(&instance_artifact, fun));

        instances.push(InstanceInventoryEntry::new(
            FactIdentity::new(
                CanonicalTextKey::new(instance_artifact.canonical_text()),
                format!("instance {root_fqn}"),
                cone.clone(),
                None,
            ),
            instance_artifact.clone(),
            CanonicalTextKey::new(root_fqn.clone()),
            family.key().type_args.clone(),
            body.clone(),
        ));

        let family_artifact =
            StageArtifactKey::new(MIR_STAGE_LABEL, &instance_artifact, CALLABLE_FAMILY_ROLE, 0);
        callable_families.push(CallableFamilyFact::new(
            FactIdentity::new(
                CanonicalTextKey::new(family_artifact.canonical_text()),
                format!("callable family {root_fqn}"),
                cone.clone(),
                None,
            ),
            CanonicalTextKey::new(root_fqn),
            body,
            vec![instance_artifact],
        ));
    }

    instances.sort_by(|left, right| {
        left.artifact
            .canonical_text()
            .cmp(&right.artifact.canonical_text())
    });
    callable_families.sort_by(|left, right| {
        left.identity
            .canonical_text()
            .cmp(right.identity.canonical_text())
    });

    InstanceFamilyInventory {
        instances,
        callable_families,
    }
}

fn collect_pass_artifact_metadata(
    materialized: &MaterializedMir,
    snapshot_key: &StageArtifactKey,
) -> PassArtifactMetadata {
    let initial_revision_key = pass_artifact_revision_key(snapshot_key, 0);
    let mut metadata = PassArtifactMetadata {
        revisions: vec![PassArtifactRevision::new(
            initial_revision_key.clone(),
            "canonical-pass-artifacts",
            0,
        )],
        callable_body_overrides: Vec::new(),
        summary_revisions: Vec::new(),
        escape_facts: Vec::new(),
    };

    for run in materialized.pass_artifacts().pipeline_runs() {
        let Some(revision) = run.output_revision else {
            continue;
        };
        metadata.revisions.push(PassArtifactRevision::new(
            pass_artifact_revision_key(snapshot_key, revision),
            run.pass.as_str(),
            revision,
        ));
    }

    let pass_view = materialized.pass_view();
    let mut overridden_body_fqns = materialized
        .pass_artifacts()
        .overridden_body_fqns()
        .collect::<Vec<_>>();
    overridden_body_fqns.sort_unstable();
    for fqn in overridden_body_fqns {
        if let Some(fun) = pass_view.callable(fqn) {
            let revision = materialized
                .pass_artifacts()
                .body_override_revision(fqn)
                .unwrap_or(0);
            let revision_key = pass_artifact_revision_key(snapshot_key, revision);
            metadata
                .callable_body_overrides
                .push(CallableBodyArtifact::new(
                    revision_key.clone(),
                    pass_artifact_body_reference(&revision_key, fun),
                ));
        }
    }

    let mut summary_owners = pass_view
        .instances()
        .map(|family| materialized_instance_artifact(materialized, family.key()))
        .collect::<Vec<_>>();
    summary_owners.sort_by_key(|key| key.canonical_text());
    metadata.summary_revisions.extend(
        summary_owners
            .into_iter()
            .map(|owner| SummaryArtifact::new(initial_revision_key.clone(), owner)),
    );

    let mut overridden_summary_instances = materialized
        .pass_artifacts()
        .overridden_summary_instances()
        .collect::<Vec<_>>();
    overridden_summary_instances.sort_by_key(|instance| {
        materialized
            .authoritative_stable_instance_key(instance)
            .map(|key| key.canonical_text())
            .unwrap_or_default()
    });
    for instance in overridden_summary_instances {
        let revision = materialized
            .pass_artifacts()
            .summary_override_revision(instance)
            .unwrap_or(0);
        let revision_key = pass_artifact_revision_key(snapshot_key, revision);
        metadata.summary_revisions.push(SummaryArtifact::new(
            revision_key,
            materialized_instance_artifact(materialized, instance),
        ));
    }

    let escape_body_count = pass_view.escape_facts().callables().count();
    if escape_body_count > 0 {
        let revision = materialized
            .pass_artifacts()
            .escape_facts_revision()
            .unwrap_or(0);
        let revision_key = pass_artifact_revision_key(snapshot_key, revision);
        metadata
            .escape_facts
            .push(EscapeFactsArtifact::new(revision_key, escape_body_count));
    }

    metadata
}

fn collect_pass_pipeline_metadata(
    materialized: &MaterializedMir,
    snapshot_key: &StageArtifactKey,
) -> MirPassPipelineMetadata {
    let runs = materialized
        .pass_artifacts()
        .pipeline_runs()
        .iter()
        .map(|record| {
            let mut run = MirPassRun::new(record.pass.clone(), record.enabled);
            run.input_revision = Some(pass_artifact_revision_key(
                snapshot_key,
                record.input_revision,
            ));
            run.output_revision = record
                .output_revision
                .map(|revision| pass_artifact_revision_key(snapshot_key, revision));
            run.changed_bodies = record.changed_bodies;
            run.changed_summaries = record.changed_summaries;
            run.produced_escape_facts = record.produced_escape_facts;
            run
        })
        .collect();
    MirPassPipelineMetadata { runs }
}

fn pass_artifact_revision_key(snapshot_key: &StageArtifactKey, revision: u32) -> StageArtifactKey {
    StageArtifactKey::new(MIR_STAGE_LABEL, snapshot_key, PASS_ARTIFACT_ROLE, revision)
}

fn materialized_instance_artifact(
    materialized: &MaterializedMir,
    instance: &InstanceKey,
) -> StageArtifactKey {
    let stable_instance_key = materialized
        .authoritative_stable_instance_key(instance)
        .expect("materialized MIR instance should have a stable exported identity");
    StageArtifactKey::new(
        MIR_STAGE_LABEL,
        &stable_instance_key,
        MATERIALIZED_INSTANCE_ROLE,
        0,
    )
}

fn materialized_body_reference(owner: &StageArtifactKey, fun: &MirFunDecl) -> MirBodyReference {
    body_reference(owner, MATERIALIZED_BODY_ROLE, fun)
}

fn pass_artifact_body_reference(owner: &StageArtifactKey, fun: &MirFunDecl) -> MirBodyReference {
    body_reference(owner, "pass_artifact_body", fun)
}

fn body_reference(owner: &StageArtifactKey, role: &str, fun: &MirFunDecl) -> MirBodyReference {
    let owner_key = CanonicalTextKey::new(owner.canonical_text());
    MirBodyReference::new(
        BodyVersionKey::new(&owner_key, role, 0),
        owner_key,
        fun.fqn.clone(),
        Some(fun.return_ty),
    )
}

fn collect_root_inventories(
    file: &MirFile,
    stable_cone_key: &StableConeKey,
    source_cones: &HashMap<PathBuf, crate::cone::SourceConeInfo>,
    source_cone_order: &HashMap<StableConeKey, u32>,
) -> RootInventories {
    let mut callable_bodies = BTreeMap::new();
    let mut initializers = BTreeMap::new();
    let mut initializer_dependencies = Vec::new();
    let mut extern_globals = BTreeMap::new();
    let mut metadata_roots = BTreeMap::new();

    for (item_index, item) in file.items.iter().enumerate() {
        match item {
            MirItem::Fun(fun) if fun.body.is_some() => {
                callable_bodies
                    .entry(fun.fqn.clone())
                    .or_insert_with(|| callable_body_root_fact(item_index, fun, stable_cone_key));
            }
            MirItem::InitializerRoot(root) => {
                if let Entry::Vacant(entry) = initializers.entry(root.fqn.clone()) {
                    let root_cone_key = source_path_cone_key(
                        root.source_path.as_path(),
                        source_cones,
                        stable_cone_key,
                    );
                    entry.insert(initializer_root_fact(
                        item_index,
                        root,
                        &root_cone_key,
                        source_cone_order.get(&root_cone_key).copied(),
                    ));
                    initializer_dependencies.extend(initializer_dependency_facts(root));
                }
            }
            MirItem::ExternGlobal(root) => {
                if let Entry::Vacant(entry) = extern_globals.entry(root.fqn.clone()) {
                    let root_cone_key = source_path_cone_key(
                        root.source_path.as_path(),
                        source_cones,
                        stable_cone_key,
                    );
                    entry.insert(extern_global_root_fact(
                        item_index,
                        root,
                        &root_cone_key,
                        source_cone_order.get(&root_cone_key).copied(),
                    ));
                }
            }
            MirItem::Metadata(root) => {
                metadata_roots
                    .entry(root.fqn().to_string())
                    .or_insert_with(|| metadata_root_fact(item_index, root, stable_cone_key));
            }
            MirItem::Fun(_) | MirItem::Todo { .. } => {}
        }
    }

    RootInventories {
        callable_bodies: callable_bodies.into_values().collect(),
        initializers: initializers.into_values().collect(),
        initializer_dependencies,
        extern_globals: extern_globals.into_values().collect(),
        metadata_roots: metadata_roots.into_values().collect(),
    }
}

fn source_path_cone_key(
    source_path: &Path,
    source_cones: &HashMap<PathBuf, crate::cone::SourceConeInfo>,
    fallback: &StableConeKey,
) -> StableConeKey {
    source_cones
        .get(source_path)
        .map(|info| info.stable_key.clone())
        .unwrap_or_else(|| fallback.clone())
}

fn collect_mir_metadata_facts(file: &MirFile, stable_cone_key: &StableConeKey) -> MirMetadataFacts {
    let mut nominal_direct_supertypes = BTreeMap::new();

    for item in &file.items {
        match item {
            MirItem::Metadata(MirMetadataRoot::Nominal(nominal)) => {
                nominal_direct_supertypes
                    .entry(nominal.fqn.clone())
                    .or_insert_with(|| {
                        nominal_direct_supertypes_fact(
                            &nominal.fqn,
                            MirNominalOwnerKind::Nominal,
                            &nominal.supertypes,
                            stable_cone_key,
                        )
                    });
            }
            MirItem::Metadata(MirMetadataRoot::Object(object)) => {
                nominal_direct_supertypes
                    .entry(object.fqn.clone())
                    .or_insert_with(|| {
                        nominal_direct_supertypes_fact(
                            &object.fqn,
                            MirNominalOwnerKind::Object,
                            &object.supertypes,
                            stable_cone_key,
                        )
                    });
            }
            _ => {}
        }
    }

    MirMetadataFacts {
        nominal_direct_supertypes: nominal_direct_supertypes.into_values().collect(),
    }
}

fn nominal_direct_supertypes_fact(
    fqn: &str,
    owner_kind: MirNominalOwnerKind,
    supertypes: &[crate::mir::SupertypeMetadata],
    stable_cone_key: &StableConeKey,
) -> NominalDirectSupertypesFact {
    let direct_supertypes = supertypes
        .iter()
        .filter_map(|supertype| supertype.fqn.clone())
        .collect();
    NominalDirectSupertypesFact::new(
        FactIdentity::new(
            CanonicalTextKey::new(format!("mir_metadata:nominal_direct_supertypes:{fqn}")),
            format!("nominal direct supertypes {fqn}"),
            stable_cone_key.clone(),
            None,
        ),
        owner_kind,
        fqn,
        direct_supertypes,
    )
}

fn callable_body_root_fact(
    item_index: usize,
    fun: &MirFunDecl,
    stable_cone_key: &StableConeKey,
) -> MirRootFact {
    let kind = MirRootKind::CallableBody;
    let identity = root_identity(kind, &fun.fqn, stable_cone_key);
    let body_owner = identity.key.clone();
    let body = MirBodyReference::new(
        BodyVersionKey::new(&body_owner, DIRECT_STYLE_BODY_ROLE, 0),
        body_owner,
        fun.fqn.clone(),
        Some(fun.return_ty),
    );

    MirRootFact::new(
        identity,
        kind,
        fun.fqn.clone(),
        MirItemReference::new(item_index),
        MirRootDetail::CallableBody,
    )
    .with_ty(Some(fun.ty))
    .with_body(Some(body))
    .with_span(Some(fun.span))
}

fn initializer_root_fact(
    item_index: usize,
    root: &MirInitializerRoot,
    stable_cone_key: &StableConeKey,
    source_cone_order: Option<u32>,
) -> MirRootFact {
    let kind = MirRootKind::Initializer;
    MirRootFact::new(
        root_identity(kind, &root.fqn, stable_cone_key),
        kind,
        root.fqn.clone(),
        MirItemReference::new(item_index),
        MirRootDetail::Initializer {
            kind: fact_initializer_root_kind(root.kind),
            has_initializer: root.has_initializer,
            dependency_count: root.dependencies.len(),
        },
    )
    .with_ty(root.ty)
    .with_source_path(Some(normalize_dump_path(&root.source_path)))
    .with_source_cone_order(source_cone_order)
    .with_span(Some(root.span))
}

fn extern_global_root_fact(
    item_index: usize,
    root: &MirExternGlobalRoot,
    stable_cone_key: &StableConeKey,
    source_cone_order: Option<u32>,
) -> MirRootFact {
    let kind = MirRootKind::ExternGlobal;
    MirRootFact::new(
        root_identity(kind, &root.fqn, stable_cone_key),
        kind,
        root.fqn.clone(),
        MirItemReference::new(item_index),
        MirRootDetail::ExternGlobal {
            storage: fact_global_storage_kind(root.storage),
            mutable: root.mutable,
            symbol: root.symbol.clone(),
            initializer_absent: root.initializer_absent,
            unsafe_required: root.unsafe_required,
        },
    )
    .with_ty(Some(root.ty))
    .with_source_path(Some(normalize_dump_path(&root.source_path)))
    .with_source_cone_order(source_cone_order)
    .with_span(Some(root.span))
}

fn metadata_root_fact(
    item_index: usize,
    root: &MirMetadataRoot,
    stable_cone_key: &StableConeKey,
) -> MirRootFact {
    let kind = MirRootKind::Metadata;
    MirRootFact::new(
        root_identity(kind, root.fqn(), stable_cone_key),
        kind,
        root.fqn().to_string(),
        MirItemReference::new(item_index),
        MirRootDetail::Metadata {
            kind: fact_metadata_root_kind(root),
        },
    )
    .with_ty(metadata_root_ty(root))
    .with_span(Some(metadata_root_span(root)))
}

fn root_identity(kind: MirRootKind, fqn: &str, stable_cone_key: &StableConeKey) -> FactIdentity {
    FactIdentity::new(
        CanonicalTextKey::new(format!("mir_root:{}:{fqn}", kind.label())),
        fqn,
        stable_cone_key.clone(),
        None,
    )
}

fn fact_initializer_root_kind(kind: crate::mir::InitializerRootKind) -> FactInitializerRootKind {
    match kind {
        crate::mir::InitializerRootKind::RuntimeImmutableVal => {
            FactInitializerRootKind::RuntimeImmutableVal
        }
        crate::mir::InitializerRootKind::RuntimeMutableVar { storage } => match storage {
            crate::hir::TopLevelVarStorage::Global => {
                FactInitializerRootKind::RuntimeMutableGlobalVar
            }
            crate::hir::TopLevelVarStorage::ThreadLocal => {
                FactInitializerRootKind::RuntimeMutableThreadLocalVar
            }
        },
        crate::mir::InitializerRootKind::ObjectSingleton => {
            FactInitializerRootKind::ObjectSingleton
        }
    }
}

fn initializer_dependency_facts(root: &MirInitializerRoot) -> Vec<MirInitializerDependencyFact> {
    root.dependencies
        .iter()
        .map(|dependency| MirInitializerDependencyFact {
            owner_fqn: root.fqn.clone(),
            target_fqn: dependency.fqn.clone(),
            kind: fact_initializer_dependency_kind(dependency.kind),
        })
        .collect()
}

fn fact_initializer_dependency_kind(
    kind: crate::mir::InitializerDependencyKind,
) -> FactInitializerDependencyKind {
    match kind {
        crate::mir::InitializerDependencyKind::TopLevelValue => {
            FactInitializerDependencyKind::TopLevelValue
        }
        crate::mir::InitializerDependencyKind::ObjectSingleton => {
            FactInitializerDependencyKind::ObjectSingleton
        }
    }
}

fn fact_global_storage_kind(storage: crate::hir::TopLevelVarStorage) -> MirGlobalStorageKind {
    match storage {
        crate::hir::TopLevelVarStorage::Global => MirGlobalStorageKind::Global,
        crate::hir::TopLevelVarStorage::ThreadLocal => MirGlobalStorageKind::ThreadLocal,
    }
}

fn fact_metadata_root_kind(root: &MirMetadataRoot) -> MirMetadataRootKind {
    match root {
        MirMetadataRoot::TypeAlias(_) => MirMetadataRootKind::TypeAlias,
        MirMetadataRoot::Nominal(_) => MirMetadataRootKind::Nominal,
        MirMetadataRoot::Object(_) => MirMetadataRootKind::Object,
        MirMetadataRoot::ExtensionProperty(_) => MirMetadataRootKind::ExtensionProperty,
    }
}

fn metadata_root_span(root: &MirMetadataRoot) -> crate::span::Span {
    match root {
        MirMetadataRoot::TypeAlias(alias) => alias.span,
        MirMetadataRoot::Nominal(nominal) => nominal.span,
        MirMetadataRoot::Object(object) => object.span,
        MirMetadataRoot::ExtensionProperty(prop) => prop.span,
    }
}

fn metadata_root_ty(root: &MirMetadataRoot) -> Option<TypeId> {
    match root {
        MirMetadataRoot::TypeAlias(alias) => Some(alias.ty),
        MirMetadataRoot::Nominal(_) | MirMetadataRoot::Object(_) => None,
        MirMetadataRoot::ExtensionProperty(prop) => Some(prop.ty),
    }
}

fn validate_bodies(
    file: &MirFile,
    types: &TypeStore,
    unit_ty: TypeId,
    bool_ty: TypeId,
) -> Result<(), MirLowerError> {
    file.validate_production(types, unit_ty, bool_ty)
        .map_err(|error| MirLowerError::InvalidMir {
            fqn: error.body_fqn().unwrap_or("<file>").to_string(),
            error: Box::new(error),
        })
}

fn lower_mir_stage_unvalidated(
    hir_output: HirStageOutput,
) -> (DirectStyleMirStageOutput, TypeId, TypeId) {
    let facts = MirLoweringFacts::from_hir_facts(hir_output.lowered_hir(), hir_output.hir_facts());
    let mut lowered_hir = hir_output.into_lowered_hir();
    let stable_cone_key = lowered_hir.stable_cone_key.clone();
    let builtins = lowered_hir.types.intern_builtins();
    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let types = std::mem::replace(&mut lowered_hir.types, TypeStore::new());

    (
        DirectStyleMirStageOutput::new(
            LoweredMir { file, types },
            stable_cone_key,
            &lowered_hir.source_cones,
            &lowered_hir.source_cone_order,
        ),
        builtins.unit,
        builtins.bool_,
    )
}

pub(crate) fn run(hir_output: HirStageOutput) -> Result<DirectStyleMirStageOutput, MirLowerError> {
    let (output, unit_ty, bool_ty) = lower_mir_stage_unvalidated(hir_output);
    validate_bodies(output.file(), output.types(), unit_ty, bool_ty)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::DirectStyleMirStageOutput;
    use crate::ast;
    use crate::mir::{
        AggregateTransportKind, ArrayTransportOperation, CallKind, GcIntrinsicOperation,
        GcIntrinsicPairing, GcRootLifetime, HandlerArmKind, InitializerDependencyKind,
        InitializerRootKind, Item, MemberTarget, MetadataRoot, MirBoxingReason, MirCallableAbiKind,
        MirCallableImplPlan, MirLoweringFacts, MirTransportKind, Operand, Pattern,
        RuntimeCastFailure, RuntimeCastResult, RuntimePatternTypeTestKind,
        RuntimeTypeDescriptorKind, RuntimeTypeParameterizedMatch, RuntimeTypeStaticFold, Rvalue,
        StatementKind, TerminatorKind, UnwindAction, ValueTransportMetadata,
        lower_hir_file_for_dump_with_facts,
    };
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::ty::TypeStore;
    use scoop_project_model::StableConeKey;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    fn run_fixture(phase: &str, name: &str) -> DirectStyleMirStageOutput {
        let session = session();
        let source = load_fixture(phase, name);
        let typed_hir_output =
            super::super::load_hir_stage_output_for_dump(&session, &source).unwrap();
        super::run(typed_hir_output).expect("fixture 应可通过 MIR stage")
    }

    fn callable_body<'a>(output: &'a DirectStyleMirStageOutput, fqn: &str) -> &'a crate::mir::Body {
        output
            .callable_body(fqn)
            .and_then(|fun| fun.body.as_ref())
            .unwrap_or_else(|| panic!("应找到 callable body: {fqn}"))
    }

    fn validated_callable_body<'a>(
        output: &'a DirectStyleMirStageOutput,
        fqn: &str,
    ) -> &'a crate::mir::Body {
        let body = callable_body(output, fqn);
        body.validate_direct_style()
            .unwrap_or_else(|err| panic!("MIR body `{fqn}` 应通过验证器: {err}"));
        body
    }

    fn unit_operand_is_visible_in_body(
        output: &DirectStyleMirStageOutput,
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
    fn direct_mir_stage_output_is_constructible() {
        let session = session();
        let source = SourceFile::new_virtual(
            "<mem>",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        );

        let typed_hir_output =
            super::super::load_hir_stage_output_for_dump(&session, &source).unwrap();
        let output = super::run(typed_hir_output).unwrap();

        assert_eq!(output.file().items.len(), 2);
        assert!(output.callable_body("sample.helper").is_some());
        assert!(output.callable_body("sample.main").is_some());
        assert!(output.stable_dump().contains("FunDecl"));
        assert!(output.stable_dump().contains("mir_facts {"));
        assert_eq!(output.mir_facts().roots.callable_bodies.len(), 2);
        assert!(output.mir_facts().snapshots.canonical.is_none());
    }

    #[test]
    fn mir_stable_dump_normalizes_workspace_source_paths() {
        let session = session();
        let source = load_fixture("mir_lowered", "top_level_roots.scoop");

        let output = super::super::load_direct_style_mir_stage_output_for_dump(&session, &source)
            .expect("top-level roots fixture should produce strict MIR");
        let dump = output.stable_dump();

        assert!(
            dump.contains("source_path: \"tests/fixtures/mir_lowered/top_level_roots.scoop\""),
            "stable dump should use workspace-relative source paths: {dump}"
        );
        assert!(
            !dump.contains(env!("CARGO_MANIFEST_DIR")),
            "stable dump must not embed machine-local manifest paths: {dump}"
        );
    }

    #[test]
    fn mir_stable_dump_uses_semantic_types_and_stable_labels() {
        let session = session();
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
    fn mir_item_graph_publishes_top_level_roots() {
        let session = session();
        let source = SourceFile::new_virtual(
            "<mem>/mir_item_graph.scoop",
            r#"package sample
import scoop.core.*

typealias Alias = Int
interface Named
struct Point(val x: Int) : Named

val Base: Int = 1
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
            super::super::load_hir_stage_output_for_dump(&session, &source).unwrap();
        let output = super::run(typed_hir_output).unwrap();

        assert!(
            output
                .file()
                .items
                .iter()
                .all(|item| !matches!(item, Item::Todo { .. })),
            "MIR item graph must not contain top-level declaration Todo: {:#?}",
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
        let runtime_fact = output
            .mir_facts()
            .roots
            .initializer("sample.Runtime")
            .expect("runtime initializer should be published as a MIR fact");
        assert!(matches!(
            &runtime_fact.detail,
            scoopc_mir_facts::roots::MirRootDetail::Initializer {
                kind: scoopc_mir_facts::roots::MirInitializerRootKind::RuntimeImmutableVal,
                has_initializer: true,
                dependency_count: 1,
            }
        ));
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
        let native_fact = output
            .mir_facts()
            .roots
            .extern_global("sample.NativeCounter")
            .expect("extern global should be published as a MIR fact");
        assert!(matches!(
            &native_fact.detail,
            scoopc_mir_facts::roots::MirRootDetail::ExternGlobal {
                storage: scoopc_mir_facts::roots::MirGlobalStorageKind::Global,
                symbol,
                initializer_absent: true,
                unsafe_required: true,
                ..
            } if symbol == "native_counter"
        ));
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
            output
                .mir_facts()
                .roots
                .metadata_root("sample.Alias")
                .map(|fact| &fact.detail),
            Some(scoopc_mir_facts::roots::MirRootDetail::Metadata {
                kind: scoopc_mir_facts::roots::MirMetadataRootKind::TypeAlias,
            })
        ));
        assert!(matches!(
            output.metadata_root("sample.Point"),
            Some(MetadataRoot::Nominal(nominal)) if nominal.name == "Point"
        ));
        assert_eq!(
            output
                .mir_facts()
                .metadata
                .direct_supertypes("sample.Point"),
            Some(["sample.Named".to_string()].as_slice())
        );
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
    fn mir_place_contract_lowers_assignment_places() {
        let output = run_fixture("mir_lowered", "assignment_places.scoop");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("Todo"),
            "MIR assignment place lowering must not leak Todo placeholders: {dump}"
        );

        let native = output
            .extern_global_root("mir_lowered.assignment_places.NativeCounter")
            .expect("extern global root should be published");
        assert!(native.unsafe_required);

        let body = validated_callable_body(&output, "mir_lowered.assignment_places.use");
        let mut saw_global_store = false;
        let mut saw_extern_store = false;
        let mut captured_local_assign_count = 0usize;
        let mut box_value_store_count = 0usize;

        for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
            match &stmt.kind {
                StatementKind::StoreTopLevelVar { fqn, .. }
                    if fqn == "mir_lowered.assignment_places.G" =>
                {
                    saw_global_store = true;
                }
                StatementKind::StoreTopLevelVar { fqn, .. }
                    if fqn == "mir_lowered.assignment_places.NativeCounter" =>
                {
                    saw_extern_store = true;
                }
                StatementKind::Assign { target, .. }
                    if body.locals[target.as_u32() as usize].name.as_deref()
                        == Some("captured") =>
                {
                    captured_local_assign_count += 1;
                }
                StatementKind::StoreMember { member, .. }
                    if matches!(
                        member.resolved.as_ref(),
                        Some(MemberTarget::Value { fqn })
                            if fqn == "mir_lowered.assignment_places.Box.value"
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
            captured_local_assign_count >= 2,
            "mutable local should remain an ordinary assignable local: {dump}"
        );
        assert!(
            box_value_store_count >= 2,
            "direct and nested member stores should target Box.value: {dump}"
        );
    }

    #[test]
    fn mir_closure_var_capture_is_rejected_before_mir() {
        let session = session();
        let source = SourceFile::new_virtual(
            "<mem>/closure_mutable_capture_per_call.scoop",
            r#"package sample
import scoop.core.*

fun callTwice(f: () -> Int): Int {
    val a: Int = f()
    val b: Int = f()
    return a * 100 + b * 10
}

fun main(): Int {
    var x: Int = 0
    val f: () -> Int = {
        x = x + 1
        x
    }
    return callTwice(f) + x
}
"#,
        );

        let err = super::super::load_hir_stage_output_for_dump(&session, &source)
            .expect_err("closure var capture should be rejected before MIR");
        let report = format!("{err:?}");
        assert!(
            report.contains("closure_var_capture_not_allowed")
                || report.contains("ClosureVarCaptureNotAllowed"),
            "expected closure var capture diagnostic, got: {report}"
        );
    }

    #[test]
    fn mir_place_contract_rejects_invalid_inputs_before_mir() {
        let session = session();
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
            let err = super::super::load_hir_stage_output_for_dump(&session, &source).unwrap_err();
            let report = format!("{err:?}");
            assert!(
                report.contains(expected),
                "expected diagnostic `{expected}` for {name}, got: {report}"
            );
        }
    }

    #[test]
    fn mir_call_contract_lowers_typed_call_sites() {
        let output = run_fixture("mir_lowered", "call_contracts.scoop");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("Todo"),
            "MIR call lowering must not leak Todo placeholders: {dump}"
        );

        let main = validated_callable_body(&output, "mir_lowered.call_contracts.main");
        let apply = validated_callable_body(&output, "mir_lowered.call_contracts.callFn");
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
                    if callee_fqn == "mir_lowered.call_contracts.namedDefault" {
                        assert_eq!(
                            args.len(),
                            2,
                            "default args should be canonicalized before MIR direct call lowering: {stmt:#?}"
                        );
                        saw_named_default_call = true;
                    }
                    if callee_fqn == "mir_lowered.call_contracts.ext" {
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
                } if class_fqn == "mir_lowered.call_contracts.Box" && args.len() == 1 => {
                    assert_eq!(ctor.ordered_param_count, 1);
                    assert!(ctor.selected_ctor_span.is_some());
                    saw_class_ctor = true;
                }
                StatementKind::Assign {
                    value: Rvalue::SizeOf { value_ty },
                    ..
                } if output.types().display(*value_ty).to_string()
                    == "mir_lowered.call_contracts.Box" =>
                {
                    saw_size_of = true;
                }
                StatementKind::Assign {
                    value: Rvalue::TypeMetadataLiteral(metadata),
                    ..
                } if metadata.source_fqn.as_deref() == Some("mir_lowered.call_contracts.Box") => {
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
            "mir_lowered.call_contracts.direct",
            "mir_lowered.call_contracts.generic",
            "mir_lowered.call_contracts.namedDefault",
            "mir_lowered.call_contracts.ext",
            "mir_lowered.call_contracts.Singleton.get",
            "mir_lowered.call_contracts.callFn",
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
    fn mir_funptr_calls_lower_to_explicit_funptr_kind() {
        let session = session();
        let source = SourceFile::new_virtual(
            "<mem>/mir_funptr.scoop",
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
            super::super::load_hir_stage_output_for_dump(&session, &source).unwrap();
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
    fn mir_ctor_default_args_lower_to_ordered_class_ctor() {
        let session = session();
        let source = SourceFile::new_virtual(
            "<mem>/mir_ctor_default_args.scoop",
            r#"package sample

class Pair(val first: Int = 7, val second: Int)

fun main(): Int {
    val pair: Pair = Pair(second = 6)
    return pair.first + pair.second
}
"#,
        );

        let typed_hir_output =
            super::super::load_hir_stage_output_for_dump(&session, &source).unwrap();
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
    fn mir_value_primitives_record_typecheck_and_cast_metadata() {
        let output = run_fixture("mir_lowered", "runtime_typecheck_cast.scoop");
        let body = validated_callable_body(&output, "mir_lowered.runtime_typecheck_cast.inspect");
        let mut saw_iface_is = false;
        let mut saw_other_not_is = false;
        let mut saw_parameterized_holder_is = false;
        let mut saw_as_panic = false;
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
                        ) if fqn == "mir_lowered.runtime_typecheck_cast.IFace" => {
                            saw_iface_is = true;
                        }
                        (
                            RuntimeTypeDescriptorKind::Nominal {
                                fqn,
                                kind: Some(ast::TypeKind::Class),
                            },
                            ast::TypeCheckOp::NotIs,
                        ) if fqn == "mir_lowered.runtime_typecheck_cast.Other" => {
                            saw_other_not_is = true;
                        }
                        (
                            RuntimeTypeDescriptorKind::Nominal {
                                fqn,
                                kind: Some(ast::TypeKind::Class),
                            },
                            ast::TypeCheckOp::Is,
                        ) if fqn == "mir_lowered.runtime_typecheck_cast.Holder" => {
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
                            RuntimeCastFailure::Panic { message },
                            RuntimeCastResult::Target { ty },
                        ) => {
                            assert_eq!(*ty, *target_ty);
                            assert_eq!(message, "class cast failed");
                            saw_as_panic = true;
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
        assert!(saw_as_panic, "missing `as` failure panic metadata");
        assert!(saw_asq_none, "missing `as?` none-result metadata");
    }

    #[test]
    fn mir_value_primitives_not_null_assert_is_explicit_match_and_panic() {
        let output = run_fixture("mir_lowered", "not_null_assert.scoop");
        let body = validated_callable_body(&output, "mir_lowered.not_null_assert.unwrap");
        let mut saw_some_match = false;
        let mut saw_none_match = false;
        let mut saw_extract = false;
        let mut saw_panic = false;

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
                    StatementKind::Assign {
                        value:
                            Rvalue::Call {
                                kind: CallKind::Direct { callee_fqn },
                                ..
                            },
                        ..
                    } => saw_panic |= callee_fqn == "scoop.core.panic",
                    _ => {}
                }
            }
        }

        assert!(saw_some_match, "`!!` success arm should test Some payload");
        assert!(saw_none_match, "`!!` failure arm should test None");
        assert!(saw_extract, "`!!` success arm should extract payload");
        assert!(saw_panic, "`!!` failure arm should call panic");
    }

    #[test]
    fn mir_value_primitives_pattern_is_type_metadata_is_classified() {
        let output = run_fixture("mir_lowered", "pattern_is_type.scoop");
        let body = validated_callable_body(&output, "mir_lowered.pattern_is_type.classify");
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
                    } if fqn == "mir_lowered.pattern_is_type.Box" => {
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
    fn mir_value_primitives_reject_open_function_type_cast_before_mir() {
        let session = session();
        let source = SourceFile::new_virtual(
            "<mem>/open_function_type_cast.scoop",
            r#"package sample
import scoop.core.*

fun bad() {
    val f: () -> Int / Pure! = { 1 }
    val a: Any = f
    val g: (() -> Int / Pure)? = a as? (() -> Int / Pure)
    val _ = g
}
"#,
        );
        let err = super::super::load_hir_stage_output_for_dump(&session, &source)
            .expect_err("open function type runtime cast must be rejected before MIR");
        let report = format!("{err:?}");
        assert!(
            report.contains("FunctionTypeCastNotSupported")
                || report.contains("function_type_cast_not_supported"),
            "expected function-type cast diagnostic, got: {report}"
        );
    }

    #[test]
    fn mir_aggregate_transport_records_composite_contracts() {
        let output = run_fixture("mir_lowered", "aggregate_transport.scoop");
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
            body.validate_direct_style()
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
                        Rvalue::MakeClosure { env_contract, .. }
                            if !env_contract.captures.is_empty() =>
                        {
                            saw_closure_env = true;
                            assert!(
                                env_contract.captures.iter().all(|capture| !capture.mutable),
                                "typecheck should reject mutable closure captures before MIR: {dump}"
                            );
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
    fn mir_composite_transport_metadata_contracts() {
        mir_aggregate_transport_records_composite_contracts();
    }

    #[test]
    fn mir_value_boxing_transport_contract() {
        let output = run_fixture("mir_lowered", "value_boxing_transport.scoop");
        let dump = output.stable_dump();
        assert!(
            !dump.contains("Todo"),
            "value boxing transport fixture must not leak MIR Todo: {dump}"
        );

        let top = output
            .initializer_root("mir_lowered.value_boxing_transport.topAny")
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
    fn mir_no_todo_stage_validator_rejects_item_todo() {
        const SYNTHETIC_ITEM_TODO_REASON: &str = "synthetic item todo";

        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let file = crate::mir::File {
            items: vec![crate::mir::Item::Todo {
                span: crate::span::Span::new(0, 1),
                kind: SYNTHETIC_ITEM_TODO_REASON.to_string(),
            }],
        };

        let err = super::validate_bodies(&file, &types, builtins.unit, builtins.bool_)
            .expect_err("production stage validator should reject item Todo");
        let rendered = err.to_string();
        assert!(rendered.contains("<file>"));
        assert!(rendered.contains(SYNTHETIC_ITEM_TODO_REASON));
    }

    #[test]
    fn direct_mir_stage_keeps_callable_body_query_surface_stable() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let output = DirectStyleMirStageOutput::new(
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
            StableConeKey::new("fixture", "0.0.0"),
            &HashMap::new(),
            &HashMap::new(),
        );

        assert_eq!(
            output.callable_body_fqns().collect::<Vec<_>>(),
            vec!["sample.main"]
        );
        assert!(output.callable_body("sample.main").is_some());
        assert_eq!(output.mir_facts().roots.callable_bodies[0].item.index, 0);
    }

    #[test]
    fn mir_effect_site_contract_keeps_dispatch_and_resume_sites_explicit() {
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
                if callee_fqn == "a.id" && callee_fqn_2 == "a.callFn"
        ));

        let apply_body = callable_body(&direct_output, "a.callFn");
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
    fn mir_effect_site_contract_records_perform_and_handle_metadata() {
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
    #[should_panic(expected = "perform source-site contract missing before MIR lowering")]
    fn mir_effect_site_contract_missing_perform_contract_is_stage_error() {
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
        let _ = lower_with_empty_contracts(&source);
    }

    #[test]
    #[should_panic(expected = "handle source-site contract missing before MIR lowering")]
    fn mir_effect_site_contract_missing_handle_contract_is_stage_error() {
        let source = load_fixture("mir_lowered", "handle_perform.scoop");
        let _ = lower_with_empty_contracts(&source);
    }

    #[test]
    fn mir_effect_site_contract_canonicalizes_resume_unit_sugar() {
        let output = run_fixture("mir_lowered", "continuation_resume_unit_sugar.scoop");
        let body = callable_body(&output, "fixtures.mir_lowered.resumeUnit");

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
                } if callee_fqn == "fixtures.mir_lowered.takesUnit" => Some(args),
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

    fn lower_with_empty_contracts(
        source: &SourceFile,
    ) -> (crate::mir::File, crate::ty::TypeId, crate::ty::TypeId) {
        let session = session();
        let typed_hir_output = super::super::load_hir_stage_output_for_dump(&session, source)
            .expect("typed HIR should pass before forged contract lowering");
        let facts = MirLoweringFacts::default();
        let mut lowered_hir = typed_hir_output.into_lowered_hir();
        let builtins = lowered_hir.types.intern_builtins();
        let file = lower_hir_file_for_dump_with_facts(
            builtins,
            &mut lowered_hir.types,
            &lowered_hir.file,
            &lowered_hir.member_funs,
            &facts,
        );
        (file, builtins.unit, builtins.bool_)
    }

    #[test]
    fn mir_cfg_existing_control_flow_samples_validate() {
        let while_output = run_fixture("mir", "while_break_continue.scoop");
        validated_callable_body(&while_output, "a.main");

        let if_when_output = run_fixture("mir", "if_when.scoop");
        validated_callable_body(&if_when_output, "a.main");
    }

    #[test]
    fn mir_cfg_handle_finally_boundary_is_explicit() {
        let output = run_fixture("mir_lowered", "handle_finally_boundary.scoop");

        let completes = validated_callable_body(&output, "fixtures.mir_lowered.body_completes");
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

        let raised = validated_callable_body(&output, "fixtures.mir_lowered.handled_raise");
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
    fn mir_policy_gates_keep_resume_unwind_cleanup_contract() {
        let output = run_fixture("mir_lowered", "handle_finally_boundary.scoop");
        let raised = validated_callable_body(&output, "fixtures.mir_lowered.handled_raise");

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
    fn mir_policy_gates_publish_gc_pin_handle_intrinsic_contracts() {
        let session = session();
        let source = SourceFile::new_virtual(
            "<mem>/gc_policy_gates.scoop",
            r#"package fixtures.mir_lowered

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
        let typed_hir_output = super::super::load_hir_stage_output_for_dump(&session, &source)
            .expect("GC policy fixture should typecheck before MIR");
        let output = super::run(typed_hir_output).expect("GC policy fixture should lower to MIR");
        let body = callable_body(&output, "fixtures.mir_lowered.main");
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
    fn mir_gc_handle_raw_uintptr_token_stays_scalar() {
        let session = session();
        let source = SourceFile::new_virtual(
            "<mem>/gc_handle_uintptr_policy.scoop",
            r#"package fixtures.mir_lowered

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
        let typed_hir_output = super::super::load_hir_stage_output_for_dump(&session, &source)
            .expect("GC handle raw UIntPtr fixture should typecheck before MIR");
        let output = super::run(typed_hir_output)
            .expect("GC handle raw UIntPtr fixture should lower to MIR");
        let body = callable_body(&output, "fixtures.mir_lowered.main");

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
                } if callee_fqn == "fixtures.mir_lowered.handleTokenSlotTake" => Some(transport),
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
    fn mir_cfg_effect_boundary_inside_expr_context_uses_explicit_blocks() {
        let output = run_fixture("mir_lowered", "effect_boundary_inside_expr_context.scoop");
        let body = validated_callable_body(&output, "fixtures.mir_lowered.main");

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
                    } if callee_fqn == "fixtures.mir_lowered.box_int"
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
    fn mir_cfg_escape_continuation_finally_materializes_continuation_local() {
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
