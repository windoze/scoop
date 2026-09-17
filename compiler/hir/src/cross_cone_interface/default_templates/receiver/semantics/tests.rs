use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOrigin, DefinitionOwnerChain, Effect, GcEffect, LocalValueSelector,
    PackagePath, PersistentFunctionId, SignatureTypeKey, SourceContextKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceIdentity, SourceSpan,
};

use super::*;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableModalityV1, CallableOperatorRoleV1,
    CallableSafetyV1, CallableSourceEffectsV1, CanonicalBinderListV1, CanonicalBooleanV1,
    CanonicalSourceParameterShapesV1, ExportDefinitionSourceV1, PublicDeclarationOwnerV1,
    PublicLookupAccessV1, TemplateLocalDefinitionV1, TemplateLocalRecordV1, TemplateReceiverV1,
};

#[test]
fn receiver_presence_and_local_facts_match_callable_interface() {
    let receiver_type = binder(0);
    let receiver = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, receiver_type.clone()).unwrap(),
    );
    let locals = local_table(receiver_type.clone(), CanonicalBooleanV1::False);

    assert_eq!(
        receiver.validate_semantics(&callable(Some(receiver_type)), &locals),
        Ok(())
    );
    assert_eq!(
        OptionalTemplateReceiverV1::Absent.validate_semantics(
            &callable(None),
            &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        ),
        Ok(())
    );
}

#[test]
fn receiver_presence_must_match_callable_interface() {
    assert_eq!(
        OptionalTemplateReceiverV1::Absent.validate_semantics(
            &callable(Some(binder(0))),
            &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        ),
        Err(TemplateReceiverSemanticValidationError::Missing {
            expected: Box::new(binder(0)),
        })
    );

    let receiver = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, binder(0)).unwrap(),
    );
    assert_eq!(
        receiver.validate_semantics(
            &callable(None),
            &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        ),
        Err(TemplateReceiverSemanticValidationError::Unexpected {
            actual: Box::new(binder(0)),
        })
    );
}

#[test]
fn receiver_requires_an_immutable_local_with_the_same_type() {
    let receiver = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, binder(0)).unwrap(),
    );
    let owner = callable(Some(binder(0)));

    assert_eq!(
        receiver.validate_semantics(
            &owner,
            &CanonicalTemplateLocalTableV1::try_new(Vec::new()).unwrap(),
        ),
        Err(TemplateReceiverSemanticValidationError::MissingLocal(
            LocalValueSelector::This
        ))
    );
    assert_eq!(
        receiver.validate_semantics(&owner, &local_table(binder(0), CanonicalBooleanV1::True),),
        Err(TemplateReceiverSemanticValidationError::MutableLocal)
    );
    assert_eq!(
        receiver.validate_semantics(&owner, &local_table(binder(1), CanonicalBooleanV1::False),),
        Err(TemplateReceiverSemanticValidationError::LocalType {
            expected: Box::new(binder(0)),
            actual: Box::new(binder(1)),
        })
    );
}

#[test]
fn receiver_type_must_match_the_callable_receiver_type() {
    let receiver = OptionalTemplateReceiverV1::Present(
        TemplateReceiverV1::try_new(LocalValueSelector::This, binder(0)).unwrap(),
    );
    assert_eq!(
        receiver.validate_semantics(
            &callable(Some(binder(1))),
            &local_table(binder(0), CanonicalBooleanV1::False),
        ),
        Err(TemplateReceiverSemanticValidationError::CallableType {
            expected: Box::new(binder(1)),
            actual: Box::new(binder(0)),
        })
    );
}

fn local_table(
    value_type: SignatureTypeKey,
    mutable: CanonicalBooleanV1,
) -> CanonicalTemplateLocalTableV1 {
    CanonicalTemplateLocalTableV1::try_new(vec![
        TemplateLocalRecordV1::try_new(
            LocalValueSelector::This,
            value_type,
            mutable,
            TemplateLocalDefinitionV1::Source(origin()),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn callable(receiver: Option<SignatureTypeKey>) -> CallableInterfaceRecordV1 {
    let owner = if receiver.is_some() {
        PublicDeclarationOwnerV1::Extension
    } else {
        PublicDeclarationOwnerV1::TopLevel
    };
    CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Function(function().id()),
        owner,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        receiver,
        CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
        binder(0),
        CallableSourceEffectsV1::try_new(
            Effect::Ordinary,
            CallableSafetyV1::Safe,
            GcEffect::Managed,
            CallableImplementationV1::Scoop,
            CallableOperatorRoleV1::None,
            CallableInfixV1::Ordinary,
        )
        .unwrap(),
        CallableModalityV1::Final,
        PublicLookupAccessV1::DirectOnly,
    )
    .unwrap()
}

fn function() -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("extensionDefault").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn origin() -> ExportDefinitionSourceV1 {
    let source = SourceIdentity::single_file();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(1, 2).unwrap(), &context).unwrap(),
    )
}

const fn binder(index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth: 0, index }
}
