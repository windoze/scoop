//! Canonical body projection. Arena handles and debug data are never hashed.

use std::collections::HashMap;

use scoop_identity::CallableDefinitionOwner;
use scoop_mir as mir;
use scoop_profile::{Fingerprint, Position};
use scoop_wire::{CanonicalHashStream, WireEncode};

use crate::cfg::{CallSite, CallSiteKind, CleanupTransfer, SuspendCall};

mod expressions;
mod references;

pub(super) fn function(
    module: &mir::Module,
    function: &mir::Function,
    order: &[mir::BlockId],
    sites: &[CallSite],
    positions: &[Position],
) -> Fingerprint {
    let mut writer = Writer {
        module,
        body: &function.body,
        stream: CanonicalHashStream::new(),
        blocks: order
            .iter()
            .enumerate()
            .map(|(index, &id)| (id, index as u64))
            .collect(),
        locals: HashMap::new(),
        exceptions: HashMap::new(),
    };
    writer.tag("scoop-mir-profile-body-v1");
    writer.scalar(&function.gc_effect);
    writer.ty(&function.return_ty);
    writer.number(function.params.len() as u64);
    for parameter in &function.params {
        writer.ty(&parameter.ty);
        writer.local(parameter.local);
    }
    writer.number(order.len() as u64);
    for &id in order {
        let block = &function.body.blocks[id];
        writer.optional_block(block.unwind);
        writer.number(
            block
                .statements
                .iter()
                .filter(|statement| {
                    !matches!(statement.kind, mir::StatementKind::ProfileCounter(_))
                })
                .count() as u64,
        );
        for statement in &block.statements {
            writer.statement(statement);
        }
        writer.terminator(&block.terminator);
        writer.number(
            function
                .body
                .loop_header_polls
                .iter()
                .any(|poll| poll.header() == id) as u64,
        );
    }
    // Registrations have no ordinary MIR call statement yet. Retain their
    // exact method, operands, order and pending cleanup transfers as well.
    writer.number(sites.len() as u64);
    for site in sites {
        writer.block(site.block);
        writer.number(site.statement as u64);
        match &site.kind {
            CallSiteKind::Call => writer.tag("call"),
            CallSiteKind::Suspend(SuspendCall::Yield) => writer.tag("yield"),
            CallSiteKind::Suspend(SuspendCall::Registration {
                method,
                receiver,
                destination,
            }) => {
                writer.tag("registration");
                writer.callee(method.callee.into());
                writer.ty(&mir::Type::Interface(method.interface));
                writer.number(u64::from(method.slot));
                writer.expr(receiver);
                writer.optional_local(*destination);
            }
        }
        writer.number(site.pending.0.len() as u64);
        for transfer in &site.pending.0 {
            match transfer {
                CleanupTransfer::Fallthrough(target) => {
                    writer.tag("fallthrough");
                    writer.block(target.block());
                }
                CleanupTransfer::ReturnUnit => writer.tag("return-unit"),
                CleanupTransfer::ReturnValue(value) => {
                    writer.tag("return-value");
                    writer.local(*value);
                }
                CleanupTransfer::Break(target) => {
                    writer.tag("break");
                    writer.block(target.block());
                }
                CleanupTransfer::Continue(target) => {
                    writer.tag("continue");
                    writer.block(target.block());
                }
                CleanupTransfer::ManagedThrow { exception, unwind } => {
                    writer.tag("throw");
                    writer.local(*exception);
                    writer.optional_block(unwind.map(|target| target.block()));
                }
            }
        }
    }
    writer.number(positions.len() as u64);
    for position in positions {
        writer.scalar(&position.kind);
    }
    writer.stream.finalize().into()
}

struct Writer<'a> {
    module: &'a mir::Module,
    body: &'a mir::Body,
    stream: CanonicalHashStream,
    blocks: HashMap<mir::BlockId, u64>,
    locals: HashMap<mir::LocalId, u64>,
    exceptions: HashMap<mir::ExceptionStateId, u64>,
}

impl Writer<'_> {
    fn bytes(&mut self, bytes: &[u8]) {
        self.stream
            .update_byte_span(bytes)
            .expect("in-memory profile input fits u64");
    }

    fn tag(&mut self, tag: &str) {
        self.bytes(tag.as_bytes());
    }
    fn number(&mut self, value: u64) {
        self.stream.update_raw(&value.to_le_bytes());
    }
    fn wire(&mut self, value: &impl WireEncode) {
        self.stream
            .update_canonical_cbor_span(value)
            .expect("typed MIR identities encode canonically");
    }

    // Only closed scalar operations with no arena references use their Rust
    // names. Structural MIR and all references have explicit projections.
    fn scalar(&mut self, value: &impl std::fmt::Debug) {
        self.tag(&format!("{value:?}"));
    }

    fn block(&mut self, block: mir::BlockId) {
        self.number(self.blocks[&block]);
    }

    fn optional_block(&mut self, block: Option<mir::BlockId>) {
        self.number(u64::from(block.is_some()));
        if let Some(block) = block {
            self.block(block);
        }
    }

    fn local(&mut self, local: mir::LocalId) {
        let next = self.locals.len() as u64;
        let fresh = !self.locals.contains_key(&local);
        let index = *self.locals.entry(local).or_insert(next);
        self.number(index);
        self.number(u64::from(fresh));
        if fresh {
            let definition = &self.body.locals[local];
            self.number(u64::from(definition.mutable));
            self.ty(&definition.ty);
        }
    }

    fn optional_local(&mut self, local: Option<mir::LocalId>) {
        self.number(u64::from(local.is_some()));
        if let Some(local) = local {
            self.local(local);
        }
    }

    fn exception(&mut self, state: mir::ExceptionStateId) {
        let next = self.exceptions.len() as u64;
        let index = *self.exceptions.entry(state).or_insert(next);
        self.number(index);
    }

    fn statement(&mut self, statement: &mir::Statement) {
        use mir::StatementKind as S;
        match &statement.kind {
            S::ProfileCounter(_) => {}
            S::Expr(value) => {
                self.tag("expr");
                self.expr(value);
            }
            S::Call(effect) => {
                self.tag("call");
                let call = match effect {
                    mir::CallEffect::Unit(call) => {
                        self.tag("unit");
                        call
                    }
                    mir::CallEffect::Value { destination, call } => {
                        self.tag("value");
                        self.local(*destination);
                        call
                    }
                };
                self.call(call);
            }
            S::ValDecl { local, init } => {
                self.tag("declare");
                self.local(*local);
                self.expr(init);
            }
            S::Assign { local, value } => {
                self.tag("assign");
                self.local(*local);
                self.expr(value);
            }
            S::GlobalAssign { global, value } => {
                self.tag("global-assign");
                self.global(*global);
                self.expr(value);
            }
            S::PublishReleaseReady { class, receiver } => {
                self.tag("release-ready");
                self.ty(&mir::Type::Class(*class));
                self.expr(receiver);
            }
            S::ArraySet {
                array_type,
                array,
                index,
                value,
            } => {
                self.tag("array-set");
                self.ty(&mir::Type::Class(*array_type));
                self.expr(array);
                self.expr(index);
                self.expr(value);
            }
            S::FieldSet {
                object,
                index,
                value,
            } => {
                self.tag("field-set");
                self.expr(object);
                self.number(u64::from(*index));
                self.expr(value);
            }
            S::AtomicFieldStore {
                kind,
                object,
                index,
                value,
            } => {
                self.tag("atomic-field-store");
                self.scalar(kind);
                self.expr(object);
                self.number(u64::from(*index));
                self.expr(value);
            }
            S::Eh(operation) => match operation {
                mir::EhStatement::LandingPad { cleanup, state } => {
                    self.tag("landing-pad");
                    self.number(u64::from(*cleanup));
                    self.exception(*state);
                }
                mir::EhStatement::BeginCatch { state } => {
                    self.tag("begin-catch");
                    self.exception(*state);
                }
                mir::EhStatement::EndCatch => self.tag("end-catch"),
                mir::EhStatement::Forward {
                    source,
                    destination,
                } => {
                    self.tag("forward");
                    self.exception(*source);
                    self.exception(*destination);
                }
            },
        }
    }

    fn terminator(&mut self, terminator: &mir::Terminator) {
        match terminator {
            mir::Terminator::Goto(target) => {
                self.tag("goto");
                self.block(*target);
            }
            mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                self.tag("branch");
                self.expr(cond);
                self.block(*then_block);
                self.block(*else_block);
            }
            mir::Terminator::Return { value } => {
                self.tag("return");
                self.expr(value);
            }
            mir::Terminator::Throw { exception, unwind } => {
                self.tag("throw");
                self.expr(exception);
                self.optional_block(*unwind);
            }
            mir::Terminator::Rethrow { unwind } => {
                self.tag("rethrow");
                self.optional_block(*unwind);
            }
            mir::Terminator::Resume { state } => {
                self.tag("resume");
                self.exception(*state);
            }
            mir::Terminator::Trap { message } => {
                self.tag("trap");
                self.tag(message);
            }
            mir::Terminator::Unreachable => self.tag("unreachable"),
        }
    }

    fn call(&mut self, call: &mir::Call) {
        use mir::CallTarget as T;
        match &call.target {
            T::Direct(callee) => {
                self.tag("direct");
                self.callee(*callee);
            }
            T::Virtual {
                callee,
                slot,
                receiver,
            } => {
                self.tag("virtual");
                self.callee((*callee).into());
                self.number(u64::from(*slot));
                self.expr(receiver);
            }
            T::Interface {
                callee,
                interface,
                slot,
                receiver,
            } => {
                self.tag("interface");
                self.callee((*callee).into());
                self.ty(&mir::Type::Interface(*interface));
                self.number(u64::from(*slot));
                self.expr(receiver);
            }
            T::Closure {
                function_type,
                receiver,
            } => {
                self.tag("closure");
                self.signature(*function_type);
                self.expr(receiver);
            }
            T::NativePointer {
                function_type,
                receiver,
                call_mode,
            } => {
                self.tag("native-pointer");
                self.signature(*function_type);
                self.scalar(call_mode);
                self.expr(receiver);
            }
            T::FunctionBridge {
                function_type,
                receiver,
            } => {
                self.tag("function-bridge");
                self.signature(*function_type);
                self.expr(receiver);
            }
        }
        self.number(call.args.len() as u64);
        for argument in &call.args {
            self.expr(argument);
        }
    }
}
