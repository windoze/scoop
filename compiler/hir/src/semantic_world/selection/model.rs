use std::collections::BTreeMap;

use std::sync::Arc;

use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, PersistentExactTypeId, PersistentPropertyId,
    PersistentSourceContextId, PersistentTypeAliasId, SignatureTypeKey, SourceContextKey,
    SourceIdentity,
};

use crate::{
    CallableDeclarationRecordV1, CallableSourceInterfaceV1, DirectImportedTargetBinding,
    ExportConstValueV1, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1, ImportedTarget,
    ParamFreeNominalCallableV1, PropertyDeclarationRecordV1, SourceRecord,
    TypeAliasInterfaceRecordV1,
};

#[derive(Clone, Debug)]
pub(super) struct ImportedDependencyDefinitionSources {
    pub(super) records: BTreeMap<SourceIdentity, SourceRecord>,
    pub(super) contexts: BTreeMap<PersistentSourceContextId, SourceContextKey>,
}

impl ImportedDependencyDefinitionSources {
    pub(super) fn resolve(
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

/// Source metadata needed to preserve the definition side of an inlined
/// dependency expression.
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
/// dependency binding. It owns the interface needed by lowering after the
/// borrowed artifact views have been released.
#[derive(Clone, Debug)]
pub struct ImportedDependencyCallableCandidate {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) provider: ConeIdentity,
    pub(super) interface: CallableDeclarationRecordV1,
    pub(super) source: Option<CallableSourceInterfaceV1>,
    pub(super) capability: Option<ParamFreeNominalCallableV1>,
    pub(super) default_templates: BTreeMap<ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
    pub(super) callable_body: Option<Arc<crate::ExportGenericCallableBodyV1>>,
}

impl ImportedDependencyCallableCandidate {
    pub fn callable_body(&self) -> Option<&crate::ExportGenericCallableBodyV1> {
        self.callable_body.as_deref()
    }

    pub const fn target(&self) -> ImportedTarget {
        self.binding.target()
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn interface(&self) -> &CallableDeclarationRecordV1 {
        &self.interface
    }

    pub const fn source_interface(&self) -> Option<&CallableSourceInterfaceV1> {
        self.source.as_ref()
    }

    pub const fn capability(&self) -> Option<&ParamFreeNominalCallableV1> {
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
/// binding. Its exact type is present when the value can be materialized.
#[derive(Clone, Debug)]
pub struct ImportedDependencyConstantCandidate {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) provider: ConeIdentity,
    pub(super) record: ExportConstValueV1,
    pub(super) exact_type: Option<PersistentExactTypeId>,
    pub(super) definition_sources: Arc<ImportedDependencyDefinitionSources>,
}

/// An owned public dependency property reached through one direct binding.
/// Accessor selection remains a separate typed step so a property cannot be
/// mistaken for a callable declaration or a provider storage identity.
#[derive(Clone, Debug)]
pub struct ImportedDependencyPropertyCandidate {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) provider: ConeIdentity,
    pub(super) interface: PropertyDeclarationRecordV1,
}

/// An owned transparent type-alias candidate reached through one direct
/// dependency binding. Its final target was already expanded and validated
/// over the complete dependency closure before this request-local snapshot
/// was built.
#[derive(Clone, Debug)]
pub struct ImportedDependencyTypeAliasCandidate {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) provider: ConeIdentity,
    pub(super) interface: TypeAliasInterfaceRecordV1,
    pub(super) expansion: SignatureTypeKey,
}

impl ImportedDependencyTypeAliasCandidate {
    pub const fn target(&self) -> ImportedTarget {
        self.binding.target()
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
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
        self.provider
    }

    pub const fn interface(&self) -> &PropertyDeclarationRecordV1 {
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
        self.provider
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

/// Typed reference to one selected dependency callable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedDependencyCallableRef {
    pub(super) callable: CallableTemplateOrigin,
}

impl ImportedDependencyCallableRef {
    pub const fn declaration(self) -> CallableTemplateOrigin {
        self.callable
    }
}

/// Complete HIR input for one selected dependency callable.
#[derive(Clone, Debug)]
pub struct SelectedImportedDependencyCallable {
    pub(super) provider: ConeIdentity,
    pub(super) interface: CallableDeclarationRecordV1,
    pub(super) source: Option<CallableSourceInterfaceV1>,
    pub(super) capability: ParamFreeNominalCallableV1,
    pub(super) initialization_unit: Option<scoop_identity::PersistentInitializationUnitId>,
}

impl SelectedImportedDependencyCallable {
    /// The directly ensured source unit, when this callable is a property accessor.
    pub const fn initialization_unit(
        &self,
    ) -> Option<scoop_identity::PersistentInitializationUnitId> {
        self.initialization_unit
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn interface(&self) -> &CallableDeclarationRecordV1 {
        &self.interface
    }

    pub const fn source_interface(&self) -> Option<&CallableSourceInterfaceV1> {
        self.source.as_ref()
    }

    pub const fn capability(&self) -> &ParamFreeNominalCallableV1 {
        &self.capability
    }
}

/// Complete HIR input for one inlined dependency constant.
#[derive(Clone, Debug)]
pub struct SelectedImportedDependencyConstant {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) provider: ConeIdentity,
    pub(super) record: ExportConstValueV1,
    pub(super) exact_type: PersistentExactTypeId,
}

impl SelectedImportedDependencyConstant {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
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

/// Typed reference to one selected dependency constant.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedDependencyConstantRef {
    pub(super) constant: PersistentPropertyId,
}

/// Typed reference to one selected transparent dependency alias.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedDependencyTypeAliasRef {
    pub(super) alias: PersistentTypeAliasId,
}

/// Complete HIR input for one selected dependency type alias.
#[derive(Clone, Debug)]
pub struct SelectedImportedDependencyTypeAlias {
    pub(super) binding: DirectImportedTargetBinding,
    pub(super) provider: ConeIdentity,
    pub(super) interface: TypeAliasInterfaceRecordV1,
    pub(super) expansion: SignatureTypeKey,
}

impl SelectedImportedDependencyTypeAlias {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
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

/// The selected declarations, interfaces, and import paths required by the
/// current HIR output.
#[derive(Debug)]
pub struct SelectedImportedDependencySet {
    pub(super) consumer: ConeIdentity,
    pub(super) callables: BTreeMap<CallableTemplateOrigin, SelectedImportedDependencyCallable>,
    pub(super) constants: BTreeMap<PersistentPropertyId, SelectedImportedDependencyConstant>,
    pub(super) type_aliases: BTreeMap<PersistentTypeAliasId, SelectedImportedDependencyTypeAlias>,
}

impl SelectedImportedDependencySet {
    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub fn resolve_callable(
        &self,
        reference: ImportedDependencyCallableRef,
    ) -> Option<&SelectedImportedDependencyCallable> {
        self.callables.get(&reference.callable)
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
        self.constants.get(&reference.constant)
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
        self.type_aliases.get(&reference.alias)
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
