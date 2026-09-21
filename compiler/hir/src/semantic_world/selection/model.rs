use std::collections::BTreeMap;

use std::sync::Arc;

use scoop_identity::{
    ConeIdentity, PersistentExactTypeId, PersistentSourceContextId, SignatureTypeKey,
    SourceContextKey, SourceIdentity,
};

use crate::{
    CallableInterfaceRecordV1, CallableSourceInterfaceV1, DirectImportedTargetBinding,
    ExportConstValueV1, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
    ImportedProviderCertificate, ImportedTarget, ParamFreeCoreClosedCallableV1,
    PropertyInterfaceRecordV1, SourceRecord, TypeAliasInterfaceRecordV1,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct DependencyProjectionId(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct DependencySelectionId(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct ImportedDependencyCallableId(pub(super) u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct ImportedDependencyConstantId(pub(super) u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct ImportedDependencyTypeAliasId(pub(super) u32);

#[derive(Clone, Debug)]
pub(super) struct ImportedDependencyDefinitionSources {
    pub(super) records: BTreeMap<SourceIdentity, SourceRecord>,
    pub(super) contexts: BTreeMap<PersistentSourceContextId, SourceContextKey>,
}

impl ImportedDependencyDefinitionSources {
    fn resolve(
        &self,
        source: &crate::ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        let origin = source.origin();
        Some(ImportedDependencyDefinitionSource {
            record: self.records.get(origin.source())?,
            context: self.contexts.get(&origin.context())?,
        })
    }
}

/// Authenticated provider source metadata needed to preserve the definition
/// side of an inlined dependency expression.
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
        self.definition_sources.resolve(source)
    }
}

/// An owned public constant candidate reached through one direct dependency
/// binding. Its exact type is present only for the M23-5 core-closed subset.
#[derive(Clone, Debug)]
pub struct ImportedDependencyConstantCandidate {
    pub(super) projection: DependencyProjectionId,
    pub(super) constant: ImportedDependencyConstantId,
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) record: ExportConstValueV1,
    pub(super) exact_type: Option<PersistentExactTypeId>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

/// An owned public dependency property reached through one direct binding.
/// Accessor selection remains a separate typed step so a property cannot be
/// mistaken for a callable declaration or a provider storage identity.
#[derive(Clone, Debug)]
pub struct ImportedDependencyPropertyCandidate {
    pub(super) projection: DependencyProjectionId,
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) interface: PropertyInterfaceRecordV1,
}

/// An owned transparent type-alias candidate reached through one direct
/// dependency binding. Its final target was already expanded and validated
/// over the complete dependency closure before this request-local snapshot
/// was built.
#[derive(Clone, Debug)]
pub struct ImportedDependencyTypeAliasCandidate {
    pub(super) projection: DependencyProjectionId,
    pub(super) alias: ImportedDependencyTypeAliasId,
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) interface: TypeAliasInterfaceRecordV1,
    pub(super) expansion: SignatureTypeKey,
}

impl ImportedDependencyTypeAliasCandidate {
    pub const fn target(&self) -> ImportedTarget {
        self.binding.target()
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub const fn interface(&self) -> &TypeAliasInterfaceRecordV1 {
        &self.interface
    }

    pub const fn expansion(&self) -> &SignatureTypeKey {
        &self.expansion
    }

    pub const fn binding(&self) -> &DirectImportedTargetBinding {
        &self.binding
    }
}

impl ImportedDependencyPropertyCandidate {
    pub const fn target(&self) -> ImportedTarget {
        self.binding.target()
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub const fn interface(&self) -> &PropertyInterfaceRecordV1 {
        &self.interface
    }

    pub const fn binding(&self) -> &DirectImportedTargetBinding {
        &self.binding
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportedDependencyPropertyAccessorKind {
    Getter,
    Setter,
}

impl ImportedDependencyConstantCandidate {
    pub const fn target(&self) -> ImportedTarget {
        self.binding.target()
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub const fn record(&self) -> &ExportConstValueV1 {
        &self.record
    }

    pub const fn exact_type(&self) -> Option<PersistentExactTypeId> {
        self.exact_type
    }

    pub const fn binding(&self) -> &DirectImportedTargetBinding {
        &self.binding
    }

    pub fn definition_source(
        &self,
        source: &crate::ExportDefinitionSourceV1,
    ) -> Option<ImportedDependencyDefinitionSource<'_>> {
        self.definition_sources.resolve(source)
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

/// Complete HIR-side proof for one inlined dependency constant.
#[derive(Clone, Debug)]
pub struct SelectedImportedDependencyConstant {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) record: ExportConstValueV1,
    pub(super) exact_type: PersistentExactTypeId,
}

impl SelectedImportedDependencyConstant {
    pub const fn provider(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub const fn record(&self) -> &ExportConstValueV1 {
        &self.record
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.exact_type
    }

    pub const fn binding(&self) -> &DirectImportedTargetBinding {
        &self.binding
    }
}

/// Request-local reference to one committed dependency constant proof.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedDependencyConstantRef {
    pub(super) selection: DependencySelectionId,
    pub(super) constant: ImportedDependencyConstantId,
}

/// Request-local reference to one committed transparent dependency alias.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedDependencyTypeAliasRef {
    pub(super) selection: DependencySelectionId,
    pub(super) alias: ImportedDependencyTypeAliasId,
}

/// Complete HIR-side proof for one committed dependency type alias.
#[derive(Clone, Debug)]
pub struct SelectedImportedDependencyTypeAlias {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) certificate: ImportedProviderCertificate,
    pub(super) interface: TypeAliasInterfaceRecordV1,
    pub(super) expansion: SignatureTypeKey,
}

impl SelectedImportedDependencyTypeAlias {
    pub const fn provider(&self) -> ConeIdentity {
        self.certificate.identity()
    }

    pub const fn certificate(&self) -> &ImportedProviderCertificate {
        &self.certificate
    }

    pub const fn interface(&self) -> &TypeAliasInterfaceRecordV1 {
        &self.interface
    }

    pub const fn expansion(&self) -> &SignatureTypeKey {
        &self.expansion
    }

    pub const fn binding(&self) -> &DirectImportedTargetBinding {
        &self.binding
    }
}

/// Canonical request-local sidecar that owns all selected HIR dependency
/// proofs and artifact reopen certificates.
#[derive(Debug)]
pub struct SelectedImportedDependencySet {
    pub(super) direct_binding_witnesses:
        Arc<BTreeMap<crate::ExternalHirTargetV1, Vec<crate::DependencyBindingWitnessV1>>>,
    pub(super) consumer: ConeIdentity,
    pub(super) selection: DependencySelectionId,
    pub(super) callables:
        BTreeMap<ImportedDependencyCallableId, SelectedImportedDependencyCallable>,
    pub(super) constants:
        BTreeMap<ImportedDependencyConstantId, SelectedImportedDependencyConstant>,
    pub(super) type_aliases:
        BTreeMap<ImportedDependencyTypeAliasId, SelectedImportedDependencyTypeAlias>,
}

impl SelectedImportedDependencySet {
    pub(crate) fn direct_binding_witnesses(
        &self,
        target: crate::ExternalHirTargetV1,
    ) -> &[crate::DependencyBindingWitnessV1] {
        self.direct_binding_witnesses
            .get(&target)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

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

    pub fn resolve_constant(
        &self,
        reference: ImportedDependencyConstantRef,
    ) -> Option<&SelectedImportedDependencyConstant> {
        (reference.selection == self.selection)
            .then(|| self.constants.get(&reference.constant))
            .flatten()
    }

    pub fn constants(
        &self,
    ) -> impl ExactSizeIterator<Item = &SelectedImportedDependencyConstant> + '_ {
        self.constants.values()
    }

    pub fn resolve_type_alias(
        &self,
        reference: ImportedDependencyTypeAliasRef,
    ) -> Option<&SelectedImportedDependencyTypeAlias> {
        (reference.selection == self.selection)
            .then(|| self.type_aliases.get(&reference.alias))
            .flatten()
    }

    pub fn type_aliases(
        &self,
    ) -> impl ExactSizeIterator<Item = &SelectedImportedDependencyTypeAlias> + '_ {
        self.type_aliases.values()
    }

    pub fn callable_count(&self) -> usize {
        self.callables.len()
    }

    pub fn constant_count(&self) -> usize {
        self.constants.len()
    }

    pub fn type_alias_count(&self) -> usize {
        self.type_aliases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.callables.is_empty() && self.constants.is_empty() && self.type_aliases.is_empty()
    }

    pub fn len(&self) -> usize {
        self.callables.len() + self.constants.len() + self.type_aliases.len()
    }
}
