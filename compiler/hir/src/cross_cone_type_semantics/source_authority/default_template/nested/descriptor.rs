use super::*;
use scoop_identity::{SignatureTypeKey, StructuralDefinitionPath};

#[derive(Clone, Copy, Debug)]
pub enum DefaultSourceNestedCallableDescriptorV1<'a> {
    LocalFunction(&'a DefaultLocalFunctionV1),
    Lambda(&'a DefaultLambdaV1),
    AnonymousFunction(&'a DefaultAnonymousFunctionV1),
    CallableReference(&'a DefaultCallableReferenceV1),
}
impl<'a> DefaultSourceNestedCallableDescriptorV1<'a> {
    pub fn identity(self) -> DefaultNestedCallableIdentityV1 {
        use DefaultNestedCallableIdentityV1 as Identity;
        match self {
            Self::LocalFunction(f) => Identity::LocalFunction(f.declaration()),
            Self::Lambda(f) => Identity::Lambda(f.body()),
            Self::AnonymousFunction(f) => Identity::AnonymousFunction(f.body()),
            Self::CallableReference(f) => Identity::CallableReference(f.invoke()),
        }
    }
    pub fn definition_path(self) -> &'a StructuralDefinitionPath {
        match self {
            Self::LocalFunction(f) => f.definition_path(),
            Self::Lambda(f) => f.definition_path(),
            Self::AnonymousFunction(f) => f.definition_path(),
            Self::CallableReference(f) => f.definition_path(),
        }
    }
    pub fn function_type(self) -> &'a SignatureTypeKey {
        match self {
            Self::LocalFunction(f) => f.function_type(),
            Self::Lambda(f) => f.function_type(),
            Self::AnonymousFunction(f) => f.function_type(),
            Self::CallableReference(f) => f.function_type(),
        }
    }
    pub fn captures(self) -> &'a [DefaultCaptureV1] {
        match self {
            Self::LocalFunction(f) => f.captures(),
            Self::Lambda(f) => f.captures(),
            Self::AnonymousFunction(f) => f.captures(),
            Self::CallableReference(f) => f.captures(),
        }
    }
    pub fn owner_type_parameter_count(self) -> u32 {
        match self {
            Self::LocalFunction(f) => f.owner_type_parameter_count(),
            Self::Lambda(f) => f.owner_type_parameter_count(),
            Self::AnonymousFunction(f) => f.owner_type_parameter_count(),
            Self::CallableReference(f) => f.owner_type_parameter_count(),
        }
    }
    pub fn body_arguments(self) -> DefaultNestedCallableBodyArgumentsV1<'a> {
        let arguments = match self {
            Self::LocalFunction(_) | Self::CallableReference(_) => {
                return DefaultNestedCallableBodyArgumentsV1::Absent;
            }
            Self::Lambda(f) => f.body_type_arguments(),
            Self::AnonymousFunction(f) => f.body_type_arguments(),
        };
        match arguments.explicit_arguments() {
            Some(arguments) => DefaultNestedCallableBodyArgumentsV1::Explicit(arguments),
            None => DefaultNestedCallableBodyArgumentsV1::Lexical,
        }
    }
}
