use super::*;
use crate::production::default_templates::{GenericBodyProducer, GenericInitializationProducer};
use crate::{
    CanonicalExportGenericCallableBodiesV1, DefaultBodyReferenceOccurrenceV1,
    DefaultBodyReferenceTargetV1, DefaultBodyReferenceVisitorV1, DefaultBoundCallableSourceV1,
    DefaultCallableDeclarationV1, DefaultCallableReferenceTargetViewV1,
    DefaultConstructorReferenceTargetViewV1, DefaultExpressionV1, GenericTemplateProductionError,
    SelectedImportedDependencySet, SignatureNominalWalker,
};

impl SharedSourceRoots {
    pub(in crate::production) fn with_callable_bodies(
        export: &ExportHir,
        imported: Option<&SelectedImportedDependencySet>,
        additional_nominals: &[SourceNominalId],
    ) -> Result<
        (
            Self,
            CanonicalExportGenericCallableBodiesV1,
            crate::CanonicalExportGenericInitializationsV1,
        ),
        GenericTemplateProductionError,
    > {
        let mut collection = SourceCollection::new(export).map_err(declarations)?;
        for owner in additional_nominals {
            collection
                .nominals
                .require(*owner, true)
                .map_err(declarations)?;
        }
        let mut producer = GenericBodyProducer::new(export, imported)?;
        let mut initialization_producer = GenericInitializationProducer::new(export, imported);
        let properties = export
            .properties
            .iter()
            .filter_map(|(id, _)| {
                export.property_identities[id]
                    .ordinary_id()
                    .map(|key| (key, id))
            })
            .collect::<BTreeMap<_, _>>();
        let mut bodies = Vec::new();
        let mut initializations = Vec::new();
        loop {
            collection.expand().map_err(declarations)?;
            let roots = collection.snapshot().map_err(declarations)?;
            producer.include_roots(&roots);
            initialization_producer.include_roots(&roots);
            while let Some(initialization) = initialization_producer.next_initialization()? {
                initialization.visit_direct_references(
                    &mut TemplateReferences {
                        collection: &mut collection,
                        producer: &mut producer,
                        properties: &properties,
                    },
                    &WirePath::root(),
                )?;
                initializations.push(initialization);
            }
            while let Some(body) = producer.next_body()? {
                body.visit_direct_references(
                    &mut TemplateReferences {
                        collection: &mut collection,
                        producer: &mut producer,
                        properties: &properties,
                    },
                    &WirePath::root(),
                )?;
                bodies.push(body);
            }
            if !collection.has_pending() {
                let bodies = CanonicalExportGenericCallableBodiesV1::try_new(bodies)
                    .map_err(GenericTemplateProductionError::Table)?;
                let initializations =
                    crate::CanonicalExportGenericInitializationsV1::try_new(initializations)
                        .map_err(GenericTemplateProductionError::Initialization)?;
                return Ok((roots, bodies, initializations));
            }
        }
    }
}

struct TemplateReferences<'a, 'hir> {
    collection: &'a mut SourceCollection<'hir>,
    producer: &'a mut GenericBodyProducer<'hir>,
    properties: &'a BTreeMap<scoop_identity::PersistentPropertyId, PropertyId>,
}

impl TemplateReferences<'_, '_> {
    fn callable(
        &mut self,
        owner: DefaultCallableDeclarationV1,
    ) -> Result<(), GenericTemplateProductionError> {
        if let Some(function) = self.producer.require(owner) {
            self.collection
                .sources
                .function(
                    self.collection.export,
                    function,
                    &mut self.collection.nominals,
                )
                .map_err(declarations)?;
        }
        Ok(())
    }

    fn source_callable(
        &mut self,
        owner: CallableTemplateOrigin,
    ) -> Result<(), GenericTemplateProductionError> {
        match owner {
            CallableTemplateOrigin::Function(id) => {
                self.callable(DefaultCallableDeclarationV1::Function(id))
            }
            CallableTemplateOrigin::GenericFunction(id) => {
                self.callable(DefaultCallableDeclarationV1::GenericFunction(id))
            }
            CallableTemplateOrigin::Accessor(id) => {
                self.callable(DefaultCallableDeclarationV1::PropertyAccessor(id))
            }
            // Constructor declarations follow their required nominal shape.
            CallableTemplateOrigin::Constructor(_)
            | CallableTemplateOrigin::VariantConstructor(_) => Ok(()),
        }
    }

    fn signature(
        &mut self,
        signature: &scoop_identity::SignatureTypeKey,
        path: &WirePath,
    ) -> Result<(), GenericTemplateProductionError> {
        let mut walker = SignatureNominalWalker::new(signature, path)?;
        while let Some(owner) = walker.next(path)? {
            if self.collection.index.nodes.contains_key(&owner) {
                self.collection
                    .nominals
                    .require(owner, true)
                    .map_err(declarations)?;
            }
        }
        Ok(())
    }
}

impl<'body> DefaultBodyReferenceVisitorV1<'body> for TemplateReferences<'_, '_> {
    type Error = GenericTemplateProductionError;

    fn expression(
        &mut self,
        _: u32,
        _: &'body DefaultExpressionV1,
        _: &WirePath,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,
        path: &WirePath,
    ) -> Result<(), Self::Error> {
        match occurrence.target {
            DefaultBodyReferenceTargetV1::Callable(target) => match target {
                DefaultCallableReferenceTargetViewV1::Callable(callable) => {
                    self.callable(callable.declaration())?
                }
                DefaultCallableReferenceTargetViewV1::FunctionAddress(owner) => {
                    self.callable(owner)?
                }
                DefaultCallableReferenceTargetViewV1::LocalFunction(owner) => {
                    self.source_callable(owner)?
                }
                DefaultCallableReferenceTargetViewV1::Lambda(id)
                | DefaultCallableReferenceTargetViewV1::AnonymousFunction(id) => {
                    self.callable(DefaultCallableDeclarationV1::Generated(id))?
                }
                DefaultCallableReferenceTargetViewV1::Bound(bound) => match bound.source() {
                    DefaultBoundCallableSourceV1::Class { callable, .. } => {
                        self.callable(callable.declaration())?
                    }
                    DefaultBoundCallableSourceV1::Interface { member, .. } => {
                        self.source_callable(*member)?
                    }
                },
                // These implementations are generated from their typed use.
                DefaultCallableReferenceTargetViewV1::CallableReference(_)
                | DefaultCallableReferenceTargetViewV1::DerivedEquality(_) => {}
            },
            DefaultBodyReferenceTargetV1::Type(signature) => self.signature(signature, path)?,
            DefaultBodyReferenceTargetV1::Constructor(target) => {
                let signature = match target {
                    DefaultConstructorReferenceTargetViewV1::Constructor(target) => {
                        target.owner_type()
                    }
                    DefaultConstructorReferenceTargetViewV1::Variant(target) => target.owner_type(),
                };
                self.signature(signature, path)?;
            }
            DefaultBodyReferenceTargetV1::Global(owner) => {
                if let Some(&property) = self.properties.get(&owner) {
                    self.collection
                        .sources
                        .property(
                            self.collection.export,
                            property,
                            &mut self.collection.nominals,
                        )
                        .map_err(declarations)?;
                }
            }
            // Their complete owner types are visited by the common body walk.
            DefaultBodyReferenceTargetV1::Singleton(_) | DefaultBodyReferenceTargetV1::Field(_) => {
            }
        }
        Ok(())
    }
}

fn declarations(error: Error) -> GenericTemplateProductionError {
    let error = match error {
        Error::SourceInventory(SourceInventoryError::Resource(error)) => {
            NominalInterfaceBuildError::Resource(error)
        }
        other => NominalInterfaceBuildError::Declarations(other.to_string()),
    };
    GenericTemplateProductionError::Declarations(Box::new(error))
}
