use super::*;

/// String constants keyed by their semantic owner and structural site.
///
/// A repeated property-constant read names the same immutable object, while
/// separate literal occurrences remain distinct even when their bytes match.
#[derive(Default)]
pub(super) struct StringRegistry {
    values: Arena<mir::StringConst>,
    by_identity: HashMap<mir::ImmortalObjectKey, mir::StringConstId>,
}

impl StringRegistry {
    pub(super) fn intern(
        &mut self,
        owner: mir::ImmortalObjectOwner,
        ordinal: u32,
        value: String,
    ) -> mir::StringConstId {
        let path = mir::StructuralDefinitionPath::from_first(
            mir::StructuralPathSegment::new(
                mir::StructuralDefinitionSiteRole::StringConstant,
                ordinal,
            ),
            [],
        );
        let identity = mir::ImmortalObjectKey::string_constant(owner, path);
        if let Some(&id) = self.by_identity.get(&identity) {
            assert_eq!(
                self.values[id].value, value,
                "one string-constant identity cannot name different content"
            );
            return id;
        }

        let id = self.values.alloc(mir::StringConst {
            identity: identity.clone(),
            value,
        });
        let previous = self.by_identity.insert(identity, id);
        assert!(previous.is_none(), "new string identity was absent");
        id
    }

    pub(super) fn finish(self) -> Arena<mir::StringConst> {
        self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_identity::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain, PackagePath,
        PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
    };

    fn callable(name: &str) -> mir::ImmortalObjectOwner {
        let site = SourceDeclarationSite::new(
            test_cone(),
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
            vec![],
        );
        let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        mir::ImmortalObjectOwner::Callable(CallableMaterialization::new(
            CallableTemplateOwner::Function(function),
            CallableMaterializationContext::NoSubstitution,
        ))
    }

    fn test_cone() -> scoop_identity::ConeIdentity {
        ConeCoordinate::new("test", "strings", "0.0.0")
            .unwrap()
            .identity()
            .unwrap()
    }

    #[test]
    fn equal_identity_reuses_one_constant() {
        let mut strings = StringRegistry::default();
        let owner = callable("main");
        let first = strings.intern(owner, 0, "same".to_string());
        let repeated = strings.intern(owner, 0, "same".to_string());
        assert_eq!(first, repeated);
        assert_eq!(strings.finish().len(), 1);
    }

    #[test]
    fn equal_content_at_distinct_sites_remains_distinct() {
        let mut strings = StringRegistry::default();
        let owner = callable("main");
        let first = strings.intern(owner, 0, "same".to_string());
        let second = strings.intern(owner, 1, "same".to_string());
        assert_ne!(first, second);
        assert_eq!(strings.finish().len(), 2);
    }

    #[test]
    #[should_panic(expected = "one string-constant identity cannot name different content")]
    fn equal_identity_cannot_change_content() {
        let mut strings = StringRegistry::default();
        let owner = callable("main");
        strings.intern(owner, 0, "first".to_string());
        strings.intern(owner, 0, "second".to_string());
    }
}
