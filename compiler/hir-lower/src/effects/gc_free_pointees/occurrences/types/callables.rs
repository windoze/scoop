//! Types carried by selected callables and their applications.

use super::{collect_expr_types, collect_function_type_types};
use crate::Lowerer;
use scoop_hir as hir;

pub(in crate::effects::gc_free_pointees) fn collect_callable_types(
    lowerer: &Lowerer,
    callable: hir::Callable,
    out: &mut Vec<hir::TypeId>,
) {
    match callable {
        hir::Callable::Function(_) => {}
        hir::Callable::Generic(application) => {
            out.extend(
                lowerer.instantiations[application]
                    .type_args
                    .iter()
                    .copied(),
            );
        }
        hir::Callable::Method(application) => {
            collect_method_owner_types(
                lowerer,
                lowerer.method_applications[application].owner,
                out,
            );
        }
        hir::Callable::GenericMethod(application) => {
            let application = &lowerer.generic_method_applications[application];
            collect_generic_method_owner_types(lowerer, application.owner, out);
            out.extend(application.method_arguments.iter().copied());
        }
    }
}

pub(in crate::effects::gc_free_pointees) fn collect_callable_target_types(
    lowerer: &Lowerer,
    target: hir::CallableTarget,
    out: &mut Vec<hir::TypeId>,
) {
    match target {
        hir::CallableTarget::Local(callable) => collect_callable_types(lowerer, callable, out),
        hir::CallableTarget::Application(application) => out.extend(
            lowerer.imported_generic_applications[application]
                .arguments
                .substitution(
                    &lowerer.types,
                    &lowerer.enum_applications,
                    &lowerer.struct_applications,
                    &lowerer.class_applications,
                    &lowerer.interface_applications,
                ),
        ),
        hir::CallableTarget::Dependency(_) => {}
    }
}

fn collect_method_owner_types(
    lowerer: &Lowerer,
    owner: hir::MethodOwnerApplication,
    out: &mut Vec<hir::TypeId>,
) {
    let ty = match owner {
        hir::MethodOwnerApplication::Class(application) => {
            Some(lowerer.class_applications[application].canonical_type)
        }
        hir::MethodOwnerApplication::Struct(application) => {
            Some(lowerer.struct_applications[application].canonical_type)
        }
        hir::MethodOwnerApplication::Enum(application) => {
            Some(lowerer.enum_applications[application].canonical_type)
        }
        hir::MethodOwnerApplication::Interface(application) => {
            Some(lowerer.interface_applications[application].canonical_type)
        }
        hir::MethodOwnerApplication::Object(_) => None,
    };
    out.extend(ty);
}

fn collect_generic_method_owner_types(
    lowerer: &Lowerer,
    owner: hir::GenericMethodOwner,
    out: &mut Vec<hir::TypeId>,
) {
    let ty = match owner {
        hir::GenericMethodOwner::Class(application) => {
            Some(lowerer.class_applications[application].canonical_type)
        }
        hir::GenericMethodOwner::Struct(application) => {
            Some(lowerer.struct_applications[application].canonical_type)
        }
        hir::GenericMethodOwner::Enum(application) => {
            Some(lowerer.enum_applications[application].canonical_type)
        }
        hir::GenericMethodOwner::Object(_) => None,
    };
    out.extend(ty);
}

pub(in crate::effects::gc_free_pointees) fn collect_method_callee_types(
    lowerer: &Lowerer,
    callee: hir::MethodCallee,
    out: &mut Vec<hir::TypeId>,
) {
    match callee {
        hir::MethodCallee::Callable(callable) => {
            collect_callable_target_types(lowerer, callable, out)
        }
        hir::MethodCallee::Bound(bound) => {
            let bound = &lowerer.bound_callable_refs[bound];
            out.push(bound.receiver_type);
            collect_callable_target_types(lowerer, bound.declared_callable(), out);
            collect_function_type_types(lowerer, bound.instantiated_signature, out);
            match bound.source {
                hir::BoundCallableSource::Class { bound, .. } => {
                    out.push(lowerer.class_applications[bound].canonical_type);
                }
                hir::BoundCallableSource::Interface { bound, .. } => {
                    out.push(lowerer.interface_applications[bound].canonical_type);
                }
            }
        }
        hir::MethodCallee::DerivedEquality(application) => {
            out.push(lowerer.derived_equality_applications[application].owner_ty);
        }
    }
}

pub(super) fn collect_callable_reference_types(
    lowerer: &Lowerer,
    reference: hir::CallableReferenceId,
    out: &mut Vec<hir::TypeId>,
) {
    let reference = &lowerer.callable_references[reference];
    collect_function_type_types(lowerer, reference.function_type, out);
    out.extend(reference.owner_type_arguments.iter().copied());
    out.extend(reference.captures.iter().map(|capture| capture.ty));
    for capture in &reference.captures {
        collect_expr_types(lowerer, &capture.source, out);
    }
    collect_reference_target_types(lowerer, &reference.target, out);
    if let Some(receiver) = reference.target.receiver() {
        collect_expr_types(lowerer, receiver, out);
    }
}

pub(in crate::effects::gc_free_pointees) fn collect_reference_target_types(
    lowerer: &Lowerer,
    target: &hir::CallableReferenceTarget,
    out: &mut Vec<hir::TypeId>,
) {
    if let hir::CallableReferenceTarget::BoundMember { callee, .. } = target {
        collect_method_callee_types(lowerer, *callee, out);
    } else if let Some(target) = target.callee(&lowerer.bound_callable_refs) {
        collect_callable_target_types(lowerer, target, out);
    }
}
