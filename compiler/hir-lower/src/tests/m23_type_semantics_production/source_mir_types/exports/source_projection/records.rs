use super::*;

#[test]
fn actual_mir_source_projection_rejects_reordered_fields_with_unchanged_identities_and_facts() {
    let (_, source) = fixture("combined");
    with_exports(&source, |input, dependencies, candidate| {
        let source = project(input, dependencies).unwrap();
        let mut types = candidate.types().records().to_vec();
        let index = types.iter().position(|record| {
            matches!(record.representation(), mir::MirTypeRepresentationV1::Struct { fields, .. } if fields.len() > 1)
        }).unwrap();
        let original = &types[index];
        let exact = original.exact();
        let mut representation = original.representation().clone();
        let mir::MirTypeRepresentationV1::Struct { fields, .. } = &mut representation else {
            unreachable!()
        };
        fields.reverse();
        types[index] = mir::ParamFreeMirTypeExportV1::try_new(
            mir::MirTypeBridgeAuthority {
                identities: input.identities,
                foundation: input.mir.foundation(),
            },
            exact,
            original.origin().clone(),
            original.facts(),
            representation,
            original.base_and_interfaces().clone(),
        )
        .unwrap();
        let candidate = mir::MirTypeBridgeExportConstituentsV1::new(
            mir::CanonicalParamFreeMirTypeExportsV1::try_new(types).unwrap(),
            candidate.callables().clone(),
            candidate.dispatch().clone(),
            candidate.objects().clone(),
            candidate.shapes().clone(),
            candidate.initialization_uses().clone(),
        );
        assert!(matches!(
            candidate.validate_sources(input.mir.module().cone, input.identities, &source),
            Err(mir::MirTypeBridgeSourceJoinError::Record(mir::MirTypeBridgeSourceRecordV1::Type(actual))) if actual == exact
        ));
    });
}
