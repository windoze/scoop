use super::*;
use serde_json::{Value, json};

impl GeneratorBatch {
    /// Produces the actual generator/generate parameters from the same fixed view.
    pub fn request(&self, queries: &mut dyn hir::SemanticQueries) -> Result<Value, Diagnostic> {
        let tasks = self
            .tasks
            .iter()
            .map(|task| task.request(queries))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({
            "protocolVersion": scoop_generator_protocol::PROTOCOL_VERSION, "generator": self.generator,
            "currentCone": Reference::from(queries.current_cone()),
            "coreCone": Reference::from(ConeIdentity::CORE), "tasks": tasks,
        }))
    }
}

impl GeneratorTask {
    fn request(&self, queries: &mut dyn hir::SemanticQueries) -> Result<Value, Diagnostic> {
        let context = hir::SemanticContext::Nominal(self.owner);
        let owner = queries
            .signature(self.methods.first().pending.function)
            .receiver
            .expect("an implementation method has its nominal receiver");
        let owner = queries.type_signature(context.into(), owner)?;
        let bindings = self.bindings.iter().map(|binding| {
            Ok(json!({"interface": QueryJson(&queries.type_signature(context.into(), binding.interface)?),
                "origin": QueryJson(&binding.origin)}))
        }).collect::<Result<Vec<_>, Diagnostic>>()?;
        let methods = self.methods.iter().map(|method| {
            let requirements = method.pending.requirements.iter().map(|requirement| {
                let interface = queries.type_signature(context.into(), requirement.interface)?;
                let slot = queries.requirement_slot(requirement.declaration)?;
                Ok(json!({"interface": QueryJson(&interface), "slot": Reference::from(slot)}))
            }).collect::<Result<Vec<_>, Diagnostic>>()?;
            Ok(json!({"method": method.method, "requirements": requirements,
                "signature": scoop_generator_protocol::pending_method_signature(queries, &method.pending)?,
                "origin": QueryJson(&method.origin)}))
        }).collect::<Result<Vec<_>, Diagnostic>>()?;
        Ok(json!({"owner": QueryJson(&owner), "bindings": bindings, "methods": methods}))
    }
}
