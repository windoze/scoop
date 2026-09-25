use super::*;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, PackagePath,
    PersistentGenericFunctionId, SourceDeclarationKey, SourceDeclarationSite,
};

pub(super) fn local(fixture: &Fixture) -> (CallableTemplateOrigin, Authority) {
    let key = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("local").unwrap(),
        2,
        None,
        Vec::new(),
    );
    let id = CallableTemplateOrigin::GenericFunction(
        PersistentGenericFunctionId::from_source_declaration(&key).unwrap(),
    );
    let mut authority = Authority::new(fixture);
    authority.local_keys.insert(id, key);
    (id, authority)
}
pub(super) fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}
pub(super) fn signature(parameter: SignatureTypeKey, result: SignatureTypeKey) -> SignatureTypeKey {
    SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![parameter, binder(1, 0)],
        result: Box::new(result),
    }
}

pub(super) fn validate_scope(
    fixture: &Fixture,
    candidate: &ExportDefaultTemplateV1,
    authority: &mut Authority,
) -> Result<(), ExportDefaultReferenceSetSemanticValidationError<AuthorityError>> {
    candidate.validate_reference_envelope_semantics(
        &owner_interface(fixture, PublicLookupAccessV1::DirectOnly),
        DefaultTemplateProviderShapeV1::try_new(1, 1).unwrap(),
        authority,
        &WirePath::root(),
    )
}
pub(super) fn candidate(
    fixture: &Fixture,
    local: CallableTemplateOrigin,
    signature: SignatureTypeKey,
    target: SignatureTypeKey,
    ordinary_use: bool,
) -> ExportDefaultTemplateV1 {
    let base = template(
        fixture,
        reference_set(
            Vec::new(),
            Vec::new(),
            vec![reference(
                target.clone(),
                fixture,
                ExportDefaultCallDomainV1::DirectPublic,
            )],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    );
    let function = DefaultLocalFunctionV1::try_new(
        local,
        base.definition_path().clone(),
        signature,
        Vec::new(),
        99,
    )
    .unwrap();
    let mut statements = vec![
        DefaultStatementV1::try_new(
            DefaultStatementKindV1::LocalFunction(function),
            fixture.origin(),
        )
        .unwrap(),
    ];
    if ordinary_use {
        statements.push(
            DefaultStatementV1::try_new(
                DefaultStatementKindV1::Expr(Box::new(
                    DefaultExpressionV1::try_new(
                        DefaultExpressionKindV1::UnitLiteral,
                        target,
                        fixture.origin(),
                    )
                    .unwrap(),
                )),
                fixture.origin(),
            )
            .unwrap(),
        );
    }
    let body = ExportDefaultBodyV1::try_new(statements, base.body().value().clone()).unwrap();
    ExportDefaultTemplateV1::try_new(
        base.key(),
        base.definition_root(),
        base.definition_path().clone(),
        base.locals().clone(),
        body,
        base.result().clone(),
        base.allows_suspend(),
        base.type_parameters().clone(),
        base.receiver().clone(),
        base.value_parameters().clone(),
        base.references().clone(),
        base.definition_origin().clone(),
    )
    .unwrap()
}
