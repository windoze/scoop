use scoop_identity::{Effect, ExactCallableSignature, GcEffect, SourceNominalKind};
use scoop_wire::{BudgetMeter, DecodeLimits};

use super::*;
use crate::*;

mod access;
mod defaults;
mod semantics;
mod support;
mod wire;

use support::{Fixture, effects};
fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn callable_signature_requires_receiver_execution_and_dispatch_effects() {
    let fixture = Fixture::default();
    let exact = fixture.unit.exact;
    let ordinary = effects(GcEffect::Managed, CallableImplementationV1::Scoop);
    assert_eq!(
        InheritanceCallableSignatureV1::try_new(
            ExactCallableSignature::new(Effect::Ordinary, None, vec![], exact),
            ordinary
        ),
        Err(InheritanceCallableSignatureBuildError::MissingReceiver)
    );
    assert_eq!(
        InheritanceCallableSignatureV1::try_new(
            ExactCallableSignature::new(Effect::Suspend, Some(exact), vec![], exact),
            ordinary
        ),
        Err(InheritanceCallableSignatureBuildError::Execution)
    );
    let external = effects(
        GcEffect::Managed,
        CallableImplementationV1::SourceExternScoop,
    );
    assert_eq!(
        InheritanceCallableSignatureV1::try_new(
            ExactCallableSignature::new(Effect::Ordinary, Some(exact), vec![], exact),
            external
        ),
        Err(InheritanceCallableSignatureBuildError::SourceExtern)
    );
}

#[test]
fn override_preserves_effect_contract_but_can_change_body_implementation_category() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Class);
    let slot = fixture.function(owner, "method");
    let root = fixture.signature(owner, vec![]);
    let mut target = fixture.concrete(owner, slot);
    target.signature = InheritanceCallableSignatureV1::try_new(
        root.exact_signature().clone(),
        effects(GcEffect::Managed, CallableImplementationV1::Intrinsic),
    )
    .unwrap();
    fixture.contract(
        owner,
        slot,
        InheritanceSlotImplementationV1::Concrete(target.clone()),
    );
    target.signature = InheritanceCallableSignatureV1::try_new(
        root.exact_signature().clone(),
        effects(GcEffect::NoGc, CallableImplementationV1::Intrinsic),
    )
    .unwrap();
    assert!(matches!(
        InheritanceSlotContractV1::try_new(
            slot,
            support::nominal(owner),
            fixture.declaration(slot),
            root,
            PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
            InheritanceSlotImplementationV1::Concrete(target),
            fixture.access(owner, DeclaredVisibilityV1::Public)
        ),
        Err(InheritanceSlotContractBuildError::SignatureMismatch)
    ));
}
