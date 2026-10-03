use super::*;

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    mutation: RuntimeMutation,
) {
    let bytes = match mutation {
        RuntimeMutation::Field(field) => {
            link_archive::rewrite_production(artifact, |data| corrupt_field(data, field))
        }
        RuntimeMutation::Patch(member, offset) | RuntimeMutation::Body(member, offset) => {
            link_archive::rewrite(artifact, |record, data| {
                if record.id() != member {
                    return Rewrite::Keep;
                }
                let mut changed = data.to_vec();
                changed[usize::try_from(offset).unwrap()] ^= 1;
                Rewrite::Replace(changed)
            })
        }
    };
    let changed = DecodedSlibEnvelope::open(&bytes, artifact.target_selection())
        .unwrap()
        .validate_graph()
        .unwrap()
        .decode_cross_cone_layout_link_sections()
        .unwrap()
        .into_shared_sections()
        .unwrap();
    let shared = reader::read_sections(
        reader::open_link(core).into_shared_sections().unwrap(),
        changed,
    );
    let error = shared
        .replay_link_symbol_uses(profile)
        .map(|_| panic!("partial runtime replay escaped"))
        .unwrap_err();
    assert_eq!(error.provider, reader::open_link(artifact).identity());
    let slib::SharedLirPhysicalError::LinkSymbolUses(source) = *error.source else {
        panic!("expected finalization error for {mutation:?}: {error:?}")
    };
    match mutation {
        RuntimeMutation::Field(field) => assert!(
            matches!(*source,
            slib::LayoutLinkSymbolUseError::RuntimeProjection(slib::RuntimeProductionProjectionError::FieldMismatch { field: actual })
                if actual == field),
            "wrong error for {mutation:?}: {source:?}"
        ),
        RuntimeMutation::Patch(member, _) => assert!(
            matches!(*source,
            slib::LayoutLinkSymbolUseError::FinalObjects(slib::StrongLinkObjectFinalizationError::FinalObjectMismatch(
                slib::ReconstructedScoopObjectError::ByteMismatch(actual))) if actual == member),
            "wrong error for {mutation:?}: {source:?}"
        ),
        RuntimeMutation::Body(_, _) => assert!(
            matches!(
                *source,
                slib::LayoutLinkSymbolUseError::RuntimeFingerprintMismatch
            ),
            "wrong error for {mutation:?}: {source:?}"
        ),
    }
}

fn corrupt_field(bytes: &[u8], field: u32) -> Vec<u8> {
    let range = wire::field_range(bytes, u64::from(field));
    let mut changed = bytes.to_vec();
    match field {
        3 | 6 => changed[range.end - 1] ^= 1,
        4 | 5 => {
            // Both projections use six typed tables in a closed map.
            let tables = &bytes[range.clone()];
            let record = (1..=6)
                .map(|key| wire::field_range(tables, key))
                .find(|array| !wire::array_parts(&tables[array.clone()]).is_empty())
                .unwrap();
            changed[range.start + record.end - 1] ^= 1;
        }
        _ => panic!("runtime projection only includes fields 3 through 6"),
    }
    changed
}
