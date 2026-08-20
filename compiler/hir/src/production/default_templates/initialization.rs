//! Source constructor templates join the same rooted implementation closure.

use std::collections::{BTreeSet, VecDeque};

use super::GenericTemplateProductionError as Error;
use super::entities::DefaultEntityProjector;
use crate::production::nominal_interfaces::SharedSourceRoots;
use crate::{
    CanonicalExportGenericInitializationsV1, ExportGenericNominalInitializationV1, ExportHir,
    HirNominalIdentity, HirSourceNominalIdentity, SelectedImportedDependencySet, SourceNominalId,
};

mod classes;
mod fragment;
mod release;
mod structures;
use fragment::ConstructorProjection;

#[derive(Clone, Copy)]
enum Owner {
    Class(crate::ClassId),
    Struct(crate::StructId),
}

pub(in crate::production) struct GenericInitializationProducer<'a> {
    entities: DefaultEntityProjector<'a>,
    pending: VecDeque<(scoop_identity::PersistentGenericTypeId, Owner)>,
    scheduled: BTreeSet<scoop_identity::PersistentGenericTypeId>,
}

impl<'a> GenericInitializationProducer<'a> {
    pub(in crate::production) fn new(
        export: &'a ExportHir,
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Self {
        Self {
            entities: DefaultEntityProjector::new(export, imported),
            pending: VecDeque::new(),
            scheduled: BTreeSet::new(),
        }
    }

    pub(in crate::production) fn include_roots(&mut self, roots: &SharedSourceRoots) {
        let export = self.entities.export();
        for (class, declaration) in export.classes.iter() {
            if declaration.is_declared() {
                self.schedule(
                    &export.nominal_identities[class],
                    Owner::Class(class),
                    roots,
                );
            }
        }
        for (structure, declaration) in export.structs.iter() {
            if matches!(
                declaration.representation,
                crate::StructRepresentation::Declared(_)
            ) {
                self.schedule(
                    &export.nominal_identities[structure],
                    Owner::Struct(structure),
                    roots,
                );
            }
        }
        for (object, declaration) in export.objects.iter() {
            self.schedule(
                &export.nominal_identities[object],
                Owner::Class(declaration.backing_class),
                roots,
            );
        }
    }

    fn schedule(&mut self, identity: &HirNominalIdentity, local: Owner, roots: &SharedSourceRoots) {
        let HirNominalIdentity::Source(HirSourceNominalIdentity::Generic(record)) = identity else {
            return;
        };
        if roots
            .nominals
            .values()
            .contains(&SourceNominalId::GenericTemplate(record.id()))
            && self.scheduled.insert(record.id())
        {
            self.pending.push_back((record.id(), local));
        }
    }

    pub(in crate::production) fn next_initialization(
        &mut self,
    ) -> Result<Option<ExportGenericNominalInitializationV1>, Error> {
        let Some((owner, local)) = self.pending.pop_front() else {
            return Ok(None);
        };
        match local {
            Owner::Class(class) => classes::project(&self.entities, owner, class).map(Some),
            Owner::Struct(structure) => {
                structures::project(&self.entities, owner, structure).map(Some)
            }
        }
    }
}

impl CanonicalExportGenericInitializationsV1 {
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, Error> {
        SharedSourceRoots::with_callable_bodies(export, None, &[])
            .map(|(_, _, initializations, _)| initializations)
    }

    pub fn from_dependency_hir(output: &crate::DependencyHirOutput) -> Result<Self, Error> {
        Ok(output
            .output()
            .export
            .shared_source()
            .initializations
            .clone())
    }
}
