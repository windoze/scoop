use super::*;

impl<'a> Substitution<'a, '_> {
    pub(super) fn visit(
        &mut self,
        value: &'a SignatureTypeKey,
        substitute: bool,
        depth: u64,
    ) -> Result<(), MeteredDefaultTemplateTypeSubstitutionError> {
        self.meter.check_semantic_depth(depth, self.path)?;
        self.meter.charge_work(1, self.path)?;
        self.meter.charge_nodes(1, self.path)?;
        match value {
            SignatureTypeKey::Nominal(id) => self.values.push(SignatureTypeKey::Nominal(*id)),
            SignatureTypeKey::Binder {
                depth: binder_depth,
                index,
            } => {
                if let Transform::Substitute { mapping, provider } = self.transform
                    && substitute
                {
                    let position = provider
                        .flattened_binder_position(*binder_depth, *index)
                        .map_err(DefaultTemplateTypeSubstitutionError::ProviderBinder)?;
                    let mapped = usize::try_from(position)
                        .ok()
                        .and_then(|position| mapping.arguments().get(position))
                        .ok_or(DefaultTemplateTypeSubstitutionError::MissingMapping {
                            position,
                            len: mapping.len_u32(),
                        })?;
                    self.meter.charge_edges(1, self.path)?;
                    self.meter.charge_work(1, self.path)?;
                    self.meter
                        .try_reserve_collection_slots(&mut self.tasks, 1, self.path)?;
                    self.tasks.push(Task::Visit {
                        value: mapped,
                        substitute: false,
                        depth,
                    });
                } else {
                    self.values.push(SignatureTypeKey::Binder {
                        depth: *binder_depth,
                        index: *index,
                    });
                }
            }
            SignatureTypeKey::NominalApplication { arguments, .. }
            | SignatureTypeKey::Tuple(arguments) => {
                self.children(value, arguments.as_slice(), None, substitute, depth)?;
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                self.children(value, parameters, Some(result), substitute, depth)?;
            }
            SignatureTypeKey::RawPointer(pointee) => {
                self.children(value, &[], Some(pointee), substitute, depth)?;
            }
        }
        Ok(())
    }
    fn children(
        &mut self,
        parent: &'a SignatureTypeKey,
        sequence: &'a [SignatureTypeKey],
        last: Option<&'a SignatureTypeKey>,
        substitute: bool,
        depth: u64,
    ) -> Result<(), MeteredDefaultTemplateTypeSubstitutionError> {
        self.meter
            .check_table_entries(sequence.len() as u64, self.path)?;
        let count = sequence
            .len()
            .checked_add(usize::from(last.is_some()))
            .ok_or_else(|| overflow(self.path))?;
        let child_depth = depth.checked_add(1).ok_or_else(|| overflow(self.path))?;
        self.meter.check_semantic_depth(child_depth, self.path)?;
        self.meter.charge_work(count as u64, self.path)?;
        self.meter.charge_edges(count as u64, self.path)?;
        self.meter.try_reserve_collection_slots(
            &mut self.tasks,
            count.checked_add(1).ok_or_else(|| overflow(self.path))?,
            self.path,
        )?;
        self.meter
            .try_reserve_collection_slots(&mut self.values, count, self.path)?;
        self.tasks.push(Task::Finish(parent));
        if let Some(value) = last {
            self.tasks.push(Task::Visit {
                value,
                substitute,
                depth: child_depth,
            });
        }
        self.tasks
            .extend(sequence.iter().rev().map(|value| Task::Visit {
                value,
                substitute,
                depth: child_depth,
            }));
        Ok(())
    }
}
