use super::*;
use std::rc::Rc;

impl Lowerer {
    /// Preserve facts when normal preparation checks a source region first.
    pub(crate) fn observe_prepared_body<T>(
        &mut self,
        id: hir::EditingSourceBodyId,
        root: hir::LexicalDefinitionRoot,
        file: usize,
        operation: impl FnOnce(&mut Self) -> Option<T>,
    ) -> Option<T> {
        let Some(prepared) = &self.editing_prepared_bodies else {
            return operation(self);
        };
        if prepared.contains_key(&id) {
            // A shared initializer can be generated in another terminal constructor.
            let outer_cancellation = self.cancellation.clone();
            let body_cancellation = self
                .editing_body
                .as_ref()
                .map(BodyObservation::cancellation);
            let cancellation = self.cancellation.replace(
                crate::pipeline::cancellation::FrontendCancellation::new(move || {
                    outer_cancellation
                        .as_ref()
                        .is_some_and(|check| check.cancelled())
                        || body_cancellation.as_ref().is_some_and(|check| check())
                }),
            );
            let previous = self.editing_body.take();
            let types = self.editing_types.take();
            let receiver = self.editing_receiver.take();
            let prepared = self.editing_prepared_bodies.take();
            let constructions = self.editing_constructions.take();
            let result = operation(self);
            self.cancellation = cancellation;
            self.editing_constructions = constructions;
            self.editing_prepared_bodies = prepared;
            self.editing_receiver = receiver;
            self.editing_types = types;
            self.editing_body = previous;
            return result;
        }
        let cancellation = self.editing_body.as_ref().map_or_else(
            || {
                let cancellation = self.cancellation.clone();
                Rc::new(move || cancellation.as_ref().is_some_and(|check| check.cancelled()))
                    as Rc<dyn Fn() -> bool>
            },
            BodyObservation::cancellation,
        );
        let check = cancellation.clone();
        let observation =
            BodyObservation::new(id, root, file, self.diagnostics.len(), move || check());
        let previous = self.editing_body.replace(observation);
        let result = operation(self);
        let observed = self.editing_body.take().expect("active source observation");
        self.editing_body = previous;
        if !observed.cancelled() {
            let facts = observed.finish(self, result.is_some());
            if !cancellation() {
                self.editing_prepared_bodies
                    .as_mut()
                    .expect("editing keeps facts from source preparation")
                    .insert(id, facts);
            }
        }
        result
    }
}
