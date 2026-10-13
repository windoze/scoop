use super::*;
use scoop_identity::{ExecutableEntryDeclaration, GeneratedCallableKey};

#[test]
fn generated_entry_preserves_its_identity_alongside_a_source_main() {
    let mut foundation = CanonicalHirFoundation::empty();
    let main = function("main");
    let generated = CborIdentityRecord::from_key(GeneratedCallableKey::TestEntry {
        cone: ConeIdentity::CORE,
    })
    .unwrap();
    let exact = unit_exact_record();
    foundation.set_functions(vec![main]).unwrap();
    foundation
        .set_generated_callables(vec![generated.clone()])
        .unwrap();
    foundation.set_exact_types(vec![exact.clone()]).unwrap();
    let entry = ExecutableEntryIdentity::for_test(
        &generated,
        ExactCallableSignature::new(scoop_identity::Effect::Ordinary, None, vec![], exact.id()),
    )
    .unwrap();
    assert_eq!(
        entry.declaration(),
        ExecutableEntryDeclaration::Generated(generated.id())
    );
    let output = HirOutputContractV1::Executable(Box::new(entry));
    let decode = || {
        decode_canonical::<crate::DecodedHirOutputContractV1>(&encode(&output).unwrap()).unwrap()
    };
    assert_eq!(
        decode().validate_against(ConeIdentity::CORE, &foundation),
        Ok(output.clone())
    );
    assert!(matches!(
        decode().validate_against(ConeIdentity::SINGLE_FILE, &foundation),
        Err(crate::HirOutputContractValidationError::ForeignEntry { .. })
    ));
    foundation.set_generated_callables(vec![]).unwrap();
    assert!(matches!(
        decode().validate_against(ConeIdentity::CORE, &foundation),
        Err(crate::HirOutputContractValidationError::UnknownEntry(_))
    ));
}
