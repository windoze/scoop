//! Source release blocks retain a distinct owner and never create functions.

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{ClassId, ForbiddenSuspendContext, Lowerer, Owner, SuspensionContext};

mod body;
mod fields;
mod restrictions;

impl Lowerer {
    pub(crate) fn lower_release_blocks(
        &mut self,
        classes: &[(ClassId, &ast::ClassDecl, usize)],
        objects: &[(hir::ObjectId, crate::declarations::ObjectSource<'_>, usize)],
    ) {
        for &(class, declaration, file) in classes {
            self.current_file = file;
            let mut blocks = declaration
                .members
                .iter()
                .filter_map(|member| match member {
                    ast::ClassMember::ReleaseBlock(block) => Some(block),
                    _ => None,
                });
            let Some(block) = blocks.next() else { continue };
            for duplicate in blocks {
                self.error(
                    duplicate.span,
                    "a class may declare only one `release` block".into(),
                );
            }
            if self.classes[class].modifier != hir::ClassModifier::Final
                || !self.classes[class].is_declared()
            {
                self.error(
                    block.span,
                    "`release` requires an ordinary final class".into(),
                );
                continue;
            }
            let owner_type =
                self.class_applications[self.classes[class].self_application].canonical_type;
            if let Some(throwable) = self.throwable_ty(block.span)
                && self.is_subtype(owner_type, throwable)
            {
                self.error(
                    block.span,
                    "a Throwable subtype cannot declare a `release` block".into(),
                );
                continue;
            }
            let body = self.lower_release_body(class, block);
            let owner = self.nominal_identity(Owner::Class(class)).declaration_id();
            let hook = self.release_hooks.alloc(hir::ExportReleaseHook {
                owner,
                requirements: Vec::new(),
                body,
                span: block.span,
            });
            self.classes[class].release_policy = hir::ReleasePolicy::SynchronousGcFree {
                hook: hir::ExportReleaseHookRef::Template(hook),
            };
        }
        for &(_, object, file) in objects {
            self.current_file = file;
            for member in object.members() {
                if let ast::ClassMember::ReleaseBlock(block) = member {
                    self.error(
                        block.span,
                        format!(
                            "`release` blocks are not allowed in {}",
                            object.description()
                        ),
                    );
                }
            }
        }
    }
}
