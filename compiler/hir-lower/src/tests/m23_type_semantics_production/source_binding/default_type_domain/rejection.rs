use super::*;

#[test]
fn default_type_domains_reject_wrong_arity_and_provider_binder_scope() {
    with_local(|core, fixture, table, required, templates| {
        let foundation = fixture.bind().unwrap();
        let declarations = foundation
            .bind_default_access_declarations(table, required, &mut meter())
            .unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        let domains = Domains::new(
            &declarations,
            &[],
            inputs.protocols().fundamental_types(),
            &mut meter(),
        )
        .unwrap();
        let applied = templates
            .iter()
            .find(|(name, _)| *name == "applied")
            .unwrap()
            .1
            .result();
        let Type::NominalApplication { origin, arguments } = applied else {
            panic!("real generic application")
        };
        let wrong = Type::NominalApplication {
            origin: *origin,
            arguments: scoop_identity::NonEmptyVec::from_first(
                arguments.as_slice()[0].clone(),
                [arguments.as_slice()[0].clone()],
            ),
        };
        assert!(matches!(
            domains.type_source_domain(&wrong, &scope("applied"), &mut meter()),
            Err(Error::Arity {
                expected: 1,
                actual: 2,
                ..
            })
        ));
        for binder in [
            Type::Binder { depth: 1, index: 0 },
            Type::Binder { depth: 0, index: 1 },
        ] {
            assert!(matches!(
                domains.type_source_domain(&binder, &scope("binder"), &mut meter()),
                Err(Error::Binder(_))
            ));
        }
        let combined = templates
            .iter()
            .find(|(name, _)| *name == "Enclosing.combined")
            .unwrap()
            .1
            .result();
        assert!(matches!(
            domains.type_source_domain(combined, &scope("binder"), &mut meter()),
            Err(Error::Binder(_))
        ));
    });
}

#[test]
fn default_type_domains_require_declaration_sources_even_for_known_type_identities() {
    with_local(|core, fixture, _, _, templates| {
        let foundation = fixture.bind().unwrap();
        let table = Table::try_new(vec![], &mut meter()).unwrap();
        let declarations = foundation
            .bind_default_access_declarations(&table, &BTreeSet::new(), &mut meter())
            .unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        let domains = Domains::new(
            &declarations,
            &[],
            inputs.protocols().fundamental_types(),
            &mut meter(),
        )
        .unwrap();
        for ty in [
            templates[0].1.result(),
            templates
                .iter()
                .find(|(name, _)| *name == "visible")
                .unwrap()
                .1
                .result(),
        ] {
            assert!(
                matches!(domains.type_source_domain(ty, &scope("nominal"), &mut meter()), Err(Error::Access(error)) if matches!(*error, hir::DefaultSourceAccessBindingError::MissingKey(_)))
            );
        }
        for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
            let ty = Type::Nominal(builtin.identity_record().id());
            assert!(
                domains
                    .type_source_domain(&ty, &scope("builtin"), &mut meter())
                    .unwrap()
                    .is_universal()
            );
        }
        let unit = Type::Nominal(inputs.protocols().fundamental_types().unit().persistent());
        for ty in [
            Type::RawPointer(Box::new(unit.clone())),
            Type::NativeFunctionPointer {
                calling_convention: scoop_identity::CallingConvention::C,
                parameters: vec![],
                result: Box::new(unit),
            },
        ] {
            assert!(matches!(
                domains.type_source_domain(&ty, &scope("builtin"), &mut meter()),
                Err(Error::MissingProvider(scoop_identity::ConeIdentity::CORE))
            ));
        }
    });
}
