//! Persistent identity framework tests: definition keys, exact types,
//! diagnostic names, callable bodies, ODR groups and the mangler.

use std::collections::BTreeMap;

use crate::persistent::{
    AtomOwnerStep, CallableArguments, CallableBodyKey, CallableOdrMemberId, DefinitionKey,
    DiagnosticNameContext, ExactTypeKey, ExactTypeTable, GeneratedNominalRole, MainCallableBodyId,
    ManagedFunctionEffect, NativeCallingConvention, NoCallableArguments, NominalAtom,
    NominalKindTag, OdrGroupId, OdrMemberId, OdrMemberRole, OwnerApplication, OwnerKind, OwnerStep,
    PersistentCallableBodyId, PersistentExactTypeId, PersistentFunctionId, PersistentGenericTypeId,
    PersistentTypeId, SpecializationKey, StrongCallableDefinitionOwner, SymbolKind,
    decode_callable_body_key, mangle, truncated_runtime_id,
};
use crate::{ConeCoordinate, ConeIdentity};

fn cone() -> ConeIdentity {
    ConeIdentity::of(&ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap())
}

#[test]
fn definition_keys_hash_by_kind_domain() {
    let key = DefinitionKey::Source {
        package: "dev.example".to_owned(),
        owner: vec![OwnerStep {
            kind: OwnerKind::Type,
            id: [7; 32],
        }],
        name: "render".to_owned(),
        signature: b"sig-bytes".to_vec(),
    }
    .canonical_cbor();

    let function = PersistentFunctionId::from_definition_key(cone(), &key);
    let type_id = PersistentTypeId::from_definition_key(cone(), &key);
    // Same key bytes, different kinds → different ids.
    assert_ne!(function.as_bytes(), type_id.as_bytes());
    // Deterministic.
    assert_eq!(
        function.as_bytes(),
        PersistentFunctionId::from_definition_key(cone(), &key).as_bytes()
    );
    // Different cone → different id.
    let other_cone = ConeIdentity::of(&ConeCoordinate::new("dev.example", "app", "0.2.0").unwrap());
    assert_ne!(
        function.as_bytes(),
        PersistentFunctionId::from_definition_key(other_cone, &key).as_bytes()
    );

    let generated = DefinitionKey::GeneratedByOwner {
        owner: OwnerStep {
            kind: OwnerKind::Function,
            id: *function.as_bytes(),
        },
        structural_path: b"closure-1".to_vec(),
    }
    .canonical_cbor();
    assert_ne!(generated, key);
}

#[test]
fn mangler_shape() {
    let id = PersistentFunctionId::from_definition_key(cone(), b"key");
    let symbol = mangle(SymbolKind::Function, id.as_bytes());
    assert!(symbol.starts_with("scoop$1$fn$"));
    assert_eq!(symbol.len(), "scoop$1$fn$".len() + 64);
    assert!(
        symbol
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '$')
    );
    // Different kind tag changes the prefix only.
    assert_ne!(
        mangle(SymbolKind::Method, id.as_bytes()),
        mangle(SymbolKind::Function, id.as_bytes())
    );
    // Two same-name entities across cones cannot collide.
    let other = PersistentFunctionId::from_definition_key(
        ConeIdentity::of(&ConeCoordinate::new("org.other", "app", "0.1.0").unwrap()),
        b"key",
    );
    assert_ne!(
        mangle(SymbolKind::Function, id.as_bytes()),
        mangle(SymbolKind::Function, other.as_bytes())
    );
}

#[test]
fn runtime_id_truncation_is_nonzero_and_deterministic() {
    let id = PersistentExactTypeId::from_validated([9; 32]);
    assert_ne!(truncated_runtime_id(id.as_bytes()), 0);
    assert_eq!(
        truncated_runtime_id(id.as_bytes()),
        truncated_runtime_id(id.as_bytes())
    );
    // The zero-prefix fallback still yields nonzero.
    assert_ne!(truncated_runtime_id(&[0u8; 32]), 0);
}

type AtomMaps = (
    BTreeMap<[u8; 32], NominalAtom>,
    BTreeMap<[u8; 32], NominalAtom>,
);

fn nominal_table() -> (ExactTypeTable, AtomMaps) {
    let mut exact = ExactTypeTable::new();
    let mut nominal_atoms = BTreeMap::new();
    let mut generic_atoms = BTreeMap::new();

    let unit = {
        let declaration = PersistentTypeId::from_definition_key(cone(), b"unit-decl");
        let key = ExactTypeKey::Nominal { declaration };
        nominal_atoms.insert(
            *declaration.as_bytes(),
            NominalAtom {
                coordinate: ConeCoordinate::new("scoop", "scoop.core", "0.1.0").unwrap(),
                package: "scoop.core".to_owned(),
                owner: vec![],
                kind: NominalKindTag::Struct,
                name: "Unit".to_owned(),
            },
        );
        exact.intern(key).unwrap()
    };
    let int = {
        let declaration = PersistentTypeId::from_definition_key(cone(), b"int-decl");
        let key = ExactTypeKey::Nominal { declaration };
        nominal_atoms.insert(
            *declaration.as_bytes(),
            NominalAtom {
                coordinate: ConeCoordinate::new("scoop", "scoop.core", "0.1.0").unwrap(),
                package: "scoop.core".to_owned(),
                owner: vec![],
                kind: NominalKindTag::Struct,
                name: "Int".to_owned(),
            },
        );
        exact.intern(key).unwrap()
    };
    let _boxed_int = {
        let origin = PersistentGenericTypeId::from_definition_key(cone(), b"box-decl");
        generic_atoms.insert(
            *origin.as_bytes(),
            NominalAtom {
                coordinate: ConeCoordinate::new("scoop", "scoop.core", "0.1.0").unwrap(),
                package: "scoop.core".to_owned(),
                owner: vec![],
                kind: NominalKindTag::Struct,
                name: "Box".to_owned(),
            },
        );
        let key = ExactTypeKey::application(origin, vec![int]).unwrap();
        exact.intern(key).unwrap()
    };
    let _ = unit;
    (exact, (nominal_atoms, generic_atoms))
}

#[test]
fn exact_type_ids_are_structural() {
    let (mut exact, _) = nominal_table();
    let int = exact
        .intern(ExactTypeKey::Nominal {
            declaration: PersistentTypeId::from_definition_key(cone(), b"int-decl"),
        })
        .unwrap();
    // Same structure re-interns to the same id.
    let again = exact
        .intern(ExactTypeKey::Nominal {
            declaration: PersistentTypeId::from_definition_key(cone(), b"int-decl"),
        })
        .unwrap();
    assert_eq!(int, again);

    // Tuple and function shapes.
    let tuple = ExactTypeKey::tuple(vec![int]).unwrap();
    let tuple_id = exact.intern(tuple).unwrap();
    let function = ExactTypeKey::Function {
        effect: ManagedFunctionEffect::Ordinary,
        parameters: vec![int],
        result: int,
    };
    let function_id = exact.intern(function).unwrap();
    assert_ne!(tuple_id, function_id);
    // Suspend differs from ordinary.
    let suspend = ExactTypeKey::Function {
        effect: ManagedFunctionEffect::Suspend,
        parameters: vec![int],
        result: int,
    };
    assert_ne!(exact.intern(suspend).unwrap(), function_id);
    // Raw pointer and native function pointer are distinct categories.
    let raw = exact
        .intern(ExactTypeKey::RawPointer { pointee: int })
        .unwrap();
    let native = exact
        .intern(ExactTypeKey::NativeFunctionPointer {
            calling_convention: NativeCallingConvention::C,
            parameters: vec![int],
            result: int,
        })
        .unwrap();
    assert_ne!(raw, native);
    assert_ne!(raw, function_id);

    exact.validate().unwrap();
}

#[test]
fn exact_type_shapes_are_checked() {
    let mut exact = ExactTypeTable::new();
    let missing = PersistentExactTypeId::from_validated([1; 32]);
    // Constructors enforce collection shape only.
    assert!(ExactTypeKey::tuple(vec![]).is_err());
    let origin = PersistentGenericTypeId::from_definition_key(cone(), b"origin");
    assert!(ExactTypeKey::application(origin, vec![]).is_err());
    // Interning a key with an unregistered child fails.
    assert!(
        exact
            .intern(ExactTypeKey::RawPointer { pointee: missing })
            .is_err()
    );
    assert!(
        exact
            .intern(ExactTypeKey::tuple(vec![missing]).unwrap())
            .is_err()
    );
}

#[test]
fn canonical_diagnostic_names_follow_the_grammar() {
    let (exact, (nominal_atoms, generic_atoms)) = nominal_table();
    let int = exact
        .get(&{
            // Recover the int id by structural lookup.
            let key = ExactTypeKey::Nominal {
                declaration: PersistentTypeId::from_definition_key(cone(), b"int-decl"),
            };
            PersistentExactTypeId::of(&key)
        })
        .map(|_| {
            PersistentExactTypeId::of(&ExactTypeKey::Nominal {
                declaration: PersistentTypeId::from_definition_key(cone(), b"int-decl"),
            })
        })
        .unwrap();
    let boxed = PersistentExactTypeId::of(
        &ExactTypeKey::application(
            PersistentGenericTypeId::from_definition_key(cone(), b"box-decl"),
            vec![int],
        )
        .unwrap(),
    );
    let context = DiagnosticNameContext {
        exact: &exact,
        nominal_atoms: &nominal_atoms,
        generic_atoms: &generic_atoms,
    };

    let int_name = context.name(&int).unwrap();
    assert_eq!(
        int_name,
        "n(c=scoop%3Ascoop.core%3A0.1.0;p=scoop.core;o=-;k=S;x=Int)"
    );

    let boxed_name = context.name(&boxed).unwrap();
    assert_eq!(
        boxed_name,
        "a(n(c=scoop%3Ascoop.core%3A0.1.0;p=scoop.core;o=-;k=S;x=Box);[n(c=scoop%3Ascoop.core%3A0.1.0;p=scoop.core;o=-;k=S;x=Int)])"
    );

    // Escaping: a coordinate/name with ':' and spaces percent-escapes.
    let mut escaped_atoms = nominal_atoms.clone();
    let escaped_decl = PersistentTypeId::from_definition_key(cone(), b"esc-decl");
    let mut exact2 = exact.clone();
    let escaped_id = exact2
        .intern(ExactTypeKey::Nominal {
            declaration: escaped_decl,
        })
        .unwrap();
    escaped_atoms.insert(
        *escaped_decl.as_bytes(),
        NominalAtom {
            coordinate: ConeCoordinate::new("dev.example", "app", "0.1.0").unwrap(),
            package: "dev.example".to_owned(),
            owner: vec![AtomOwnerStep {
                kind: NominalKindTag::Class,
                name: "Outer".to_owned(),
            }],
            kind: NominalKindTag::Enum,
            name: "My State".to_owned(),
        },
    );
    // Intern the remaining shapes before borrowing the table.
    let generated = exact2
        .intern(ExactTypeKey::GeneratedNominal {
            role: GeneratedNominalRole::CoroutineFrame,
            identity: [0xAB; 32],
        })
        .unwrap();
    let fn_id = exact2
        .intern(ExactTypeKey::Function {
            effect: ManagedFunctionEffect::Ordinary,
            parameters: vec![int],
            result: int,
        })
        .unwrap();
    let tuple_id = exact2
        .intern(ExactTypeKey::tuple(vec![int, int]).unwrap())
        .unwrap();
    let raw_id = exact2
        .intern(ExactTypeKey::RawPointer { pointee: int })
        .unwrap();
    let native_id = exact2
        .intern(ExactTypeKey::NativeFunctionPointer {
            calling_convention: NativeCallingConvention::C,
            parameters: vec![],
            result: int,
        })
        .unwrap();
    let context2 = DiagnosticNameContext {
        exact: &exact2,
        nominal_atoms: &escaped_atoms,
        generic_atoms: &generic_atoms,
    };
    assert_eq!(
        context2.name(&escaped_id).unwrap(),
        "n(c=dev.example%3Aapp%3A0.1.0;p=dev.example;o=C:Outer;k=E;x=My%20State)"
    );
    let expected = format!("g(r=03;i={})", "ab".repeat(32));
    assert_eq!(context2.name(&generated).unwrap(), expected);
    let expected_fn = format!("f(o;[{int_name}]->{int_name})");
    assert_eq!(context2.name(&fn_id).unwrap(), expected_fn);
    assert_eq!(
        context2.name(&tuple_id).unwrap(),
        format!("t([{int_name},{int_name}])")
    );
    assert_eq!(context2.name(&raw_id).unwrap(), format!("r({int_name})"));
    assert_eq!(
        context2.name(&native_id).unwrap(),
        format!("x(c;[]->{int_name})")
    );
}

#[test]
fn name_budget_and_cycles_are_rejected() {
    let (mut exact, (nominal_atoms, generic_atoms)) = nominal_table();
    // Build a deep chain that shares one leaf exponentially: t(a,a),
    // t(b,b) with b=t(a,a), ... 30 levels.
    let int = PersistentExactTypeId::of(&ExactTypeKey::Nominal {
        declaration: PersistentTypeId::from_definition_key(cone(), b"int-decl"),
    });
    let mut current = exact
        .intern(ExactTypeKey::tuple(vec![int]).unwrap())
        .unwrap();
    for _ in 0..30 {
        current = exact
            .intern(ExactTypeKey::tuple(vec![current, current]).unwrap())
            .unwrap();
    }
    let context = DiagnosticNameContext {
        exact: &exact,
        nominal_atoms: &nominal_atoms,
        generic_atoms: &generic_atoms,
    };
    // The expanded name would exceed any sane bound; the printer must
    // reject by cost, not by materializing it.
    assert!(matches!(
        context.name(&current),
        Err(crate::persistent::ExactTypeError::NameTooLarge(_))
    ));

    // A manual cycle cannot be interned (children must exist first), so
    // craft one by table surgery to prove the printer detects it.
    let a = exact
        .intern(ExactTypeKey::tuple(vec![int]).unwrap())
        .unwrap();
    let mut broken = exact.clone();
    broken
        .entries_mut()
        .insert(*a.as_bytes(), ExactTypeKey::tuple(vec![a]).unwrap());
    let context = DiagnosticNameContext {
        exact: &broken,
        nominal_atoms: &nominal_atoms,
        generic_atoms: &generic_atoms,
    };
    assert!(matches!(
        context.name(&a),
        Err(crate::persistent::ExactTypeError::NameCycle)
    ));
}

#[test]
fn callable_body_keys_round_trip() {
    let owner = StrongCallableDefinitionOwner::from_function(
        PersistentFunctionId::from_definition_key(cone(), b"main"),
    );
    let strong = CallableBodyKey::Strong { owner };
    assert_eq!(
        decode_callable_body_key(&strong.canonical_bytes()),
        Ok(strong.clone())
    );

    let odr = CallableBodyKey::Odr {
        member: CallableOdrMemberId::from_bytes([3; 32]),
    };
    assert_eq!(
        decode_callable_body_key(&odr.canonical_bytes()),
        Ok(odr.clone())
    );

    let main = MainCallableBodyId::from_bytes(*PersistentCallableBodyId::of(&strong).as_bytes());
    let root = CallableBodyKey::RootGateway {
        root_cone: cone(),
        main,
    };
    assert_eq!(
        decode_callable_body_key(&root.canonical_bytes()),
        Ok(root.clone())
    );

    let init = CallableBodyKey::InitializationStartupGateway {
        unit: crate::persistent::PersistentInitializationUnitId::from_definition_key(
            cone(),
            b"unit",
        ),
    };
    assert_eq!(decode_callable_body_key(&init.canonical_bytes()), Ok(init));

    // Unknown tag 0 is rejected, as are truncated payloads.
    assert!(decode_callable_body_key(&[0, 0, 0, 0, 1]).is_err());
    assert!(decode_callable_body_key(&[]).is_err());
    assert!(decode_callable_body_key(&[3, 0, 0, 0, 1]).is_err());

    // Distinct variants hash to distinct ids.
    assert_ne!(
        PersistentCallableBodyId::of(&strong),
        PersistentCallableBodyId::of(&root)
    );
}

#[test]
fn odr_groups_and_members() {
    let origin = PersistentGenericTypeId::from_definition_key(cone(), b"box");
    let int = PersistentExactTypeId::of(&ExactTypeKey::Nominal {
        declaration: PersistentTypeId::from_definition_key(cone(), b"int-decl"),
    });
    let string_type = PersistentExactTypeId::of(&ExactTypeKey::Nominal {
        declaration: PersistentTypeId::from_definition_key(cone(), b"string-decl"),
    });

    let box_int = SpecializationKey::Nominal {
        origin,
        arguments: vec![int],
    };
    let box_string = SpecializationKey::Nominal {
        origin,
        arguments: vec![string_type],
    };
    let box_int_group = OdrGroupId::of(&box_int);
    assert_ne!(box_int_group, OdrGroupId::of(&box_string));

    // Members of one group: same group, different roles → different ids.
    let td = OdrMemberId::of(&box_int_group, OdrMemberRole::TypeDescriptor, b"");
    let scan = OdrMemberId::of(&box_int_group, OdrMemberRole::ScanProgram, b"");
    assert_ne!(td.as_bytes(), scan.as_bytes());
    // Owner path participates.
    let td_owner = OdrMemberId::of(&box_int_group, OdrMemberRole::TypeDescriptor, b"owner/a");
    assert_ne!(td.as_bytes(), td_owner.as_bytes());
    // Same construction is deterministic.
    assert_eq!(
        td.as_bytes(),
        OdrMemberId::of(&box_int_group, OdrMemberRole::TypeDescriptor, b"").as_bytes()
    );

    // Callable variants distinguish no-owner/no-arguments markers from
    // empty vectors structurally.
    let callable_origin =
        crate::persistent::PersistentGenericCallableId::from_definition_key(cone(), b"generic-fn");
    let none_key = SpecializationKey::callable(
        callable_origin,
        OwnerApplication::None(crate::persistent::NoOwnerApplication),
        CallableArguments::None(NoCallableArguments),
    );
    let empty_vec_key = SpecializationKey::callable(
        callable_origin,
        OwnerApplication::None(crate::persistent::NoOwnerApplication),
        CallableArguments::Exact(vec![]),
    );
    assert_ne!(none_key.canonical_cbor(), empty_vec_key.canonical_cbor());

    // Structural-type group is its own variant.
    let structural = SpecializationKey::StructuralType { exact_type: int };
    assert_ne!(OdrGroupId::of(&structural), box_int_group);
}
