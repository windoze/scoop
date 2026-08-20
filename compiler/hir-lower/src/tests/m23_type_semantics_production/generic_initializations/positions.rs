use super::*;
use hir::{
    DeclarationTypeSiteValidationError as Error, HirCallableTypePositionV1 as Part,
    HirDependencyTypePositionV1 as Position,
};
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
};

#[test]
fn generic_constructor_and_accessor_sites_use_their_actual_applications() {
    with_hir_source(STANDALONE, |output, core| {
        let interface = public_projection::public_interface_with_core(output, core);
        let mut foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        foundation
            .complete_cross_cone_interface_source_points(
                output.output().export.module(),
                &interface,
            )
            .unwrap();
        let local = output.output().local.module();
        let validate = |root, part| {
            foundation.validate_declaration_type_position(
                local.cone,
                Position::CallableSignature(root, part),
                &[&core.source_foundation],
            )
        };
        for site in interface
            .external_references()
            .records()
            .iter()
            .flat_map(|reference| reference.type_sites().records())
            .filter(|site| site.as_expression().is_none())
        {
            foundation
                .validate_declaration_type_position(
                    local.cone,
                    site.position(),
                    &[&core.source_foundation],
                )
                .unwrap();
        }
        let constructors = local
            .class_constructors
            .iter()
            .filter(|(_, constructor)| {
                matches!(
                    constructor.materialization.context(),
                    CallableMaterializationContext::Application(_)
                )
            })
            .map(|(_, constructor)| constructor.materialization)
            .collect::<Vec<_>>();
        assert!(constructors.len() >= 2);
        for root in &constructors {
            validate(*root, Part::Result).unwrap();
            for part in [Part::Receiver, Part::Parameter(u32::MAX)] {
                assert!(matches!(
                    validate(*root, part),
                    Err(Error::SignaturePosition(..))
                ));
            }
            foundation
                .validate_declaration_type_position(
                    local.cone,
                    Position::ConstructorInitializerResult(*root),
                    &[],
                )
                .unwrap();
        }
        let wrong =
            CallableMaterialization::new(constructors[0].template(), constructors[1].context());
        assert!(matches!(
            validate(wrong, Part::Result),
            Err(Error::Materialization(_))
        ));
        let mut missing = foundation.clone();
        missing.set_constructors(Vec::new()).unwrap();
        assert!(matches!(
            missing.validate_declaration_type_position(
                local.cone,
                Position::CallableSignature(constructors[0], Part::Result),
                &[],
            ),
            Err(Error::MissingIdentity { .. })
        ));

        let mut accessors = 0;
        for (_, function) in local.functions.iter() {
            let root = function.materialization;
            if matches!(root.template(), CallableTemplateOwner::Accessor(_))
                && matches!(
                    root.context(),
                    CallableMaterializationContext::Application(_)
                )
            {
                accessors += 1;
                validate(root, Part::Receiver).unwrap();
                validate(root, Part::Result).unwrap();
                assert!(matches!(
                    validate(root, Part::Parameter(u32::MAX)),
                    Err(Error::SignaturePosition(..))
                ));
            }
        }
        assert!(accessors > 0);
        let structure = local.struct_constructors.iter().next().unwrap().1;
        assert!(matches!(
            foundation.validate_declaration_type_position(
                local.cone,
                Position::ConstructorInitializerResult(structure.materialization),
                &[],
            ),
            Err(Error::StoragePosition(_))
        ));
    });
}
