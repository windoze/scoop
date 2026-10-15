use super::*;
use scoop_lir_lower::LayoutAbiExportInputV1;

pub(super) fn probe(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
) {
    let named = |name| {
        expected
            .direct_callables()
            .exports()
            .iter()
            .find(|record| assertions::callable_name(input, record.target().into()) == name)
            .unwrap()
    };
    let value = named("SharedAbiEmpty.wide");
    let donor = named("SharedAbiEmpty.drop");
    assert!(matches!(
        value.abi_signature().arguments()[1],
        scoop_identity::ScoopAbiArgument::Indirect(_)
    ));
    assert!(matches!(
        value.abi_signature().result(),
        scoop_identity::ScoopAbiReturn::Indirect(_)
    ));
    assert_eq!(
        donor.abi_signature().result(),
        scoop_identity::ScoopAbiReturn::UnitVoid
    );
    let empty = named("SharedAbiEmpty.empty");
    let parameters = empty.abi_signature().signature().parameters();
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0], parameters[1]);
    assert!(
        empty
            .abi_signature()
            .arguments()
            .iter()
            .all(|argument| matches!(argument, scoop_identity::ScoopAbiArgument::ElidedZst(_)))
    );
    assert!(matches!(
        empty.abi_signature().result(),
        scoop_identity::ScoopAbiReturn::ElidedZst(_)
    ));
    assert_ne!(value.abi_signature(), donor.abi_signature());
    assert_ne!(value.root_plan(), donor.root_plan());
}
