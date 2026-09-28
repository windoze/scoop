# M23-7 实现进展

目标：完成 [M23-7 设计](DESIGN.md) 的全部能力与完成门；每个功能通过验证后提交，定期清理构建目录。当前阶段为实现中，本文件不替代最终验收。

## 2026-09-27：ODR member 摘要身份

- `DigestOwnerAndRoleKey` 使用 `OdrMemberDefinition(OdrMemberId)`，owner tag 为 11；`DigestKind::OdrDefinition` 仍为 8。旧 group owner tag 8 在读取时拒绝。
- digest reader 查询实际 member 身份，不能用同字节的 group 身份满足引用；同组不同 member 产生不同 node 身份。
- LIR identity-foundation 升至 `/2`，现有完整 profile 的必需 section、兼容摘要和固定向量同步更新，旧 `/1` 产物与缓存需要重建。现有 Strong profile 仍拒绝 ODR 定义，完整 generic profile 在实际模板与对象生产接通后切换。
- 已执行 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets`，均通过。
- `cargo test -q -p scoop-identity -p scoop-lir -p scoop-slib` 通过：333 项 identity、449 项 LIR、574 项 slib，以及 1 项 doc-test，共 1357 项，无失败或忽略。

本项完成的是摘要的 typed owner 与编码迁移。逐 member ABI/definition 内容计算、ODR 对象发射与合并仍须在后续实现中完成，不能据此认定 ODR 能力已经可用。

## 2026-09-27：泛型 callable 正文基础表示

- 新增独立的 `ExportGenericCallableBodyV1`，保存原 typed callable owner、局部值、实际参数顺序、带显式 `return` 的语句正文、返回类型、effects、binder 映射、既有条件约束与定义位置。表达式和语句复用默认值已有节点，不伪造默认参数 owner 或尾部返回表达式。
- 新增正文的索引编码、typed 引用读取与按 owner 排序的 canonical 表；读取拒绝缺失的 owner、越界或重复的参数索引，以及重复或非规范顺序的正文记录。extern/intrinsic implementation 不能携带 Scoop 正文。
- 增加 10 项格式与引用测试，覆盖泛型返回正文往返、参数 ABI 顺序、两组 binder 与条件约束、必要字段、外来 owner 解析失败和非法正文表；沿用既有模板节点测试工具。
- 已执行 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets`；`cargo test -q -p scoop-hir -p scoop-slib` 通过 881 项 HIR 和 574 项 slib 测试，共 1455 项，无失败或忽略。

本项是正在接入的 HIR 数据表示，尚未进入正式共有 section 的生产、读取或消费方具体化路径；构造初始化与 delegate template 也仍须实现。当前 profile 未据此宣告泛型能力可用，后续必须以真实源码和 `.slib` 消费完成验证。

## 2026-09-27：真实源码泛型正文进入正式产物

- 正式共有 HIR section 的必需 field 11 保存 callable body 表。源码投影复用默认值已有节点与遍历，保留函数、宿主与方法 binder、显式返回、effects、条件约束和定义位置；abstract、intrinsic、extern 不填写空正文。
- 支持声明与正文在同一工作队列中收集；每个实际正文只投影一次。真实引用的 private generic helper、泛型名义类型及属性进入支持声明，参数自由 helper 只保留声明，未引用的 private generic helper 不导出。
- lambda/anonymous function 具有定义序捕获类型表，共享 expression tag 59 读取该表；嵌套捕获来源区分当前局部值和外层捕获。局部函数沿用真实前置捕获参数，产物不保存 request-local BindingId。读取拒绝越界捕获、缺失捕获输入和未知来源种类。
- 泛型正文的完整定义位置和实际外来引用进入原有 canonical 表；`TemplateDependency` 使用 role tag 10，不产生消费方 lookup observation。默认值继续使用自己的根与访问规则。
- `hir/cross-cone-interface` 升至 `/31`；required inventory、reader、profile 指纹及固定向量同步迁移，旧 `/30` 产物与缓存需要重建。后续实际调用记录使用 `/32`，构造、委托模板按设计分别增加 field 12、13，并使用 `/33`、`/34`。
- 增加独立、组合、private 支持三份真实源码 fixture 与 HIR golden；覆盖泛型函数、扩展、宿主和方法两组 binder、局部函数、嵌套闭包、私有类型与属性。实际 driver 发布三份 `.slib` 后，经正式 Compile/Link reader 验证并读取上述正文和支持声明。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过。885 项 HIR、1239 项 HIR lowering、574 项 slib 测试及 1 项实际产物集成测试通过，共 2699 项；源码 golden 在关闭更新选项后另行复验通过。
- 确认构建与测试结束后执行 `cargo clean`，删除 2686 个构建文件，回收 5.0 GiB。

本项完成源码 → 共有 HIR → `.slib` 发布与读取。消费方具体化、构造初始化与 delegate template、逐 member ODR 定义和跨 Cone 链接运行仍在后续主线，当前 Strong profile 没有据此宣告泛型机器能力完成。

## 2026-09-27：模板私有具体类型贯通布局与实际发布

- Export HIR 输出现在保留一次收集完成的共有声明与泛型正文闭包。LocalConcrete 的 shape support、HIR 类型语义和正式共有 section 复用同一结果；普通依赖的已选记录在收集前传入，避免从公开 binding 子集重新收集而丢掉模板专用支持类型。
- 删除已无调用的旧名义类型根入口。该变更使用现有 typed 声明与模板节点，不新增 wire 字段、profile、授权关系或另一套类型检查。
- 新增 `concrete-support.scoop`：泛型正文直接构造私有 `HiddenValue`，并调用返回该类型的私有参数自由 `seed`。只有实际需要的 `HiddenValue` 进入共有支持声明和 shape roots，未使用的 `UnusedValue` 不进入；公开 binding 仍只有泛型函数。
- 新 fixture 保存 HIR/MIR/LIR golden。MIR 具有真实 helper、构造正文和必要 shape support，LIR 具有对应 ABI、layout 与 descriptor。driver 的四份泛型正文 fixture 均发布真实 `.slib` 后使用完整 layout Compile/Link reader，包含对象与符号关系检查；新 fixture 的私有类型表示读取成功。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过。885 项 HIR、1240 项 HIR lowering、114 项 MIR lowering 和 1 项实际产物集成测试通过，共 2240 项；新增及既有 HIR golden 随回归复验，MIR/LIR golden 在关闭更新选项后复验通过。
- 构建与测试结束后执行 `cargo clean`，删除 2463 个构建文件，回收 3.6 GiB。

本项修复模板支持类型在机器产物生产中的缺口。外来泛型调用的消费方具体化、`TemplateSupportHidden` 的完整生产选择、ODR 和后续主线仍需继续，不据此宣告跨 Cone 泛型调用已可运行。

## 2026-09-27：消费方泛型 callable 进入共有具体化队列

- 依赖候选目录提供原 callable body。消费方用独立的 imported template、符号化 application 和 concrete function ID 保存三个阶段，不为外来声明分配当前 Cone 的 `FunctionId`。完整正文在 HIR 输出前完成，同一原声明与类型实参通过既有具体化队列去重，支持正常递归。
- 真实源码覆盖显式实参、隐式推断、消费方本地 struct、当前泛型转发到外来泛型、private generic helper、泛型扩展、前置参数默认值及默认值内泛型调用。正文和默认值共用节点替换；正文求值位置保持 provider 定义位置，默认值仍保持独立的使用位置。
- 候选使用既有约束求解和 MSC。导入候选的声明类型在同一个比较作用域中解析，避免用不同候选的 arena 索引比较；落选候选不提交模板。独立重载 fixture 验证元组声明优先，歧义诊断保留原类型参数、参数名和签名。
- 导入声明保留 kind/nominal bound、NoGC 与 GC-free pointee 条件。普通 Cone 通过 core 的实际 typed role 解析 `Ptr`/`FunPtr` 类型；组合 fixture 验证泛型指针参数和返回值、NoGC 调用及违反约束的消费方诊断位置。原 core 测试中“省略类型实参必报错”的阶段性断言改为显式实参数量错误。
- 新测试将实际 provider 源码投影并读取为依赖声明，完成 consumer HIR → canonical identity foundation → MIR，保存 HIR/MIR golden。已有四份 provider 的真实 `.slib` 发布和完整 reader 回归通过；本项尚未通过正式 driver 运行泛型消费方。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。885 项 HIR、1245 项 HIR lowering、114 项 MIR lowering 和 1 项实际产物集成测试通过，共 2245 项；HIR lowering 的旧断言更新后已定向复验，golden 在关闭更新开关的回归中验证。
- 构建与测试结束后执行 `cargo clean`，删除 2513 个构建文件，回收 4.3 GiB。

本项完成普通泛型 callable 的 HIR/MIR 消费基础。postponed 实参、完整模板节点、名义宿主与方法 binder、外来 generic call 的共有调用记录、真实 ODR 机器产物及其余完成门继续实施；当前 Strong profile 仍不承载泛型消费方产物。

## 2026-09-27：泛型实际调用记录保留 application 与定义位置

- 参数自由调用与泛型调用共用一次 executable occurrence 遍历、绑定记录与定义位置投影。泛型调用从具体函数的 materialization 取得既有 canonical application，不复制原声明、binder、实参或签名。
- 实际 call-site 增加必需 field 8，区分 Direct 与 Application。reader 连接 provider 的原声明、宿主和 callable 两组实参，复用既有签名类型替换后检查每个逻辑参数及结果；type-use 收集复用已检查的实际类型，不再次处理未替换 binder。
- 实例化正文中的求值位置按实际 provider 的模板定义检查；普通消费方调用和默认值展开继续保留自己的求值位置。当前 Cone、provider 与 core 的 identity graph 沿正式 reader 的依赖导入方式连接。
- `hir/cross-cone-interface` 升至 `/32`，required inventory、格式固定向量与 profile 指纹同步更新；旧 `/31` 及更早产物需要重建。构造和委托模板的后续版本顺延为 `/33`、`/34`。
- 新增真实源码生产与读取测试，覆盖显式调用、隐式推断、消费方本地 struct、默认值调用和嵌套模板调用；检查 application 往返、替换后签名、provider 定义位置，以及缺失 application、错配原声明和旧格式缺失字段等错误。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。886 项 HIR、1246 项 HIR lowering、114 项 MIR lowering、574 项 slib 和 1 项实际发布/读取集成测试通过，共 2821 项；源码 golden 在关闭更新开关时复验。
- 确认构建与测试进程结束后执行 `cargo clean`，删除 2618 个构建文件，回收 4.7 GiB。

本项完成泛型实际调用的共有 HIR 生产与读取合同。MIR/LIR 的统一物化、ODR 对象与单 image 链接运行继续实施，当前 Strong profile 仍不承载泛型消费方产物。

## 2026-09-27：共有 MIR 机器输入保留泛型 callable

- `ConeMirInput` 直接消费 `DependencyMirOutput` 已持有的唯一 canonical foundation，删除额外传入另一份 foundation 的接口及配对检查。物化根保留完整 `CallableSignatureSubject`，Strong 与 ODR 函数进入同一套 MIR 到 LIR lowering。
- core bootstrap bridge 继续保存参数自由 Strong callable 子集，生产与读取均按该子集核对覆盖；ODR 的 application、member 与逻辑签名沿原 canonical 表保存。普通源码导出、构造、派发、初始化服务查询选择实际 Strong 目标，不再因同模块存在泛型函数而拒绝。
- driver 的共有 MIR 输入不提前施加历史 Strong profile 限制。历史 Strong 产物组装仍在发布边界检查 MIR 的 ODR 限制，既有 reader 保持相同限制；生成类型与泛型初始化单元的完整物化仍属于后续实现。
- 真实 provider/consumer fixture 的 11 个 ODR callable 根贯通共有机器输入，并与每个实际函数一一对应。core、provider、consumer 沿正式依赖 identity graph 的导入方式完成 MIR foundation 和 bridge 序列化往返；缺失实际 Strong bridge 仍被拒绝。
- 补齐 HIR 接口 `/32` 迁移后的 43 份实际产物指纹快照。逐份核对仅 `ArtifactFingerprint` 改变，`CodeFingerprint`、`RuntimeImageFingerprint` 及其余对象、符号和依赖内容保持；两项阶段性能力诊断同步到仍未接通的 ODR generated shape 边界。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。1247 项 HIR lowering、337 项 MIR、114 项 MIR lowering、137 项 LIR lowering 和 574 项 slib 测试通过，共 2409 项，无失败或忽略。
- 27 项 driver 实际产物回归通过，包含导入类、primitive、core、属性初始化、布局、ABI、派发、依赖 shape、完整 Compile/Link reader 及源码编译链接运行；26 项受快照或诊断变更影响的测试在关闭全部更新开关后复验通过。本批共验证 2436 项测试。
- 确认编译与测试进程结束后执行 `cargo clean`，删除 2456 个构建文件，回收 3.4 GiB。

本项完成泛型 callable 的共有 MIR 输入与读取合同。LIR 完整定义、ODR 对象与正式泛型消费方产物仍需继续，不能据此认定跨 Cone 泛型链接运行已经完成。

## 2026-09-27：共有 LIR 保留 ODR 定义与异常控制流

- `ConeLirOutput` 与 `ConeLirFoundation` 保留实际 producer 和唯一 canonical foundation。源 callable 的实际 group/member 沿 MIR 传入，LIR 不重复发布源 member；实际 callable、safepoint 及其它物理 registration 在所属 group 中生成各自 member，物理定义直接使用对应的 Strong/ODR plan。
- trap C string、runtime scan 和 EH/stackmap 关联 atom 使用实际 body plan 与稳定局部路径。primary 沿用原 kind-specific symbol，ODR 符号及其关联边界保留 `OdrWeak`。拆出物理定义投影及其局部写入状态，替换原只生成 Strong 的分支。
- 普通 callable bridge 只检查自身导出项的 body、符号、definition plan 和 primary atom，不重建整张 Strong symbol 表。共有 foundation 从已验证数据构造时复用已完成的归属检查；历史 Strong 产物的写入和读取边界继续拒绝 ODR。
- driver 的物理依赖选择从实际 MIR 类型、callable、dispatch 和初始化 unit 引用取得需求，消费同一批完整依赖产物；去掉对整个参数自由类型导出 section 的先行依赖。生产路径与泛型 LIR 回归复用该函数，没有补造 descriptor 或省略实际引用。
- 泛型正文和默认值补齐共用的 `try`、顺序 `catch`、`finally` 与 `throw` 展开，保留原定义位置和 catch local selector；后续异常与 GC lowering 使用既有实现。
- 实际集成回归先发布 provider `.slib`，移走 provider 源码，再构造基础调用、异常控制流和两个组合消费方。组合 fixture 具有七个 ODR callable，覆盖含 `String` 字段的本地 struct、indirect/sret、GC roots、泛型转发、捕获异常、finally、重新展开和默认值控制流；不同消费方共有的四个实例具有一致的 body、plan、registration 与关联 atom 身份。
- 三组源码 fixture 保存 HIR/MIR/LIR golden，LIR foundation 通过 core/provider/current 的实际 identity graph 完成读取往返。独立 MIR trap 回归检查两个 producer 的同一 ODR body 生成相同的关联常量与弱符号。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。1247 项 HIR lowering、337 项 MIR、449 项 LIR、138 项 LIR lowering、301 项 codegen/runtime、574 项 slib 和 27 项 driver 集成测试通过，共 3073 项；LIR/slib 读取复用调整后已复验，九份新 golden 在关闭更新开关后验证，25 项既有布局产物测试全部通过。
- 确认编译与测试进程结束后执行 `cargo clean`，删除 2672 个构建文件，回收 4.2 GiB。

本项完成泛型 callable 的共有 LIR 输出与物理定义身份。完整 ODR production/digest、对象发射、正式泛型消费方产物和单 image 链接运行仍需继续；普通整数除法所需的 `ArithmeticException`/`Option<String>` 表示也保留在泛型名义类型主线，不能据此宣告 M23-7 完成。

## 2026-09-27：共有 production 与真实 ODR callable 对象

- 生产定义、符号、registration 和摘要查询复用 foundation 中已解析的 typed subject 与实际 Strong/ODR plan。源 callable 不重复发布 member 或解码 key；物理符号表拒绝不同 member 认领同一个 primary symbol。
- 逐 member digest graph 接通自身 LIR、对象和所属 stackmap leaves。ODR registration 的对象节点依赖实际写入记录的 body/stackmap 字段，image 汇总实际 Strong/ODR registration 节点；普通调用不递归加入被调用者摘要。
- 既有对象分区与发射支持 ODR callable，实际定义使用 `weak_odr`，跨对象的必需声明保持普通 LLVM external declaration。callable/safepoint registration 写入真实 group/member，Strong image table 使用 registration 的实际符号请求。
- LLVM 22 的 Mach-O weak alias 会变成局部弱引用，因此沿既有 native atom-boundary 物化步骤写入正确的 external weak-definition 标志；primary、runtime scan、常量、EH、LSDA 与 stackmap 的边界及非空范围均在真实对象中验证。
- 共有接口统一为 `ConeProductionSection`、`ObjectSymbolSurface`、`DigestFinalizationPlan` 和 `EmittedConeObjectSet`，原实现模块随之移动，所有生产与读取调用方同步；历史 Strong profile 及其拒绝 ODR 的发布/读取边界保留原有含义，本项没有提前切换正式 wire profile。
- 实际回归先发布 provider `.slib` 并移走源码，四个 consumer 经共有 HIR/MIR/LIR 生成真实 Mach-O 对象。基础、异常控制流与含引用 struct 组合验证弱符号、registration 字段、indirect/sret、GC roots、Invoke/LSDA/stackmap，以及不同 consumer 的共享实例生成完全相同的独立函数对象 bytes。
- 完整 production section 使用实际 core/provider 定义和依赖布局完成读取往返，再编码与原 bytes 相同；读回 digest graph 与原计划相同。九份 HIR/MIR/LIR golden 在关闭更新开关时验证。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。450 项 LIR、138 项 LIR lowering、301 项 codegen/runtime、574 项 slib，以及 2 项泛型产物与 25 项既有布局产物回归通过，共 1490 项，无失败或忽略。
- 确认编译与测试进程结束后执行 `cargo clean`，删除 2603 个构建文件，回收 4.6 GiB。

本项完成共有生产计划、临时 ODR 对象与 production section 读取。canonical LIR 内容摘要、逐 member ABI/definition 数值、最终补丁回填、正式泛型 profile、对象 reader/合并和单 image 链接运行仍需继续；临时对象中的最终摘要槽依合同保持零，不能据此认定 M23-7 完成。

## 2026-09-27：共有机器对象 reader 识别 ODR 定义

- Mach-O reader 保留 external section definition 的 Strong/weak 标志，并与实际 symbol plan 的 linkage 逐项核对；拒绝局部 weak definition、weak undefined reference、未计划的定义和 Strong/weak 错配。已有 section、range、padding、relocation 与 patch 检查继续共用。
- LIR symbol plan 保存 foundation 已解析的实际 definition owner，物理符号和 relocation 沿同一记录传递归属。定义 owner 的 ODR 分支与当前产物内的必需引用都使用 `OdrMemberId`，物理 producer 只用于定位对象，不进入 canonical relocation。
- callable/safepoint registration 读取实际 group/member 及对应 digest graph。stackmap 直接查询已验证的 callable 定义与物理 member，不再从当前 producer 重建 Strong definition plan；底层函数地址 relocation 同时接受已识别的 Strong 与 weak 定义。
- `link-identity-closure` 升至 `/4`，defined owner 新增 tag 5、final undefined requirement 新增 tag 9；规范化对象 relocation 的 runtime target 使用新 tag 14。已退役的 requirement tag 2、8 和 runtime target tag 13 不复用。对象 verifier 升至 `scoop-lir/2`，logical unit-set key 保持，required inventory、profile 指纹与格式固定向量同步迁移；历史 Strong profile 仍在语义边界拒绝 ODR。
- 四个真实 consumer 继续使用已发布且移走源码的 provider，经现有对象准备与 registration 读取路径检查基础调用、异常控制流和含引用 struct 组合。实际 ODR owner/requirement 使用正式 wire 编码往返，错配 member 被拒绝；物理符号标志与 stackmap 增加对应正反例。
- 更新 49 份实际产物快照，逐份核对仅 `ArtifactFingerprint` 和 `CodeFingerprint` 改变；`RuntimeImageFingerprint`、布局、符号、依赖与其余内容保持。对象 capability 改变物理 member ID，profile 更新改变产物兼容性指纹，旧缓存需要重建。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。450 项 LIR、576 项 slib 与 87 项 driver 实际产物回归通过，共 1113 项；受快照迁移影响的 25 项布局测试在关闭全部更新开关后复验通过，包含实际源码编译、链接、运行、初始化、异常、ABI 和完整产物读取。
- 确认构建与测试进程结束后执行 `cargo clean`，删除 2471 个构建文件，回收 3.5 GiB。

本项完成共有 ODR 机器对象读取。逐 member canonical LIR、ABI/definition 内容摘要、最终补丁回填、正式泛型 profile 与单 image 链接运行继续实施。

## 2026-09-27：callable 对象摘要覆盖完整关联数据

- callable 的 ObjectDefinition 统一覆盖 primary text、runtime scan、取址常量、LSDA、EH frame、compact unwind 和 LLVM stackmap；关联 atom 按 ID 排序，已有规范化 stackmap 记录继续作为直接输入，不重复解析。
- Mach-O 本地 UNSIGNED section 引用根据实际 section ordinal 与对象内目标地址定位同一声明的 atom，摘要保留 atom 相对 offset 并清除物理地址；本地 label 使用已验证的 atom 归属，其余符号继续使用真实 Strong/ODR typed target 和 relocation。范围外的目标仍拒绝。
- 先计算完整函数正文摘要，再计算 callable registration 对象摘要。ODR 登记保留实际 group/member，代入正文 ObjectDefinition，并把该值列为直接依赖；入口来自真实已验证 relocation。Strong 登记继续使用原有置零字段及独立正文依赖合同。
- 删除提前计算 callable registration 对象叶子的独立入口。driver 和共有 reader 保留同一份已验证登记记录，随正文计算一次产出两类对象叶子。shape 引用分类直接使用实际 consumer 与完整 physical imports，生产端和 reader 共用入口。
- 对象 verifier 升为 `scoop-lir/3`，旧对象与缓存重建；`link-identity-closure/4`、persistent identity 和 runtime C ABI 保持。固定正文/Strong 登记摘要及 capability 所影响的物理 member wire 向量同步更新。
- 四个真实 consumer 使用已发布且移走源码的 provider，比较共享泛型实例的正文及 ODR 登记对象摘要。异常回归分别修改真实 Mach-O 对象中的 LSDA、EH frame 和 compact unwind 非 relocation 数据，确认所属正文与 ODR 登记摘要都改变，其他函数摘要保持。
- 更新 49 份实际产物快照；逐文件断言只有 Artifact、Code 和 RuntimeImage fingerprint 字段改变，布局、符号、依赖及 HIR/MIR/LIR golden 内容保持。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。576 项 slib、116 项 driver library、7 项 CLI 和 1 项真实编译器 capability 测试通过，共 700 项，无失败或忽略；包含源码编译、完整产物读取、链接与运行。 受快照迁移影响的 25 项布局测试在关闭全部更新开关后复验通过。
- 确认构建与测试进程结束后执行 `cargo clean`，删除 2533 个构建文件，回收 3.9 GiB。

本项完成完整 callable ObjectDefinition 及 ODR 登记对象叶子。canonical LIR、逐 member ABI/最终 definition 摘要、补丁回填、正式泛型 profile 与泛型单 image 链接运行继续实施，不能据此认定 M23-7 已完成。

## 2026-09-27：物理名义类型保留完整 exact-type 身份

- 最终 LIR 的 `StructDef` 和 `EnumDef` 必需携带 `PersistentExactTypeId`。普通、C 布局和 intrinsic struct 从 MIR 的实际物理类型记录传入；enum 使用完整 application 的 source/generated exact-type record。后续字段与扫描填写保留同一身份，完整 key 继续复用 `LirMeta.exact_types`。
- 这使 canonical LIR 能从局部表示引用直接取得持久身份；名称、arena 位置和相同布局不承担全局类型判等。已有符号、布局、对象格式和 runtime ABI 保持。
- 新增相同名称及物理布局的 struct/enum 回归，确认它们仍保留不同 exact-type ID。真实 provider `.slib` 在移走源码后，由四个泛型消费方逐项核对 MIR → LIR 身份及完整 key 的保留；九份 HIR/MIR/LIR golden 在关闭更新开关时验证。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。450 项 LIR、139 项 LIR lowering、301 项 codegen/runtime 和 2 项实际泛型产物回归通过，共 892 项，无失败或忽略。

本项补齐 canonical LIR 编码所需的物理名义类型身份。LIR 内容摘要、逐 member ABI/最终 definition 摘要、回填和泛型产物链接运行继续实施。

## 2026-09-28：实际 LIR 函数正文的 canonical 内容摘要

- 新增 `scoop-lir-definition-v1` 正文内容叶子，直接编码最终 `Function` 的 ABI、GC effect、全部 56 类指令、正常/异常 CFG、存储与 root plan。类型、函数、全局存储、桥和 safepoint 使用实际持久身份；调用目标、签名和 root-scan 局部表按使用处的完整值编码，不采用 arena ID、诊断名称、dump 或 LLVM 文本。
- 入口可达块按固定后继顺序编号，local/temp 先按普通定义与使用确定次序，再据此排序根集合。未使用槽和不可达块不进入该语义投影；所有实际对象 atom 仍由既有 ObjectDefinition 验证。具有相同布局的名义类型继续保留不同 exact-type 身份。
- 共有 production section 增加必需 field 13，形成 field 2～9、13 的九字段 product。记录按 callable-body ID 排序，精确覆盖 foundation 的全部实际 Function，包括普通 Strong/ODR 正文、root gateway 和初始化 startup gateway。摘要表顺序独立于 foundation 的引用拓扑顺序，`main` 必须先于 gateway 解析不意味着其 body ID 必须更小。
- producer 从实际最终 LIR 计算正文摘要；reader 检查记录集合、typed 引用、顺序和固定宽度后复用。两种现有 reference schema 的 capability 同步升至 `strong-production/13`、`/14`，必需 inventory、profile 与固定向量更新，旧 `/11`、`/12` 产物和缓存重建。没有新增 runtime ABI 或另一套正文语义检查。
- 增加 10 项独立回归与固定摘要向量，覆盖 arena/块/局部表顺序、诊断名称、操作数与操作、控制流、ABI、exact identity、根集合、异常 liveness、unwind 正文、缺失/重复/未知正文、摘要宽度及 gateway 拓扑顺序。既有实际 lowering fixture 均计算正式内容叶子；真实泛型产物回归在移走 provider 源码后，通过四个消费方比较相同 ODR 正文的规范化摘要并重放正式 reader。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 与 `cargo build -p scoopc` 通过。460 项 LIR、139 项 LIR lowering、301 项 codegen/runtime、576 项 slib 测试通过。driver 库首次 114/116 通过，两项 gateway 排序问题修复后均复验通过；另有 7 项 CLI 单测与 1 项实际编译器能力查询通过，共覆盖 1600 个独立测试，无忽略。
- 49 个既有产物快照只更新 Artifact/Code 摘要，runtime 摘要及其他结构不变；25 项布局/发布组合测试在关闭所有快照更新开关后复验通过。九份泛型 HIR/MIR/LIR golden 全程未启用更新开关并通过。
- 最终格式化与全工作区 lint 再次通过；确认全部构建、测试和编译器进程结束后执行 `cargo clean`，删除 2678 个构建文件，回收 4.7 GiB。

本项完成真实 Function 内容摘要的生产、持久化和读取。其他物理 member 的 canonical LIR 叶子、逐 member ABI/最终 definition 摘要、补丁回填、正式泛型 profile 和泛型单 image 链接运行继续实施，不能据此认定 M23-7 已完成。

## 2026-09-28：callable 与 safepoint ODR 注册摘要及实际回填

- 两类注册记录沿原共有对象生产路径，按实际 `RegistrationDefinitionOwner` 选择 StrongRegistration 或 OdrDefinition。新增独立的 ODR ABI/definition 内容类型与注册摘要结果；没有另设发布器、对象读取路线或 runtime ABI。
- callable registration 的 canonical LIR 覆盖 body 与 typed entry；safepoint registration 覆盖 site、runtime id、owner body、site role 和 root-pair count。ABI 摘要描述注册记录自身的 kind/version/byte size。按 member 汇总自身 primary 的 LIR/object 叶子，仅 safepoint 另含所属 site 的 normalized stackmap；callable 引用的 body 不被误计为 registration 自身的 object leaf。
- ODR safepoint 的 ObjectDefinition 先代入实际 normalized-stackmap 字段并记录对应直接输入，再计算最终 definition；callable 继续复用已有 body-definition 上游值。自身 definition 槽保持零，最终由已有唯一 patch site 回填。
- image 表引用从已验证的实际 primary definition 核对 typed entity、registration role、symbol 与 Strong/ODR owner，修复旧代码一律推导 Strong 目标的问题。RuntimeImage 的记录编码保留真实 linkage/group/member/definition，DAG 输入使用对应 digest kind。原有 relocation 形式、宽度、零 addend、范围与内容检查保持。
- 旧 Strong manifest 注册表明确拒收 ODR 摘要，避免错误套用旧字段或遗漏成员。两种 Strong production 格式、现有摘要向量及 runtime C record 布局不变；完整 ODR member 目录与正式泛型 profile 仍待后续接通。
- 真实泛型 fixture 已走到 registration、RuntimeImage、entry 的最终对象回填；四个消费方比较共同 member 的 ABI/LIR/definition。固定 callable/safepoint 的六个摘要向量并核对实际32-byte补丁。分别改变 LSDA、EH frame、compact unwind 与 stackmap，验证所属 definition 改变，注册 ABI/LIR 保持；stackmap 变化同时传至所属 safepoint 和 callable registration，其他 callable 不变。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和真实 `scoopc` 构建通过。576 项 slib、116 项 driver 库、7 项 CLI 单测和1项实际编译器能力查询通过，共700项，无失败或忽略。两项扩展后的泛型回归先独立运行，再随完整 driver 复验；完整库测试耗时753.52秒，使用本次构建的配套编译器，所有既有 golden 均未修改。
- 最终格式化与全工作区 lint 再次通过且无警告；确认构建、测试和编译器进程全部结束后执行 `cargo clean`，删除2568个构建文件，回收4.2 GiB。

本项接通两类注册 member 的完整数值摘要与实际回填。callable-body member 自身的 ABI/最终 definition、其他物理角色、完整 member 目录及重复定义合并、正式泛型 profile 与单 image 运行继续实施，M23-7 仍未完成。

## 2026-09-28：callable-body member 的 ABI 与最终定义摘要

- ODR callable 的 ABI 从最终 Function 的 GC effect、实际 Scoop ABI 签名、参数/返回值传递方式、物理类型、布局和 scan 计算；与已有 canonical LIR 共用编码，计算 ABI 不遍历正文或 CFG。实际 DispatchAdapter 等角色由 member key 保留。
- 既有 refined callable member 引用在构造/解析时保留 group/role，wire 仍只编码原 member ID。producer 与 reader 从同一已解析 body key 读取这些关系，支持归属上游 IR 的 source member，不把 key 重复发布到 LIR 本地表。MIR 初始化错误的较大 subject 改为盒装字段，避免扩大正常返回值。
- production field 13 的 ODR callable record 必需保存 ABI 摘要，Strong record 保持原两字段；reader 明确拒绝 ODR 缺少 ABI、Strong 额外带 ABI、记录集合和身份不符。经验证的数据以完整 Strong/ODR sum 表达，不在后续阶段补猜。
- 函数成员的最终 definition 汇总已有 canonical LIR、完整 body ObjectDefinition 和按 site 排序的所属 normalized-stackmap leaves。callable 与 registration 复用同一逐 member 编码及结果类型；物理生产和两种 Link reader 都接入共有计算入口。没有再次解析对象/stackmap、递归包含 callee 摘要或增加发布路线。
- callable registration 的 body-definition 字段继续写入 ObjectDefinition，Strong 摘要和注册摘要向量不变。完整函数 member ABI/LIR/definition 已由真实 generic fixture 固定向量，并加入不同消费 Cone 的内容比较；四类 EH/stackmap 内容变化均传至所属函数 member。单独改变持久化 LIR 或 ABI 的回归确认两者各自进入正确的比较字段，未改变的对象、注册和其他函数保持原值。
- 新增五项 LIR 回归并扩展既有 identity 与真实泛型组合测试。333 项 identity、465 项 LIR、337 项 MIR、576 项 slib、116 项 driver 库、7 项 CLI 单测及1项实际编译器能力查询全部通过，共1835个独立测试，无失败或忽略。两项真实泛型测试先独立通过，再随完整 driver 复验；完整库测试耗时755.91秒，使用本次构建的配套编译器。
- 全工作区格式化、lint 和配套编译器构建通过；最终 lint 无警告，Strong 格式版本、注册摘要向量及全部已有 golden 保持不变。确认全部构建、测试和编译器进程结束后执行 `cargo clean`，删除2670个构建文件，回收4.8 GiB。

本项补齐 callable-body 的逐 member ABI/definition。其他物理角色的摘要、完整 member 目录与重复定义合并、正式泛型 profile 和单 image 运行继续实施，M23-7 仍未完成。

## 2026-09-28：物理 ODR 成员目录接入共有产物投影

- `single-cone-production` 升至 `/2`，现有两种生产投影共用必需的 field 11 ODR 目录。原 field 5 只保存六类 registration 的 Strong 子集，field 4 继续保留全部实际注册 identity；ODR registration 和 callable-body 使用同一成员目录。
- 目录从现有最终摘要结果提取 group/member/role/ABI/definition，与已验证对象索引的全部 ODR primary definition 精确核对。重复、多余或缺少摘要的物理成员均拒绝；普通外部引用及纯语义成员不进入目录。没有再次解析对象、重算摘要或复制上游 identity key。
- group 与组内 member 按 ID bytes 排序，已列出的 group 必须非空；Strong 产物使用空目录。reader 拒绝重复、逆序、错误 record shape、摘要宽度、未知 role、GeneratedNominal 和本阶段未启用的 ReleaseHook，并逐项比较已有重建结果中的成员身份、角色及两类摘要。
- Code 的 manifest 投影保留 field 1～6，追加同义 field 11；两种 writer、共有 reader、required inventory、三个 profile 的固定格式/摘要以及 compatibility 向量同步迁移。旧 `/1` 产物与缓存需重建，现有 Strong profile 的语义边界继续拒绝 ODR，runtime C ABI 和 Strong registration 摘要算法保持。
- 同步49份实际产物及 Code golden，逐份核对差异仅为 Code/Artifact 摘要；RuntimeImage 摘要、HIR/MIR/LIR 语义、物理引用和登记内容均未改变。固定向量由原真实 fixture 生成，更新开关关闭后再验证。
- 真实泛型 fixture 从 metadata 回填推进到共有 `PreparedLayoutObjects::finalize`、Code 摘要和最终 manifest 往返验证。四个消费方核对完整实际成员集合与 Strong 注册子集；修改 ABI 或 definition 后，原 Code 结果拒绝该 manifest。原函数/注册摘要固定向量及 EH/stackmap 变化回归继续通过。
- 新增五项目录测试、一项必需字段测试及一项跨 profile 旧版本拒绝测试。关闭全部 golden 更新开关后，全工作区共5143项测试通过，无失败或忽略；包含583项 slib、116项 driver 库测试及两项文档测试。完整 driver 库测试耗时757.88秒，使用本次构建的配套编译器，两项真实泛型回归随全仓再次通过。
- 全工作区 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和 `cargo build -p scoopc` 通过，最终 lint 无警告。确认全部构建、测试和编译器进程结束后执行 `cargo clean`，删除2690个构建文件，回收4.7 GiB。

本项完成当前已发射物理成员的目录和共有生产/读取投影。其他物理角色的摘要、跨产物重复成员合并、正式 generic profile 和单 image 运行继续实施，M23-7 仍未完成。

## 2026-09-28：保存 generic profile 与共有 foundation 迁移进度

- 正式完整 layout 产物沿原有 producer/reader 迁移到 `cross-cone-generic/1`，production section 改为 `cone-production/1`。required inventory 采用已经实现的 payload；后续构造和委托格式按实际功能升级，规范、设计与路线图同步说明这一迁移顺序。
- 共有 HIR/MIR 查询、native ABI、源码位置、类型表示和产物装配改为借用 canonical foundation。reader 共享本次已经验证的数据，泛型产物不再经过 `OdrFree` 包装；历史 Strong 格式仍在自身边界检查原有限制。
- 新增真实 provider、consumer、下游 Cone 的发布与运行回归，复用已有单 image 和 moving-GC harness。实际运行中，provider 发布并移走源码后，standalone 泛型 consumer 已通过正式入口发布，HIR/MIR/LIR 三份现有 golden 均一致。
- 新增测试 `generic_consumers_publish_reusable_artifacts_and_run_with_moving_gc` 仍失败：下游读取 consumer 的 HIR foundation 时，局部值来源校验报告 `MissingSourceAnchor`。当前 reader 只在本产物查找模板声明的来源锚点，后续需要接入已验证的原 provider foundation 查询。combined 分支、真实链接与 moving-GC 执行尚未运行到，不能视为通过。
- profile descriptor/fingerprint 的固定向量和受迁移影响的产物 golden 尚未同步；本批未执行全工作区测试。此前5143项全仓通过的记录仅属于上一批物理 ODR 成员目录变更，不适用于当前提交。
- 提交前 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 均通过，lint 无警告；已保留失败的真实回归及完整后续路径。

本次按用户要求提交当前工作进度。generic profile 迁移和跨 Cone 泛型发布、消费、运行闭环尚未完成，M23-7 仍在实施中。

## 2026-09-28：泛型消费产物的 HIR 来源与声明位置读取

- HIR foundation 的依赖读取入口接收实际可达 provider 的 canonical foundation。实例化局部值的来源锚点先查本地声明，再借用已验证依赖中的原模板记录；不复制外来声明，也不重新解码或验证 provider。layout reader 按原依赖顺序提供这一闭包，原来源缺失、错配和位置范围检查保持。
- 声明类型位置接受泛型函数的真实 application，核对其 typed 原声明与 root 一致，再从当前或依赖的原声明检查 receiver、参数下标和 result。局部值继续保存消费方 materialization 和 provider 源码位置；普通函数、存储和构造位置的原有检查保持。
- 增加外来泛型参数来源的反例，覆盖缺少 provider 锚点、来源被替换和未声明的源码位置；真实泛型消费 fixture 补充完整 HIR foundation 编解码验证，以及缺少声明、错误 application、缺失 application context、非法 receiver 和参数下标检查。
- `cargo fmt --all` 与 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过且无警告。887项 HIR 测试、7项真实泛型消费回归、5项声明位置回归和9项完整 layout reader 回归通过，共908项，无失败或忽略，现有 golden 未修改。
- 真实三 Cone 发布测试已越过此前的 `MissingSourceAnchor` 和声明位置 `Materialization` 错误，当前失败推进到 MIR 调用对接的 `UnmaterializedHirSelection`：读取方仍把泛型调用当作外部 Strong 定义查找。尚未执行到最终链接和 moving GC；profile 固定向量及受影响的产物 golden 仍待同步，本批未运行全工作区测试。

本项完成泛型消费产物的这两处 HIR 读取缺口。继续沿同一发布、消费和运行回归接通 MIR 中已有的 ODR 定义，M23-7 尚未完成。

## 2026-09-28：泛型函数产物贯通下游发布、链接与移动 GC

- HIR→MIR 调用对接从当前 canonical MIR 的真实 ODR callable-body 签名建立 application 索引，按原 typed 模板匹配每次调用及其 root，检查完整 receiver、逻辑参数和 result。普通直接调用仍使用已有 Strong 定义；Unit 参数不能因物理 ABI 消除而从逻辑签名中缺失。
- MIR/LIR 外部依赖闭包按实际 call site 区分 provider 的直接 Strong 调用和消费方已有的泛型实例。普通 Strong shape 目录只选择完整注册集合中的 Strong 子集，ODR body 与 registration 继续沿原有完整 member 检查，不再因依赖包含 ODR 而拒绝整份产物。
- 实际泛型调用的类型用途按 exact type 原声明的 Cone 归属收集；消费方普通函数体内的私有类型不再被误要求出现在共有名义声明表。复合签名中的外来类型及既有共享表示、字段和局部值的依赖闭包保持。
- 真实三 Cone 回归全部通过：发布 provider 后移走源码，消费方沿正式入口发布，再移走消费方源码，最后仅用 `.slib` 编译和发布下游。独立案例以本地 class 引用穿过泛型调用并跨两次分配保持存活；组合案例覆盖消费方本地 struct、泛型转调、异常恢复和默认参数。两例都通过完整 artifact reader、真实对象链接、正常运行和 `SCOOP_GC_STRESS_MOVE=1` 运行；压力运行还断言实际发生 GC，六份既有 HIR/MIR/LIR golden 保持。
- `cargo fmt --all` 与 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过，lint 无警告。887项 HIR 测试、4项调用对接测试、12项 layout reader 测试、39项跨 Cone 闭包测试及1项上述真实发布运行测试通过，共943项，无失败或忽略。此前单独运行的26项类型用途测试包含在887项 HIR 测试中，不重复计数。
- profile descriptor/fingerprint 固定向量和受 profile 迁移影响的产物 golden 尚待同步，本批未执行全工作区测试。确认构建、测试和编译器进程全部结束后执行 `cargo clean`，删除2726个构建文件，回收5.2 GiB。

本项完成公共泛型函数和消费方本地值/引用类型的首条真实产物运行闭环。构造和委托模板、其他物理角色、跨产物重复成员合并及全部功能组合仍按原阶段验收继续实施，M23-7 尚未完成。

## 2026-09-28：generic profile 固定向量与旧格式迁移回归

- 同步 `cross-cone-generic/1` 的 canonical descriptor 与固定 fingerprint。逐字段核对本次格式差异仅为 profile ID 和 LIR inventory 中 `strong-production/14` 被 `cone-production/1` 替代，其他字段保持；沿用既有编码和摘要算法。
- 补齐新旧 production 格式的双向冲突测试：Compile/Link 两种 view 均拒绝混装，旧项或新项标为 optional 也不能绕过冲突检查。旧 layout-strong profile 的 `/1`、`/2`、`/3` 均要求重建；设计中被替代的 Strong production 版本同步修正为实际 `/14`。
- 同步编译缓存 key 与缓存记录的固定向量。新增旧 profile 缓存记录反例，保留原正例的正确 fingerprint，断言读取明确报告 `UnknownArtifactProfile`，避免以摘要损坏代替旧格式退役的验证。
- 使用原有真实 fixture 更新45份快照，并逐份比较完整内容：43份仅首行 Artifact fingerprint 改变，2份仅首行 profile 名称改变；Code、RuntimeImage 摘要、语义和物理引用均保持。共有 reader 的注释同步反映 canonical foundation 与实际格式，移除过时的 ODR-free 描述。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 与配套 `scoopc` 构建通过，lint 无警告；16项 profile 回归和23项缓存回归通过。所有快照更新开关关闭后的全工作区测试仍在运行，已完成的17个测试批次共5020项通过，无失败或忽略；driver 中泛型产物发布、下游消费、真实链接与移动 GC 回归也已通过，但尚未取得完整全仓结果。

本次按用户要求提交当前迁移基线。后续构造、委托和类型 application 的真实 payload 仍须同步升级 section 与 inventory；M23-7 尚未完成。全仓测试继续运行，构建目录待相关进程结束后再清理。

## 2026-09-28：共有 reader 合并跨产物 ODR 成员

- 共有 Compile/Link reader 在各产物 symbol/object 读取结束后，按实际 group/member 建立定义并集。完整 canonical key 借用各自已验证的 identity graph；重复成员比较 ABI 和最终 definition，兼容后才加入物理候选。没有直接依赖关系的 sibling 也在同一入口比较。
- 合并结果随完整 closure 保留，每个成员必需包含首个候选及其余兼容候选，候选指向真实 Cone、`SlibMemberId` 和 primary symbol。相同 symbol 被不同 owner 认领时拒绝；普通 Strong 引用与各产物的必要闭包继续由原 reader 检查。
- 冲突报告两个 Cone、group/member、role 及不同的 key、ABI、LIR、object 或 stackmap 部分。只在 definition 不一致时查询已有内容叶子以细化诊断，不重新解析对象、计算摘要或重放 HIR；没有新增 wire、runtime ABI、发布器或验证状态链。
- 新增真实 provider、两个 sibling 和 consumer 的独立及组合 fixture。provider 与 sibling 发布后移走源码，consumer 仅用 `.slib` 编译。共同实例覆盖整数、provider 引用类型、默认参数、try/finally 和异常恢复；两例都完成真实链接、普通运行与移动 GC，并断言实际发生收集。逐项核对全部成员和物理候选，反转产物输入顺序后完整 key、ABI、definition 与候选集合保持一致。
- 新增真实正文冲突回归：两个 provider 版本的泛型正文仅有一个常量不同，消费产物各自通过完整 reader 后进入同一合并函数。相同 group/member 的 key 和 ABI 保持一致，实际 definition 不同被明确拒绝；重复传入同一产物也不能成为第二个候选。这项定向检查不替代原完整依赖图的产物指纹检查。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和本次配套 `scoopc` 构建通过，lint 无警告。584项 slib 测试及全部5项真实泛型产物回归通过，共589项，无失败或忽略；既有 golden 未修改。确认本轮独立构建目录没有运行进程后，清理1501个构建文件，回收2.5 GiB。上一提交的全仓验证仍在原构建目录运行，结果与本轮验证分别记录。

本项接通当前函数和注册成员的跨产物合并，以及两个 sibling 的真实泛型运行闭环。生成实体的独立成员并集、其余物理角色和 TD/storage/cell 地址合并仍须随实际功能继续验证；构造、委托及全部阶段验收尚未完成。

## 2026-09-28：generic profile 迁移基线完成全仓验证

- `c28a106b9` 对应的完整工作区验证结束，命令退出码为0；共5147项测试通过，无失败、忽略或警告。包括117项 driver 库测试、真实配套编译器能力查询和两项文档测试；driver 库测试耗时2468.55秒。
- 全程关闭快照更新开关。profile descriptor、编译缓存固定向量、旧格式拒绝测试及45份迁移快照通过，泛型函数产物的发布、再次消费、真实链接和移动 GC 回归也随全仓复验通过。
- 该全仓结果对应成员合并功能之前的迁移基线；`c36f9eb79` 的584项 slib 与5项泛型产物验证另见上一条记录，后续局部函数等变更继续单独验证，不能把本次结果当作尚未完成的 M23-7 总体验收。
- 确认原全仓验证及新一批定向构建进程全部结束后，执行 `cargo clean --target-dir target`，删除4309个构建文件，回收7.8 GiB。

## 2026-09-28：导入私有 helper、局部函数及多层捕获完成产物运行闭环

- 导入实现目录直接复用共有 callable body 表和定义位置。普通私有 helper 仍调用 provider 的实际定义，泛型私有 helper 与局部函数进入同一具体化队列；词法正文不会成为可按名字导入的源码接口，未调用的局部声明不触发机器实例化。
- 局部普通函数保持原 `PersistentFunctionId`，有自身参数的局部 generic 函数保持原 `PersistentGenericFunctionId`；继承实参通过 enclosing callable application 表达，自身实参进入独立的 callable 参数组。已经在定义处完成解析的词法正文使用有序替换 binder，与参与源码重载推断的声明参数明确区分，不补造无约束签名。
- 局部声明标记在导入时消除，实际调用按原 ABI 先传捕获值、再传显式参数。多层捕获继续关联外层真实值及其 application，隐藏参数不取得另一份局部值身份。源码签名的参数下标排除捕获前缀；实际 MIR/LIR ABI 保留前缀，声明位置 reader 同时处理 generic root 和带 enclosing application 的局部普通函数。
- 新增独立及组合 fixture，覆盖 private ordinary/generic helper、定义处重载绑定、扩展调用、普通及局部递归、局部函数自身类型参数、两层词法捕获、消费方本地引用类型和 provider 引用类型。provider 与消费方发布后移走源码，下游仅以 `.slib` 消费；provider 可以只经 support path 提供。两例均完成正式发布、完整 reader、真实对象链接、普通运行及移动 GC，并检查实际发生收集。
- provider 自身也从源码生成相同实例，另外两个消费 Cone 以不同调用顺序从产物具体化。真实闭包中至少9个共同 callable-body 成员均有三方物理候选，其中包含三个局部函数实现；ABI、LIR、object、stackmap 及最终 definition 通过同一 ODR 合并入口，实际链接运行通过。
- 固化6份 HIR/MIR/LIR golden 和5份诊断 golden。反例分别拒绝直接导入 private generic helper、private ordinary helper、局部函数，以及对引用类型实例化 `@NoGC` 和捕获可变局部值；逐项断言源码位置、表达式与信息，且失败时不发布产物。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过，lint 无警告。关闭快照更新开关后，887项 HIR、1247项 HIR lowering、584项 slib 及全部6项泛型产物回归通过，共2724项，无失败或忽略；既有 golden 未修改。本批未重跑全工作区测试，不复用先前迁移基线的全仓结果作为本批验收。
- 确认所有相关构建及测试进程结束，且 `target` 中没有打开的文件后执行 `cargo clean --target-dir target`，删除2368个构建文件，回收3.5 GiB。本项沿用当前 wire 和 runtime ABI，不新增发布器、验证状态链或语义重放流程。

本项完成导入泛型正文中 helper 与局部直接调用的组合闭环。lambda、匿名函数、callable reference、构造和委托模板及其他阶段验收继续按原设计推进，M23-7 尚未完成。

## 2026-09-28：构造初始化模板进入共有 HIR 生产与读取

- 泛型 class 按原名义声明保存一次 common initialization，各构造器分别保留 primary、secondary `this`、terminal `super` 的实际委托和正文；泛型 struct 保留 primary 值构造与 secondary 委托。字段与 delegate backing field 的写入、`init` 和次构造正文保持源码顺序。
- 委托实参、字段初始化与纯语句正文复用同一 typed 执行片段，保存局部值表、语句和实际结果列表；没有结果的正文不填充假的 Unit。构造输入使用原参数位置与 `This` selector，字段访问保留原 typed identity，定义位置与外来引用进入现有闭包。
- 共有声明、callable body 与构造初始化在同一工作队列收集；实际引用的 private generic helper 进入支持正文，未使用的 private 名义声明不导出。reader 在记录构造边界验证顺序、引用形状、构造种类、字段写入和结果数量，不重复类型检查或布局计算。
- `hir/cross-cone-interface` 升至 `/33`，增加必需 field 12；required inventory、reader、profile descriptor、固定摘要和空 section 编码同步迁移，旧 `/32` 产物和缓存需要重建。
- 新增独立与组合源码 fixture 和两份 HIR golden，覆盖泛型 struct/class 主次构造、继承、多个 terminal 构造器、普通及委托属性和私有 helper。真实源码投影后完成 canonical 编码、解码和引用闭包验证；格式反例拒绝缺失/重复/错误 owner、构造种类不符、非法参数写入、错误结果数量及 struct 携带 class common sequence。旧默认值反例继续拒绝作用域外的构造参数。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过，无警告。关闭快照更新开关后，887 项 HIR、1249 项 HIR lowering、584 项 slib，以及 `scoop` 的 93 项单测和 2 项命令集成测试通过，共 2815 项，无失败或忽略；profile 固定向量与编译缓存回归均包含在内。

本项完成构造执行模板的生产与读取，不代表消费方泛型名义类型已完成。实际发布测试已暴露泛型类型描述符、扫描与分发表的 ODR 物理链路缺口，修复继续沿同一对象与摘要入口推进；MIR/LIR golden、产物消费、链接和移动 GC 尚未作为本项验收通过。

## 2026-09-28：泛型名义类型的实际 shape 进入共有 ODR 物理产物

- 当前 Cone 从源码实例化的泛型名义类型沿已有 exact type 与 specialization group 进入 MIR/LIR；core 中已有的 `Option<String>` 也生成其实际 shape 和类型注册。layout、scan、TD、dispatch 和类型注册分别使用实际 member，LLVM 按原 symbol request 发射 WeakODR 定义；HIR 发布已有名义 application group，后续 stage 只引用，不重复复制上游身份。
- 新增 production 必需 field 14，从最终 LIR metadata 保存实际 ODR shape 的 canonical LIR/ABI 摘要。固定布局包含有序字段偏移、对齐与表示，变长数组包含元素与完整实例表示，扫描包含实际程序，TD 与分发表保留 typed 目标及 ABI 顺序。reader 检查精确成员集合、顺序和引用后复用，不重做语义或布局。
- shape 对象摘要沿已有 atom 范围和 relocation 归一化计算，包含 TD 诊断及 itable directory 等关联内容。类型注册复用同一摘要入口，依次取得 TD/layout 内容、注册对象与自身 member，最终目录和 image 使用实际 Strong/ODR 分类。删除仅转发中间结果的注册对象和依赖包装链；既有 Strong 类型摘要固定向量保持不变。
- 构造器和属性访问器的声明类型位置按实际 application 查询原声明；字段和 class 初始化结果接受原泛型名义 owner。补充构造 root/application 混装、非法 receiver/参数位置、缺失声明和 struct 初始化位置反例，保留原来源与位置边界检查。
- `cone-production` 由 `/1` 升至 `/2`；共用生产结构的历史 `strong-production` 由 `/13` 升至 `/15`，不复用历史 `/14`。required inventory、canonical profile descriptor、固定摘要、旧格式拒绝测试与语言规范、实现规范及设计说明同步；旧产物和缓存需重建，runtime C ABI 保持。
- 独立和组合源码 fixture 固化四份 MIR/LIR golden，覆盖值构造、class 主次构造、多个 terminal/this 委托、继承、引用字段、普通及委托属性。provider 发布后移走源码，consumer 调用其普通公共入口并再发布、移走源码，下游仅以 `.slib` 编译；两例均完成完整 reader、真实链接、普通运行与移动 GC，并核对实际收集和每个 shape/类型注册的 ODR 目录。该消费路径验证 provider 已物化的泛型类型，不代替消费方从模板新建名义实例的验收。
- 新增 shape 内容回归验证生产 Cone 与 arena 顺序不影响摘要，字段偏移/扫描/dispatch 顺序影响对应内容，TD 诊断不改变 ABI；reader 拒绝缺少、重复、乱序与未知成员。旧泛型数组拒绝测试改为核对实际分配和七个 shape 成员，其中包含独立的 ArrayElement 扫描。
- HIR lowering 与 driver 的真实 core 辅助代码改用已有 canonical foundation，移除旧 ODR-free 中间包装。原负例继续检查缺失声明、生成实体和初始化引用；core 新增实例沿实际 ODR 身份检查，其普通导出分区保持原含义。

- 同步并逐份核对145份既有快照：41份增加实际 application 布局，41份反映对应类型注册数量，49份仅摘要字段变化；其余14份记录新增 ODR 摘要节点、类型注册、patch 或 atom 边界符号。两个原有组合 fixture 的泛型 Box 也进入实际物化，其新增布局与源码字段一致；已有普通导出、callable 和 stackmap 记录保持。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过，lint 无警告。分批完成工作区各测试目标的验证：5026项非 driver 测试（含2项文档测试）、121项 driver 库测试、7项 CLI 单测和1项配套编译器能力查询，共5155项独立测试。初次回归的旧 Strong 假设及相应快照均已修正并复验；关闭全部更新开关后的最终30项 driver 回归全部通过，耗时266.28秒，覆盖此前全部25项失败，最终没有未解决失败或忽略。
- 确认所有构建、测试和编译器进程结束，且 `target` 中没有打开的文件。恢复该目录缺失的标准 Cargo 缓存标记后，执行 `cargo clean --target-dir target`，删除3085个构建文件，回收6.5 GiB。

本项完成当前 Cone 已实例化名义类型的 ODR shape 与类型登记产物闭环。消费方从模板新建名义实例、递归扫描子程序的对象 atom、生成实体与其他物理角色、完整组合和地址合并仍待继续，M23-7 尚未完成。

## 2026-09-28：保存消费方泛型名义类型与闭合存储进度

- 导入名义声明目录统一使用原 `NominalDeclarationOwner`，同时保留参数自由声明与泛型模板。导入 struct、enum、class、interface 保存完整有序实参；字段、variant payload、父类型和成员签名按原 binder 替换，类型相等、推断、递归检查与具体化缓存同时比较声明和实参。没有复制 provider 声明到当前源码 arena，也没有按名字补造身份。
- 泛型 application 沿已有 exact type builder 和 specialization group 生成当前 Cone 的 ODR 表示。参数自由外部类型用途只选择实际 Strong 表示，继续递归保留实参中的普通依赖；GC-free 与指针约束使用实际替换后的字段。普通 owner 的字段或构造参数为闭合泛型类型时，自动根、source callable 选择及 exact facts 不再把整个 owner 标为 source-only。泛型继承与派发仍待后续接通。
- 字段类型位置改为所属 exact type 与原 typed field ID 的组合，区分同一泛型字段的多个实例。外来泛型实例可以关联原 provider 的字段来源；object 使用源对象 exact type，并核对字段属于其生成的 backing class。HIR 接口升级至 `/34`，tag 6、7 增加必需 field 3；编解码、版本拒绝、两种受影响 profile 的 descriptor 与固定指纹同步更新，旧产物需要重建。
- MIR 普通签名与字段的依赖遍历接受闭合 application，继续按 Strong/ODR 的实际归属处理引用。只有需要 GC-free 的 `NoGC` 签名查询 GC 事实；缺少普通 MIR 类型导出的 application 直接借用已有 HIR exact facts，不重新推导字段语义。新增真实源码回归验证 GC-free 的泛型值签名可往返读取，错误的引用事实仍被拒绝。
- 新增泛型 enum payload 的独立与组合 fixture，覆盖消费方本地引用、嵌套 payload、含引用的大值、ZST，以及类型参数数量、ref/value bounds 和不变性的反例。真实产物测试保留 provider 发布后移走源码、consumer 再发布并移走源码、下游仅用 `.slib` 编译、链接与移动 GC 的完整路径；已有六份 HIR/MIR/LIR golden。该运行路径在本批闭合存储与字段位置扩展前曾通过，不能作为当前提交的最新运行结论。
- 当前两项 driver 回归 `generic_nominal_consumers_create_payload_instances_from_artifacts` 与 `runtime_exception_storage_is_materialized_before_mir_and_publication` 均失败于真实 core 发布：`LayoutExports(Layout(MissingDependency(...)))`。HIR 与 MIR 已通过，LIR 的普通布局投影仍把泛型字段所需布局当成外部参数自由布局查询。后续须把实际 ODR 表示接入这条既有布局与 ABI 路径，不能删掉原 core 异常字段或改用简化 fixture。runtime 正例的四份新 MIR golden 尚未生成；本次未取得最新的发布、链接和移动 GC 成功结果。
- 关闭更新开关后，非 driver 工作区的 5031 项独立测试全部通过，无忽略。初次出现的八份 MIR golden 差异已逐份核对，仅涉及自动生成实体编号；更新后重新运行全部 1255 项 HIR lowering 和 582 项 slib 测试，均通过。`cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 与配套 `scoopc` 构建通过，lint 无警告。完整 driver 测试尚未通过，本记录不代表 M23-7 验收完成。

- 确认所有构建、测试及编译器进程结束，且 `target` 中没有打开的文件后，执行 `cargo clean --target-dir target`，删除 2819 个构建文件，回收 4.9 GiB。

本次按用户要求提交当前工作进度。消费方泛型名义类型的前端与字段实例位置已接通，闭合泛型存储的 LIR 发布闭环继续实施，M23-7 仍未完成。

## 2026-09-28：保存实际泛型表示与布局通道进度

- 原 MIR 类型表示表新增实际 nominal application，origin 保留 `PersistentGenericTypeId`，完整实参使用原 exact key；字段、enum variant 与 payload 保留原 typed ID。struct、enum、class 存储及 Array/MutableArray 元素沿同一表投影，普通 Strong 与 application 的 ODR 定义继续区分。相同 application 的等值 MIR 表示可以复用，矛盾表示和普通 Strong 重复仍拒绝。
- HIR exact facts 纳入已经具体化的 nominal application 及实际存储依赖；导入 enum 直接使用 concrete variant 保留的身份，删除回到当前 ExportHir 查找外来声明的路径。导入 Array/MutableArray 保留实际 intrinsic family 与元素类型。MIR 只要求实际机器实例和其引用闭包，不强制将 HIR 的全部 specialization group 发射成机器类型。
- 实际 application 接入既有 LIR value/instance layout、descriptor 与类型 registration。registration 根据原 definition plan 选择 Strong 或 ODR 摘要节点；布局查询可以复用不同物理 producer 的等值 ODR 定义。删除上一批临时加入的 GC facts 回调，`NoGC` 检查统一查询完整 MIR 类型记录中的既有 facts。
- MIR 为实际选中的外部调用登记 receiver、参数与结果的完整物理签名类型，补齐隐式异常 initializer 的 `Unit` 结果。沿完整 HIR 的 exact identity 和既有类型 lowering 转换，不从名字补造类型，也不把未选中的 provider 声明变成机器根。
- HIR `cross-cone-type-semantics/9`、MIR `cross-cone-type-bridge/2`、LIR `cross-cone-layout-abi/4` 与 required inventory、canonical profile descriptor、固定 fingerprint 同步。补齐旧 major 在 Compile/Link 两种 view、含 optional 混装时的拒绝测试；三份 spec 与设计说明同步，runtime C ABI 保持。
- 新增独立和组合源码 fixture，覆盖泛型值、嵌套 enum payload、class 引用与泛型数组；核对实际 HIR/MIR 生产、表示往返、原 owner 和 field ID、等值记录合并、矛盾表示拒绝及 `NoGC` 引用事实拒绝。`cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过，lint 无警告。
- 非 driver 全工作区首次验证有 5033 项通过、1 项旧断言失败。该断言仍要求 application 没有独立 MIR 类型表示；更新为核对 application 自身的类型依赖，同时保留原结构类型规则，并复跑全部 338 项 MIR 测试，全部通过。合计 5035 项独立非 driver 测试通过，无忽略，包含 1256 项 HIR lowering 和 584 项 slib 测试；定向运行的 18 项 profile 与 24 项类型表示测试已包含在总数中。
- 真实 core 已能生产、发布并被消费方读取，但两条 driver 回归仍未通过：`runtime_exception_storage_is_materialized_before_mir_and_publication` 在消费方遇到 `Layout(Selection(Semantic(DuplicateTarget(Layout(...)))))`，共有 LIR 语义闭包仍拒绝重复 ODR 布局；`generic_nominal_consumers_create_payload_instances_from_artifacts` 在 standalone 消费方遇到 `Layout(MirSection(MissingDependency(Type(...))))`，泛型 payload 的类型依赖尚未完整闭合。原 fixture 保留，四份 runtime 正例 MIR golden 尚未生成，本批没有新的完整发布、链接或移动 GC 成功结论。
- 确认构建、测试及编译器进程结束，且 `target` 中没有打开的文件。核对该目录为本工作区构建缓存后恢复缺失的标准 Cargo 缓存标记，执行 `cargo clean --target-dir target`，删除 2897 个构建文件，回收 6.0 GiB。

本次按用户要求提交当前工作进度。实际泛型表示已进入共有类型与布局通道，产物语义闭包仍须继续补齐；消费方构造、继承与完整派发及其余阶段验收尚未完成，M23-7 仍未完成。

## 2026-09-28：共有 ODR 布局引用恢复真实异常产物发布

- LIR 语义闭包按实际 `(provider, target)` 保留记录，同一 ODR definition plan 可有多个物理提供方。带 provider 的根、布局、descriptor 和 dispatch 引用精确选择原记录；未指定物理位置的语义引用按 Cone identity 稳定选择。普通 Strong target、同一 provider 内的重复及错误 provider 仍拒绝。跨产物内容兼容性继续由原 ODR member 合并入口检查，没有新增 wire、摘要算法或重复内容校验。
- 原 `runtime_exception_storage_is_materialized_before_mir_and_publication` 回归的六个源码用例全部完成 core 消费和正式产物发布。四份此前缺失的 MIR golden 已从真实输出生成并逐份核对：`Option<String>` 异常存储、失败分支的 class 分配、外部 initializer、throw、默认实参展开及有符号除法的溢出分支均保留；两个不执行隐式异常路径的已有快照保持。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过。全部 465 项 LIR 测试通过，关闭快照更新开关后的六用例发布回归通过；本项未重跑全部工作区测试，也不替代链接运行与移动 GC 验收。
- 泛型 payload 消费仍在 standalone 的 MIR 依赖闭包失败。已通过原 exact identity 定位缺项为消费方 `consumer.nominals.Number`：该类型用于 `Parcel<Number>` 的实际 payload，却未进入共有 MIR 表示清单。临时定位输出已删除，下一步补齐这类实际存储依赖所需的私有支持声明，保持原可见性和 typed identity。

本项完成重复 ODR 布局的物理引用处理及原异常存储的产物发布回归；M23-7 的其余主线继续实施。

## 2026-09-28：消费方泛型 payload 的私有表示依赖

- 完成 HIR 时，从实际已物化的 nominal application 沿既有类型子节点收集表示依赖，将当前 Cone 所需的源码名义声明补入同一共有支持集合。声明保留原 typed identity 与可见性，object 通过实际 backing class 关系找到原声明；外来声明仍由原 provider 提供，不复制到当前源码 arena。
- 支持根增加时，同步扩展已有源码/模板投影和 `LocalShapeSupportPlan`，让普通私有类型的 MIR 表示、有限生成类型和 LIR 布局进入完整产物引用闭包。没有新增 wire、public binding 或平行声明表；所有本地私有物理声明不会因此自动导出，加入无关私有声明的既有字节稳定性检查继续通过。
- 新增独立与组合源码 fixture，覆盖私有 class 引用、泛型 struct 和 enum 的嵌套存储、零大小值、私有 object，以及仅在未调用模板中出现的闭合泛型构造。检查原可见性、支持根、字段 exact identity、完整类型记录往返与未使用类型不发射。MIR 测试辅助入口改为借用正式路径的完整 canonical foundation，允许真实 ODR 构造器签名，保留历史 Strong 边界的拒绝规则。
- 原 `generic_nominal_consumers_create_payload_instances_from_artifacts` 的 standalone 和 combined 全部通过：provider 发布后移走源码，consumer 从模板生成此前不存在的 payload 实例，再发布并移走源码；下游仅用 `.slib` 完成读取、编译、链接、普通运行和移动 GC，保留实际收集断言。四份 MIR/LIR golden 的变化已逐份核对，仅补充支持类型对应的既有 box/coroutine 表示及其 descriptor；原 payload 布局和执行正文保留。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过。非 driver 工作区的 5036 项测试全部通过，无忽略，包含 1257 项 HIR lowering 和 584 项 slib 测试；定向的 25 项类型表示测试包含在上述总数中。关闭快照更新后，泛型产物测试有 7 项通过，原异常存储六用例发布回归通过。
- 扩大的泛型产物回归仍有 1 项失败：`generic_initializations_survive_publication_and_execute_with_moving_gc` 在 standalone 发布时遇到 `LayoutExports(MissingDispatch(...))`。实际泛型 class 已有类型与布局，但尚未进入共有 MIR dispatch 清单；下一步沿原物理派发表补齐此路径，不以空表绕过实际成员。
- 确认编译、测试及配套编译器进程结束，且 `target` 没有打开的文件后，执行 `cargo clean --target-dir target`，删除 2672 个构建文件，回收 4.5 GiB。

本项完成泛型 payload 私有存储依赖与原消费产物运行闭环；泛型 class dispatch、构造模板的实际导入及其余 M23-7 验收继续实施。

## 2026-09-28：泛型名义实例的 callable 与分发表使用实际 ODR 目标

- MIR callable binding、分发表和 LIR exact ABI 共用 `CallableDefinitionOwner::Strong/Odr`，沿已有 callable body、application、specialization group 和 member identity 表达实际定义。泛型 method、accessor、class initializer 和 struct constructor 从已经完成的 LocalConcrete HIR 与 MIR 正文生成完整记录；普通调用入口仍保持其原 Strong 约束，没有把模板声明充作机器定义。
- 泛型 class 的共有分发表直接投影实际 MIR vtable/itable，保留原 slot identity、顺序、完整签名、GC effect、继承覆写、接口默认实现和 abstract trap。泛型抽象接口方法生成真实 trap 正文，删除旧 shell 路径及其重复 suspend 登记。语义依赖沿具体 receiver 的继承关系找到实际 Strong/ODR callable；等值 ODR 类型、callable 和 dispatch 记录可在原 MIR 依赖索引中复用，Strong 重复与矛盾记录仍拒绝。
- LocalConcrete HIR 分别保存直接接口与完整传递实现集合，接口保留实际父接口。MIR 类型导出使用直接关系，实际派发表继续使用完整接口集合；导入与本地具体化执行同一规则，reader 保留与原声明的精确关系检查。LIR ABI、分发表、物理引用、对象和注册继续进入原 production 与 ODR 合并通道。
- `mir/cross-cone-type-bridge` 升至 `/3`，`lir/cross-cone-layout-abi` 升至 `/5`，`lir/cross-cone-layout-link-closure` 升至 `/3`。required inventory、profile descriptor、固定 fingerprint、旧 major 拒绝测试及三份 spec、设计说明同步；`cone-production/2` 和 runtime C ABI 保持，旧产物与缓存需要重建。
- 新增独立与组合 dispatch fixture 和六份 HIR/MIR/LIR golden，覆盖泛型继承、虚方法覆写、多层接口、默认方法、abstract trap 和引用 payload。核对每个实际 slot 的 ODR 正文、完整 MIR/LIR ABI 与 WeakODR 符号，反例拒绝错误 application member 和以 Strong 模板声明代替实际定义。两例均完成 provider 发布并移走源码、consumer 再发布并移走源码、下游仅以 `.slib` 编译、真实链接、普通运行及移动 GC，保留实际收集断言；原构造初始化的 standalone/combined 两例也恢复完整运行闭环。
- core 的异常及含闭合泛型字段的普通声明现在核对实际类型、布局、构造和访问器导出，替换旧的 source-only 假定。整数异常默认实参改为正式发布正例，MIR golden 保留零除异常的分配、外部初始化、throw 和有符号溢出分支。源码清单反例继续检查声明所需表示；删除实际 application 类型记录的反例移至完整 MIR 引用闭包，避免把所有 HIR application 强制变成机器根。既有 dispatch 与依赖图快照中的 slot、实体 ID 和顺序保持，实际定义目标显式增加 Strong 包装。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过，无警告。关闭全部快照更新开关后，非 driver 工作区的 5037 项测试全部通过，无失败或忽略，包含 1258 项 HIR lowering、114 项 MIR lowering、584 项 slib 和 2 项文档测试。首次完整 driver 库验证有 98 项通过、25 项失败；修复后关闭更新开关复验 26 项相关测试，全部通过，涵盖全部 25 项原失败，耗时 302.08 秒。其余 97 项库测试及 7 项 CLI、1 项配套编译器测试已在本批首次完整 driver 运行中通过；本记录不将分批验证写成一次完整工作区命令。
- 确认全部构建、测试与配套编译器进程结束，且 `target` 中没有打开的文件。核对该目录为本工作区构建缓存后恢复缺失的标准 Cargo 缓存标记，执行 `cargo clean --target-dir target`，删除 3001 个构建文件，回收 6.6 GiB。

本项完成 provider 已物化泛型 class 的共有 callable、dispatch 与产物运行闭环。消费方从模板构造、继承和再次实例化完整泛型 method，以及 ODR boxing/adjust 等组合继续沿剩余主线推进，M23-7 尚未完成。

## 2026-09-28：消费方实例化泛型构造模板

- 构造候选继续使用原普通或泛型名义 owner，宿主实参共用已有推断、具名参数、默认值和 bound 检查。普通构造保留调用上下文的期望类型，类型别名只为泛型宿主固定完整实参。导入构造模板、已解析 application 与具体构造分别使用独立 typed ID，原 constructor identity 和实际 owner application 保留到既有具体化队列；没有复制 provider 声明到消费方源码 arena。
- 消费 struct primary/secondary、class primary、terminal secondary 与 `this` 委托的真实初始化片段。class 基类与 `this` 委托使用同一已分配接收者，字段初始化与 `init` 按源码顺序且仅在 terminal 构造执行；struct secondary 在完成值构造后执行正文，字段保持不可变。消费方本地类可以继承导入的泛型基类，嵌套宿主实参按实际 application 替换。
- 导入 final 属性按原实现选择实际访问器正文或 storage，构造及属性中的字段引用保留原 typed field ID，并共用既有基类字段前缀计算。局部函数捕获构造参数和构造正文局部值复用原 capture selector，补齐源码 lowering 中对构造参数的捕获实参生成。实例产生正常 ODR 构造、访问器与布局，继续沿原 MIR、LIR、对象及运行时通道发布。
- 共享可移植表达式增加必需的原 `EvaluationOrigin`，定义位置与实际求值位置分别往返保存。已在构造委托中展开的默认值可以定义于另一文件；消费正文时恢复原求值位置，消费默认参数模板时仍按本次使用点展开。source record、context 和稀疏位置点复用已有 foundation 与 reader 检查，相同位置复用验证结果；没有新增来源资格或重复语义检查。HIR interface 升至 `/35`，两种受影响 profile 的 descriptor、固定指纹和旧版本拒绝测试同步；三份 spec 与设计同步，后续 delegate 格式顺延至 `/36`，runtime C ABI 保持。
- 新增 12 组独立与组合正例、36 份 HIR/MIR/LIR golden，覆盖主次构造、两种 class 委托、公共初始化顺序、泛型继承、跨文件默认值、参数推断、具名调用、固定实参类型别名、局部捕获、引用 payload、大值和 ZST。别名构造与直接构造复用同一实际实例。四个 negative fixture 检查 kind bound、实参数量、private 构造访问和不可变属性，并断言错误信息及源码位置；wire 与 reader 反例拒绝缺失求值位置和超出实际 source map 的位置。
- 真实产物用例先发布 provider 并移走源码，再由 consumer 产生此前不存在的构造实例、发布并移走源码；下游仅依赖 `.slib`，在 consumer 未调用的泛型函数中再次实例化新类型。每组核对实际构造 ODR 角色、完整 MIR/LIR ABI、物理 provider 与 shape 合并目录，并完成链接、普通运行与移动 GC，保留实际收集断言。跨文件默认值另核对定义来自 `src/base.scoop`、求值来自 `src/main.scoop`。
- 同步 41 份 core 产物快照及两份下游 shape 依赖快照，逐份确认只有整包 artifact fingerprint 变化，代码、runtime 指纹与其余内容全部保持；两种 profile 的 descriptor 也只改变 HIR interface 的 major 字节。原布局、ABI、类型与引用检查继续执行；普通构造沿用原有诊断快照验证上下文期望类型。
- `cargo fmt --all` 和 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过，无警告。非 driver 工作区首次有 5038 项通过、3 项旧格式断言失败；更新一个表达式 product 头和两个 profile 固定向量后，完整复验 888 项 HIR 与 585 项 slib 测试，全部通过。合计 5041 项独立非 driver 测试通过，无忽略，包含 1260 项 HIR lowering 和 2 项文档测试；定向重复验证不重复计数。
- 完整 driver 库首次有 100 项通过、24 项失败：三项来自普通构造的上下文期望类型被覆盖，另 21 项使用旧的整包指纹。修正候选准备后，再次完整运行全部 1260 项 HIR lowering 测试并重建配套 `scoopc`，均通过；新增的别名组合也完成前端与真实产物运行验证。同步格式指纹后，关闭全部快照更新开关复验 25 项 driver 测试，全部通过，覆盖全部 24 项原失败和新增构造闭环，耗时 378.11 秒。其余 99 项库测试、7 项 CLI 测试及 1 项配套编译器测试已在本批完整运行中通过；分批合计 5173 项独立测试通过，无未解决失败或忽略，本记录不将分批验证写成一次完整工作区命令。
- 确认所有构建、测试和配套编译器进程结束，且 `target` 中没有打开的文件。核对该目录为本工作区 Cargo 构建缓存后恢复缺失的标准缓存标记，执行 `cargo clean --target-dir target`，删除 2887 个构建文件，回收 6.0 GiB。

本项完成消费方从真实产物实例化、执行与再发布泛型构造的闭环。完整成员与虚派发、delegate、其他生成实体及剩余组合继续推进，M23-7 尚未完成。

## 2026-09-28：消费方实例化泛型成员正文与虚调用

- 导入成员沿接收者的实际继承闭包取得声明所属的完整 application，固定宿主参数组；显式类型参数、`_` 和实参推断只绑定方法参数组。最具体候选比较继续按声明分别处理两组参数，不使用当前调用的推断结果。成员 application 保留所属类型和方法自身实参，具体化键使用原 kind-specific `MethodOwner`，仅正文替换时临时组合宿主与方法实参；没有复制 provider 声明到本地源码 arena。
- class、struct、enum 和参数自由宿主上的泛型成员复用已有模板正文与具体化队列。默认参数、继承成员和局部具名函数捕获保留原定义及词法父实例；泛型宿主的普通虚方法、覆写和强制 `super` 调用保留实际 dispatch。再次发布模板时区分普通方法调用和强制基类调用，下游可以继续对新类型实例化并执行动态派发。
- 共有支持类型闭包补入实际导出泛型成员 ABI 的 receiver、参数和结果类型。私有类型即使只用作方法自身实参，也能获得下游所需的完整表示；不相关的私有声明仍不进入共有支持根。收集范围限于实际成员 ABI，普通泛型函数保持原输出，不新增发布表或 public binding。
- 共有 LIR 语义闭包对 callable ABI 使用已有 ODR definition plan，允许同一实际定义在不同物理 provider 中保留记录，继续拒绝 Strong 重复和同一 provider 内重复；内容兼容性仍由原 ODR member 合并入口检查。真实运行测试的 descriptor 收集同步接受同一 ODR 符号与 definition plan，不再假定每个 exact type 只由一个 Cone 发射。
- 新增 10 组独立与组合正例和 30 份 HIR/MIR/LIR golden，覆盖宿主与方法两组参数、默认值、继承、局部捕获、class/struct/enum 成员、参数自由宿主、虚调用、`super` 和重载选择。ABI 组合包含 ZST、含三个引用字段的 24 字节值、间接参数、sret 和对应 GC roots。四个 negative fixture 检查方法 kind bound、显式实参数量、宿主参数类型与重载歧义，同时断言错误信息和消费方源码位置。
- 每组先发布 provider 并移走源码，再由 consumer 新建成员实例、发布并移走源码；下游仅依赖 `.slib`，通过 consumer 的泛型正文实例化新的私有类型。测试核对实际成员 ODR 定义、完整 MIR/LIR ABI 和物理 provider，并完成链接、普通运行与移动 GC，保留实际发生收集的断言。新增重载三份快照后，已有 27 份成员快照保持不变。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过，无警告。首次非 driver 全工作区有 5042 项通过、1 项快照失败；首次完整 driver 有 122 项库测试通过、3 项快照失败，另 7 项 CLI 和 1 项配套编译器测试通过。四项失败均来自支持类型收集超出了实际导出成员 ABI；收紧范围后恢复原快照，没有把多余支持类型写入普通泛型函数的既有预期。
- 最终完整复验全部 1262 项 HIR lowering 测试并重建配套 `scoopc`，随后关闭快照更新开关，复验 11 项泛型产物测试，全部通过，耗时 61.51 秒，涵盖全部原失败、新成员、构造、初始化、dispatch、helper、ODR 与下游再发布。分批合计 5176 项独立测试通过，无未解决失败或忽略；定向重复验证不重复计数，本记录不将分批结果写成一次完整工作区命令。
- 确认所有构建、测试及配套编译器进程结束，且 `target` 中没有打开的文件。核对该目录为本工作区 Cargo 构建缓存并恢复缺失的标准缓存标记后，执行 `cargo clean --target-dir target`，删除 2756 个构建文件，回收 5.5 GiB。

本项完成消费方从产物实例化、执行与再次发布具体泛型成员正文，以及泛型 class 的虚调用和 `super` 闭环。完整泛型接口、抽象成员、消费方覆写与访问控制及其余主线继续推进，M23-7 尚未完成。

## 2026-09-28：保存泛型接口与抽象成员消费进度

- 泛型抽象成员从共有声明和原 definition origin 建立完整宿主参数、receiver、参数局部值及结果类型，进入既有抽象方法与 trap lowering；不为抽象声明制造共享执行正文。默认方法继续消费真实模板，接口继承与抽象 override 保留原实现选择。对应实现规范和设计说明同步。
- 导入 struct/enum 与 class 共用接口实现解析，具体化传递完整 conformance。消费方本地类型实现导入泛型接口、继承导入泛型类时，按实际 owner application 替换成员与属性签名；泛型抽象 getter/setter 进入已有成员 application 路径。补齐 `ImportedGenericCall` 作为调用语句的处理，以支持返回 `Unit` 的真实接口调用。
- MIR 对同时出现在调用根与接口表中的同一具体方法复用已登记的函数和实例，修正真实产物测试发现的重复登记。没有放宽实例唯一性断言，也没有把未调用的泛型接口默认方法一律物化。
- 新增 8 组正例和 5 个反例，现有两个 HIR 测试合计覆盖 18 组正例与 9 个反例并全部通过。场景包括默认方法、抽象覆写、struct/enum 装箱、消费方实现、接口属性、ZST 和含引用的大值；反例断言缺失实现、错误覆写、不变性、抽象 `super` 调用及只读属性赋值的诊断位置和信息。目前生成 `interface-class` 与 `interface-abstract` 共 6 份 HIR/MIR/LIR golden，其余新增场景的产物快照尚未生成。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过；提交前再次格式化与 lint 通过，无警告，`git diff --check` 通过。本批尚未完整运行非 driver 工作区或全部 driver 测试，不沿用上一批测试总数作为本批结论。
- 真实成员产物测试中的原 10 组用例及新增 `interface-class` 已完成发布、移走源码、下游再次实例化与发布、链接、普通运行和移动 GC。`interface-abstract` 的 consumer 三阶段发布成功，但 downstream 发布失败于 `Layout(MirExports(Dispatch(MissingCallable(Odr(...)))))`；后续 6 个新增正例尚未执行到。当前共有 MIR 派发表取得根声明的槽签名时仍依赖该声明的机器 callable binding；下游调用更具体的抽象 override 时，根默认方法没有被调用或物化，暴露此不必要依赖。后续须区分槽的签名契约与实际机器实现，保留必要类型、引用和 ABI 检查，继续完成真实产物链路。
- 确认全部构建、测试和配套编译器进程结束，且 `target` 中没有打开的文件。核对该目录为本工作区 Cargo 构建缓存并恢复缺失的标准缓存标记后，执行 `cargo clean --target-dir target`，删除 2494 个构建文件，回收 3.9 GiB。

本次按用户要求提交当前工作进度。完整泛型接口的产物消费仍有上述未解决失败，M23-7 尚未完成。

## 2026-09-28：保存接口槽契约与 ODR 装箱接入进度

- MIR 将接口声明的槽契约与实际机器表项分开。接口保存原 typed slot、槽位置和完成替换的完整签名，不再制造自身 itable；类和值类型的物理表项继续保存必需实现，并与接口契约逐槽匹配。签名依赖与实际 callable 依赖分别从真实记录收集，不再要求未调用或被抽象 override 压制的默认正文具有机器 binding。
- 普通接口与泛型 application 共用上述结构，源码覆写选择仍来自 HIR。MIR producer、reader、语义引用和 LIR 派发表消费同步调整；新增回归覆盖没有根 callable binding 的接口契约、实际 override，以及缺失或错误槽种类的拒绝。`mir/cross-cone-type-bridge` 升至 `/4`，profile descriptor、固定 fingerprint、旧版本拒绝测试及三份 spec、设计说明同步，旧产物和缓存需重建，runtime C ABI 保持。
- MIR materialization plan 开始保留生成类型的 Strong/ODR 归属，LIR 使用原 ODR group；生成 box、step/slot 沿原类型表示投影，装箱 adjust 和目标成员使用完整 Strong/ODR callable subject。这部分尚未接通完整产物路径，不能作为生成实体功能完成的结论。
- 提交前执行 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建，均通过，lint 无警告。全部 341 项 MIR 与 585 项 slib 单测通过；两个泛型成员 HIR 测试也通过，覆盖 18 组正例与 9 个反例。合计 928 项单测通过，本次未运行完整工作区回归。
- 关闭快照更新开关后，真实成员产物测试中的原 10 组用例，以及 `interface-class`、`interface-abstract`，均完成发布、移走源码、下游再次实例化与发布、链接、普通运行和移动 GC。原抽象覆写的 `MissingCallable` 已解决。测试随后在 `interface-struct` 的发布入口失败于 `Layout(MirExports(Types(Bridge(MissingShapeSupportSource { ... }))))`；当前生成类型投影发生在 application 类型加入源表示表之前，尚需调整生产顺序，并完成装箱目标及生成类型验证的实际 ODR 路径。后续 5 组用例尚未执行，本次产物测试整体仍失败。
- 其余新增接口场景的三阶段 golden、普通接口及 core dispatch 快照、既有结构装箱与 adapter 的旧拒绝测试仍待随完整路径更新和复验。不得通过跳过真实产物、物化无关默认正文或删去失败断言来完成这些验收。
- 确认构建、测试和配套编译器进程结束，且 `target` 没有打开的文件。通过 Cargo metadata 核对实际构建目录，恢复缺失的标准缓存标记后执行 `cargo clean --target-dir target`，删除 2674 个构建文件，回收 4.4 GiB。

本次按用户要求以 WIP 提交保存当前变更。接口抽象覆盖的产物闭环已恢复，ODR 装箱和其余主线继续实施，M23-7 尚未完成。

## 2026-09-28：泛型接口与 ODR 装箱的真实产物闭环

- MIR 先收齐实际名义 application，再投影生成 box 与 step/slot，修复生成表示先于 payload 的生产顺序。装箱 adjust 与目标成员使用完整 Strong/ODR callable subject，保留原角色、receiver、参数、结果和 GC effect 检查；泛型 box 的 descriptor 与 adjust 继续归原 ODR group/member。reader 将实际按需 helper 纳入类型表示清单，复用已完成的角色和字段检查，并核对所需的 GC 与接口关系，不要求发射未使用的支持族。
- 泛型抽象 setter 的隐式参数使用原 accessor 定义位置，普通命名参数保留自身来源，修复再次发布时缺失局部值 definition origin 的错误。接口只有完整槽契约，具体类和值类型保留实际派发表；普通与泛型接口共用生产和读取路径，未使用或被抽象 override 压制的默认正文不再成为机器依赖。实现规范与设计说明同步。
- 普通依赖值的 conformance 保留完整语义数据，但其已有 box 与 adjust 由 provider 提供，不再额外登记消费方没有调用的成员执行根。本地值和泛型 application 继续收集实际装箱实现。MIR 的导入 callable 映射保留所选定义的完整物理签名，装箱 thunk 直接由该签名取得 receiver、参数和结果，消除对旧 Interface 调用标记的依赖；调用方式与定义签名各自承担原职责，未放宽未引用 external callable 的拒绝检查。
- 结构 payload 的 MIR 语义引用沿 tuple 收集实际 nominal/application 叶子，function 和 pointer 使用已有表示。实际 box payload 与 inline array element 所需的结构 ManagedValue layout 和 scan 由 LIR 发射，沿用已有 StructuralType group；嵌套 tuple 复用现有 value constituent，不递归制造多余独立布局。producer 和 reader 共用既有 tuple/qualified-pointer 布局算法，保留本地物理定义、大小、对齐和扫描程序检查，没有增加格式版本或 runtime C ABI。
- 完成原有 18 组泛型成员和接口正例的真实产物链路，补齐剩余 6 组接口场景的 18 份 HIR/MIR/LIR golden。场景包含 class、struct、enum、接口默认方法、抽象覆写、消费方本地实现、属性、ZST 和含引用的大值；每组均发布 provider 后移走源码、由 consumer 实例化并再次发布、下游通过 `.slib` 对新类型实例化，最后链接、普通运行与移动 GC。产物断言同时核对实际 box descriptor 的 WeakODR linkage 和 adjust 的 DispatchAdapter member。
- 将结构装箱及函数类型变体 adapter 的旧拒绝测试改为真实产物正例，保留源码、三阶段 dump、发布、下游消费、链接和运行断言。新增独立 tuple 装箱与嵌套 tuple、ZST、三个外来引用的组合 fixture；组合在装箱后继续分配并发生移动 GC，解包后通过公开成员验证引用仍有效。core 的 Any 参数用例也完成结构 box 的真实发布与运行；三部分共新增 12 份 HIR/MIR/LIR golden。
- 同步 41 份既有产物快照，逐份核对只改变整包 artifact fingerprint，代码和 runtime 指纹及依赖关系保持。三份既有 dispatch/export 快照改为接口槽契约及空的接口自身 itable，实际 class 与 box 的物理表保持。普通依赖回归中的 18 份 HIR 快照逐份核对只有 callable use 编号变化，MIR/LIR 快照保持；没有删除真实语言规则、产物或运行断言。
- 补齐既有 HIR selection、HIR lowering 和 driver provider 单测所需的 source record、context 与函数 definition origin。修正测试数据以满足原完整来源合同，生产入口仍拒绝缺失来源，没有增加重复语义验证。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 与配套 `scoopc` 构建通过，无警告。修复回归发现的执行根和装箱签名问题后，关闭全部快照更新开关，完整运行 `cargo test --workspace`，5180 项测试全部通过，无失败或忽略，包含 126 项 driver 库测试、7 项 CLI、1 项配套编译器及 2 项文档测试。driver 库用时 342.22 秒；上述产物用例均在本次完整命令中再次验证。
- 确认全部构建、测试和配套编译器进程结束，且 `target` 中没有打开的文件。通过 Cargo metadata 核对构建目录及已有标准缓存标记后，执行 `cargo clean --target-dir target`，删除 2732 个构建文件，回收 5.1 GiB。

本项完成泛型接口及上述 ODR 装箱、结构 payload 与 adapter 场景的产物运行闭环。其他生成执行实体、delegate 和剩余组合仍沿下面主线推进，M23-7 尚未完成。

## 2026-09-28：泛型 protected 访问与产物格式简化

- 导入 protected 区域使用已有 `SourceNominalId`，同时保留普通声明与 generic template 的原身份。继承关系沿共有声明及其真实父类查询，词法类、显式接收者、构造器、方法和独立 setter 共用原规则；泛型实参不变性继续由正常类型检查负责。protected override 保留原槽区域，public 槽与 setter 的覆盖检查没有放宽。
- 删除持久 slot 中可从声明推导的 domain、重复访问域重放及只有测试调用的 protected 访问证明 API；`InheritanceSlotContractV1` 的 field 5 退役且不复用。声明位置、typed owner、签名、effect、slot、abstract target modality、实际实现引用与继承路径检查保留。`cross-cone-type-semantics/10`、required inventory、固定 profile descriptor 与 fingerprint 同步，旧 major 和旧七字段 slot 均拒绝；三份规范、M23-7 设计与 M23-6 后续修订说明一致，runtime C ABI 保持。
- 修复普通类继承泛型基类时的产物读取：实际 Strong 成员仍以自身普通声明的 exact type 为 receiver，MIR 签名按原 owner 精确匹配，不再额外要求该类进入只覆盖非泛型继承闭包的旧 HIR inheritance 清单。类型存在性复用共有声明和实际 MIR 表示，不生成替代布局或访问凭证。
- 新增 9 组正例与 9 组反例及 36 份 golden。正例覆盖两组 binder 的 protected 方法、继承链与 `super`、虚方法覆写、主次构造器、独立 setter、protected setter 覆写及基类静态调用、本地函数词法捕获、ZST、含三个引用的大值、普通子类与 object。每组均发布 provider 后移走源码，消费并再次发布，下游只读产物并为新类型实例化，最后链接普通运行与移动 GC；provider 未预先实例化泛型的断言保持。反例对基类/兄弟类接收者、外部构造、setter 不可见及覆写缩窄逐一锁定源码位置、诊断，并确认不产生目标产物。
- 新增 HIR 正反例与 foundation 生成检查通过；原有 18 组泛型成员、接口和值类型装箱真实产物回归通过，既有 HIR/MIR/LIR golden 保持一致。HIR 与 slib 的 1457 项库测试通过，包含新格式固定字节、指纹和旧版本拒绝验证。
- 同步更新 43 份既有 core 与 shape dependency 产物快照，逐份确认仅 `artifact` 指纹变化，代码、runtime 指纹、依赖图和物理定义保持一致。未通过更新开关接受其他输出差异。
- 最终依次执行 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，均通过；随后关闭全部快照更新开关，使用本轮构建的真实配套 `scoopc` 执行 `cargo test --workspace --no-fail-fast`，5167 项通过、0 失败、0 忽略。其中 driver 全部 127 项通过，耗时 346.54 秒；完整日志位于 `/tmp/scoop-m23-7-protected-workspace-verified.log`。
- 确认构建、测试和配套编译器进程全部结束，且 `target` 中没有打开的文件；通过 Cargo metadata 核对实际目录、确认非符号链接并恢复标准缓存标记后，执行 `cargo clean --target-dir target`，删除 2802 个构建文件，回收 5.7 GiB。

本项完成上述泛型访问及覆写组合的真实产物闭环；M23-7 的其余主线仍须继续，未宣布阶段完成。

## 剩余主线

1. 继续共用可移植节点，完成 delegate template 的生产、读取与消费；补齐其他物理角色的内容摘要，接入已有成员合并入口，随实际 payload 同步升级正式 profile inventory。
2. 在已通过的私有 helper、定义处绑定、局部函数捕获、成员默认值与两组 binder 基础上，补齐 vararg、组合 bound、bound dispatch，以及 lambda、匿名函数和 callable reference 的捕获组合。
3. 在已完成的泛型 class 共有 callable/dispatch、消费方构造与成员、泛型接口及属性、protected 方法/构造/setter、消费方覆写、普通子类与 object 闭环基础上，继续覆盖其他成员组合，以及递归扫描程序的实际对象 atom。
4. 在已完成的泛型与结构装箱、函数类型变体 adapter 基础上，继续完成其他 adapter、coroutine 与按需 shape support，验证共同 member 一致、独立 member 并集、EH/stackmap 和实际地址合并。
5. 泛型委托扩展属性接入完整 LazyAccess application、现有初始化协调、失败共享与移动 GC。
6. 切换 core、driver、reader/publisher、cache 与全部 fixture，删除无调用的旧路径，完成真实配套编译器和 runtime 的全仓验收。

验收始终以源码与实际产物为依据。最终必须逐项核对设计第 12、14 节，不能用局部单测替代跨 Cone 链接运行或宣布阶段完成。
