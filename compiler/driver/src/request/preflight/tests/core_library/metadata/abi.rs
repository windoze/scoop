use super::*;
use scoop_identity::{
    CoreBuiltinNominal, GcEffect, ScoopAbiArgument, ScoopAbiReturn, ScoopAbiValueShape,
};

pub(super) fn assert_abis(production: &scoop_slib::ValidatedCrossConeSemanticsProduction) {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let integer = intrinsic_type(
        production,
        scoop_hir::IntrinsicTypeKind::Integer(scoop_hir::IntegerKind::SIGNED_32),
    );
    let string = intrinsic_type(production, scoop_hir::IntrinsicTypeKind::String);
    let cases = [
        ("userCoreAbiUnit", vec![unit]),
        ("userCoreAbiMixed", vec![unit, integer, string]),
    ];
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
        assert_eq!(abi.gc_effect(), GcEffect::Managed);
        let [ScoopAbiArgument::ElidedZst(unit), tail @ ..] = abi.arguments() else {
            panic!("both functions elide the leading Unit argument");
        };
        assert_eq!(
            (unit.byte_size(), unit.shape()),
            (0, ScoopAbiValueShape::Aggregate)
        );
        match tail {
            [] => {
                assert_eq!(name, "userCoreAbiUnit");
                assert_eq!(abi.result(), ScoopAbiReturn::UnitVoid);
            }
            [
                ScoopAbiArgument::Direct(integer),
                ScoopAbiArgument::Direct(string),
            ] => {
                assert_eq!(name, "userCoreAbiMixed");
                assert_eq!(
                    (integer.byte_size(), integer.shape()),
                    (4, ScoopAbiValueShape::Scalar)
                );
                assert_eq!(
                    (string.byte_size(), string.shape()),
                    (8, ScoopAbiValueShape::Scalar)
                );
                assert_eq!(abi.result(), ScoopAbiReturn::Direct(*string));
            }
            arguments => panic!("unexpected core ABI arguments: {arguments:?}"),
        }
    }
}
