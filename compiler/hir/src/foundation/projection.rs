//! Projection of the complete HIR graphs into their canonical identity delta.

use std::collections::BTreeMap;

use scoop_identity::{CborIdentityRecord, DefinitionOriginRecord, PersistentId, SourceIdentity};

use super::*;
use crate::{ExportHir, HirNativeBoundaryTypeDefinitions, LocalConcreteHir};

mod type_semantics;

impl CanonicalHirFoundation {
    /// Builds an ordinary HIR foundation together with the exact external
    /// nominal authority required by its imported-core exact types.
    pub fn from_ordinary_output(
        output: &crate::OrdinaryHirOutput<'_>,
    ) -> Result<Self, HirFoundationBuildError> {
        let hir = output.output();
        let mut foundation =
            Self::from_modules(&hir.export, &hir.local, &hir.native_boundary_types)?;
        let declared_source_types = foundation
            .types
            .iter()
            .map(CborIdentityRecord::id)
            .chain(
                foundation
                    .generated_types
                    .iter()
                    .map(CborIdentityRecord::id),
            )
            .collect::<std::collections::BTreeSet<_>>();
        let declared_generic_types = foundation
            .generic_types
            .iter()
            .map(CborIdentityRecord::id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut external_source_types = std::collections::BTreeSet::new();
        let mut external_generic_types = std::collections::BTreeSet::new();
        for record in &foundation.exact_types {
            match record.key() {
                ExactTypeKey::Nominal(source) if !declared_source_types.contains(source) => {
                    if !output.imported_core().contains_hir_identity(*source) {
                        return Err(HirFoundationBuildError::MissingCoreExternalSourceType(
                            *source,
                        ));
                    }
                    external_source_types.insert(*source);
                }
                ExactTypeKey::NominalApplication { origin, .. }
                    if !declared_generic_types.contains(origin) =>
                {
                    if !output.imported_core().contains_hir_identity(*origin) {
                        return Err(HirFoundationBuildError::MissingCoreExternalGenericType(
                            *origin,
                        ));
                    }
                    external_generic_types.insert(*origin);
                }
                ExactTypeKey::Nominal(_)
                | ExactTypeKey::NominalApplication { .. }
                | ExactTypeKey::Tuple(_)
                | ExactTypeKey::Function { .. }
                | ExactTypeKey::RawPointer(_)
                | ExactTypeKey::NativeFunctionPointer { .. } => {}
            }
        }
        for record in &foundation.native_boundary_types {
            match record.owner() {
                crate::NativeBoundaryNominalOwner::Concrete(source)
                    if !declared_source_types.contains(&source) =>
                {
                    if !output.imported_core().contains_hir_identity(source) {
                        return Err(HirFoundationBuildError::MissingCoreExternalSourceType(
                            source,
                        ));
                    }
                    external_source_types.insert(source);
                }
                crate::NativeBoundaryNominalOwner::GenericTemplate(origin)
                    if !declared_generic_types.contains(&origin) =>
                {
                    if !output.imported_core().contains_hir_identity(origin) {
                        return Err(HirFoundationBuildError::MissingCoreExternalGenericType(
                            origin,
                        ));
                    }
                    external_generic_types.insert(origin);
                }
                crate::NativeBoundaryNominalOwner::Concrete(_)
                | crate::NativeBoundaryNominalOwner::GenericTemplate(_) => {}
            }
        }
        foundation.set_core_external_source_types(external_source_types.into_iter().collect())?;
        foundation.set_core_external_generic_types(external_generic_types.into_iter().collect())?;
        Ok(foundation)
    }

    /// Build the complete HIR identity delta from the two structurally
    /// isolated HIR graphs and their shared native-boundary witness.
    pub fn from_modules(
        export: &ExportHir,
        local: &LocalConcreteHir,
        native_boundary_types: &HirNativeBoundaryTypeDefinitions,
    ) -> Result<Self, HirFoundationBuildError> {
        let mut foundation = Self::empty();

        project_nominals(export, &mut foundation)?;
        project_functions_and_constructors(export, local, &mut foundation)?;
        project_properties_and_members(export, &mut foundation)?;
        project_exact_types(export, local, &mut foundation)?;

        foundation
            .set_export_bindings(export.export_binding_identities.iter().cloned().collect())?;
        foundation.set_callable_applications(local.callable_applications.records().to_vec())?;
        foundation
            .set_dispatch_slots(export.dispatch_slot_identities.records().cloned().collect())?;
        foundation
            .set_initialization_units(export.initialization_unit_identities.records().to_vec())?;
        foundation
            .set_source_contexts(export.source_context_identities.iter().cloned().collect())?;
        foundation.set_local_bindings(
            export
                .local_binding_identities
                .iter()
                .map(|identity| identity.record().clone())
                .collect(),
        )?;
        foundation.set_local_values(local.local_value_identities.records().to_vec())?;
        foundation.set_callback_registrations(
            export.callback_registration_identities.records().to_vec(),
        )?;
        foundation.set_source_native_contracts(
            export
                .source_native_contracts
                .iter()
                .map(|contract| contract.record().clone())
                .collect(),
        )?;
        foundation.set_odr_groups(odr_group_records(local)?)?;
        foundation.set_odr_members(local.callable_applications.odr_member_records().to_vec())?;

        let definition_origins = definition_origin_records(export, local);
        foundation.set_sources(source_records(
            &export.source_files,
            &definition_origins,
            &[],
        )?)?;
        foundation.set_definition_origins(definition_origins)?;
        foundation.set_native_boundary_types(native_boundary_types.records().to_vec())?;
        Ok(foundation)
    }

    /// Completes the sparse source-point tables with every position embedded
    /// in the already-projected cross-Cone definition-source table.
    ///
    /// The cross-Cone interface is projected after the identity foundation
    /// because its external-reference closure needs that typed authority.
    /// Calling this method closes the one intentional construction cycle
    /// before either product is serialized or validated as an artifact.
    pub fn complete_cross_cone_source_points(
        &mut self,
        export: &ExportHir,
        required: &crate::CanonicalExportDefinitionSourcesV1,
    ) -> Result<(), HirFoundationBuildError> {
        self.set_sources(source_records(
            &export.source_files,
            &self.definition_origins,
            required.sources(),
        )?)
    }
}

fn project_nominals(
    export: &ExportHir,
    foundation: &mut CanonicalHirFoundation,
) -> Result<(), HirFoundationBuildError> {
    let mut types = BTreeMap::new();
    let mut generic_types = Vec::new();
    let mut generated_types = BTreeMap::new();

    for builtin in [
        scoop_identity::CoreBuiltinNominal::Unit,
        scoop_identity::CoreBuiltinNominal::Any,
    ] {
        insert_identity(
            &mut types,
            export.nominal_identities.core_builtin(builtin),
            HirFoundationTable::Type,
        )?;
    }

    for identity in export
        .structs
        .iter()
        .map(|(id, _)| &export.nominal_identities[id])
        .chain(
            export
                .enums
                .iter()
                .map(|(id, _)| &export.nominal_identities[id]),
        )
        .chain(
            export
                .classes
                .iter()
                .map(|(id, _)| &export.nominal_identities[id]),
        )
        .chain(
            export
                .interfaces
                .iter()
                .map(|(id, _)| &export.nominal_identities[id]),
        )
        .chain(
            export
                .objects
                .iter()
                .map(|(id, _)| &export.nominal_identities[id]),
        )
    {
        match identity {
            crate::HirNominalIdentity::Source(crate::HirSourceNominalIdentity::Concrete(
                record,
            )) => insert_identity(&mut types, record, HirFoundationTable::Type)?,
            crate::HirNominalIdentity::Source(crate::HirSourceNominalIdentity::Generic(record)) => {
                generic_types.push(record.clone());
            }
            crate::HirNominalIdentity::Generated(record) => insert_identity(
                &mut generated_types,
                record,
                HirFoundationTable::GeneratedType,
            )?,
        }
    }

    foundation.set_types(types.into_values().collect())?;
    foundation.set_generic_types(generic_types)?;
    foundation.set_generated_types(generated_types.into_values().collect())
}

fn project_functions_and_constructors(
    export: &ExportHir,
    local: &LocalConcreteHir,
    foundation: &mut CanonicalHirFoundation,
) -> Result<(), HirFoundationBuildError> {
    let mut functions = Vec::new();
    let mut generic_functions = Vec::new();
    let mut constructors = Vec::new();
    let mut generated = BTreeMap::new();

    for (id, _) in export.functions.iter() {
        match &export.function_identities[id] {
            crate::HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Plain(record)) => {
                functions.push(record.clone());
            }
            crate::HirFunctionIdentity::Source(crate::HirSourceFunctionIdentity::Generic(
                record,
            )) => generic_functions.push(record.clone()),
            crate::HirFunctionIdentity::PropertyAccessor(_) => {}
            crate::HirFunctionIdentity::LexicalGenerated(record)
            | crate::HirFunctionIdentity::Initialization { record, .. } => insert_identity(
                &mut generated,
                record,
                HirFoundationTable::GeneratedCallable,
            )?,
            crate::HirFunctionIdentity::DerivedEquality(applications) => {
                for application in applications {
                    insert_identity(
                        &mut generated,
                        application.record(),
                        HirFoundationTable::GeneratedCallable,
                    )?;
                }
            }
        }
    }

    for (id, _) in export.struct_constructors.iter() {
        constructors.push(export.constructor_identities[id].clone());
    }
    for (id, _) in export.class_constructors.iter() {
        match &export.constructor_identities[id] {
            crate::HirClassConstructorIdentity::Source(record) => {
                constructors.push(record.clone());
            }
            crate::HirClassConstructorIdentity::ZeroArgumentAdapter { record, .. } => {
                insert_identity(
                    &mut generated,
                    record,
                    HirFoundationTable::GeneratedCallable,
                )?;
            }
        }
    }
    for (_, reference) in local.callable_references.iter() {
        insert_identity(
            &mut generated,
            reference.identity.callable_record(),
            HirFoundationTable::GeneratedCallable,
        )?;
    }

    foundation.set_functions(functions)?;
    foundation.set_generic_functions(generic_functions)?;
    foundation.set_constructors(constructors)?;
    foundation.set_generated_callables(generated.into_values().collect())
}

fn project_properties_and_members(
    export: &ExportHir,
    foundation: &mut CanonicalHirFoundation,
) -> Result<(), HirFoundationBuildError> {
    let mut properties = Vec::new();
    let mut extension_properties = Vec::new();
    for (id, _) in export.properties.iter() {
        match &export.property_identities[id] {
            crate::HirPropertyIdentity::Ordinary(record) => properties.push(record.clone()),
            crate::HirPropertyIdentity::Extension(record) => {
                extension_properties.push(record.clone());
            }
        }
    }
    foundation.set_properties(properties)?;
    foundation.set_extension_properties(extension_properties)?;
    foundation.set_object_values(
        export
            .singleton_values
            .iter()
            .map(|(id, _)| export.object_value_identities[id].record().clone())
            .collect(),
    )?;
    foundation.set_type_aliases(
        export
            .type_aliases
            .iter()
            .map(|(id, _)| export.type_alias_identities[id].clone())
            .collect(),
    )?;
    foundation.set_property_accessors(
        export
            .property_getters
            .iter()
            .map(|(id, _)| export.property_accessor_identities[id].record().clone())
            .chain(
                export
                    .property_setters
                    .iter()
                    .map(|(id, _)| export.property_accessor_identities[id].record().clone()),
            )
            .collect(),
    )?;
    foundation.set_fields(field_records(export))?;
    let (variants, variant_fields) = enum_member_records(export);
    foundation.set_enum_variants(variants)?;
    foundation.set_enum_variant_fields(variant_fields)
}

fn field_records(export: &ExportHir) -> Vec<FieldRecord> {
    let mut records = Vec::new();
    for (structure, declaration) in export.structs.iter() {
        for index in 0..declaration.semantic_fields().len() {
            let index = u32::try_from(index)
                .expect("a validated source struct field index always fits in u32");
            let field = crate::StructFieldRef::checked(&export.structs, structure, index)
                .expect("a field from the source struct forms a checked field reference");
            records.push(export.field_identities[field].clone());
        }
    }
    records.extend(
        export
            .class_fields
            .iter()
            .map(|(field, _)| export.field_identities[field].clone()),
    );
    records
}

fn enum_member_records(
    export: &ExportHir,
) -> (Vec<EnumVariantRecord>, Vec<EnumVariantFieldRecord>) {
    let mut variants = Vec::new();
    let mut fields = Vec::new();
    for (enumeration, declaration) in export.enums.iter() {
        for (variant_index, declaration) in declaration.variants.iter().enumerate() {
            let variant_index = u32::try_from(variant_index)
                .expect("a validated source enum variant index always fits in u32");
            let variant = crate::EnumVariantRef::checked(&export.enums, enumeration, variant_index)
                .expect("a variant from the source enum forms a checked variant reference");
            variants.push(export.enum_member_identities[variant].clone());
            for field_index in 0..declaration.fields.len() {
                let field_index = u32::try_from(field_index)
                    .expect("a validated source enum field index always fits in u32");
                let field =
                    crate::EnumVariantFieldRef::checked(&export.enums, variant, field_index)
                        .expect("a field from the source variant forms a checked field reference");
                fields.push(export.enum_member_identities[field].clone());
            }
        }
    }
    (variants, fields)
}

fn project_exact_types(
    export: &ExportHir,
    local: &LocalConcreteHir,
    foundation: &mut CanonicalHirFoundation,
) -> Result<(), HirFoundationBuildError> {
    let mut records = BTreeMap::new();
    for (ty, _) in export.types.iter() {
        if let Some(record) = export.type_identities[ty].exact() {
            insert_identity(&mut records, record, HirFoundationTable::ExactType)?;
        }
    }
    for (ty, _) in local.types.iter() {
        insert_identity(
            &mut records,
            &local.exact_type_identities[ty],
            HirFoundationTable::ExactType,
        )?;
    }
    foundation.set_exact_types(records.into_values().collect())
}

fn odr_group_records(
    local: &LocalConcreteHir,
) -> Result<Vec<OdrGroupRecord>, HirFoundationBuildError> {
    let mut records = BTreeMap::new();
    for record in local.callable_applications.odr_group_records() {
        insert_identity(&mut records, record, HirFoundationTable::OdrGroup)?;
    }
    Ok(records.into_values().collect())
}

fn definition_origin_records(
    export: &ExportHir,
    local: &LocalConcreteHir,
) -> Vec<DefinitionOriginRecord> {
    export
        .export_definition_origins
        .records()
        .iter()
        .cloned()
        .chain(
            local
                .local_value_identities
                .definition_origins()
                .records()
                .iter()
                .cloned(),
        )
        .collect()
}

struct RequiredSourcePoints {
    offsets: Vec<u64>,
    first_origin: RequiredSourceOrigin,
}

enum RequiredSourceOrigin {
    Foundation(scoop_identity::DefinitionOriginSubject),
    CrossConeInterface,
}

fn source_records(
    source_files: &[crate::SourceFileMetadata],
    definitions: &[DefinitionOriginRecord],
    interface_sources: &[crate::ExportDefinitionSourceV1],
) -> Result<Vec<SourceRecord>, HirFoundationBuildError> {
    let mut required = BTreeMap::<SourceIdentity, RequiredSourcePoints>::new();
    for record in definitions {
        let origin = record.origin();
        let span = origin.span();
        let points =
            required
                .entry(origin.source().clone())
                .or_insert_with(|| RequiredSourcePoints {
                    offsets: Vec::new(),
                    first_origin: RequiredSourceOrigin::Foundation(record.subject()),
                });
        points.offsets.extend([span.start_byte(), span.end_byte()]);
    }
    for source in interface_sources {
        let origin = source.origin();
        let span = origin.span();
        let points =
            required
                .entry(origin.source().clone())
                .or_insert_with(|| RequiredSourcePoints {
                    offsets: Vec::new(),
                    first_origin: RequiredSourceOrigin::CrossConeInterface,
                });
        points.offsets.extend([span.start_byte(), span.end_byte()]);
    }

    let mut records = Vec::with_capacity(source_files.len());
    for source in source_files {
        let points = required
            .remove(&source.identity)
            .map_or_else(Vec::new, |points| points.offsets);
        let record = if let Some(record) = &source.canonical_record {
            if record.identity() != &source.identity {
                return Err(HirFoundationBuildError::CanonicalSourceIdentityMismatch {
                    metadata: source.identity.clone(),
                    record: record.identity().clone(),
                });
            }
            record.require_points(points).map_err(|error| {
                HirFoundationBuildError::CanonicalSourcePoints {
                    source: source.identity.clone(),
                    error,
                }
            })?;
            record.clone()
        } else {
            SourceRecord::from_utf8(source.identity.clone(), &source.source, points).map_err(
                |error| HirFoundationBuildError::SourceRecord {
                    source: source.identity.clone(),
                    error,
                },
            )?
        };
        records.push(record);
    }
    if let Some((source, points)) = required.into_iter().next() {
        return Err(match points.first_origin {
            RequiredSourceOrigin::Foundation(subject) => {
                HirFoundationBuildError::UnknownDefinitionSource {
                    source,
                    subject_tag: subject.kind_tag(),
                    subject: subject.raw_id(),
                }
            }
            RequiredSourceOrigin::CrossConeInterface => {
                HirFoundationBuildError::UnknownCrossConeDefinitionSource(source)
            }
        });
    }
    Ok(records)
}

fn insert_identity<I, K>(
    records: &mut BTreeMap<I, CborIdentityRecord<I, K>>,
    record: &CborIdentityRecord<I, K>,
    table: HirFoundationTable,
) -> Result<(), HirFoundationBuildError>
where
    I: PersistentId,
    K: scoop_identity::CborIdentityKey<I> + Clone + Eq,
{
    let identity = record.id();
    if let Some(previous) = records.get(&identity) {
        if previous != record {
            return Err(HirFoundationBuildError::DuplicateIdentity {
                table,
                identity: *identity.as_array(),
            });
        }
        return Ok(());
    }
    records.insert(identity, record.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CoreBuiltinNominal, DefinitionOrigin, DefinitionOriginSubject, SourceContextKey, SourceSpan,
    };

    use super::*;

    #[test]
    fn source_projection_collects_foundation_and_cross_cone_origin_endpoints() {
        let source = SourceIdentity::single_file();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let origin =
            DefinitionOrigin::new(source.clone(), SourceSpan::new(1, 3).unwrap(), &context)
                .unwrap();
        let definition = DefinitionOriginRecord::new(
            DefinitionOriginSubject::Type(CoreBuiltinNominal::Unit.identity_record().id()),
            origin,
        );
        let interface_source = crate::ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 4).unwrap(), &context)
                .unwrap(),
        );
        let metadata = crate::SourceFileMetadata {
            provider: crate::IntrinsicProviderId::from_raw(7),
            identity: source,
            name: "main.scoop".to_string(),
            source: "aé\nz".to_string(),
            canonical_record: None,
        };

        let records = source_records(&[metadata], &[definition], &[interface_source]).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0]
                .points()
                .iter()
                .map(crate::SourcePointRecord::byte_offset)
                .collect::<Vec<_>>(),
            vec![0, 1, 3, 4]
        );
        assert_eq!(records[0].point(1).unwrap().column(), 2);
        assert_eq!(records[0].point(3).unwrap().column(), 3);
    }

    #[test]
    fn source_projection_rejects_an_origin_outside_the_module() {
        let source = SourceIdentity::single_file();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let subject =
            DefinitionOriginSubject::Type(CoreBuiltinNominal::Unit.identity_record().id());
        let origin =
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 0).unwrap(), &context)
                .unwrap();

        assert!(matches!(
            source_records(&[], &[DefinitionOriginRecord::new(subject, origin)], &[]),
            Err(HirFoundationBuildError::UnknownDefinitionSource {
                source: actual_source,
                subject_tag,
                subject: actual_subject,
            }) if actual_source == source
                && subject_tag == subject.kind_tag()
                && actual_subject == subject.raw_id()
        ));
    }

    #[test]
    fn source_projection_rejects_a_cross_cone_origin_outside_the_module() {
        let source = SourceIdentity::single_file();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let required = crate::ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 0).unwrap(), &context)
                .unwrap(),
        );

        assert!(matches!(
            source_records(&[], &[], &[required]),
            Err(HirFoundationBuildError::UnknownCrossConeDefinitionSource(actual))
                if actual == source
        ));
    }
}
