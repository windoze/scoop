use super::*;
use crate::SourceCallReceiver;

fn receiver_call(fixture: &Fixture) -> HirDependencyCallSiteV1 {
    let site = fixture.site(0, vec![0]).unwrap();
    HirDependencyCallSiteV1::try_new(
        site.position(),
        site.origin().clone(),
        site.arguments().to_vec(),
        site.result(),
        vec![0],
        SourceCallReceiver::Receiver {
            static_type: fixture.unit,
        },
    )
    .unwrap()
}

#[test]
fn source_call_receiver_round_trips_and_is_required_on_the_wire() {
    let fixture = Fixture::new();
    for site in [fixture.site(0, vec![0]).unwrap(), receiver_call(&fixture)] {
        let bytes = encode(&site).unwrap();
        let decoded: DecodedHirDependencyCallSiteV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(
            decoded
                .resolve(&mut fixture.graph(), &WirePath::root())
                .unwrap(),
            site
        );
        let mut missing = bytes;
        missing.truncate(missing.len() - 1 - encode(&site.receiver()).unwrap().len());
        assert!(decode_canonical::<DecodedHirDependencyCallSiteV1>(&missing).is_err());
        missing[0] = 0xa6;
        let error = decode_canonical::<DecodedHirDependencyCallSiteV1>(&missing).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 7,
                actual: 6
            }
        );
    }
}

#[test]
fn source_call_receiver_rejects_unknown_tags_and_inexact_payloads() {
    let fixture = Fixture::new();
    let site = fixture.site(0, vec![0]).unwrap();
    for malformed in [vec![0xa1, 0, 3], vec![0xa1, 0, 2], vec![0xa2, 0, 1, 1, 0]] {
        let mut bytes = encode(&site).unwrap();
        bytes.truncate(bytes.len() - encode(&site.receiver()).unwrap().len());
        bytes.extend(malformed);
        let error = decode_canonical::<DecodedHirDependencyCallSiteV1>(&bytes).unwrap_err();
        assert_eq!(error.path(), &WirePath::root().field(7));
    }
}

#[test]
fn source_call_receiver_requires_an_argument() {
    let fixture = Fixture::new();
    let site = receiver_call(&fixture);
    assert_eq!(
        HirDependencyCallSiteV1::try_new(
            site.position(),
            site.origin().clone(),
            vec![],
            site.result(),
            vec![0],
            site.receiver(),
        ),
        Err(HirDependencyCallSiteBuildError::MissingReceiverArgument)
    );
}
