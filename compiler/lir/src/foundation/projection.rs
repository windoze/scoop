use super::*;
use crate::{Function, Module};

impl CanonicalLirFoundation {
    /// Project the identities introduced by final LIR entities into the
    /// arena-independent foundation tables.
    ///
    /// The projection reads the complete identity relations owned by each
    /// function. It never scans instructions to reconstruct callable-body or
    /// safepoint semantics from local arena ids.
    pub fn from_module(module: &Module) -> Result<Self, LirFoundationBuildError> {
        let mut foundation = Self::from_functions(&module.functions)?;
        foundation.set_layouts(
            module
                .meta
                .layouts
                .iter()
                .map(|(_, layout)| layout.identity.identity_record().clone())
                .collect(),
        )?;
        foundation.set_runtime_types(
            module
                .meta
                .type_descriptors
                .iter()
                .map(|(_, descriptor)| descriptor.runtime_type)
                .collect(),
        )?;
        Ok(foundation)
    }

    fn from_functions(functions: &[Function]) -> Result<Self, LirFoundationBuildError> {
        let mut callable_bodies = Vec::with_capacity(functions.len());
        let safepoint_count = functions
            .iter()
            .map(|function| function.safepoints.len())
            .sum();
        let mut safepoint_sites = Vec::with_capacity(safepoint_count);
        let mut safepoints = Vec::with_capacity(safepoint_count);

        for (function_index, function) in functions.iter().enumerate() {
            let callable_body = function.callable_body.id();
            callable_bodies.push(function.callable_body.identity_record().clone());
            for identity in function.safepoints.iter() {
                if identity.owner() != callable_body {
                    return Err(LirFoundationBuildError::SafepointOwnerMismatch {
                        function: function_index,
                        site: *identity.site_id().as_array(),
                        expected: *callable_body.as_array(),
                        actual: *identity.owner().as_array(),
                    });
                }
                safepoint_sites.push(identity.site_record().clone());
                safepoints.push(SafepointMappingRecord::from_identity(identity));
            }
        }

        let mut foundation = Self::empty();
        foundation.set_callable_bodies(callable_bodies)?;
        foundation.set_safepoint_sites(safepoint_sites)?;
        foundation.set_safepoints(safepoints)?;
        Ok(foundation)
    }
}

#[cfg(test)]
mod tests {
    use la_arena::Arena;
    use scoop_identity::{
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
        PersistentFunctionId, SafepointSiteRole, SourceDeclarationKey, SourceDeclarationSite,
    };

    use super::*;
    use crate::{
        AbiReturn, BasicBlock, CallTargets, CallableBodyIdentity, CallingConvention, GcEffect,
        SafepointIdentities, SafepointIdentity, SafepointSiteRef, ScoopAbiSignature, Terminator,
    };

    #[test]
    fn projects_callable_bodies_and_safepoint_relations_without_block_scanning() {
        let first = callable_body("first");
        let first_site =
            SafepointIdentity::new(first.id(), SafepointSiteRole::NativeSafeTransition, 0).unwrap();
        let second_site =
            SafepointIdentity::new(first.id(), SafepointSiteRole::ManagedPoll, 0).unwrap();
        let second = callable_body("second");
        let functions = vec![
            function(
                first,
                SafepointIdentities::checked(vec![
                    (SafepointSiteRef::from_u32(4), second_site.clone()),
                    (SafepointSiteRef::from_u32(2), first_site.clone()),
                ])
                .unwrap(),
            ),
            function(second, SafepointIdentities::default()),
        ];

        let foundation = CanonicalLirFoundation::from_functions(&functions).unwrap();

        assert_eq!(foundation.callable_bodies.len(), 2);
        assert_eq!(foundation.safepoint_sites.len(), 2);
        assert_eq!(foundation.safepoints.len(), 2);
        let mut expected = vec![first_site, second_site];
        expected.sort_by_key(SafepointIdentity::site_id);
        for ((site, mapping), identity) in foundation
            .safepoint_sites
            .iter()
            .zip(&foundation.safepoints)
            .zip(expected)
        {
            assert_eq!(site.id(), identity.site_id());
            assert_eq!(mapping.site(), identity.site_id());
            assert_eq!(mapping.safepoint(), identity.runtime_id());
        }
    }

    #[test]
    fn rejects_a_site_owned_by_another_callable_body() {
        let function_body = callable_body("function");
        let other_body = callable_body("other");
        let site =
            SafepointIdentity::new(other_body.id(), SafepointSiteRole::ManagedCall, 0).unwrap();
        let functions = vec![function(
            function_body,
            SafepointIdentities::checked(vec![(SafepointSiteRef::from_u32(0), site)]).unwrap(),
        )];

        let error = CanonicalLirFoundation::from_functions(&functions).unwrap_err();

        assert!(matches!(
            error,
            LirFoundationBuildError::SafepointOwnerMismatch { function: 0, .. }
        ));
    }

    #[test]
    fn rejects_a_callable_body_claimed_by_two_functions() {
        let body = callable_body("duplicate");
        let functions = vec![
            function(body.clone(), SafepointIdentities::default()),
            function(body, SafepointIdentities::default()),
        ];

        let error = CanonicalLirFoundation::from_functions(&functions).unwrap_err();

        assert!(matches!(
            error,
            LirFoundationBuildError::DuplicateIdentity {
                table: LirFoundationTable::CallableBody,
                ..
            }
        ));
    }

    fn function(callable_body: CallableBodyIdentity, safepoints: SafepointIdentities) -> Function {
        let mut blocks = Arena::new();
        let entry = blocks.alloc(BasicBlock {
            name: "entry".to_string(),
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        });
        Function {
            callable_body,
            gc_effect: GcEffect::Managed,
            symbol: "projection_test".to_string(),
            signature: ScoopAbiSignature::new(
                Vec::new(),
                AbiReturn::UnitVoid,
                CallingConvention::Cdecl,
            ),
            call_targets: CallTargets::default(),
            safepoints,
            locals: Arena::new(),
            temps: Arena::new(),
            blocks,
            entry,
        }
    }

    fn callable_body(name: &str) -> CallableBodyIdentity {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let declaration = SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        CallableBodyIdentity::for_function(function).unwrap()
    }
}
