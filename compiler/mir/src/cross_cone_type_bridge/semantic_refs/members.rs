use super::collector::Collector;
use super::*;
use scoop_identity::{
    DefinitionOwnerAtom, DuplicateSignatureKey, OptionalSignatureType, PropertyAccessorKey,
    PropertyOwner, SourceDeclarationKey,
};

impl Collector<'_> {
    /// Member dependencies retain their source or generated callable identity.
    pub fn member_target(
        &mut self,
        target: CallableDefinitionOwner,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        let source = match target {
            CallableDefinitionOwner::Odr(_) => {
                return self.push(MirTypeBridgeTargetV1::Callable(target));
            }
            CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::GeneratedCallable(
                id,
            )) => {
                let key = self.graph.canonical_key::<_, GeneratedCallableKey>(id)?;
                if !matches!(key.as_ref(), GeneratedCallableKey::DerivedEquality { .. }) {
                    return Err(MirTypeBridgeReferenceError::NonMemberCallableTarget(target));
                }
                return self.push(MirTypeBridgeTargetV1::Callable(target));
            }
            CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::Function(id)) => {
                self.graph.canonical_key::<_, SourceDeclarationKey>(id)?
            }
            CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::PropertyAccessor(
                id,
            )) => {
                let accessor = self.graph.canonical_key::<_, PropertyAccessorKey>(id)?;
                let PropertyOwner::Property(property) = accessor.owner() else {
                    return Err(MirTypeBridgeReferenceError::NonMemberCallableTarget(target));
                };
                self.graph
                    .canonical_key::<_, SourceDeclarationKey>(property)?
            }
            _ => return Err(MirTypeBridgeReferenceError::NonMemberCallableTarget(target)),
        };
        let ordinary = matches!(
            source.duplicate_signature(),
            DuplicateSignatureKey::Function {
                type_parameter_count: 0,
                receiver: OptionalSignatureType::Absent,
                ..
            } | DuplicateSignatureKey::Property {
                type_parameter_count: 0,
                receiver: OptionalSignatureType::Absent
            }
        );
        if !ordinary
            || !matches!(
                source.owners().owners().last(),
                Some(DefinitionOwnerAtom::Type(_))
            )
        {
            return Err(MirTypeBridgeReferenceError::NonMemberCallableTarget(target));
        }
        self.push(MirTypeBridgeTargetV1::Callable(target))
    }
    pub fn dispatch_target(
        &mut self,
        target: CallableDefinitionOwner,
    ) -> Result<(), MirTypeBridgeReferenceError> {
        if let CallableDefinitionOwner::Strong(StrongCallableDefinitionOwner::GeneratedCallable(
            id,
        )) = target
        {
            let key = self.graph.canonical_key::<_, GeneratedCallableKey>(id)?;
            if !matches!(
                key.as_ref(),
                GeneratedCallableKey::DispatchAdjust { .. }
                    | GeneratedCallableKey::BoxingAdjust { .. }
                    | GeneratedCallableKey::DerivedEquality { .. }
            ) {
                return Err(MirTypeBridgeReferenceError::GeneratedExecutionGate);
            }
            self.push(MirTypeBridgeTargetV1::Callable(target))
        } else {
            self.member_target(target)
        }
    }
}
