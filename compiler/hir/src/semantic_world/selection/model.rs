use std::collections::BTreeMap;

use std::sync::Arc;

use scoop_identity::{ConeIdentity, PersistentSourceContextId, SourceContextKey, SourceIdentity};

use crate::{
    CallableInterfaceRecordV1, CallableSourceInterfaceV1, DirectImportedTargetBinding,
    ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1, ImportedProviderCertificate,
    ImportedTarget, ParamFreeCoreClosedCallableV1, SourceRecord,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct DependencyProjectionId(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct DependencySelectionId(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct ImportedDependencyCallableId(pub(super) u32);

#[derive(Clone, Debug)]
pub(super) struct ImportedDependencyDefinitionSources {
    pub(super) records: BTreeMap<SourceIdentity, SourceRecord>,
    pub(super) contexts: BTreeMap<PersistentSourceContextId, SourceContextKey>,
}

/// Authenticated provider source metadata needed to preserve the definition
/// side of an imported default expression.
#[derive(Clone, Copy, Debug)]
pub struct ImportedDependencyDefinitionSource<'a> {
    record: &'a SourceRecord,
    context: &'a SourceContextKey,
}

impl<'a> ImportedDependencyDefinitionSource<'a> {
    pub const fn record(self) -> &'a SourceRecord {
        self.record
    }

    pub const fn context(self) -> &'a SourceContextKey {
        self.context
    }
}

/// An owned source-level callable candidate obtained through one direct
/// dependency binding. It can outlive the borrowed artifact views used to
/// build the semantic world, but it can only be committed by a clone of the
/// exact selection plan that minted it.
#[derive(Clone, Debug)]
pub struct ImportedDependencyCallableCandidate {
    pub(super) projection: DependencyProjectionId,
    pub(super) callable: ImportedDependencyCallableId,
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) interface: CallableInterfaceRecordV1,
    pub(super) source: Option<CallableSourceInterfaceV1>,
    pub(super) capability: Option<ParamFreeCoreClosedCallableV1>,
    pub(super) default_templates: BTreeMap<ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

impl ImportedDependencyCallableCandidate {
    pub const fn target(&self) -> ImportedTarget {
        self.binding.target()
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub const fn interface(&self) -> &CallableInterfaceRecordV1 {
        &self.interface
    }

    pub const fn source_interface(&self) -> Option<&CallableSourceInterfaceV1> {
        self.source.as_ref()
    }

    pub const fn capability(&self) -> Option<&ParamFreeCoreClosedCallableV1> {
        self.capability.as_ref()
    }

    pub const fn binding(&self) -> &DirectImportedTargetBinding {
        &self.binding
    }

    pub fn default_template(
        &self,
        key: ExportDefaultTemplateKeyV1,
    ) -> Option<&ExportDefaultTemplateV1> {
        self.default_templates.get(&key)
    }

    pub fn definition_source(
        &self,
        source: &crate::ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        let origin = source.origin();
        Some(ImportedDependencyDefinitionSource {
            record: self.definition_sources.records.get(origin.source())?,
            context: self.definition_sources.contexts.get(&origin.context())?,
        })
    }
}

/// Request-local reference to one committed dependency callable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedDependencyCallableRef {
    pub(super) selection: DependencySelectionId,
    pub(super) callable: ImportedDependencyCallableId,
}

/// Complete HIR-side proof for one committed dependency callable.
#[derive(Clone, Debug)]
pub struct SelectedImportedDependencyCallable {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) interface: CallableInterfaceRecordV1,
    pub(super) source: Option<CallableSourceInterfaceV1>,
    pub(super) capability: ParamFreeCoreClosedCallableV1,
}

impl SelectedImportedDependencyCallable {
    pub const fn provider(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub const fn interface(&self) -> &CallableInterfaceRecordV1 {
        &self.interface
    }

    pub const fn source_interface(&self) -> Option<&CallableSourceInterfaceV1> {
        self.source.as_ref()
    }

    pub const fn capability(&self) -> &ParamFreeCoreClosedCallableV1 {
        &self.capability
    }

    pub const fn binding(&self) -> &DirectImportedTargetBinding {
        &self.binding
    }
}

/// Canonical request-local sidecar that owns all selected HIR dependency
/// proofs and artifact reopen certificates.
#[derive(Debug)]
pub struct SelectedImportedDependencySet {
    pub(super) consumer: ConeIdentity,
    pub(super) selection: DependencySelectionId,
    pub(super) callables:
        BTreeMap<ImportedDependencyCallableId, SelectedImportedDependencyCallable>,
}

impl SelectedImportedDependencySet {
    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub fn resolve_callable(
        &self,
        reference: ImportedDependencyCallableRef,
    ) -> Option<&SelectedImportedDependencyCallable> {
        (reference.selection == self.selection)
            .then(|| self.callables.get(&reference.callable))
            .flatten()
    }

    pub fn callables(
        &self,
    ) -> impl ExactSizeIterator<Item = &SelectedImportedDependencyCallable> + '_ {
        self.callables.values()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty()
    }

    pub fn len(&self) -> usize {
        self.callables.len()
    }
}
