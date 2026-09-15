use scoop_identity::DefinitionOriginSubject;

use super::*;

impl DefinitionOriginBuilder<'_> {
    pub(super) fn collect_nominals(
        &mut self,
        identities: &hir::HirNominalIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (id, declaration) in self.lowerer.structs.iter() {
            self.append_source_nominal(
                &identities[id],
                self.lowerer.struct_files.get(&id).copied(),
                declaration.span,
            )?;
        }
        for (id, declaration) in self.lowerer.enums.iter() {
            self.append_source_nominal(
                &identities[id],
                self.lowerer.enum_files.get(&id).copied(),
                declaration.span,
            )?;
        }
        for (id, declaration) in self.lowerer.classes.iter() {
            self.append_source_nominal(
                &identities[id],
                self.lowerer.class_files.get(&id).copied(),
                declaration.span,
            )?;
        }
        for (id, declaration) in self.lowerer.interfaces.iter() {
            self.append_source_nominal(
                &identities[id],
                self.lowerer.interface_files.get(&id).copied(),
                declaration.span,
            )?;
        }
        for (id, declaration) in self.lowerer.objects.iter() {
            self.append_source_nominal(
                &identities[id],
                self.lowerer.object_files.get(&id).copied(),
                declaration.span,
            )?;
        }
        Ok(())
    }

    pub(super) fn collect_functions(
        &mut self,
        identities: &hir::HirFunctionIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (id, declaration) in self.lowerer.functions.iter() {
            let Some(identity) = identities[id].source_identity() else {
                continue;
            };
            let subject = match identity {
                hir::HirSourceFunctionIdentity::Plain(record) => {
                    DefinitionOriginSubject::Function(record.id())
                }
                hir::HirSourceFunctionIdentity::Generic(record) => {
                    DefinitionOriginSubject::GenericFunction(record.id())
                }
            };
            let file = self.source_file(
                subject,
                declaration.span,
                self.lowerer.function_files.get(&id).copied(),
            )?;
            self.append(subject, file, declaration.span)?;
        }
        Ok(())
    }

    pub(super) fn collect_constructors(
        &mut self,
        identities: &hir::HirConstructorIdentities,
    ) -> Result<(), PersistentDefinitionOriginError> {
        for (id, declaration) in self.lowerer.struct_constructors.iter() {
            let subject = DefinitionOriginSubject::Constructor(identities[id].id());
            let file = self.source_file(
                subject,
                declaration.span,
                self.lowerer.struct_files.get(&declaration.owner).copied(),
            )?;
            self.append(subject, file, declaration.span)?;
        }
        for (id, declaration) in self.lowerer.class_constructors.iter() {
            let subject = match &identities[id] {
                hir::HirClassConstructorIdentity::Source(record) => {
                    DefinitionOriginSubject::Constructor(record.id())
                }
                hir::HirClassConstructorIdentity::ZeroArgumentAdapter { record, .. } => {
                    DefinitionOriginSubject::GeneratedCallable(record.id())
                }
            };
            let file = self.source_file(
                subject,
                declaration.span,
                self.lowerer.class_files.get(&declaration.owner).copied(),
            )?;
            self.append(subject, file, declaration.span)?;
        }
        Ok(())
    }

    fn append_source_nominal(
        &mut self,
        identity: &hir::HirNominalIdentity,
        file: Option<usize>,
        span: scoop_ast::Span,
    ) -> Result<(), PersistentDefinitionOriginError> {
        let Some(identity) = identity.source() else {
            return Ok(());
        };
        let subject = match identity {
            hir::HirSourceNominalIdentity::Concrete(record) => {
                DefinitionOriginSubject::Type(record.id())
            }
            hir::HirSourceNominalIdentity::Generic(record) => {
                DefinitionOriginSubject::GenericType(record.id())
            }
        };
        let file = self.source_file(subject, span, file)?;
        self.append(subject, file, span)
    }
}
