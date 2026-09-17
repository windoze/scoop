use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOrigin,
    DefinitionOwnerChain, NormalizedSourcePath, PackagePath, PendingIdentityValidation,
    PersistentTypeAliasId, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceSpan, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath, decode_canonical, encode};

use super::*;
use crate::cross_cone_interface::default_templates::expression_test_support::Fixture;
use crate::{
    CallableParameterCallingV1, CallableSourceInterfaceV1, CallableSourceParameterV1,
    CanonicalBinderUseListV1, CanonicalBooleanV1, CanonicalCallableSourceParametersV1,
    CanonicalConstValueV1, CanonicalTemplateLocalTableV1, CanonicalTemplateValueParametersV1,
    DefaultBodyOriginSiteV1, DefaultExpressionKindV1, DefaultExpressionV1, DefaultStatementKindV1,
    DefaultStatementV1, ExportConstValueV1, ExportDefaultAccessWitnessV1, ExportDefaultBodyV1,
    ExportDefaultCallDomainV1, ExportDefaultReferenceKindV1, ExportDefaultReferenceSetV1,
    ExportDefaultReferenceV1, ExportDefaultTemplateKeyV1, ExportDefaultTemplateV1,
    ExportDefinitionSourceV1, OptionalTemplateReceiverV1, PersistentLexicalRootV1,
    PublicLookupAccessV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1,
    TypeAliasInterfaceRecordV1, TypeAliasTargetV1,
};

#[test]
fn definition_source_closure_accepts_every_inline_source_category() {
    let (section, expected) = closure_section(None, None);

    assert_eq!(
        section.validate_definition_source_closure(
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        ),
        Ok(())
    );
    assert_eq!(section.definition_sources().sources().len(), expected.len());
}

#[test]
fn definition_source_closure_reports_each_missing_use_site() {
    let (_, expected) = closure_section(None, None);

    for (missing_source, site) in expected {
        let (section, _) = closure_section(Some(&missing_source), None);
        let insertion_index = section
            .definition_sources()
            .sources()
            .binary_search(&missing_source)
            .unwrap_err();

        assert_eq!(
            section.validate_definition_source_closure(
                &mut BudgetMeter::new(DecodeLimits::default()),
                &WirePath::root(),
            ),
            Err(ExportDefinitionSourceClosureValidationError::Missing {
                source: Box::new(missing_source),
                insertion_index,
                site,
            })
        );
    }
}

#[test]
fn definition_source_closure_rejects_an_unused_table_entry() {
    let extra = origin(99);
    let (section, _) = closure_section(None, Some(extra.clone()));
    let index = section
        .definition_sources()
        .sources()
        .binary_search(&extra)
        .unwrap();

    assert_eq!(
        section.validate_definition_source_closure(
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        ),
        Err(ExportDefinitionSourceClosureValidationError::Extra {
            index,
            source: Box::new(extra),
        })
    );
}

#[test]
fn empty_section_has_fixed_wire_and_resolves() {
    let mut section = empty_section();
    let bytes = encode(&section.index_for_wire().unwrap()).unwrap();

    assert_eq!(hex(&bytes), "aa0180028003800480058006800780088009800a80");

    let decoded: DecodedCrossConeHirInterfaceSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);

    let mut identities = PendingIdentityValidation::new().finish().unwrap();
    let resolved = decoded.resolve(&mut identities).unwrap();
    assert_eq!(resolved, section);
    assert!(resolved.public_bindings().records().is_empty());
    assert!(resolved.nominal_interfaces().is_empty());
    assert!(resolved.callable_interfaces().is_empty());
    assert!(resolved.property_interfaces().is_empty());
    assert!(resolved.type_aliases().is_empty());
    assert!(resolved.source_interfaces().is_empty());
    assert!(resolved.default_templates().is_empty());
    assert!(resolved.constants().is_empty());
    assert!(resolved.definition_sources().is_empty());
    assert!(resolved.external_references().is_empty());
}

#[test]
fn reader_rejects_open_or_reordered_top_level_maps() {
    let reordered = vec![
        0xaa, 0x02, 0x80, 0x01, 0x80, 0x03, 0x80, 0x04, 0x80, 0x05, 0x80, 0x06, 0x80, 0x07, 0x80,
        0x08, 0x80, 0x09, 0x80, 0x0a, 0x80,
    ];

    for bytes in [vec![0xa9], vec![0xab], reordered] {
        assert!(
            decode_canonical::<DecodedCrossConeHirInterfaceSectionV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }
}

fn empty_section() -> CrossConeHirInterfaceSectionV1 {
    CrossConeHirInterfaceSectionV1::new(
        CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
        CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}

fn closure_section(
    omitted: Option<&ExportDefinitionSourceV1>,
    extra: Option<ExportDefinitionSourceV1>,
) -> (
    CrossConeHirInterfaceSectionV1,
    Vec<(ExportDefinitionSourceV1, ExportDefinitionSourceUseSiteV1)>,
) {
    let fixture = Fixture::new();
    let alias_origin = origin(1);
    let parameter_origin = origin(2);
    let template_origin = origin(3);
    let local_origin = origin(4);
    let statement_origin = origin(5);
    let expression_origin = origin(6);
    let reference_origin = origin(7);
    let constant_origin = origin(8);
    let key =
        ExportDefaultTemplateKeyV1::new(CallableTemplateOrigin::Function(fixture.function), 0);

    let alias = PersistentTypeAliasId::from_source_declaration(&SourceDeclarationKey::type_alias(
        declaration_site(),
        CanonicalIdentifier::new("Alias").unwrap(),
    ))
    .unwrap();
    let aliases = CanonicalTypeAliasInterfacesV1::try_new(vec![
        TypeAliasInterfaceRecordV1::try_new(
            alias,
            TypeAliasTargetV1::Signature(fixture.value_type()),
            PublicLookupAccessV1::DirectOnly,
            alias_origin.clone(),
        )
        .unwrap(),
    ])
    .unwrap();

    let source_interfaces = CanonicalCallableSourceInterfacesV1::try_new(vec![
        CallableSourceInterfaceV1::try_new(
            CallableTemplateOrigin::Function(fixture.function),
            CanonicalCallableSourceParametersV1::try_new(vec![
                CallableSourceParameterV1::new(
                    CanonicalIdentifier::new("value").unwrap(),
                    fixture.value_type(),
                    CallableParameterCallingV1::Default { template: key },
                    parameter_origin.clone(),
                ),
                CallableSourceParameterV1::new(
                    CanonicalIdentifier::new("duplicateOrigin").unwrap(),
                    fixture.value_type(),
                    CallableParameterCallingV1::Required,
                    parameter_origin.clone(),
                ),
            ])
            .unwrap(),
        )
        .unwrap(),
    ])
    .unwrap();

    let local = TemplateLocalRecordV1::try_new(
        fixture.local(),
        fixture.value_type(),
        CanonicalBooleanV1::False,
        TemplateLocalDefinitionV1::Source(local_origin.clone()),
    )
    .unwrap();
    let body = ExportDefaultBodyV1::try_new(
        vec![
            DefaultStatementV1::try_new(DefaultStatementKindV1::Break, statement_origin.clone())
                .unwrap(),
        ],
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::UnitLiteral,
            fixture.value_type(),
            expression_origin.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let references = ExportDefaultReferenceSetV1::try_new(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![ExportDefaultReferenceV1::new(
            fixture.property,
            reference_origin.clone(),
            ExportDefaultAccessWitnessV1::new(
                CallableTemplateOrigin::Function(fixture.function),
                ExportDefaultCallDomainV1::DirectPublic,
            ),
        )],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let template = ExportDefaultTemplateV1::try_new(
        key,
        PersistentLexicalRootV1::Function(fixture.function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::DefaultValue, 0),
            [],
        ),
        CanonicalTemplateLocalTableV1::try_new(vec![local]).unwrap(),
        body,
        fixture.value_type(),
        CanonicalBooleanV1::False,
        CanonicalBinderUseListV1::try_new(Vec::new()).unwrap(),
        OptionalTemplateReceiverV1::Absent,
        CanonicalTemplateValueParametersV1::try_new(Vec::new()).unwrap(),
        references,
        template_origin.clone(),
    )
    .unwrap();
    let templates = CanonicalExportDefaultTemplatesV1::try_new(vec![template]).unwrap();

    let constants = CanonicalExportConstValuesV1::try_new(vec![ExportConstValueV1::new(
        fixture.property,
        fixture.value_type(),
        CanonicalConstValueV1::Boolean(CanonicalBooleanV1::True),
        constant_origin.clone(),
    )])
    .unwrap();

    let expected = vec![
        (
            alias_origin,
            ExportDefinitionSourceUseSiteV1::TypeAlias { alias_index: 0 },
        ),
        (
            parameter_origin,
            ExportDefinitionSourceUseSiteV1::CallableSourceParameter {
                source_index: 0,
                parameter_index: 0,
            },
        ),
        (
            template_origin,
            ExportDefinitionSourceUseSiteV1::DefaultTemplateRoot { template_index: 0 },
        ),
        (
            local_origin,
            ExportDefinitionSourceUseSiteV1::DefaultTemplateLocal {
                template_index: 0,
                local_index: 0,
            },
        ),
        (
            statement_origin,
            ExportDefinitionSourceUseSiteV1::DefaultTemplateBody {
                template_index: 0,
                site: DefaultBodyOriginSiteV1::Statement,
            },
        ),
        (
            expression_origin,
            ExportDefinitionSourceUseSiteV1::DefaultTemplateBody {
                template_index: 0,
                site: DefaultBodyOriginSiteV1::Expression,
            },
        ),
        (
            reference_origin,
            ExportDefinitionSourceUseSiteV1::DefaultTemplateReference {
                template_index: 0,
                kind: ExportDefaultReferenceKindV1::Global,
                reference_index: 0,
            },
        ),
        (
            constant_origin,
            ExportDefinitionSourceUseSiteV1::Constant { constant_index: 0 },
        ),
    ];
    let mut declared: Vec<_> = expected
        .iter()
        .map(|(source, _)| source.clone())
        .filter(|source| omitted != Some(source))
        .collect();
    declared.extend(extra);

    (
        CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap(),
            CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalCallableInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
            aliases,
            source_interfaces,
            templates,
            constants,
            CanonicalExportDefinitionSourcesV1::try_new(declared).unwrap(),
            CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
        ),
        expected,
    )
}

fn origin(point: u64) -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::new(
        ConeIdentity::SINGLE_FILE,
        NormalizedSourcePath::new("main.scoop").unwrap(),
    )
    .unwrap();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(point, point + 1).unwrap(), &context)
            .unwrap(),
    )
}

fn declaration_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
