use scoop_hir::*;
use scoop_identity::Effect;

use super::*;

mod identities;

#[test]
fn ordinary_default_envelope_accepts_a_local_function_own_binder_frame() {
    let fixture = fixture(1, SignatureTypeKey::Binder { depth: 0, index: 0 }, 0);
    validate_until_type_alias(&fixture.artifact())
        .validate_source_interfaces(vec![])
        .unwrap();
}

#[test]
fn ordinary_default_envelope_rejects_extra_local_function_binders_despite_the_descriptor_count() {
    let fixture = fixture(1, SignatureTypeKey::Binder { depth: 0, index: 1 }, 100);
    assert_body_scope(
        &fixture,
        Site::NestedCallableFunction,
        ScopeError::IndexOutOfRange {
            depth: 0,
            index: 1,
            arity: 1,
        },
    );
}

#[test]
fn ordinary_default_envelope_rejects_a_local_function_frame_beyond_its_real_scope() {
    let fixture = fixture(1, SignatureTypeKey::Binder { depth: 1, index: 0 }, 100);
    assert_body_scope(
        &fixture,
        Site::NestedCallableFunction,
        ScopeError::DepthOutOfRange {
            depth: 1,
            available_depths: 1,
        },
    );
}

#[test]
fn ordinary_default_envelope_does_not_invent_a_frame_for_a_nongeneric_local_function() {
    let fixture = fixture(0, SignatureTypeKey::Binder { depth: 0, index: 0 }, 100);
    assert_body_scope(
        &fixture,
        Site::NestedCallableFunction,
        ScopeError::DepthOutOfRange {
            depth: 0,
            available_depths: 0,
        },
    );
}

pub(in super::super) fn fixture(
    arity: u32,
    parameter: SignatureTypeKey,
    descriptor_count: u32,
) -> CallableSourceSurface {
    let mut fixture = default_fixture::fixture(default_fixture::Case::Defined);
    let declaration = identities::add(&mut fixture, arity);
    let template = &fixture.interface.default_templates().records()[0];
    let function_type = SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![parameter.clone()],
        result: Box::new(parameter),
    };
    let local = DefaultLocalFunctionV1::try_new(
        declaration,
        local_path(0, 1),
        function_type.clone(),
        vec![],
        descriptor_count,
    )
    .unwrap();
    let mut statements = template.body().statements().to_vec();
    statements.push(
        DefaultStatementV1::try_new(
            DefaultStatementKindV1::LocalFunction(local),
            template.definition_origin().clone(),
        )
        .unwrap(),
    );
    let locals = template.locals().records().to_vec();
    let references = template.references();
    let origin = template.definition_origin().clone();
    let mut types = references.types().to_vec();
    types.push(ExportDefaultReferenceV1::new(function_type, origin.clone()));
    let references = ExportDefaultReferenceSetV1::try_new(
        vec![ExportDefaultReferenceV1::new(
            ExportDefaultCallableTargetV1::LocalFunction { declaration },
            origin,
        )],
        references.constructors().to_vec(),
        types,
        references.globals().to_vec(),
        references.singleton_values().to_vec(),
        references.fields().to_vec(),
    )
    .unwrap();
    support::replace_contents(&mut fixture, locals, statements);
    default_fixture::replace_references(&mut fixture, references);
    fixture
}
