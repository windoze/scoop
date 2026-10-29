use super::*;

impl Serialize for QueryJson<'_, identity::LocalValueSelector> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use identity::LocalValueSelector::*;
        match self.0 {
            This => object!(serializer, {"kind": "this"}),
            EnclosingInstance => object!(serializer, {"kind": "enclosingInstance"}),
            CapturedEnvironment => object!(serializer, {"kind": "capturedEnvironment"}),
            Parameter { declaration_index } => {
                object!(serializer, {"kind": "parameter", "index": declaration_index})
            }
            LocalDeclaration { path } => {
                object!(serializer, {"kind": "localDeclaration", "path": QueryJson(path)})
            }
            BoundReceiver { path } => {
                object!(serializer, {"kind": "boundReceiver", "path": QueryJson(path)})
            }
            SuspensionResult { site } => {
                object!(serializer, {"kind": "suspensionResult", "site": QueryJson(site)})
            }
            Synthetic { path, role } => {
                object!(serializer, {"kind": "synthetic", "path": QueryJson(path), "role": QueryJson(role)})
            }
        }
    }
}

impl Serialize for QueryJson<'_, identity::StructuralDefinitionPath> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        QueryJson(self.0.segments()).serialize(serializer)
    }
}

impl Serialize for QueryJson<'_, identity::StructuralPathSegment> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"role": QueryJson(&self.0.site_role()), "ordinal": self.0.ordinal()})
    }
}

impl Serialize for QueryJson<'_, identity::StructuralDefinitionSiteRole> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use identity::StructuralDefinitionSiteRole::*;
        serializer.serialize_str(match self.0 {
            LocalDeclaration => "localDeclaration",
            Lambda => "lambda",
            DefaultValue => "defaultValue",
            AnonymousObject => "anonymousObject",
            CoroutineTransform => "coroutineTransform",
            CallbackConversion => "callbackConversion",
            CallableConversion => "callableConversion",
            DispatchAdapter => "dispatchAdapter",
            Initializer => "initializer",
            SynthesizedBridge => "synthesizedBridge",
            SyntheticValue => "syntheticValue",
            StringConstant => "stringConstant",
            ArrayConstant => "arrayConstant",
        })
    }
}

impl Serialize for QueryJson<'_, identity::SyntheticLocalRole> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use identity::SyntheticLocalRole::*;
        serializer.serialize_str(match self.0 {
            Temporary => "temporary",
            DefaultValue => "defaultValue",
            DesugaredIterator => "desugaredIterator",
            CoroutineProtocol => "coroutineProtocol",
            CallbackContext => "callbackContext",
        })
    }
}

impl Serialize for BodyJson<'_, hir::TemplateLocalRecordV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "selector": QueryJson(self.node.selector()), "valueType": self.child(self.node.value_type()),
            "mutable": self.node.mutable().value(), "definition": QueryJson(self.node.definition()),
        })
    }
}

impl Serialize for QueryJson<'_, hir::TemplateLocalDefinitionV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::TemplateLocalDefinitionV1::Source(origin) => {
                object!(serializer, {"kind": "source", "origin": QueryJson(origin)})
            }
            hir::TemplateLocalDefinitionV1::Synthetic => object!(serializer, {"kind": "synthetic"}),
        }
    }
}

impl Serialize for QueryJson<'_, identity::CallableTemplateOwner> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use identity::CallableTemplateOwner::*;
        match self.0 {
            Function(id) => object!(serializer, {"kind": "function", "id": Reference::from(*id)}),
            GenericFunction(id) => {
                object!(serializer, {"kind": "genericFunction", "id": Reference::from(*id)})
            }
            Constructor(id) => {
                object!(serializer, {"kind": "constructor", "id": Reference::from(*id)})
            }
            Accessor(id) => {
                object!(serializer, {"kind": "propertyAccessor", "id": Reference::from(*id)})
            }
            Generated(id) => object!(serializer, {"kind": "generated", "id": Reference::from(*id)}),
            VariantConstructor(id) => {
                object!(serializer, {"kind": "variantConstructor", "id": Reference::from(*id)})
            }
            ReleaseHook(id) => {
                object!(serializer, {"kind": "releaseHook", "owner": Reference::from(*id)})
            }
        }
    }
}
