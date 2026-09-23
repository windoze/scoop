use super::*;
use crate::core_protocol_test_support::{ordinary_origin, standalone_at};
use crate::{
    DecodedHirFoundation, ImportedCoreProtocols, ImportedHirFoundation, IntegerKind,
    OdrFreeHirFoundation,
};
use scoop_identity::{
    PendingIdentityValidation, PersistentTypeId, SemanticIdentitySession, SemanticOriginFingerprint,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

type NominalCase = (TypeId, PersistentTypeId, ConeIdentity);

#[test]
fn source_nominal_providers_follow_the_exact_identity_roles_for_every_provider() {
    for provider in [ConeIdentity::CORE, ordinary_origin()] {
        with_types(provider, |inputs, cases, _| {
            let exact = ExactTypeIdentities::from_types(inputs).unwrap();
            for &(ty, nominal, origin) in cases {
                assert_eq!(exact[ty].key(), &ExactTypeKey::Nominal(nominal));
                assert_eq!(inputs.source_nominal_provider(ty), Some(origin));
            }
        });
    }
}

#[test]
fn structural_combinations_keep_their_exact_identity_without_a_nominal_provider() {
    with_types(ordinary_origin(), |inputs, _, structural| {
        let exact = ExactTypeIdentities::from_types(inputs).unwrap();
        for &ty in structural {
            assert!(matches!(
                exact[ty].key(),
                ExactTypeKey::Tuple(_) | ExactTypeKey::RawPointer(_)
            ));
            assert_eq!(inputs.source_nominal_provider(ty), None);
        }
    });
}

fn with_types(
    provider: ConeIdentity,
    run: impl FnOnce(ExactTypeIdentityInputs<'_>, &[NominalCase], &[TypeId]),
) {
    let protocols = imported_protocols(provider);
    let roles = protocols.fundamental_types();
    let mut types = Arena::new();
    let mut cases = Vec::new();
    for kind in IntegerKind::ALL {
        let ty = types.alloc(Type {
            kind: TypeKind::Integer(kind),
            gc_free: true,
        });
        cases.push((ty, roles.integer(kind).persistent(), provider));
    }
    let boolean = types.alloc(Type {
        kind: TypeKind::Boolean,
        gc_free: true,
    });
    cases.push((boolean, roles.boolean().persistent(), provider));
    let string = types.alloc(Type {
        kind: TypeKind::String,
        gc_free: false,
    });
    cases.push((string, roles.string().persistent(), provider));
    for (kind, nominal) in [
        (TypeKind::Unit, CoreBuiltinNominal::Unit),
        (TypeKind::Any, CoreBuiltinNominal::Any),
    ] {
        let ty = types.alloc(Type {
            gc_free: matches!(kind, TypeKind::Unit),
            kind,
        });
        cases.push((
            ty,
            nominal.identity_record().id(),
            nominal.declaration_key().origin(),
        ));
    }
    let tuple = types.alloc(Type {
        kind: TypeKind::Tuple(vec![boolean, string]),
        gc_free: false,
    });
    let pointer = types.alloc(Type {
        kind: TypeKind::Ptr(tuple),
        gc_free: true,
    });
    run(
        ExactTypeIdentityInputs {
            types: &types,
            function_types: &Arena::new(),
            structs: &Arena::new(),
            enums: &Arena::new(),
            classes: &Arena::new(),
            interfaces: &Arena::new(),
            objects: &Arena::new(),
            core_types: ConcreteCoreTypeIdentityAuthority::Imported(roles),
        },
        &cases,
        &[tuple, pointer],
    );
}

fn imported_protocols(origin: ConeIdentity) -> ImportedCoreProtocols {
    let (surface, foundation) = standalone_at(origin);
    let decoded: DecodedHirFoundation =
        decode_canonical(&encode(&foundation).unwrap(), DecodeLimits::default()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    if origin != ConeIdentity::CORE {
        pending.register_authority(origin).unwrap();
    }
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let graph = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (hir, _, _) = session
        .import(
            origin,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &graph,
        )
        .unwrap()
        .into_parts();
    let imported = ImportedHirFoundation::from_odr_free(
        OdrFreeHirFoundation::try_new(foundation).unwrap(),
        hir,
    );
    ImportedCoreProtocols::import(&imported, &surface).unwrap()
}
