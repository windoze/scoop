use super::*;
use crate::SourceCallReceiver;

#[test]
fn source_call_receiver_role_is_derived_from_the_verified_declaration() {
    for fixture in [
        Fixture::simple(),
        Fixture::new(
            PublicDeclarationOwnerV1::Extension,
            Some(unit()),
            vec![unit()],
            unit(),
            vec![],
        ),
        Fixture::new(
            PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(nominal().id())),
            None,
            vec![unit()],
            unit(),
            vec![],
        ),
    ] {
        let expected = fixture.receiver.has_receiver();
        let wrong = if expected {
            SourceCallReceiver::NoReceiver
        } else {
            SourceCallReceiver::Receiver {
                static_type: unit_exact(),
            }
        };
        let valid = fixture.call(0, vec![unit_exact(); 2], unit_exact());
        let site = HirDependencyCallSiteV1::try_new(
            valid.position(),
            valid.origin().clone(),
            valid.arguments().to_vec(),
            valid.result(),
            vec![0],
            wrong,
        )
        .unwrap();
        assert!(
            matches!(validate(&fixture, &site), Err(HirDependencyCallSignatureError::ReceiverRole { expected: e, actual }) if e == expected && actual != e)
        );
    }
}
