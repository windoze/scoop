use scoop_identity::ConeIdentity;

use super::{
    LayoutAbiDependencyV1, LayoutAbiExportConstituentsV1, LayoutAbiSectionError,
    LayoutAbiSemanticRecordV1, LayoutAbiSemanticTargetV1,
};
use crate::{CanonicalExternalShapeLinkImportsV1, ExternalShapeLinkImportV1};

/// Actual dependency export records selected for the Strong V2 writer.
///
/// This selection is deliberately independent of the consumer's local
/// layout/ABI exports. It can therefore materialize final-LIR external
/// definitions before local V2 registrations exist; the completed layout/ABI
/// section is built and joined after Strong production.
pub struct StrongProductionDependencySelectionV2<'a> {
    consumer: ConeIdentity,
    target: crate::LirTargetProfile,
    dependencies: Vec<&'a LayoutAbiExportConstituentsV1>,
    semantic: Vec<LayoutAbiDependencyV1>,
    physical: CanonicalExternalShapeLinkImportsV1,
}

impl<'a> StrongProductionDependencySelectionV2<'a> {
    pub fn empty(
        consumer: ConeIdentity,
        target: crate::LirTargetProfile,
    ) -> Result<Self, LayoutAbiSectionError> {
        Ok(Self {
            consumer,
            target,
            dependencies: Vec::new(),
            semantic: Vec::new(),
            physical: CanonicalExternalShapeLinkImportsV1::from_checked(Vec::new())?,
        })
    }

    pub fn try_new(
        consumer: ConeIdentity,
        target: crate::LirTargetProfile,
        dependencies: &[&'a LayoutAbiExportConstituentsV1],
        physical_imports: Vec<ExternalShapeLinkImportV1>,
        roots: &[LayoutAbiDependencyV1],
    ) -> Result<Self, LayoutAbiSectionError> {
        let dependencies = super::section::dependencies::complete(consumer, target, dependencies)?;
        let physical = CanonicalExternalShapeLinkImportsV1::from_checked(physical_imports)?;
        let semantic = super::semantic_closure::close_external(consumer, &dependencies, roots)?;

        for import in physical.records() {
            if import.provider() == consumer {
                return Err(LayoutAbiSectionError::SelectedCurrentProvider);
            }
            let terminal = dependencies
                .binary_search_by_key(&import.provider(), |section| section.provider())
                .ok()
                .map(|index| dependencies[index])
                .ok_or(LayoutAbiSectionError::MissingPhysicalProvider(
                    import.provider(),
                ))?;
            let Some(target) = semantic_target(import.subject(), terminal)
                .map_err(LayoutAbiSectionError::MissingPhysicalSubject)?
            else {
                continue;
            };
            let relation = LayoutAbiDependencyV1::new(import.provider(), target);
            if semantic.binary_search(&relation).is_err() {
                return Err(LayoutAbiSectionError::MissingPhysicalSemantic(relation));
            }
        }
        Ok(Self {
            consumer,
            target,
            dependencies,
            semantic,
            physical,
        })
    }

    pub const fn consumer(&self) -> ConeIdentity {
        self.consumer
    }

    pub const fn target_profile(&self) -> crate::LirTargetProfile {
        self.target
    }

    pub const fn physical_imports(&self) -> &CanonicalExternalShapeLinkImportsV1 {
        &self.physical
    }

    pub fn semantic_record(
        &self,
        provider: ConeIdentity,
        target: super::LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'_>> {
        let relation = LayoutAbiDependencyV1::new(provider, target);
        self.semantic.binary_search(&relation).ok()?;
        let terminal = self
            .dependencies
            .binary_search_by_key(&provider, |section| section.provider())
            .ok()
            .map(|index| self.dependencies[index])?;
        terminal.record(target)
    }
}

fn semantic_target(
    subject: crate::ExternalStrongShapeSubjectV1,
    terminal: &LayoutAbiExportConstituentsV1,
) -> Result<Option<LayoutAbiSemanticTargetV1>, crate::ExternalStrongShapeSubjectV1> {
    use crate::ExternalStrongShapeSubjectV1 as Subject;
    Ok(match subject {
        Subject::Callable(target) => terminal
            .callables()
            .get(target)
            .map(|_| LayoutAbiSemanticTargetV1::Callable(target)),
        Subject::Layout(layout) => Some(LayoutAbiSemanticTargetV1::Layout(layout)),
        Subject::Scan(scan) => Some(
            terminal
                .layouts()
                .records()
                .iter()
                .find(|record| record.scan() == scan)
                .map(|record| LayoutAbiSemanticTargetV1::Layout(record.identity().layout()))
                .ok_or(subject)?,
        ),
        Subject::TypeDescriptor(exact) | Subject::TypeRegistration(exact) => {
            Some(LayoutAbiSemanticTargetV1::Descriptor(exact))
        }
        Subject::DispatchTable(table) => Some(LayoutAbiSemanticTargetV1::Dispatch(table)),
        Subject::StaticStorage(_)
        | Subject::StaticStorageRegistration(_)
        | Subject::InitializationCell(_)
        | Subject::InitializationDescriptor(_) => None,
    })
}
