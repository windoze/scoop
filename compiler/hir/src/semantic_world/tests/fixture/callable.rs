use scoop_identity::{
    BindableEntity, BindingTarget, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord,
    ConeCoordinate, DeclarationScope, DefinitionOrigin, DefinitionOriginRecord,
    DefinitionOriginSubject, DefinitionOwnerChain, Effect, ExportBindingKey, GcEffect,
    NormalizedSourcePath, PackagePath, PersistentFunctionId, SignatureTypeKey, SourceContextKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceIdentity, SourceSpan,
};

use super::*;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableInterfaceRecordV1, CallableModalityV1,
    CallableOperatorRoleV1, CallableSafetyV1, CallableSourceEffectsV1, CallableSourceInterfaceV1,
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalSourceParameterShapesV1, PublicDeclarationOwnerV1, PublicLookupAccessV1,
};

pub(crate) struct CallableProviderFixture {
    pub(crate) coordinate: ConeCoordinate,
    pub(crate) foundation: CanonicalHirFoundation,
    pub(crate) interface: CrossConeHirInterfaceSectionV1,
    pub(crate) function: PersistentFunctionId,
}

impl CallableProviderFixture {
    pub(crate) fn new(
        coordinate: ConeCoordinate,
        package: PackagePath,
        name: &str,
        result: SignatureTypeKey,
    ) -> Self {
        let origin = coordinate.identity().unwrap();
        let key = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                origin,
                package.clone(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function = CborIdentityRecord::from_key(key.clone()).unwrap();
        let binding = CborIdentityRecord::from_key(ExportBindingKey::new(
            origin,
            package,
            CanonicalIdentifier::new(name).unwrap(),
            BindingTarget::function(&key).unwrap(),
        ))
        .unwrap();
        let declaration = CallableTemplateOrigin::Function(function.id());
        let callable = CallableInterfaceRecordV1::try_new(
            declaration,
            PublicDeclarationOwnerV1::TopLevel,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            None,
            CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
            result,
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
            crate::CanonicalPersistentIdsV1::empty(),
            Vec::new(),
        )
        .unwrap();
        let source = CallableSourceInterfaceV1::try_new(
            declaration,
            crate::CanonicalCallableSourceParametersV1::try_new(Vec::new()).unwrap(),
        )
        .unwrap();
        let public = PublicExportBindingRecordV1::new(
            binding.id(),
            ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::Function(function.id()),
            },
        );
        let mut foundation = CanonicalHirFoundation::empty();
        let identity = SourceIdentity::new(
            origin,
            NormalizedSourcePath::new("declaration.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::File {
            source: identity.clone(),
        };
        let end = name.len() as u64;
        foundation
            .set_sources(vec![
                crate::SourceRecord::from_utf8(identity.clone(), name, [0, end]).unwrap(),
            ])
            .unwrap();
        foundation
            .set_source_contexts(vec![CborIdentityRecord::from_key(context.clone()).unwrap()])
            .unwrap();
        foundation
            .set_definition_origins(vec![DefinitionOriginRecord::new(
                DefinitionOriginSubject::Function(function.id()),
                DefinitionOrigin::new(identity, SourceSpan::new(0, end).unwrap(), &context)
                    .unwrap(),
            )])
            .unwrap();
        foundation.set_functions(vec![function]).unwrap();
        foundation
            .set_export_bindings(vec![binding.clone()])
            .unwrap();
        let interface = CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(vec![public]).unwrap(),
            CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalCallableInterfacesV1::try_new(vec![callable]).unwrap(),
            CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
            CanonicalCallableSourceInterfacesV1::try_new(vec![source]).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
            CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        Self {
            coordinate,
            foundation,
            interface,
            function: declaration_function(declaration),
        }
    }

    pub(crate) fn identity(&self) -> ConeIdentity {
        self.coordinate.identity().unwrap()
    }
}

fn declaration_function(declaration: CallableTemplateOrigin) -> PersistentFunctionId {
    match declaration {
        CallableTemplateOrigin::Function(function) => function,
        _ => unreachable!("the callable fixture always creates an ordinary function"),
    }
}
