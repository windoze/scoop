use super::*;

mod anonymous;
mod lambdas;
mod maybe_uninit;
pub(crate) mod primitive;
mod references;
mod reuse;
mod targets;
use reuse::{ClosureDefinition, ClosureDefinitions};

impl Lowerer {
    /// Materialize the concrete closure classes before lowering any body, so
    /// every creation expression resolves directly to a typed class id.
    pub(super) fn declare_closures(&mut self, module: &hir::Module) {
        let mut definitions = ClosureDefinitions::default();
        self.declare_lambdas(module, &mut definitions);
        self.declare_anonymous(module, &mut definitions);
        self.declare_references(module, &mut definitions);
    }
}

fn capture_source(index: usize) -> mir::ClosureFieldSource {
    mir::ClosureFieldSource::Capture {
        declaration_index: u32::try_from(index)
            .expect("a closure capture declaration index fits the identity schema"),
    }
}

fn order_closure_fields(
    identity: &mir::ClosureEnvironmentIdentity,
    fields: Vec<(mir::ClosureFieldSource, mir::Field)>,
) -> Vec<mir::Field> {
    let mut by_source = fields.into_iter().collect::<HashMap<_, _>>();
    let physical = identity
        .fields()
        .iter()
        .map(|field| {
            by_source
                .remove(&field.source())
                .expect("every persistent closure field has one physical definition")
        })
        .collect();
    assert!(
        by_source.is_empty(),
        "every physical closure field has a persistent identity"
    );
    physical
}
