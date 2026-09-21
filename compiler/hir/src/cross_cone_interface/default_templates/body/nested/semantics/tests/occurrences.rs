use super::*;
use DefaultNestedCallableSiteV1 as Site;

mod local_functions;

struct Occurrences {
    identity: DefaultNestedCallableIdentityV1,
    shape: DefaultNestedCallableIdentityShapeV1,
    abis: Vec<DefaultNestedCallableAbiShapeV1>,
    collapse: bool,
    observed: Vec<(Site, DefaultNestedCallableAuthorityQueryV1)>,
}
impl DefaultNestedCallableSemanticAuthority<AuthorityError> for Occurrences {
    fn default_nested_callable_identity_shape(
        &mut self,
        _: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        site: Site,
    ) -> Result<DefaultNestedCallableIdentityShapeV1, AuthorityError> {
        if identity != self.identity {
            return Err(AuthorityError);
        }
        self.observed
            .push((site, DefaultNestedCallableAuthorityQueryV1::Identity));
        Ok(self.shape.clone())
    }
    fn default_nested_callable_abi_shape(
        &mut self,
        _: &ExportDefaultTemplateV1,
        identity: DefaultNestedCallableIdentityV1,
        site: Site,
        arguments: DefaultNestedCallableBodyArgumentsV1<'_>,
    ) -> Result<DefaultNestedCallableAbiShapeV1, AuthorityError> {
        if identity != self.identity
            || !matches!(arguments, DefaultNestedCallableBodyArgumentsV1::Absent)
        {
            return Err(AuthorityError);
        }
        self.observed
            .push((site, DefaultNestedCallableAuthorityQueryV1::Abi));
        let index = match site {
            Site::Standalone => 0,
            Site::Body { ordinal } => usize::try_from(ordinal).map_err(|_| AuthorityError)?,
        };
        self.abis
            .get(if self.collapse { 0 } else { index })
            .cloned()
            .ok_or(AuthorityError)
    }
}

fn reference(
    f: &Fixture,
    path: &StructuralDefinitionPath,
    ty: SignatureTypeKey,
) -> DefaultCallableReferenceV1 {
    DefaultCallableReferenceV1::try_new(
        callable_reference_invoke(f.function, path.clone()),
        path.clone(),
        DefaultCallableReferenceTargetV1::Named(callable(f.function)),
        ty,
        Vec::new(),
        1,
    )
    .unwrap()
}
fn authority(reference: &DefaultCallableReferenceV1, types: Vec<SignatureTypeKey>) -> Occurrences {
    Occurrences {
        identity: DefaultNestedCallableIdentityV1::CallableReference(reference.invoke()),
        shape: identity_shape(
            DefaultNestedCallableProvenanceV1::DefaultDependency,
            reference.definition_path().clone(),
            1,
            DefaultNestedCallableBodyShapeV1::Absent,
        ),
        abis: types
            .into_iter()
            .map(|ty| abi_shape(ty, Vec::new()))
            .collect(),
        collapse: false,
        observed: Vec::new(),
    }
}

#[test]
fn repeated_identity_uses_each_occurrences_abi_and_rejects_identity_only_lookup() {
    let f = Fixture::new();
    let path = child_path(
        template(&f).definition_path(),
        StructuralDefinitionSiteRole::CallableConversion,
        0,
    );
    let first_type = function(binder(0));
    let second_type = function(binder(1));
    let first = reference(&f, &path, first_type.clone());
    let second = reference(&f, &path, second_type.clone());
    let template = template_with_body(
        &f,
        vec![expression_statement(
            &f,
            DefaultExpressionKindV1::CallableReference(first.clone()),
            first_type.clone(),
        )],
        expression(
            &f,
            DefaultExpressionKindV1::CallableReference(second),
            second_type.clone(),
        ),
    );
    for collapse in [false, true] {
        let mut authority = authority(&first, vec![first_type.clone(), second_type.clone()]);
        authority.collapse = collapse;
        let result = validate_body(&template, &mut authority);
        if collapse {
            assert!(
                matches!(result, Err(DefaultNestedCallableAbiValidationError::FunctionType { expected, actual, .. }) if *expected == first_type && *actual == second_type)
            );
        } else {
            result.unwrap();
        }
        assert_eq!(
            authority.observed,
            (0..2)
                .flat_map(|ordinal| [
                    (
                        Site::Body { ordinal },
                        DefaultNestedCallableAuthorityQueryV1::Identity
                    ),
                    (
                        Site::Body { ordinal },
                        DefaultNestedCallableAuthorityQueryV1::Abi
                    ),
                ])
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn standalone_validation_is_distinct_from_body_ordinal_zero_and_each_body_restarts() {
    let f = Fixture::new();
    let path = child_path(
        template(&f).definition_path(),
        StructuralDefinitionSiteRole::CallableConversion,
        0,
    );
    let ty = function(binder(0));
    let reference = reference(&f, &path, ty.clone());
    let template = template_with_body(
        &f,
        Vec::new(),
        expression(
            &f,
            DefaultExpressionKindV1::CallableReference(reference.clone()),
            ty.clone(),
        ),
    );
    let mut authority = authority(&reference, vec![ty]);
    reference
        .validate_nested_callable_abi_semantics(
            &template,
            &mut authority,
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .unwrap();
    validate_body(&template, &mut authority).unwrap();
    validate_body(&template, &mut authority).unwrap();
    assert_eq!(
        authority
            .observed
            .iter()
            .map(|(site, _)| *site)
            .collect::<Vec<_>>(),
        vec![
            Site::Standalone,
            Site::Standalone,
            Site::Body { ordinal: 0 },
            Site::Body { ordinal: 0 },
            Site::Body { ordinal: 0 },
            Site::Body { ordinal: 0 }
        ]
    );
}

// Raw transport fixtures exercise traversal only; the empty reference envelope
// deliberately does not claim source closure or semantic authority.
pub(super) fn assert_source_index_order(
    template: &ExportDefaultTemplateV1,
    authority: &AuthoritySet,
) {
    let source = crate::DefaultSourceTemplateV1::try_new(
        crate::ProtectedDefaultTemplateKeyV1::try_new(
            template.key().owner(),
            template.key().parameter_position(),
        )
        .unwrap(),
        template.definition_root(),
        template.definition_path().clone(),
        template.locals().clone(),
        template.body().clone(),
        template.result().clone(),
        template.allows_suspend(),
        template.type_parameters().clone(),
        template.receiver().clone(),
        template.value_parameters().clone(),
        crate::DefaultSourceReferencesV1::try_new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .unwrap(),
        template.definition_origin().clone(),
        &mut BudgetMeter::new(DecodeLimits::default()),
    )
    .unwrap();
    let index = source
        .index_nested_callables(
            &mut BudgetMeter::new(DecodeLimits::default()),
            &WirePath::root(),
        )
        .unwrap();
    let queried = authority
        .sites
        .iter()
        .zip(&authority.observed)
        .filter_map(|(site, (identity, query))| {
            (*query == DefaultNestedCallableAuthorityQueryV1::Identity)
                .then_some((*site, *identity))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        index
            .occurrences()
            .iter()
            .map(|o| (o.site(), o.descriptor().identity()))
            .collect::<Vec<_>>(),
        queried,
    );
}
