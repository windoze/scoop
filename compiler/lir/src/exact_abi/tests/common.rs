use super::*;

#[test]
fn common_abi_replay_matches_actual_exports_for_all_value_pass_modes() {
    let values: Vec<ExactLayoutExportV1> = vec![
        unit().into(),
        integer("Byte", IntegerKind::SIGNED_8).into(),
        managed().into(),
        fixtures::aggregate(false),
        fixtures::aggregate(true),
        fixtures::enumeration(&unit()),
        fixtures::enumeration(&managed()),
    ];
    let parameters = values.iter().collect::<Vec<_>>();
    for result in &values {
        let export = fixtures::function(result, &parameters);
        let signature = replay_canonical_scoop_abi_from_layouts(
            TARGET,
            export.canonical_signature().signature().clone(),
            export.call_protocol(),
            CallableAbiLayoutInputsV1 {
                receiver: CallableAbiReceiverInputV1::NoReceiver,
                parameters: &parameters,
                result,
            },
        )
        .unwrap();
        assert_eq!(&signature, export.canonical_signature());
        assert_eq!(
            encode(&signature).unwrap(),
            encode(export.canonical_signature()).unwrap()
        );
    }
}
