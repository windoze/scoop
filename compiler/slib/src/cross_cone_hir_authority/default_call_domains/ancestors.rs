//! Matches source signatures only after resolving every ancestor by typed identity.
use super::*;
use scoop_hir::NestedSourceMemberRefV1;

struct Frame<'a> {
    nominal: &'a NominalInterfaceRecordV1,
    arguments: CanonicalBinderUseListV1,
    next: usize,
    superclasses: u32,
}

impl<'a> Query<'_, 'a> {
    pub(super) fn inherited(
        &mut self,
        source: Source<'a>,
        path: &WirePath,
    ) -> Result<Vec<Inherited<'a>>, Error> {
        let mut inherited = Vec::new();
        let declaration = source.declaration;
        let PublicDeclarationOwnerV1::Nominal(owner) = declaration.owner() else {
            return Ok(inherited);
        };
        if !matches!(
            declaration.declaration(),
            CallableTemplateOrigin::Function(_)
        ) {
            return Ok(inherited);
        }
        let (nominal, _) = self.authority.visibility_nominal(owner)?;
        if nominal.kind() == PublicNominalKindV1::Interface
            && declaration.declared_visibility() == DeclaredVisibilityV1::Private
        {
            return Ok(inherited);
        }
        let arity = nominal.type_parameters().len_u32();
        let mut arguments = Vec::new();
        scoop_wire::allocation::try_reserve(&mut arguments, arity as usize, path)?;
        for index in 0..arity {
            arguments.push(SignatureTypeKey::Binder { depth: 0, index });
        }
        let mut frames = Vec::new();
        scoop_wire::allocation::try_reserve(&mut frames, 1, path)?;
        frames.push(Frame {
            nominal,
            arguments: signatures::mapping(arguments, path)?,
            next: 0,
            superclasses: 0,
        });
        while let Some(frame) = frames.last_mut() {
            let Some(parent) = frame.nominal.exact_supertypes().values().get(frame.next) else {
                frames.pop();
                continue;
            };
            frame.next += 1;

            let applied = frame.arguments.substitute_provider_type(
                signatures::shape(frame.nominal.type_parameters().len_u32(), path)?,
                parent,
            )?;
            let (owner, arguments) = match applied {
                SignatureTypeKey::Nominal(id) => (SourceNominalId::Concrete(id), Vec::new()),
                SignatureTypeKey::NominalApplication { origin, arguments } => (
                    SourceNominalId::GenericTemplate(origin),
                    arguments.into_vec(),
                ),
                _ => return Err(Error::SupertypeShape),
            };
            let (parent, _) = self.authority.visibility_nominal(owner)?;
            if parent.kind() == PublicNominalKindV1::Class {
                if !matches!(
                    frame.nominal.kind(),
                    PublicNominalKindV1::Class | PublicNominalKindV1::Object
                ) {
                    return Err(Error::SupertypeShape);
                }
                frame.superclasses += 1;
                if frame.superclasses > 1 {
                    return Err(Error::MultipleSuperclasses(frame.nominal.declaration()));
                }
            } else if parent.kind() != PublicNominalKindV1::Interface {
                return Err(Error::SupertypeShape);
            }

            if frames
                .iter()
                .any(|frame| frame.nominal.declaration() == owner)
            {
                return Err(Error::NominalCycle(owner));
            }
            if parent.type_parameters().len_u32() as usize != arguments.len() {
                return Err(Error::SupertypeArity {
                    owner,
                    expected: parent.type_parameters().len_u32(),
                    actual: arguments.len(),
                });
            }
            let arguments = signatures::mapping(arguments, path)?;
            self.collect_matches(
                source,
                parent,
                &arguments,
                frames.len(),
                &mut inherited,
                path,
            )?;

            scoop_wire::allocation::try_reserve(&mut frames, 1, path)?;
            frames.push(Frame {
                nominal: parent,
                arguments,
                next: 0,
                superclasses: 0,
            });
        }
        Ok(inherited)
    }

    fn collect_matches(
        &mut self,
        source: Source<'a>,
        nominal: &'a NominalInterfaceRecordV1,
        arguments: &CanonicalBinderUseListV1,
        distance: usize,
        inherited: &mut Vec<Inherited<'a>>,
        path: &WirePath,
    ) -> Result<(), Error> {
        for member in nominal.declaration_details().members().values() {
            let NestedSourceMemberRefV1::Function(id) = member else {
                continue;
            };
            let candidate = self.source(CallableTemplateOrigin::Function(*id))?;
            if candidate.declaration.owner()
                != PublicDeclarationOwnerV1::Nominal(nominal.declaration())
            {
                return Err(Error::CallableRole(candidate.declaration.declaration()));
            }
            if candidate.declaration.declared_visibility() == DeclaredVisibilityV1::Private
                || (candidate.declaration.declared_visibility() == DeclaredVisibilityV1::Internal
                    && candidate.key.origin() != source.key.origin())
            {
                continue;
            }
            if !signatures::matches(source, candidate, arguments, path)? {
                continue;
            }
            let mut duplicate = false;
            for previous in inherited.iter_mut() {
                if previous.source.declaration.declaration() == candidate.declaration.declaration()
                    && signatures::equal_arguments(&previous.arguments, arguments)
                {
                    previous.distance = previous.distance.min(distance);
                    duplicate = true;
                    break;
                }
            }
            if duplicate {
                continue;
            }
            let arguments = signatures::copy_arguments(arguments.arguments(), path)?;
            scoop_wire::allocation::try_reserve(inherited, 1, path)?;
            inherited.push(Inherited {
                source: candidate,
                arguments,
                distance,
                kind: nominal.kind(),
            });
        }
        Ok(())
    }
}
