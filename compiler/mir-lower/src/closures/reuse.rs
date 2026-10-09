use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ClosureDefinition {
    Body(hir::FunctionId),
    PrimitiveReference(scoop_hir::PrimitiveMemberIntrinsic),
    MaybeUninitReference(hir::MaybeUninitIntrinsic),
    Reference {
        callee: mir::Callee,
        kind: mir::CallKind,
    },
}

#[derive(Default)]
pub(super) struct ClosureDefinitions {
    by_identity: HashMap<
        scoop_identity::PersistentTypeId,
        (
            mir::ClosureClassId,
            mir::ClosureEnvironmentIdentity,
            ClosureDefinition,
        ),
    >,
}

impl ClosureDefinitions {
    pub(super) fn lookup(
        &self,
        classes: &Arena<mir::ClosureClass>,
        identity: &mir::ClosureEnvironmentIdentity,
        function_type: mir::FunctionTypeId,
        captures: &[mir::Field],
        definition: &ClosureDefinition,
    ) -> Option<mir::ClosureClassId> {
        let (class, existing, existing_definition) = self
            .by_identity
            .get(&identity.generated_type_record().id())?;
        assert_eq!(
            existing, identity,
            "one source closure has one complete identity contract"
        );
        assert_eq!(
            existing_definition, definition,
            "one source closure has one invoke target"
        );
        let existing = &classes[*class];
        assert_eq!(
            existing.function_type, function_type,
            "one source closure has one function signature"
        );
        assert!(
            existing
                .captures
                .iter()
                .map(|field| (&field.name, &field.ty))
                .eq(captures.iter().map(|field| (&field.name, &field.ty))),
            "one source closure has one physical capture signature"
        );
        Some(*class)
    }

    pub(super) fn insert(
        &mut self,
        class: mir::ClosureClassId,
        identity: mir::ClosureEnvironmentIdentity,
        definition: ClosureDefinition,
    ) {
        assert!(
            self.by_identity
                .insert(
                    identity.generated_type_record().id(),
                    (class, identity, definition)
                )
                .is_none()
        );
    }
}

impl Lowerer {
    pub(super) fn index_closure_captures(
        &mut self,
        class: mir::ClosureClassId,
        identity: &mir::ClosureEnvironmentIdentity,
        captures: &[hir::Capture],
    ) {
        for (index, capture) in captures.iter().enumerate() {
            let physical = identity
                .physical_index(capture_source(index))
                .expect("every closure capture has one physical field");
            if let Some(previous) = self
                .closure_capture_indices
                .insert((class, capture.binding), physical)
            {
                assert_eq!(
                    previous, physical,
                    "one captured binding has one physical field"
                );
            }
        }
    }
}
