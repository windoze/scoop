use scoop_wire::{decode_canonical, encode};

use super::{DecodedDependencyCallableDeclarationId, DependencyCallableDeclarationId};
use crate::{
    DecodedPersistentId, PersistentFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
    StrongCallableDefinitionOwner,
};

#[test]
fn dependency_callable_declaration_has_a_fixed_closed_wire_sum() {
    let function = PersistentFunctionId([0x11; 32]);
    let accessor = PersistentPropertyAccessorId([0x22; 32]);

    let function_bytes = encode(&DependencyCallableDeclarationId::Function(function)).unwrap();
    let accessor_bytes =
        encode(&DependencyCallableDeclarationId::PropertyAccessor(accessor)).unwrap();

    assert_eq!(
        function_bytes,
        [vec![0xa2, 0x00, 0x01, 0x01, 0x58, 0x20], vec![0x11; 32]].concat()
    );
    assert_eq!(
        accessor_bytes,
        [vec![0xa2, 0x00, 0x02, 0x01, 0x58, 0x20], vec![0x22; 32]].concat()
    );
}

#[test]
fn decoded_dependency_callable_requires_typed_resolution() {
    let function = PersistentFunctionId([0x33; 32]);
    let bytes = encode(&DependencyCallableDeclarationId::Function(function)).unwrap();
    let decoded = decode_canonical::<DecodedDependencyCallableDeclarationId>(&bytes).unwrap();
    let mut resolver = ExactResolver { function };

    let resolved = decoded.resolve(&mut resolver).unwrap();

    assert_eq!(
        resolved,
        DependencyCallableDeclarationId::Function(function)
    );
    assert_eq!(
        resolved.implementation(),
        StrongCallableDefinitionOwner::Function(function)
    );
}

struct ExactResolver {
    function: PersistentFunctionId,
}

impl PersistentIdResolver<PersistentFunctionId> for ExactResolver {
    type Error = ();

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentFunctionId>,
    ) -> Result<PersistentFunctionId, Self::Error> {
        (decoded.as_array() == self.function.as_array())
            .then_some(self.function)
            .ok_or(())
    }
}

impl PersistentIdResolver<PersistentPropertyAccessorId> for ExactResolver {
    type Error = ();

    fn resolve(
        &mut self,
        _decoded: DecodedPersistentId<PersistentPropertyAccessorId>,
    ) -> Result<PersistentPropertyAccessorId, Self::Error> {
        Err(())
    }
}
