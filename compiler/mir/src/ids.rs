//! Stage-local MIR entity ids.

use super::*;

pub type FunctionId = Idx<Function>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type GlobalId = Idx<Global>;
pub type InitializationUnitId = Idx<InitializationUnit>;
pub type InitializationFailureRootId = Idx<InitializationFailureRoot>;
pub type ObjectId = Idx<ObjectDef>;
pub type ObjectTypeId = Idx<ObjectType>;
pub type SingletonValueId = Idx<SingletonValue>;
pub type SingletonPublishedRootId = Idx<SingletonPublishedRoot>;
pub type CallbackBridgeId = Idx<CallbackBridge>;
pub type ForeignCallbackAdapterId = Idx<ForeignCallbackAdapter>;
pub type ForeignCallbackFamilyId = Idx<ForeignCallbackFamily>;
pub type ForeignCallbackBridgeId = Idx<ForeignCallbackBridge>;
pub type FunctionTypeId = Idx<FunctionType>;
pub type ClosureClassId = Idx<ClosureClass>;
pub type ClosureInvokeFunctionId = Idx<ClosureInvokeFunction>;
pub type ClosureAdapterId = Idx<ClosureAdapter>;
pub type DynamicClosureAdapterId = Idx<DynamicClosureAdapter>;
pub type ImportedCoreCallableUseId = Idx<ImportedCoreCallableUse>;
pub type MonomorphizedFunctionId = Idx<MonomorphizedFunction>;
pub type StringConstId = Idx<StringConst>;
pub type StructId = Idx<StructDef>;
pub type EnumId = Idx<EnumDef>;
pub type ClassId = Idx<ClassDef>;
pub type InterfaceId = Idx<InterfaceDef>;
pub type LocalId = Idx<Local>;
pub type BlockId = Idx<BasicBlock>;
pub type CoroutineFunctionId = Idx<CoroutineFunction>;
pub type CoroutineStepId = Idx<CoroutineStep>;
pub type CoroutineSlotId = Idx<CoroutineSlot>;
pub type CoroutineFrameId = Idx<CoroutineFrame>;
pub type CoroutineResumePointId = Idx<CoroutineResumePoint>;
pub type CoroutineSavedValueId = Idx<CoroutineSavedValue>;
pub type CoroutineFailureValueId = Idx<CoroutineFailureValue>;
