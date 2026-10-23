//! Published declaration locations, without deriving new identities or reading source.

use super::ImportedDependencySelectionPlan;
use crate::{
    AnnotationCallableV1 as Callable, AnnotationStorageRole as Storage,
    AnnotationTargetV1 as Target, ExportDefinitionSourceV1, NominalSourceShapeV1,
};
use scoop_identity::{CallableTemplateOrigin as Declaration, PropertyOwner};

impl ImportedDependencySelectionPlan {
    pub fn declaration_locations(&self) -> Vec<(Target, &ExportDefinitionSourceV1)> {
        let mut locations = Vec::new();
        for nominal in self.catalog.nominals.values() {
            locations.push((Target::Nominal(nominal.owner()), &nominal.origin));
            for (field, source) in nominal
                .interface
                .source_shape()
                .declared_fields()
                .iter()
                .zip(&nominal.field_sources)
            {
                locations.push((Target::Field(field.field()), &source.origin));
            }
            if let NominalSourceShapeV1::Enum(shape) = nominal.interface.source_shape() {
                for (variant, source) in shape.variants().iter().zip(&nominal.variant_sources) {
                    locations.push((Target::Variant(variant.variant()), &source.origin));
                    for (field, source) in variant.fields().iter().zip(&source.fields) {
                        locations.push((Target::VariantField(field.field()), &source.origin));
                    }
                }
            }
        }
        for (declaration, entry) in &self.catalog.callables {
            let callable = Callable::Declaration(*declaration);
            locations.push((Target::Callable(callable), &entry.definition_origin));
            if entry.interface.receiver().is_some() {
                locations.push((Target::CallableReceiver(callable), &entry.definition_origin));
            }
            for index in 0..entry.interface.parameters().len_u32() {
                let origin = entry
                    .source
                    .as_ref()
                    .map_or(&entry.definition_origin, |source| {
                        source.parameters().parameters()[index as usize].definition_origin()
                    });
                locations.push((Target::Parameter { callable, index }, origin));
            }
        }
        for (id, entry) in &self.catalog.generated_callables {
            if let Some(origin) = &entry.origin {
                locations.push((Target::Callable(Callable::Local(*id)), origin));
            }
        }
        self.body_locations(&mut locations);
        self.property_locations(&mut locations);
        locations.extend(
            self.catalog
                .type_aliases
                .iter()
                .map(|(id, entry)| (Target::TypeAlias(*id), entry.interface.definition_origin())),
        );
        locations.extend(self.catalog.annotations.iter().map(|(id, entry)| {
            (
                Target::Annotation(*id),
                &entry.declaration.definition_origin,
            )
        }));
        locations
    }

    fn body_locations<'a>(&'a self, locations: &mut Vec<(Target, &'a ExportDefinitionSourceV1)>) {
        use crate::DefaultCallableDeclarationV1 as BodyOwner;
        use scoop_identity::LocalValueSelector;
        for entry in self.catalog.bodies.values() {
            let body = entry.body();
            let callable = match body.owner() {
                BodyOwner::Function(id) => Callable::Declaration(Declaration::Function(id)),
                BodyOwner::GenericFunction(id) => {
                    Callable::Declaration(Declaration::GenericFunction(id))
                }
                BodyOwner::PropertyAccessor(id) => Callable::Declaration(Declaration::Accessor(id)),
                BodyOwner::Generated(id) => Callable::Local(id),
            };
            locations.push((Target::Callable(callable), body.definition_origin()));
            for selector in body.parameters() {
                match selector {
                    LocalValueSelector::Parameter {
                        declaration_index: index,
                    } => {
                        let local = body
                            .locals()
                            .get(selector)
                            .expect("published body parameter has its local record");
                        let origin = match local.definition() {
                            crate::TemplateLocalDefinitionV1::Source(origin) => origin,
                            crate::TemplateLocalDefinitionV1::Synthetic => body.definition_origin(),
                        };
                        locations.push((
                            Target::Parameter {
                                callable,
                                index: *index,
                            },
                            origin,
                        ));
                    }
                    LocalValueSelector::This => locations
                        .push((Target::CallableReceiver(callable), body.definition_origin())),
                    _ => {}
                }
            }
        }
    }

    fn property_locations<'a>(
        &'a self,
        locations: &mut Vec<(Target, &'a ExportDefinitionSourceV1)>,
    ) {
        for (owner, entry) in &self.catalog.properties {
            let reference = match owner {
                PropertyOwner::Property(id) => Target::Property(*id),
                PropertyOwner::ExtensionProperty(id) => Target::ExtensionProperty(*id),
            };
            locations.push((reference, &entry.definition_origin));
            let property = &entry.interface;
            if property.receiver().is_some() {
                locations.push((Target::PropertyReceiver(*owner), &entry.definition_origin));
            }
            let accessors = property.accessors();
            for accessor in std::iter::once(accessors.getter()).chain(accessors.setter()) {
                let declaration = Declaration::Accessor(accessor);
                let callable = Callable::Declaration(declaration);
                let origin = self
                    .catalog
                    .callables
                    .get(&declaration)
                    .map_or(&entry.definition_origin, |entry| &entry.definition_origin);
                locations.push((Target::Callable(callable), origin));
                if property.receiver().is_some() {
                    locations.push((Target::CallableReceiver(callable), origin));
                }
                if accessors.setter() == Some(accessor) {
                    locations.push((Target::Parameter { callable, index: 0 }, origin));
                }
            }
            let global_storage = property.owner() == crate::PublicDeclarationOwnerV1::TopLevel
                && accessors.getter_source().implementation().is_storage();
            if global_storage || entry.native_contract.is_some() {
                locations.push((
                    Target::PropertyStorage {
                        property: *owner,
                        role: Storage::Backing,
                    },
                    &entry.definition_origin,
                ));
            }
            if matches!(owner, PropertyOwner::ExtensionProperty(id) if self.catalog.delegates.contains_key(id))
            {
                locations.push((
                    Target::PropertyStorage {
                        property: *owner,
                        role: Storage::Delegate,
                    },
                    &entry.definition_origin,
                ));
            }
        }
        for target in self.catalog.annotated_targets.keys().chain(
            self.catalog
                .annotated_types
                .keys()
                .map(|(target, _)| target),
        ) {
            if let Target::PropertyStorage { property, .. } = target {
                let origin = &self.catalog.properties[property].definition_origin;
                locations.push((*target, origin));
            }
        }
    }
}
