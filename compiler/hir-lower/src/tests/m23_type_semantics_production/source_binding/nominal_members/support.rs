use super::*;

#[derive(Clone)]
pub(in crate::tests::m23_type_semantics_production::source_binding) struct Sources {
    pub nominals: hir::CanonicalNominalSourceContractsV1,
    pub properties: Properties,
    pub callables: Callables,
}
impl Sources {
    pub(in crate::tests::m23_type_semantics_production::source_binding) fn from_output(
        output: &hir::OrdinaryHirOutput<'_>,
        fixture: &mut Fixture,
    ) -> Self {
        macro_rules! restore {
            ($source:expr, $decoded:ty) => {{
                let source = $source;
                let bytes = encode(&source).unwrap();
                let decoded: $decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
                let restored = decoded
                    .resolve(&mut fixture.identities, &mut meter())
                    .unwrap();
                assert_eq!(encode(&restored).unwrap(), bytes);
                restored
            }};
        }
        let nominals = restore!(
            hir::CanonicalNominalSourceContractsV1::from_export_hir(
                &output.output().export,
                &fixture.source.entries().source_roots,
                &mut meter()
            )
            .unwrap(),
            hir::DecodedCanonicalNominalSourceContractsV1
        );
        let mut properties = Vec::new();
        let mut callables = BTreeSet::new();
        for record in nominals.records() {
            for member in record.members().values() {
                match member {
                    hir::NestedSourceMemberRefV1::Property(id) => properties.push(*id),
                    hir::NestedSourceMemberRefV1::Function(id) => {
                        callables.insert(CallableTemplateOrigin::Function(*id));
                    }
                    hir::NestedSourceMemberRefV1::GenericFunction(id) => {
                        callables.insert(CallableTemplateOrigin::GenericFunction(*id));
                    }
                }
            }
            if let hir::NominalSourceShapeV1::Enum(shape) = record.source_shape() {
                callables.extend(
                    shape
                        .variants()
                        .iter()
                        .map(|v| CallableTemplateOrigin::VariantConstructor(v.variant())),
                );
            }
        }
        let properties = restore!(
            Properties::from_export_hir(
                &output.output().export,
                &hir::CanonicalPersistentIdsV1::try_new(properties).unwrap(),
                &mut meter()
            )
            .unwrap(),
            hir::DecodedCanonicalNominalSourcePropertiesV1
        );
        for property in properties.records() {
            if let hir::NominalSupportPropertyPayloadV1::Runtime { interface } = property.payload()
            {
                callables.insert(CallableTemplateOrigin::Accessor(interface.getter()));
                if let hir::ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } =
                    interface.mutability()
                {
                    callables.insert(CallableTemplateOrigin::Accessor(*setter));
                }
            }
        }
        let callables = restore!(
            Callables::from_export_hir(&output.output().export, &callables, &mut meter()).unwrap(),
            hir::DecodedCanonicalNominalSourceCallablesV1
        );
        Self {
            nominals,
            properties,
            callables,
        }
    }
}
pub(super) fn with_sources(
    source: &str,
    run: impl FnOnce(
        &hir::OrdinaryHirOutput<'_>,
        &Fixture,
        &Sources,
        &hir::ImportedCoreFundamentalTypeProtocol,
    ),
) {
    with_hir_source(source, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        run(
            output,
            &fixture,
            &sources,
            inputs.protocols().fundamental_types(),
        );
    });
}
