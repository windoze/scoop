use scoop_identity::{Effect, SignatureTypeKey};
use scoop_wire::{BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, WirePath};

use super::*;

#[test]
fn walks_composite_signatures_in_source_order_and_charges_resources() {
    let signature = SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![binder(0, 0)],
        result: Box::new(SignatureTypeKey::RawPointer(Box::new(binder(0, 0)))),
    };
    let scope = SignatureBinderScopeV1::for_declaration(1, None);
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert_eq!(
        scope.validate_signature_semantics_metered(
            &signature,
            &mut NoNominals,
            &mut meter,
            &WirePath::root(),
        ),
        Ok(())
    );
    let usage = meter.usage();
    assert_eq!(usage.decoded_nodes, 4);
    assert_eq!(usage.decoded_edges, 3);
    assert_eq!(usage.validation_work_units, 7);
}

#[test]
fn preserves_signature_semantic_failures() {
    let signature = binder(0, 1);
    let scope = SignatureBinderScopeV1::for_declaration(1, None);
    let mut meter = BudgetMeter::new(DecodeLimits::default());

    assert_eq!(
        scope.validate_signature_semantics_metered(
            &signature,
            &mut NoNominals,
            &mut meter,
            &WirePath::root(),
        ),
        Err(MeteredSignatureTypeSemanticError::Semantic(
            SignatureTypeSemanticError::BinderScope(
                crate::SignatureBinderScopeError::IndexOutOfRange {
                    depth: 0,
                    index: 1,
                    arity: 1,
                }
            )
        ))
    );
}

#[test]
fn rejects_child_before_exceeding_the_shared_depth_limit() {
    let signature = SignatureTypeKey::RawPointer(Box::new(binder(0, 0)));
    let scope = SignatureBinderScopeV1::for_declaration(1, None);
    let path = WirePath::root().field(5);
    let mut meter = BudgetMeter::new(DecodeLimits {
        semantic_recursion: 1,
        ..DecodeLimits::default()
    });

    let error = scope
        .validate_signature_semantics_metered(&signature, &mut NoNominals, &mut meter, &path)
        .unwrap_err();
    assert!(matches!(
        error,
        MeteredSignatureTypeSemanticError::Resource(ref error)
            if error.kind()
                == &WireErrorKind::LimitExceeded {
                    resource: ResourceKind::SemanticRecursion,
                    limit: 1,
                    observed: 2,
                }
                && error.path() == &path
    ));
    assert_eq!(meter.usage().decoded_nodes, 1);
    assert_eq!(meter.usage().decoded_edges, 0);
    assert_eq!(meter.usage().validation_work_units, 1);
}

struct NoNominals;

impl NominalInterfaceShapeAuthority<()> for NoNominals {
    fn concrete_nominal_shape(
        &mut self,
        _: scoop_identity::PersistentTypeId,
    ) -> Result<crate::PublicNominalShapeV1, ()> {
        Err(())
    }

    fn generic_nominal_shape(
        &mut self,
        _: scoop_identity::PersistentGenericTypeId,
    ) -> Result<crate::PublicNominalShapeV1, ()> {
        Err(())
    }
}

const fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}
