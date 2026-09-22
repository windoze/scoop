use super::*;

pub(super) fn restore(
    output: &hir::DependencyHirOutput,
    section: &hir::CrossConeTypeSemanticsSectionV1,
) -> hir::CrossConeTypeSemanticsSectionV1 {
    let mut fixture = Fixture::from_output(output);
    let bytes = encode(&section.index_for_wire(&mut meter()).unwrap()).unwrap();
    let decoded: hir::DecodedCrossConeTypeSemanticsSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let restored = decoded
        .resolve(&mut fixture.identities, &mut meter(), &WirePath::root())
        .unwrap();
    assert_eq!(
        encode(&restored.index_for_wire(&mut meter()).unwrap()).unwrap(),
        bytes
    );
    restored
}

pub(super) fn assert_source_body(
    template: &hir::ProtectedDefaultTemplateV1,
    source: &hir::DefaultSourceTemplateV1,
) {
    assert_eq!(template.key(), source.key());
    assert_eq!(template.definition_root(), source.definition_root());
    assert_eq!(template.definition_path(), source.definition_path());
    assert_eq!(template.locals(), source.locals());
    assert_eq!(template.body(), source.body());
    assert_eq!(template.result(), source.result());
    assert_eq!(template.allows_suspend(), source.allows_suspend());
    assert_eq!(template.type_parameters(), source.type_parameters());
    assert_eq!(template.receiver(), source.receiver());
    assert_eq!(template.value_parameters(), source.value_parameters());
    assert_eq!(template.definition_origin(), source.definition_origin());
}

pub(super) struct Occurrences<'a, 'b> {
    source: &'a hir::DefaultSourceReferenceClosureV1<'b>,
    direct: &'a hir::DefaultSourceAccessDomainV1,
    pub count: usize,
    pub generic: usize,
}
impl<'a, 'b> Occurrences<'a, 'b> {
    pub fn new(
        source: &'a hir::DefaultSourceReferenceClosureV1<'b>,
        direct: &'a hir::DefaultSourceAccessDomainV1,
    ) -> Self {
        Self {
            source,
            direct,
            count: 0,
            generic: 0,
        }
    }
}
impl hir::ProtectedDefaultReferenceBodySemanticAuthority<&'static str> for Occurrences<'_, '_> {
    fn validate_default_reference_occurrence(
        &mut self,
        key: hir::ProtectedDefaultTemplateKeyV1,
        occurrence: hir::DefaultBodyReferenceOccurrenceV1<'_>,
        witness: &hir::ProtectedDefaultAccessWitnessV1,
        receiver: hir::ProtectedDefaultReferenceReceiverV1<'_>,
        _meter: &mut BudgetMeter,
        _path: &WirePath,
    ) -> Result<(), &'static str> {
        let expected = &self.source.occurrences()[self.count];
        assert_eq!(key, self.source.template());
        assert_eq!(witness.owner(), key.owner());
        assert_eq!(
            occurrence.definition_origin,
            expected.body().definition_origin
        );
        assert_eq!(
            format!("{:?}", occurrence.target),
            format!("{:?}", expected.body().target)
        );
        assert_eq!(
            format!("{receiver:?}"),
            format!("{:?}", expected.context().receiver())
        );
        match witness.view() {
            hir::ProtectedDefaultAccessWitnessViewV1::ParamFree(witness) => {
                assert_eq!(
                    witness.target_domain().domain(),
                    expected.source().witness().target_domain().persistent()
                );
                assert!(self.direct.generic_subclasses().is_empty());
                assert_eq!(
                    witness.direct_call_domain().domain(),
                    self.direct.persistent()
                );
            }
            hir::ProtectedDefaultAccessWitnessViewV1::GenericSourceMetadata { .. } => {
                self.generic += 1
            }
        }
        self.count += 1;
        Ok(())
    }
}

pub(super) fn reference_counts(references: &hir::ProtectedDefaultReferenceSetV1) -> [usize; 6] {
    [
        references.callables().len(),
        references.constructors().len(),
        references.types().len(),
        references.globals().len(),
        references.singleton_values().len(),
        references.fields().len(),
    ]
}

pub(super) fn outline(
    output: &hir::DependencyHirOutput,
    templates: &hir::CanonicalProtectedDefaultTemplatesV1,
) -> String {
    let export = output.output().export.module();
    let mut names = BTreeMap::new();
    for (id, function) in export.functions.iter() {
        if let hir::HirFunctionIdentity::Source(identity) = &export.function_identities[id] {
            let owner = match identity {
                hir::HirSourceFunctionIdentity::Plain(record) => {
                    CallableTemplateOrigin::Function(record.id())
                }
                hir::HirSourceFunctionIdentity::Generic(record) => {
                    CallableTemplateOrigin::GenericFunction(record.id())
                }
            };
            names.insert(owner, function.name.clone());
        }
    }
    for (id, constructor) in export.class_constructors.iter() {
        if let Some(record) = export.constructor_identities[id].source_record() {
            names.insert(
                CallableTemplateOrigin::Constructor(record.id()),
                format!("{} constructor", export.classes[constructor.owner].name),
            );
        }
    }
    for (id, constructor) in export.struct_constructors.iter() {
        names.insert(
            CallableTemplateOrigin::Constructor(export.constructor_identities[id].id()),
            format!("{} constructor", export.structs[constructor.owner].name),
        );
    }
    let mut lines = templates
        .records()
        .iter()
        .map(|template| {
            let label = |owner| {
                names
                    .get(&owner)
                    .cloned()
                    .unwrap_or_else(|| format!("{owner:?}"))
            };
            format!(
                "{}[{}] <- {}: locals={}, refs={:?}, receiver={:?}, binders={:?}\n",
                label(template.key().owner()),
                template.key().parameter_position(),
                label(template.definition_root().declaration()),
                template.locals().records().len(),
                reference_counts(template.references()),
                template.receiver(),
                template.type_parameters()
            )
        })
        .collect::<Vec<_>>();
    lines.sort();
    lines.concat()
}

pub(super) fn snapshot(name: &str, dump: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tests/fixtures/m23-type-protected-production/{name}.scoop.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_DEFAULT_PRODUCTION_SNAPSHOTS").is_some() {
        std::fs::write(path, dump).unwrap();
    } else {
        assert_eq!(dump, std::fs::read_to_string(path).unwrap());
    }
}

pub(super) fn direct_domains(
    output: &hir::DependencyHirOutput,
) -> BTreeMap<CallableTemplateOrigin, hir::DefaultSourceAccessDomainV1> {
    let export = output.output().export.module();
    export
        .source_parameter_interfaces
        .iter()
        .map(|protocol| {
            let direct = match protocol.owner {
                hir::ExportParameterOwner::Function(id) => &export.functions[id].access.lookup,
                hir::ExportParameterOwner::ClassConstructor(id) => {
                    &export.class_constructors[id].access.lookup
                }
                hir::ExportParameterOwner::StructConstructor(id) => {
                    &export.struct_constructors[id].access.lookup
                }
                hir::ExportParameterOwner::VariantConstructor(id) => {
                    &export.enums[id.enumeration()].access.lookup
                }
            };
            let source = hir::DefaultSourceAccessWitnessV1::from_export_hir(
                export,
                &hir::ExportDefaultAccessWitness {
                    owner: protocol.owner,
                    call_domain: hir::CallDomain {
                        direct: direct.clone(),
                        slot: None,
                    },
                    target_domain: hir::AccessDomain::universal(),
                },
                &mut meter(),
            )
            .unwrap();
            (source.owner(), source.direct_call_domain().clone())
        })
        .collect()
}
