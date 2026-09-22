use super::*;
use hir::{DefaultConstructorRefV1 as Constructor, DefaultFieldRefV1 as Field};
use scoop_identity::SignatureTypeKey;

#[test]
fn value_domain_routes_reject_claimed_owner_types_and_member_properties_as_globals() {
    for source in [SOURCE, COMBINATIONS] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, _, foundation| {
                let wrong = SignatureTypeKey::Nominal(inputs.core_types.unit().persistent());
                let mut owners = 0;
                let mut globals = 0;
                for template in inputs.templates.records() {
                    for reference in template.references().constructors() {
                        let target = match reference.target() {
                            Constructor::Struct { declaration, .. } => Constructor::Struct { declaration: *declaration, owner_type: wrong.clone() },
                            Constructor::Class { declaration, .. } => Constructor::Class { declaration: *declaration, owner_type: wrong.clone() },
                            Constructor::Variant { declaration, .. } => Constructor::Variant { declaration: *declaration, owner_type: wrong.clone() },
                        };
                        rejected_owner(domains.value_source_domain(Target::Constructor(&target), &mut meter()).unwrap_err());
                        owners += 1;
                    }
                    for reference in template.references().fields() {
                        let target = match reference.target() {
                            Field::Struct { declaration, .. } => Field::Struct { declaration: *declaration, owner_type: wrong.clone() },
                            Field::Class { declaration, .. } => Field::Class { declaration: *declaration, owner_type: wrong.clone() },
                            Field::Tuple { .. } => continue,
                        };
                        rejected_owner(domains.value_source_domain(Target::Field(&target), &mut meter()).unwrap_err());
                        owners += 1;
                        let subject = foundation.default_field_access_subject(reference.target(), &mut meter()).unwrap();
                        if let hir::DefaultSourceFieldAccessSubjectV1::Declaration(scoop_identity::DefinitionOriginSubject::Property(property)) = subject {
                            let error = domains.value_source_domain(Target::Global(property), &mut meter()).unwrap_err();
                            assert!(matches!(error, hir::DefaultSourceDomainError::Target(error) if matches!(*error, hir::DefaultSourceTargetSubjectError::GlobalScope(id) if id == property)));
                            globals += 1;
                        }
                    }
                }
                assert!(owners >= 6 && globals >= 2);
            });
        });
    }
}

fn rejected_owner(error: hir::DefaultSourceDomainError) {
    assert!(
        matches!(error, hir::DefaultSourceDomainError::Target(error) if matches!(*error, hir::DefaultSourceTargetSubjectError::AppliedOwner(_)))
    );
}
