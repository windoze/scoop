use scoop_identity::ConeIdentity;
use scoop_wire::encode;

use super::RegistrationIdentitySurfaceV1;
use crate::{CanonicalLirFoundation, ConeLirFoundation};

#[test]
fn six_empty_tables_have_a_fixed_wire_shape() {
    let foundation = empty_foundation();
    let surface = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();

    assert!(surface.static_storages().is_empty());
    assert!(surface.immortal_objects().is_empty());
    assert!(surface.initialization_units().is_empty());
    assert!(surface.type_registrations().is_empty());
    assert!(surface.safepoints().is_empty());
    assert!(surface.callables().is_empty());
    assert_eq!(
        hex(&encode(&surface).unwrap()),
        "a6018002800380048005800680"
    );
}

fn empty_foundation() -> ConeLirFoundation {
    ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty()).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
