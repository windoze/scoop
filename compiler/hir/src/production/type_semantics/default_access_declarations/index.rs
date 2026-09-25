use super::*;

pub(super) fn collect<'a>(
    export: &'a ExportHir,
) -> Result<BTreeMap<Subject, Declaration<'a>>, Error> {
    let mut index = Index {
        export,
        entries: BTreeMap::new(),
    };
    for local in nominals::all_nominals(export) {
        let Some(source) = nominals::identity(export, local)?.source() else {
            continue;
        };
        let subject = match nominals::source_id(source) {
            SourceNominalId::Concrete(id) => Subject::Type(id),
            SourceNominalId::GenericTemplate(id) => Subject::GenericType(id),
        };
        index.add(
            subject,
            source.declaration(),
            nominals::nominal_access(export, local).declared,
        )?;
    }
    for (id, function) in export.functions.iter() {
        let HirFunctionIdentity::Source(identity) = &export.function_identities[id] else {
            continue;
        };
        let subject = match identity {
            HirSourceFunctionIdentity::Plain(record) => Subject::Function(record.id()),
            HirSourceFunctionIdentity::Generic(record) => Subject::GenericFunction(record.id()),
        };
        index.add(subject, identity.declaration(), function.access.declared)?;
    }
    for (id, source) in export.struct_constructors.iter() {
        let identity = &export.constructor_identities[id];
        index.add(
            Subject::Constructor(identity.id()),
            identity.key(),
            source.access.declared,
        )?;
    }
    for (id, source) in export.class_constructors.iter() {
        if export.nominal_identities[source.owner].source().is_none() {
            continue;
        }
        let Some(identity) = export.constructor_identities[id].source_record() else {
            continue;
        };
        index.add(
            Subject::Constructor(identity.id()),
            identity.key(),
            source.access.declared,
        )?;
    }
    index.properties()?;
    Ok(index.entries)
}
struct Index<'a> {
    export: &'a ExportHir,
    entries: BTreeMap<Subject, Declaration<'a>>,
}
impl<'a> Index<'a> {
    fn add(
        &mut self,
        subject: Subject,
        key: &'a SourceDeclarationKey,
        visibility: DeclaredVisibility,
    ) -> Result<(), Error> {
        if key.origin() != self.export.cone {
            return Ok(());
        }

        if self
            .entries
            .insert(subject, Declaration { key, visibility })
            .is_some()
        {
            return Err(invalid("duplicate sealed default access declaration"));
        }
        Ok(())
    }
    fn properties(&mut self) -> Result<(), Error> {
        for (id, property) in self.export.properties.iter() {
            let identity = &self.export.property_identities[id];
            let subject = match identity {
                HirPropertyIdentity::Ordinary(record) => Subject::Property(record.id()),
                HirPropertyIdentity::Extension(record) => Subject::ExtensionProperty(record.id()),
            };
            let key = identity.declaration();
            self.add(subject, key, property.access.declared)?;
            let getter_id = property.capability.getter();
            let getter = &self.export.property_getters[getter_id];
            if getter.implementation != PropertyAccessorImplementation::Constant {
                self.add(
                    Subject::PropertyAccessor(
                        self.export.property_accessor_identities[getter_id].id(),
                    ),
                    key,
                    getter.access.declared,
                )?;
            }
            if let Some(setter_id) = property.capability.setter() {
                let setter = &self.export.property_setters[setter_id];
                self.add(
                    Subject::PropertyAccessor(
                        self.export.property_accessor_identities[setter_id].id(),
                    ),
                    key,
                    setter.access.declared,
                )?;
            }
        }
        Ok(())
    }
}
