use scoop_identity::{
    CallableTemplateOrigin, CanonicalIdentifier, DefinitionOwnerAtom, GcEffect,
    NominalDeclarationOwner, SourceDeclarationKey,
};

use super::*;
use crate::*;

pub(super) struct Fixture {
    pub surface: CoreCompilerProtocolSurfaceV1,
    foundation: CanonicalHirFoundation,
    operations: Vec<(IntrinsicFunctionKind, CoreProtocolCallableV1)>,
}

impl Fixture {
    pub fn new(origin: scoop_identity::ConeIdentity) -> Self {
        let (surface, foundation, operations) = test_support::standalone_with_intrinsics_at(origin);
        Self {
            surface,
            foundation,
            operations,
        }
    }

    pub fn callable(
        &self,
        kind: IntrinsicFunctionKind,
    ) -> (SourceDeclarationKey, CallableInterfaceRecordV1) {
        let (_, callable) = self
            .operations
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .unwrap();
        let (declaration, source) = match callable.definition() {
            CoreProtocolCallableDefinitionV1::Function(id) => (
                CallableTemplateOrigin::Function(id),
                self.foundation.function_by_bytes(id.as_array()).unwrap().1,
            ),
            CoreProtocolCallableDefinitionV1::GenericFunction(id) => (
                CallableTemplateOrigin::GenericFunction(id),
                self.foundation
                    .generic_function_by_bytes(id.as_array())
                    .unwrap()
                    .1,
            ),
            _ => panic!("source intrinsic callable"),
        };
        let owner = match source.owners().owners() {
            [] => PublicDeclarationOwnerV1::TopLevel,
            [DefinitionOwnerAtom::Type(id)] => {
                PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::Concrete(*id))
            }
            [DefinitionOwnerAtom::GenericType(id)] => {
                PublicDeclarationOwnerV1::Nominal(NominalDeclarationOwner::GenericTemplate(*id))
            }
            _ => panic!("top-level or direct nominal member"),
        };
        let record = CallableInterfaceRecordV1::try_new(
            declaration,
            owner,
            binders(source.duplicate_signature().type_parameter_count()),
            None,
            parameters(callable.signature().parameters()),
            callable.signature().result().clone(),
            effects(kind, callable.signature().effect()),
            CallableModalityV1::Final,
            PublicLookupAccessV1::DirectOnly,
            crate::CanonicalPersistentIdsV1::empty(),
        )
        .unwrap();
        (source.clone(), record)
    }
}

pub(super) fn replace_signature(
    record: &CallableInterfaceRecordV1,
    signature: SignatureCallableShape,
    kind: IntrinsicFunctionKind,
) -> CallableInterfaceRecordV1 {
    CallableInterfaceRecordV1::try_new(
        record.declaration(),
        if signature.receiver().is_present() {
            crate::PublicDeclarationOwnerV1::Extension
        } else {
            record.owner()
        },
        record.type_parameters().clone(),
        match signature.receiver() {
            scoop_identity::OptionalSignatureType::Absent => None,
            scoop_identity::OptionalSignatureType::Present(value) => Some(value.as_ref().clone()),
        },
        parameters(signature.parameters()),
        signature.result().clone(),
        effects(kind, signature.effect()),
        record.modality(),
        record.access(),
        crate::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap()
}

pub(super) fn binders(count: u32) -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(
        (0..count)
            .map(|i| {
                TypeParameterBinderV1::new(
                    CanonicalIdentifier::new(&format!("T{i}")).unwrap(),
                    TypeParameterBoundsV1::Unconstrained,
                )
            })
            .collect(),
    )
    .unwrap()
}

fn parameters(types: &[SignatureTypeKey]) -> CanonicalSourceParameterShapesV1 {
    CanonicalSourceParameterShapesV1::try_new(
        types
            .iter()
            .enumerate()
            .map(|(i, ty)| {
                SourceParameterShapeV1::new(
                    CanonicalIdentifier::new(&format!("p{i}")).unwrap(),
                    ty.clone(),
                )
            })
            .collect(),
    )
    .unwrap()
}

fn effects(kind: IntrinsicFunctionKind, execution: Effect) -> CallableSourceEffectsV1 {
    let gc = match kind.integer_gc_effect() {
        Some(crate::GcEffect::NoGc) => GcEffect::NoGc,
        Some(crate::GcEffect::Managed) | None => GcEffect::Managed,
    };
    CallableSourceEffectsV1::try_new(
        execution,
        CallableSafetyV1::Safe,
        gc,
        CallableImplementationV1::Intrinsic(kind),
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}
