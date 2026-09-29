use super::*;

impl Concretizer<'_> {
    pub(super) fn local_capture_bindings(
        &self,
        function: export::FunctionId,
    ) -> Vec<export::BindingId> {
        let mut declarations = self
            .source
            .local_functions
            .iter()
            .filter(|(_, declaration)| declaration.function == function);
        let Some((_, declaration)) = declarations.next() else {
            return Vec::new();
        };
        let bindings = declaration
            .captures
            .iter()
            .map(|capture| capture.binding)
            .collect::<Vec<_>>();
        for (_, other) in declarations {
            assert!(
                other
                    .captures
                    .iter()
                    .map(|capture| capture.binding)
                    .eq(bindings.iter().copied()),
                "all descriptors of a local body have the same ordered capture bindings"
            );
        }
        bindings
    }
}
