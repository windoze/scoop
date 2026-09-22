use super::*;
use scoop_identity::{CanonicalScoopStorage, CoreBuiltinNominal, ScoopAbiArgument, ScoopAbiReturn};

pub(super) fn assert_abis(production: &scoop_slib::ValidatedCrossConeSemanticsProduction) {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let integer = CoreNativeBoundaryNominal::Signed32.concrete_id().unwrap();
    let string = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site(),
        name("String"),
        SourceNominalKind::Class,
        0,
    ))
    .unwrap();
    let cases = [
        ("userCoreAbiUnit", vec![unit]),
        ("userCoreAbiMixed", vec![unit, integer, string]),
    ];
    let mut dump = String::new();
    for (name, parameters) in cases {
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                site(),
                super::name(name),
                0,
                None,
                parameters
                    .into_iter()
                    .map(SignatureTypeKey::Nominal)
                    .collect(),
            ))
            .unwrap();
        let declaration = DependencyCallableDeclarationId::Function(function);
        let mir = production.mir_cross_cone().export(declaration).unwrap();
        let lir = production.lir_cross_cone().export(declaration).unwrap();
        let abi = lir.abi_signature();
        assert_eq!(abi.signature(), mir.signature());
        let arguments = abi
            .arguments()
            .iter()
            .map(|argument| match argument {
                ScoopAbiArgument::ElidedZst(value) => storage("elided", value),
                ScoopAbiArgument::Direct(value) => storage("direct", value),
                ScoopAbiArgument::Indirect(value) => storage("indirect", value),
            })
            .collect::<Vec<_>>()
            .join(",");
        let result = match abi.result() {
            ScoopAbiReturn::UnitVoid => "unit".to_owned(),
            ScoopAbiReturn::ElidedZst(value) => storage("elided", &value),
            ScoopAbiReturn::Direct(value) => storage("direct", &value),
            ScoopAbiReturn::Indirect(value) => storage("indirect", &value),
        };
        dump.push_str(&format!(
            "{name}: {:?} args=[{arguments}] result={result}\n",
            abi.gc_effect()
        ));
    }
    assert_eq!(
        dump,
        include_str!("../../../../../../../../tests/fixtures/core-library/abi.snap")
    );
}

fn storage(passing: &str, value: &CanonicalScoopStorage) -> String {
    format!("{passing}:{}:{:?}", value.byte_size(), value.shape())
}
