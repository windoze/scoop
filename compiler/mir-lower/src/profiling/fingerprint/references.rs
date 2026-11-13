use super::*;

impl Writer<'_> {
    pub(super) fn ty(&mut self, ty: &mir::Type) {
        if let Some(source) = self.module.meta.source_exact_types.get(ty) {
            self.tag("source-type");
            self.wire(&source.identity_record().id());
            return;
        }
        use mir::GeneratedExactTypeLocation as G;
        let generated = match ty {
            mir::Type::Class(id) => Some(G::Class(*id)),
            mir::Type::Enum(id, _) => Some(G::Enum(*id)),
            mir::Type::Context(storage) => Some(G::Context(*storage)),
            _ => None,
        };
        if let Some(record) =
            generated.and_then(|location| self.module.meta.generated_exact_types.get(location))
        {
            self.tag("generated-type");
            self.wire(&record.exact_record().id());
            return;
        }
        match ty {
            mir::Type::Context(storage) => {
                self.tag("context");
                self.wire(&storage.exact_record().id());
            }
            mir::Type::Unit => self.tag("unit"),
            mir::Type::Integer(kind) => {
                self.tag("integer");
                self.tag(kind.canonical_name());
            }
            mir::Type::MachineScalar(kind) => {
                self.tag("machine-scalar");
                self.tag(kind.name());
            }
            mir::Type::Boolean => self.tag("boolean"),
            mir::Type::String => self.tag("string"),
            mir::Type::Any => self.tag("any"),
            mir::Type::Tuple(fields) => {
                self.tag("tuple");
                self.number(fields.len() as u64);
                for field in fields {
                    self.ty(field);
                }
            }
            mir::Type::Ptr(pointee) => {
                self.tag("pointer");
                self.ty(pointee);
            }
            mir::Type::Function(signature) => {
                self.tag("function-type");
                self.signature(*signature);
            }
            mir::Type::FunPtr(signature) => {
                self.tag("native-function-type");
                self.signature(*signature);
            }
            mir::Type::GcLeafFunPtr(signature) => {
                self.tag("gc-leaf-function-type");
                self.signature(*signature);
            }
            mir::Type::Struct(_)
            | mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::Enum(_, _) => {
                unreachable!("a nominal MIR type retains its source or generated exact identity")
            }
        }
    }

    pub(super) fn signature(&mut self, id: mir::FunctionTypeId) {
        let signature = &self.module.function_types[id];
        self.number(u64::from(signature.is_suspend));
        self.number(u64::from(signature.receiver.is_some()));
        if let Some(receiver) = &signature.receiver {
            self.ty(receiver);
        }
        self.number(signature.parameter_types.len() as u64);
        for parameter in &signature.parameter_types {
            self.ty(parameter);
        }
        self.ty(&signature.return_type);
    }

    pub(super) fn function(&mut self, id: mir::FunctionId) {
        let subject = self
            .module
            .meta
            .callable_signature_subject(id)
            .expect("a MIR callable retains its implementation subject");
        let owner = CallableDefinitionOwner::try_from(subject)
            .expect("a concrete callable has a definition owner");
        self.wire(&owner);
    }

    pub(super) fn callee(&mut self, callee: mir::Callee) {
        match callee {
            mir::Callee::User(id) => {
                self.tag("scoop");
                self.function(id);
            }
            mir::Callee::Monomorphized(id) => {
                self.tag("scoop");
                self.function(self.module.meta.instances[id].function);
            }
            mir::Callee::External(id) => {
                self.tag("scoop");
                let owner: CallableDefinitionOwner = self.module.meta.external_callables[id]
                    .reference()
                    .implementation()
                    .into();
                self.wire(&owner);
            }
            mir::Callee::Extern(id) => {
                self.tag("native");
                let function = &self.module.extern_functions[id];
                self.wire(&function.source_contract);
            }
            mir::Callee::Runtime(mir::RuntimeFn::PrepareThrowable { trace_bytes }) => {
                self.tag("prepare-throwable");
                self.ty(&mir::Type::Class(trace_bytes));
            }
            mir::Callee::Runtime(function) => {
                self.tag("runtime");
                self.tag(function.symbol());
            }
        }
    }

    pub(super) fn global(&mut self, id: mir::GlobalId) {
        let global = &self.module.globals[id];
        // StaticStorageOwner consists only of typed persistent owners.
        self.scalar(&global.storage_owner);
        self.ty(&global.ty);
        self.number(u64::from(global.mutable));
    }

    pub(super) fn closure(&mut self, id: mir::ClosureClassId) {
        let identity = self
            .module
            .meta
            .generated_exact_types
            .get(mir::GeneratedExactTypeLocation::Closure(id))
            .expect("a closure class retains its generated exact identity");
        self.wire(&identity.exact_record().id());
    }

    pub(super) fn immortal(&mut self, id: mir::ImmortalConstId) {
        let value = &self.module.immortals[id];
        match &value.value {
            mir::ImmortalConstValue::String(value) => {
                self.tag("string");
                self.tag(value);
            }
            mir::ImmortalConstValue::ByteArray { array_type, bytes } => {
                self.tag("bytes");
                self.ty(&mir::Type::Class(*array_type));
                self.bytes(bytes);
            }
            mir::ImmortalConstValue::EmbeddedFileArray { array_type, files } => {
                self.tag("embedded-files");
                self.ty(&mir::Type::Class(*array_type));
                self.number(files.len() as u64);
                for file in files {
                    self.immortal(file.path);
                    self.immortal(file.bytes);
                }
            }
        }
    }
}
