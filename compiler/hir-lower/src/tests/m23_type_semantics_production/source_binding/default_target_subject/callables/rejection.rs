use super::*;
use hir::{
    DefaultCallableDeclarationV1 as Declaration, DefaultCallableRefV1,
    DefaultNestedCallableIdentityV1 as Nested,
};
use scoop_identity::{CallableTemplateOrigin, OptionalSignatureType};
mod missing;
mod scopes;

fn target(output: &hir::DependencyHirOutput, name: &str, position: u32) -> Callable {
    template(output, name, position).references().callables()[0]
        .target()
        .clone()
}
fn direct(declaration: Declaration) -> Callable {
    Callable::Callable(
        DefaultCallableRefV1::try_new(declaration, OptionalSignatureType::Absent, vec![]).unwrap(),
    )
}
#[test]
fn default_callable_roles_cannot_confuse_nested_bodies_or_source_declarations() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        let foundation = fixture.bind().unwrap();
        let Callable::Lambda { body: lambda } = target(output, "lambda", 0) else {
            panic!("lambda target");
        };
        let Callable::AnonymousFunction { body: anonymous } = target(output, "anonymous", 0) else {
            panic!("anonymous target");
        };
        let Callable::CallableReference { invoke } = target(output, "reference", 0) else {
            panic!("reference target");
        };
        let Callable::Callable(plain) = target(output, "direct", 0) else {
            panic!("plain target");
        };
        let Declaration::Function(function) = plain.declaration() else {
            panic!("source function");
        };
        for wrong in [
            Callable::AnonymousFunction { body: lambda },
            Callable::Lambda { body: anonymous },
            Callable::Lambda { body: invoke },
            Callable::CallableReference { invoke: lambda },
            Callable::LocalFunction {
                declaration: CallableTemplateOrigin::Function(function),
            },
        ] {
            assert!(matches!(
                foundation.default_callable_access_subject(&wrong, &mut meter()),
                Err(Error::NestedRole(_))
            ));
        }
        for (id, expected) in [
            (lambda, Nested::Lambda(lambda)),
            (anonymous, Nested::AnonymousFunction(anonymous)),
            (invoke, Nested::CallableReference(invoke)),
        ] {
            assert_eq!(
                foundation
                    .default_callable_access_subject(
                        &direct(Declaration::Generated(id)),
                        &mut meter()
                    )
                    .unwrap(),
                Access::Nested(expected)
            );
        }
        // Function-address routing proves identity only, not address eligibility.
        assert_eq!(
            foundation
                .default_callable_access_subject(
                    &Callable::FunctionAddress {
                        declaration: plain.declaration()
                    },
                    &mut meter()
                )
                .unwrap(),
            Access::Declaration(Subject::Function(function))
        );
        let export = output.output().export.module();
        let generated_equality = export
            .functions
            .iter()
            .find_map(|(id, _)| match &export.function_identities[id] {
                hir::HirFunctionIdentity::DerivedEquality(applications) => applications
                    .first()
                    .map(|application| application.record().id()),
                _ => None,
            })
            .unwrap();
        assert!(
            matches!(foundation.default_callable_access_subject(&direct(Declaration::Generated(generated_equality)), &mut meter()), Err(Error::CallableRole(Declaration::Generated(id))) if id == generated_equality)
        );
    });
}

#[test]
fn default_global_targets_reject_member_properties_and_absent_storage_declarations() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        let foundation = fixture.bind().unwrap();
        let export = output.output().export.module();
        let (_, field) = export.class_fields.iter().next().unwrap();
        let member = export.property_identities[field.property]
            .ordinary_id()
            .unwrap();
        assert!(
            matches!(foundation.default_global_access_subject(member, &mut meter()), Err(Error::GlobalScope(id)) if id == member)
        );
        let source = template(output, "global", 0);
        let id = *source.references().globals()[0].target();
        let mut canonical = fixture.foundation.as_canonical().clone();
        canonical.set_properties(vec![]).unwrap();
        let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let foundation = fixture
            .source
            .bind_to_foundation(&artifact, &fixture.identities, &mut meter())
            .unwrap();
        assert!(
            matches!(foundation.default_global_access_subject(id, &mut meter()), Err(Error::MissingDeclaration(Subject::Property(actual))) if actual == id)
        );
    });
}
