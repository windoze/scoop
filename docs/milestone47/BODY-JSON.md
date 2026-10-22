# M47 逻辑正文 JSON

本文件细化 [JSON 基础编码](JSON.md)与[协议](PROTOCOL.md)的正文读取。声明、类型、数值常量和来源使用 JSON 基础编码；数组保持原有语义顺序。可选项用 null，空数组仍是实际存在的空表。正文结果的 Available／PendingGeneration／NotPublished／NoBody 由查询外围表达，不能用空正文代表后三种状态。

## 根与局部作用域

普通 callable 正文保存 `owner`、`locals`、`parameters`、`statements`、`result`、`effects`、`typeParameters`、`predicates`、`origin`、`captureTypes` 和 `contextParameters`。参数是本正文实际 local selector，context 参数独立保留名称与类型；局部表每项保存 selector、valueType、mutable 和 definition。definition 区分 source（带 origin）与 synthetic。

默认值保存 `key: {owner, parameter}`、`definitionRoot`、`definitionPath`、`locals`、`body: {statements, value}`、`result`、`allowsSuspend`、`typeArguments`、`receiver`、`previousParameters` 和 `origin`。key 是当前参数接口，definitionRoot 是原提供方，两者在继承时可以不同。receiver 保存 local 与 valueType；此前参数保存 position 与 local。

local selector 是带 kind 的闭合结构：this、enclosingInstance、capturedEnvironment 无附加字段；parameter 带 index；localDeclaration、boundReceiver 带 path；suspensionResult 带 site；synthetic 带 path 与 role。path 是 `{role, ordinal}` 数组，保留原结构位置，不能解释为源码 offset 或 arena 下标。绑定的 owner 与 scope 随原 capture 关系保留。

应用后的默认值保持相同节点结构、声明引用和来源；所有属于原提供方的类型按真实实参投影，typeArguments 保存本次实参。局部泛型函数的 functionType 使用新引入的 binder 层，其 capture 使用外层作用域。requirement 的 implementation 片段使用自己的定义类型；其 typeArguments 单独按外层应用投影，不能直接用外层 binder 映射改写片段定义。

## 表达式与语句

表达式为 `{operation, resultType, origin, evaluationOrigin}`，语句为 `{operation, origin}`。operation 使用显式 kind，与以下字段组成封闭结构；不发送 Rust Debug 名称或 CBOR tag。嵌套表达式、语句、模式和目标继续使用本文件的完整结构。

- 字面量：integerLiteral、floatLiteral 的 value 使用基础数值对象；booleanLiteral 的 value 为布尔值，charLiteral 为单个 Unicode 标量字符串，stringLiteral 为原字符串并另带 owner。unitLiteral、noneLiteral、companionValue、unreachable 无附加字段；tupleLiteral 与 arrayLiteral 保存 elements。
- 值与引用：local 保存 selector；capture 保存 index；globalRead 保存 property；singletonValue 保存 object；annotationKey 保存 annotation；genericDelegateStorageRead 保存 delegate；contextLookup 保存 declaration、parameter 与 diagnostic。
- 聚合：structInit／classInit 保存 constructor 与 arguments；structConstruct 保存 ownerType 与 fields；variantConstruct 保存 variant 与 arguments；variantTest／variantPayloadProject 保存 operand 与 variant／field。fieldAccess 保存 receiver 与 field；releaseFieldLoad 保存 ownerType 与 declaration。
- 调用：call 保存 callee、arguments 与 receiver 的静态类型；localFunctionCall 另保留 declaration 和 captures；callableCall 保存 callee、functionType 与 arguments；methodCall／directSuperMethodCall 保存 receiver、callee 与 arguments。functionAddress 保存 declaration。
- 词法 callable：lambda、anonymousFunction、callableReference 保存 callable 描述。描述保留实际 body／invoke 引用、definitionPath、functionType、captures 与 ownerTypeParameterCount；lambda／anonymousFunction 另有 bodyTypeArguments，callableReference 另有已绑定 target。
- 转换：functionCoercion 保存 source、sourceFunctionType、targetFunctionType；isInstance／cast 保存 operand、checkedType，cast 另有 optional。box、unbox、upcast、someWrap、isSome、ptrFromNonZeroULong、ptrToULong、charCode、charFromCodeUnchecked、ptrCast、arrayLen、arrayClone、atomicNew 保存 operand；unwrap 另有 trapOnNone。sizeOf／alignOf 保存 valueType。
- 运算：binary、primitiveBinary、floatBinary 保存 operator 与 lhs／rhs；unary、primitiveUnary、floatUnary 保存 operator 与 operand；float 运算另有 floatKind。integerOperation 保存完整 operation，区分 unary／binary 及实际整数类型。integerConversion 保存 sourceKind、targetKind、operand；floatConversion 保存 conversion 与 operand。
- 存储：ptrLoad／ptrStore 保存 pointer 与可选 offset，store 另有 value；ptrOffset 保存 pointer、offset、subtract；addressOf 保存 place。index／arraySet 保存 access、receiver、index，arraySet 另有 value；arrayGenerate 保存 count 与 initializer；arrayAssembly 保存 assembly（elementType、resultType、按顺序的 element／spread parts）。
- 原子与未初始化存储：atomic 保存 expression（object、valueKind 与完整 operation）；load、store、rmw、compareExchange 保留实际值、操作、成功／失败内存序与返回类别。maybeUninit 保存 operation，区分 uninit、initialized、assumeInit。foreignCallbackRegister 保存 registration、closure；foreignCallbackOperation 保存 operator、callback。
- 语句：expr、return、throw 保存 value；valDecl 保存 pattern、init；assign 保存 target、value；if 保存 condition、thenBody、elseBody；while 保存 loopKind、conditionSetup、condition、body；break／continue 保存 target（实际词法循环深度）。contextScope 保存 value、body；initializationEnsure 保存 unit；genericDelegateEnsure 保存 delegate；localFunction 保存 function；memberRequirementCall 保存 contract、binding、arguments、result。when／try 分别保存 when／try 描述。

## 模式、控制流与调用关系

模式 kind 为 binding（local）、wildcard、literal（value、equality、subjectType）、variant（variant、fields）、tuple（elements）、struct（ownerType、fields）；字段模式保留 index 与 pattern。literal equality 保留 char／integer／float／ordinary 类别以及所需类型或真实 callable。

when 保存可选 subject、有序 arms 和 fallback。每个 arm 保存 condition、可选 guard、body、origin；condition 为 case（pattern）、predicate（guard）或 always。guard 保存 setup 与 condition；fallback 为 fallthrough、else（body）、irrefutableArm／patternMatrix（subjectType）、enumPatternMatrix（subjectType、ownerType）。try 保存 body、catches 与可选 finallyBody；每个 catch 保存 local、valueType、body、origin。

callable 应用保存 declaration、可选 owner、typeArguments 与 requirements。method callee 为 callable、bound（receiverType、source、instantiatedSignature）或 derivedEquality（ownerType）；bound source 区分 class callable 与 interface member，保留实际 bound。构造器、variant、字段及 tuple index 均保留原 typed 引用／所属应用。赋值目标和 addressOf 仅表达原 HIR 已允许的可寻址形式。

requirement binding 保留原 declaration／parameter／requirement 引用，value 区分 forwarded 与 implementation。implementation 保存原片段的 locals／statements／results、输入位置和本次类型实参。捕获保存 source、valueType、firstUseOrigin 和 binding；source 区分实际 local 与 enclosingCapture，binding 区分当前 source 与明确的定义 owner／scope／selector／definition。读取这些关系不展开另一正文，也不重新进行名称绑定。
