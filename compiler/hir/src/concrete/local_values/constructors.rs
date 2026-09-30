use super::*;

impl<'a> LocalValueIdentityBuilder<'a> {
    pub(super) fn collect_class_constructors(
        &mut self,
    ) -> Result<Vec<ClassConstructorLocalValues>, LocalValueIdentityError> {
        let mut output = Vec::with_capacity(self.inputs.class_constructors.len());
        for (constructor_id, constructor) in self.inputs.class_constructors.iter() {
            let owner = constructor.materialization;
            let receiver_location = LocalValueLocation::ClassReceiver {
                constructor: raw_arena_index(constructor_id),
            };
            let receiver = self.record_source(
                owner,
                LocalValueSelector::This,
                constructor.origin,
                receiver_location,
            )?;
            let parameters = constructor
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let location = LocalValueLocation::ClassParameter {
                        constructor: raw_arena_index(constructor_id),
                        parameter: index as u32,
                    };
                    let identity = self.record_source(
                        owner,
                        LocalValueSelector::Parameter {
                            declaration_index: index as u32,
                        },
                        parameter.definition,
                        location,
                    )?;
                    self.bind(owner.context(), parameter.binding, identity);
                    Ok(identity)
                })
                .collect::<Result<Vec<_>, LocalValueIdentityError>>()?;
            let body = match &constructor.kind {
                ClassConstructorKind::This { body, .. }
                | ClassConstructorKind::Terminal { body } => body,
            };
            let locals = self.collect_locals(
                owner,
                body.locals.iter().map(|(local, value)| {
                    (
                        local,
                        value,
                        LocalValueLocation::ClassLocal {
                            constructor: raw_arena_index(constructor_id),
                            local: raw_arena_index(local),
                        },
                    )
                }),
            )?;
            output.push(ClassConstructorLocalValues {
                receiver,
                parameters,
                locals,
            });
        }
        Ok(output)
    }

    pub(super) fn collect_struct_constructors(
        &mut self,
    ) -> Result<Vec<StructConstructorLocalValues>, LocalValueIdentityError> {
        let mut output = Vec::with_capacity(self.inputs.struct_constructors.len());
        for (constructor_id, constructor) in self.inputs.struct_constructors.iter() {
            let owner = constructor.materialization;
            let parameters = constructor
                .parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let location = LocalValueLocation::StructParameter {
                        constructor: raw_arena_index(constructor_id),
                        parameter: index as u32,
                    };
                    let identity = self.record_source(
                        owner,
                        LocalValueSelector::Parameter {
                            declaration_index: index as u32,
                        },
                        parameter.definition,
                        location,
                    )?;
                    self.bind(owner.context(), parameter.binding, identity);
                    Ok(identity)
                })
                .collect::<Result<Vec<_>, LocalValueIdentityError>>()?;
            let kind = match &constructor.kind {
                StructConstructorKind::Primary => StructConstructorLocalValueKind::Primary,
                StructConstructorKind::Secondary {
                    arguments, body, ..
                } => {
                    let receiver_location = LocalValueLocation::StructReceiver {
                        constructor: raw_arena_index(constructor_id),
                    };
                    let receiver = self.record_source(
                        owner,
                        LocalValueSelector::This,
                        constructor.origin,
                        receiver_location,
                    )?;
                    let argument_locals = self.collect_locals(
                        owner,
                        arguments.locals.iter().map(|(local, value)| {
                            (
                                local,
                                value,
                                LocalValueLocation::StructArgumentLocal {
                                    constructor: raw_arena_index(constructor_id),
                                    local: raw_arena_index(local),
                                },
                            )
                        }),
                    )?;
                    let body_locals = self.collect_locals(
                        owner,
                        body.locals.iter().map(|(local, value)| {
                            (
                                local,
                                value,
                                LocalValueLocation::StructBodyLocal {
                                    constructor: raw_arena_index(constructor_id),
                                    local: raw_arena_index(local),
                                },
                            )
                        }),
                    )?;
                    StructConstructorLocalValueKind::Secondary {
                        receiver,
                        argument_locals,
                        body_locals,
                    }
                }
            };
            output.push(StructConstructorLocalValues { parameters, kind });
        }
        Ok(output)
    }
}
