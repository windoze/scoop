use scoop_identity::DefinitionOriginSubject;

use super::*;

impl DefinitionOriginBuilder<'_> {
    pub(super) fn collect_initialization_units(
        &mut self,
        identities: &hir::HirInitializationUnitIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (id, declaration) in self.lowerer.initialization_units.iter() {
            let subject = DefinitionOriginSubject::InitializationUnit(identities[id].id());
            let file = match declaration.kind {
                hir::InitializationUnitKind::EagerTopLevel { property, .. }
                | hir::InitializationUnitKind::GenericDelegatedExtension { property, .. } => {
                    self.lowerer.property_source_file(property)
                }
                hir::InitializationUnitKind::LazySingleton { value, .. } => {
                    let object = self.lowerer.singleton_values[value].declaration;
                    self.lowerer.object_files.get(&object).copied()
                }
            };
            let file = self.source_file(subject, declaration.span, file)?;
            self.append(subject, file, declaration.span)?;
        }
        Ok(())
    }

    pub(super) fn collect_local_bindings(&mut self, identities: &hir::HirLocalBindingIdentities) {
        for identity in identities.iter() {
            self.append_existing(
                DefinitionOriginSubject::LocalBinding(identity.record().id()),
                identity.origin(),
            );
        }
    }

    pub(super) fn collect_callbacks(
        &mut self,
        identities: &hir::HirCallbackRegistrationIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (id, declaration) in self.lowerer.foreign_callback_registrations.iter() {
            let subject = DefinitionOriginSubject::CallbackRegistration(identities[id].id());
            let root = match &declaration.definition {
                hir::ForeignCallbackDefinition::Source { root, .. } => *root,
                hir::ForeignCallbackDefinition::Imported {
                    definition_origin, ..
                } => {
                    self.append_existing(subject, definition_origin);
                    continue;
                }
            };
            let file = match root {
                hir::LexicalDefinitionRoot::Function(function) => {
                    self.lowerer.function_files.get(&function).copied()
                }
                hir::LexicalDefinitionRoot::ClassConstructor(constructor) => self
                    .lowerer
                    .class_files
                    .get(&self.lowerer.class_constructors[constructor].owner)
                    .copied(),
                hir::LexicalDefinitionRoot::StructConstructor(constructor) => self
                    .lowerer
                    .struct_files
                    .get(&self.lowerer.struct_constructors[constructor].owner)
                    .copied(),
                hir::LexicalDefinitionRoot::VariantConstructor(variant) => {
                    self.lowerer.enum_files.get(&variant.enumeration()).copied()
                }
            };
            let file = self.source_file(subject, declaration.span, file)?;
            self.append(subject, file, declaration.span)?;
        }
        Ok(())
    }

    pub(super) fn collect_native_contracts(
        &mut self,
        contracts: &hir::HirSourceNativeContracts,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for contract in contracts.iter() {
            let subject = DefinitionOriginSubject::SourceNativeContract(contract.record().id());
            let (file, span) = match contract.owner() {
                hir::HirSourceNativeContractOwner::Function(function) => (
                    self.lowerer.function_files.get(&function).copied(),
                    self.lowerer.functions[function].span,
                ),
                hir::HirSourceNativeContractOwner::Global(global) => {
                    let declaration = &self.lowerer.globals[global];
                    (
                        self.lowerer.property_source_file(declaration.property),
                        declaration.span,
                    )
                }
            };
            let file = self.source_file(subject, span, file)?;
            self.append(subject, file, span)?;
        }
        Ok(())
    }
}
