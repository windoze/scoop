use scoop_wire::{BudgetMeter, DecodeLimits, decode_canonical, encode};

use super::super::{ExactCallableSignature, OptionalExactOwner};
use super::{DecodedExactCallableSignature, MeteredExactCallableSignatureResolutionError};
use crate::{DecodedPersistentId, Effect, PersistentExactTypeId, PersistentIdResolver};

struct Resolver {
    calls: usize,
}
impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
    type Error = &'static str;
    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.calls += 1;
        id.verify(PersistentExactTypeId([7; 32]))
            .map_err(|_| "unknown exact")
    }
}

#[test]
fn metered_exact_signature_charges_before_resolving_or_allocating_parameters() {
    let exact = PersistentExactTypeId([7; 32]);
    let signature = ExactCallableSignature::new(Effect::Ordinary, Some(exact), vec![exact], exact);
    let decoded = decode_canonical::<DecodedExactCallableSignature>(
        &encode(&signature).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    let mut resolver = Resolver { calls: 0 };
    let mut limited = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        decoded.clone().resolve_metered(&mut resolver, &mut limited),
        Err(MeteredExactCallableSignatureResolutionError::Resource(_))
    ));
    assert_eq!(resolver.calls, 0);
    let actual = decoded
        .resolve_metered(
            &mut resolver,
            &mut BudgetMeter::new(DecodeLimits::default()),
        )
        .unwrap();
    assert_eq!(actual, signature);
    assert_eq!(actual.receiver(), OptionalExactOwner::Present(exact));
    assert_eq!(resolver.calls, 3);
}
