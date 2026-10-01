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

## 2026-09-28：泛型计算扩展属性的产物消费与再次发布

- 导入 getter/setter 保留实际 `PersistentPropertyAccessorId`，从所属 property 取得 binder，沿既有 generic template/application 队列具体化。扩展 accessor 使用 `NoOwner` 与 property 实参建立原有 application key；共有正文再次发布时保存原 accessor 与实参，下游消费继续请求同一模板，不复制成普通函数声明或制造命名参数协议。产物格式与 runtime C ABI 保持。
- getter 候选仅从接收者推断实参，最具体候选比较使用未实例化的声明 receiver；结果期望与赋值右侧不反向决定 property 实参。setter 复用 getter 探测得到的参数组，沿现有 place 类型检查、适配与求值顺序处理赋值、复合赋值及前后自增；纯写入只物化 setter 正文。只读、独立 setter 可见性、结果/右侧类型与 kind bound 继续使用正常诊断。
- 修复同一 application 在 consumer 与 downstream 重复物化时的 Strong 专用查询假设。LIR 派发表按实际 Local/External 引用选择 provider 的完整 callable ABI，reader 借用保存的引用定位 provider，再以 MIR 的真实实现目标核对；已有 slot、body、签名和 ODR 内容合并检查保留。当前 MIR 表示与 LIR layout 已定义的 descriptor 直接使用 Local 引用，不再因依赖中还有相同 ODR descriptor 而误报重复。
- 真实重复应用运行暴露了 abstract trap 的多余托管字符串：MIR 现以显式 `Trap { message: String }` 终结符保存原生诊断，结构化 lowering 使用对应终止语句，LIR 复用 body 所属的既有 C 字符串 atom。消息不再进入托管 String 池或生成 immortal 登记；删除旧 `RuntimeFn::Trap` 普通调用路径及其可缺失的 LIR 调用结果。普通 trap、共享 trap block 和跨 producer 的 ODR 常量回归通过，runtime 的对象范围检查保持。
- 新增 8 组正例、5 组反例和 29 份 golden。正例覆盖读写、纯写、接收者继承/typealias、重载选择、局部捕获与私有 helper、Unit/ZST、含三个引用的大值、求值顺序及自增。每组发布 provider 后移走源码，consumer 实例化并再次发布，下游仅使用产物为重复的 Int 和新的引用类型实例化，再完成真实链接、普通运行与移动 GC；provider 未预先实例化泛型的断言保留。反例逐一锁定唯一错误、消费方源码位置和诊断，并确认不产生目标产物。
- 同步 28 份既有 MIR/LIR golden，逐份核对只有 trap 消息文本、无用托管常量删除及常量编号调整。另有 61 份产物快照变化：41 份 core 产物摘要、2 份仅依赖整包摘要、4 份代码摘要，以及 14 份登记、摘要图和符号数量；数量差异分别对应实际删除的 2 或 3 个 trap 托管对象及其登记/边界符号。已有布局、ABI、引用关系和损坏产物的拒绝断言保留。
- 最终 `cargo fmt --all` 与 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过，无警告。同版实现的首次 `cargo test --workspace --no-fail-fast` 共 5169 项，5145 项通过、24 项仅因旧快照失配；完成上述快照更新与差异核对后，关闭全部更新开关，复验覆盖全部失配的 25 项聚合回归，25 项全部通过，耗时 288.17 秒。两轮之间仅修改快照和进度文档，本批 5169 项均已取得通过结果，无忽略。全仓与最终复验日志分别为 `/tmp/scoop-m23-7-extension-workspace.log`、`/tmp/scoop-m23-7-extension-affected-verified.log`。
- 确认构建、测试及配套编译器进程结束，且 `target` 没有打开的文件；通过 Cargo metadata 核对目录后恢复缺失的标准缓存标记，执行 `cargo clean --target-dir target`，删除 2600 个构建文件，回收 4.6 GiB。

本项完成泛型计算扩展属性的上述产物运行闭环。泛型委托扩展属性的 LazyAccess 与其余主线继续实施，M23-7 尚未完成。

## 2026-09-28：泛型托管 String 常量的跨产物合并

- 泛型 callable 中的托管 String 沿已有 `ImmortalObjectKey` 保留 materialization 与结构化定义位置，直接取得原 callable 的 ODR group。对象及 immortal registration 分别形成实际物理 member，发射为保留地址意义的 weak ODR 定义；普通 Strong 常量沿用原有登记和摘要合同。实现规范与阶段设计同步，未增加常量池、文本去重或 runtime ABI。
- 共有 physical content 表补入 String 对象：ABI 覆盖对象、String exact type、尺寸和对齐，LIR 摘要再覆盖 UTF-8 内容。投影复用已计算的 String 语义；reader 复用实际对象读取与 ObjectDefinition，登记的最终 ODR definition 直接依赖登记对象、String 对象和登记 LIR 摘要。目录、补丁、image 摘要与冲突诊断沿既有成员入口接入，不重放对象解析或布局验证。
- 物理 descriptor/callable 索引保留不同 provider 的同一 ODR 定义，引用继续绑定保存的 provider。同一 provider 重复、重复 Strong 或不同 definition identity 仍被拒绝，内容一致性交由原 ODR 合并检查。真实运行 harness 在该合并结果上按 member 登记一次，保留 runtime 对象范围检查。
- 新增独立与组合两组真实产物用例、9 个源码 fixture 和 12 份 HIR/MIR/LIR golden。先发布 provider 与两个相互独立的 consumer，再移走各自源码，由下游只读产物再次实例化；覆盖函数、分支位置、泛型 class 字段初始化与方法、计算扩展属性、局部函数，以及 Int、Long 和下游新建引用类型。真实链接、普通运行与移动 GC 断言同 member 地址一致、不同 application/定义位置的同文字符串地址独立，并检查内容与 GC 后可用性。
- 新增等长内容冲突用例：保持声明、实参、member identity 与 ABI 不变，仅改变字符串内容，对象及登记的 definition 均改变，共有 ODR 合并拒绝冲突。两项新增聚合测试在关闭快照更新开关后通过，耗时 17.03 秒；12 份新 golden 已逐份审阅。
- 最终 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 及真实配套 `scoopc` 构建均通过，无警告。关闭全部快照更新开关运行 `cargo test --workspace --no-fail-fast`，5171 项通过、0 失败、0 忽略；其中 driver 全部 130 项通过，耗时 353.29 秒，既有快照无需更新。完整日志为 `/tmp/scoop-m23-7-immortal-workspace.log`。
- 确认本仓库构建、测试及配套编译器进程结束，且 `target` 中没有打开的文件；通过 Cargo metadata 核对实际目录及标准缓存标记后，执行 `cargo clean --target-dir target`，删除 1560 个构建文件，回收 3.0 GiB。

本项完成上述泛型托管 String 的产物与运行闭环。泛型委托扩展属性的 LazyAccess 及其余主线仍须继续，M23-7 尚未完成。

## 2026-09-29：泛型委托扩展属性的产物消费闭环

- source generic delegate 使用独立模板，完整 receiver 实参具体化为共享 storage、LazyAccess unit 和 failure root。导入模板保留原 property、effective type 与 initialization generated callable，和本地模板共用具体化队列；initializer 保留真实正文，ensure 使用已有初始化协议。声明级 unit 不进入参数自由 startup 服务。
- 共有 HIR interface 的必需 field 13 保存委托模板，内部存储读写和 ensure 使用 typed property 与非空实参组。interface 升至 `/36`，profile inventory、固定向量与 fingerprint 同步；HIR 导出实际 application unit/group，MIR 导出初始化 callable 成员，reader 从原声明核对类型位置与初始化角色。
- 静态存储、初始化 cell/descriptor、登记与 callable 使用实际 Strong/ODR definition plan、符号和 relocation。共有 canonical shape 与成员摘要覆盖这些定义；存储布局/scan 沿 exact type 归属，登记摘要复用已验证的对象 leaves，存储内容排除消费方 layout-provider 路由信息。
- 新增三份真实源码 fixture 及 HIR/MIR/LIR golden：两个 receiver 对象共享同一完整参数组的可写 delegate，三个完整参数组分别物化独立 unit。provider 和 consumer 发布后移走源码，由第三个 Cone 仅消费 `.slib` 再次编译、链接，普通模式与移动 GC 压力模式均返回 42。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过，无警告。七个相关库共 3785 项测试取得通过结果；首轮的六项旧 wire/profile 断言更新后，HIR 与 slib 共 1457 项重新通过。真实产物测试在关闭快照更新后通过，日志为 `/tmp/scoop-m23-7-delegate-verified.log`。本项尚未执行阶段最终全仓回归。
- 确认构建与测试进程结束，通过 Cargo metadata 核对 `target` 路径及缓存内容，恢复缺失的标准缓存标记后执行 `cargo clean --target-dir target`，删除 2777 个文件，回收 5.8 GiB。

本项完成基础委托模板的源码、产物读写和执行链路；跨 sibling 的初始化一次、失败状态共享、求值顺序及其他组合仍继续验证，M23-7 尚未完成。

## 2026-09-29：跨 sibling 委托初始化与失败共享

- 外部初始化服务目录只收集参数自由的 Strong unit；泛型 receiver application 在使用方物化，重复定义沿共有 ODR 合并入口处理。实际 HIR 调用根同时接受 initialization application 和该 application 内的 generated callable，并要求 MIR 中存在同组、同角色实体的实际 callable body。新增负例确认只有 ensure 不能满足 initializer，另一组 receiver arguments 也不能借用该 body。
- 参数自由 callable 的签名允许声明作用域内的闭合 nominal application，例如 core 的 `IllegalStateException` 构造参数 `Option<String>`。未知 nominal origin 与未替换 binder 仍被拒绝；已有名义类型声明、exact type 和 ABI 规则保持，未增加类型识别旁路。
- 新增 provider、两个独立 sibling 和 consumer 的四份源码及六份 HIR/MIR/LIR golden。发布后移走各 provider 源码，真实产物编译、合并、链接和执行验证同一参数组的 `by`、`provideDelegate` 只运行一次，多个 receiver 共享可写状态，两组不同完整实参分别初始化，失败不重试且跨 sibling 共享。运行 harness 按已合并 storage identity 登记一次 GC root，避免重复扫描同一槽位。
- 失败断言遵守现有 throw-by-value 语义：不同 catch 各自物化异常副本，通过 payload 中托管引用的身份、修改后的内容与失败计数验证共享；同时核对实际 cell、storage、failure root 及 registration 的 ODR 合并。普通模式与移动 GC 压力模式均返回 42，实际收集后 delegate 和 failure payload 仍可使用。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 与同版配套 `scoopc` 构建通过。HIR 与 slib 共 1458 项库测试通过；关闭快照更新后的真实 sibling 聚合测试通过，用时 12.78 秒，日志为 `/tmp/scoop-m23-7-delegate-siblings-verified.log`。本批尚未执行最终全仓回归。

本项完成委托的 sibling 初始化与失败共享；求值顺序、更多有效 delegate 表示及损坏产物组合继续推进，M23-7 尚未完成。

## 2026-09-29：外来委托角色与完整访问组合

- 消费方声明的扩展属性可以从外来 class、struct、enum、interface 的完整 receiver 实参识别 binder。委托成员和扩展复用普通 callable probe、最具体候选比较与调用生成；本地方法与继承的依赖方法在同一层比较，扩展 import alias 保留原 typed role。泛型宿主中的普通方法不再因实现需要单态化而被误判为带自身类型参数的方法。
- 源级访问器和 initializer 直接保存实际 imported call。局部委托保留已选 target、实际参数类型与 effect，在读取、赋值和局部函数捕获中复用；已降低的协议参数保持单次求值，不引入默认参数展开或多余临时变量。移除失去调用者的两条独立委托重载包装入口。
- 新增 16 个真实产物正例及 48 份 HIR/MIR/LIR golden，包含 receiver/RHS/by/provide/get/set 顺序、只读和只写物化、ZST、24-byte 大值、含引用值、别名及转导出、直接与间接初始化 cycle、本地与外来 receiver、成员与扩展委托、局部捕获和混合重载。provider 与 consumer 发布后移走源码，第三个 Cone 用自身引用类型再次实例化；普通运行与移动 GC 压力模式均通过。
- 18 个语言反例检查真实 span 与诊断，覆盖从结果或 RHS 推导 binder、runtime receiver、非法 `this`、角色的 suspend/generic/default/vararg、参数数量与返回类型、缺失 operator/setter、只读及类型不匹配、初始化挂起和混合重载歧义。非法 default/vararg 角色直接在声明参数处拒绝，避免请求无关的隐式数组。原 M21 委托反例重新接入真实编译，退役“泛型扩展属性不能委托”的旧诊断。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 与最新配套 `scoopc` 构建通过。关闭委托快照更新后运行完整 workspace，共执行 5180 项，5157 项通过；23 项旧断言或快照不匹配已修正，其中包括 Strong/ODR 存储链接、闭合泛型签名对应 getter 的普通 callable 归属及旧产物摘要。修正后 codegen 的 301 项全部通过，关闭全部相关快照更新后 core 导出组的 22 项全部通过，用时 290.72 秒；全仓中其余测试和全部委托用例均已通过。日志分别为 `/tmp/scoop-m23-7-delegate-workspace.log`、`/tmp/scoop-m23-7-delegate-codegen-verified.log` 和 `/tmp/scoop-m23-7-delegate-core-verified.log`。
- core 快照差异已检查：补充表不再重复列出已进入普通 callable 表的闭合签名 getter，实际 getter 保留；代码及 runtime 摘要不变，产物摘要随元数据迁移更新。确认所有本仓库构建与测试进程结束、`target` 无打开文件，通过 Cargo metadata 核对目录并恢复缺失的标准缓存标记后执行 `cargo clean --target-dir target`，删除 2904 个文件，回收 6.6 GiB。

本项完成委托访问、外来角色和上述语言组合。初始化正文中的更多生成实体及完整 unit 的损坏产物验证仍继续，M23-7 尚未完成。

## 2026-09-29：委托 unit 的真实损坏产物验证

- 复用已经完成发布、移走源码、下游消费与运行的真实委托产物，分别互换 initializer/ensure、删除 unit、删除 delegate storage、删除 failure root 和删除 initializer callable。修改后重新编码原 archive 并同步 LIR 与产物摘要，确保反例进入正式语义 reader，而非被外层摘要差异提前拒绝。
- 五项反例精确断言消费方 provider 与实际错误类型：角色互换定位到原 unit 的 `initializer_role`，缺失 unit/storage/root 对应登记表长度，缺失 callable 对应完整登记 surface。复用现有正确性检查，没有增加生产阶段验证、格式字段或公共测试工厂。
- `cargo fmt --all` 与 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过，无警告。使用最新配套 `scoopc`，关闭快照更新后，基础委托产物聚合测试通过，用时 10.42 秒；包含真实发布、下游再次消费、链接、普通运行、移动 GC 及上述五种反例。日志为 `/tmp/scoop-m23-7-delegate-artifact-verified.log`。

本项完成委托 unit 的上述损坏产物验证。初始化正文的生成实体和其他组合继续推进，M23-7 尚未完成。

## 2026-09-29：委托 initializer 的局部函数与外来初始化依赖

- initializer 中的局部函数保留原 source declaration，普通局部函数通过 `EnclosingInitializationApplication` 继承实际 receiver 参数组；局部泛型函数另保留自身实参。生成正文沿原 generated callable、unit 与 property/type 关系查询声明所属 Cone。源位置从已读取的依赖 foundation 取得，不复制外来声明或增加 initializer binding。
- 泛型 initializer 直接访问外来 object/property 时，依赖关联本次物化的 application unit；该 local unit 与被引用的参数自由 Strong 初始化服务分别处理。生产端使用实际 MIR root，reader 复用已验证的 LIR unit 登记检查本地存在性，外部服务目录仍只收集真正的 Strong 服务。移除这条合法路径上的旧 generic unit gate，缺失本地 unit 与错误 dependency provider 的反例继续拒绝。
- 新增独立与组合两组真实产物用例、四份源码与六份 HIR/MIR/LIR golden，覆盖 initializer 局部函数、引用捕获、局部泛型递归、默认参数和直接访问外来 object。provider 与 consumer 发布后移走源码，下游仅凭产物再次实例化新的引用类型、链接并执行；普通运行与移动 GC 均通过。组合验证初始化只执行一次、24 字节含引用 delegate 的间接参数/sret、实际 GC 扫描，以及实际 unit 指向 provider 的外部初始化依赖。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 及最新配套 `scoopc` 构建通过，无警告。HIR、MIR、slib 的 1799 项测试与 HIR lowering 的 1267 项测试全部通过。关闭全部快照更新后，12 项 driver 聚合回归全部通过，耗时 286.31 秒，覆盖全部委托正反例及受影响的 core 初始化、MIR/LIR 导出与 reader 路径；六份新 golden 已检查，既有快照无需更新。日志分别为 `/tmp/scoop-m23-7-delegate-initializer-libs.log`、`/tmp/scoop-m23-7-delegate-initializer-hir-lower.log` 与 `/tmp/scoop-m23-7-delegate-initializer-verified.log`。本批为定向验证，不替代阶段最终全仓验收。

本项完成 initializer 局部函数、默认参数及外来初始化依赖的真实产物闭环；closure 与其他生成实体继续推进，M23-7 尚未完成。

## 2026-09-29：委托 initializer 的闭包与捕获组合

- 导入 lambda 与匿名函数保留原 generated body、定义路径和有序 binder 替换，正文捕获槽关联定义处 binding，创建表达式保存本次实际捕获来源。函数值调用复用完整函数类型和位置参数；具体化生成既有 concrete 闭包实体，MIR/LIR 沿普通环境、invoke 和 GC 路径处理。未增加产物格式、runtime ABI 或外来源码副本。
- 捕获源保留首次引用的定义位置，并使用闭包创建处的真实求值上下文，修复再次发布时 initializer 执行来源与声明文件上下文混用的问题。局部函数前置捕获参数在已有局部值索引中关联原值，闭包及内层局部函数继续复用同一身份，解析不依赖函数遍历顺序。MIR 去除要求环境和捕获值 context 完全相等的旧限制，继续检查实际字段、invoke、ODR group 与 canonical identity；词法引用由 HIR 完成解析。实现规范和设计同步。
- 新增六组真实产物用例、八份源码与十八份 HIR/MIR/LIR golden，覆盖无捕获 lambda、引用捕获、匿名函数、嵌套闭包、局部泛型递归与默认参数、ZST 和 24 字节含三个引用的值，以及局部函数返回闭包后再调用内层捕获函数。provider 与 consumer 发布后移走源码，下游仅凭产物以自身引用类型再次实例化、链接和执行；六组在普通模式与移动 GC 下均返回 42，并核对初始化一次及实际外来初始化依赖。大值内联进入闭包环境，三个引用位置保留在 GC 扫描表中。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 及最新配套 `scoopc` 构建均通过，无警告。关闭快照更新后运行 `cargo test --workspace --no-fail-fast`，5184 项通过、0 失败、0 忽略，其中 driver 全部 139 项通过，耗时 362.72 秒。新增捕获身份断言、MIR 外层值身份保留用例及六组真实产物用例均通过；十八份新 golden 中的捕获顺序、invoke、环境布局与 GC roots 已检查，既有快照无需更新。完整日志为 `/tmp/scoop-m23-7-delegate-closures-workspace.log`。
- 上一批局部函数功能提交后，确认本仓库构建、测试进程已结束、`target` 无打开文件，并通过 Cargo metadata 核对实际目录和缓存标记，执行 `cargo clean --target-dir target`，删除 1607 个构建文件，回收 3.1 GiB。本批使用重新构建的配套编译器完成验证；全仓通过后再次确认无占用，核对实际目录及内容并恢复缺失的标准缓存标记，再用 Cargo 删除 2508 个构建文件，回收 4.5 GiB。

本项完成 initializer 的 lambda、匿名函数及上述捕获组合。callable reference、默认值和其他主线继续推进，M23-7 尚未完成。

## 2026-09-29：委托 initializer 的函数引用与动态派发

- 导入的函数引用保留提供方 invoke key、词法父模板和有序宿主实参，实际目标区分泛型 callable application 与普通依赖 use。命名函数、局部函数、绑定成员及绑定/未绑定扩展复用既有 concrete 函数引用和 MIR invoke；普通依赖目标直接调用提供方代码。捕获输入与 lambda 共用定义处 binding、创建处求值和前置参数 ABI，没有复制外来源码函数或增加产物格式、runtime ABI。
- Export HIR 的 invoke key 投影支持初始化函数、访问器和词法生成函数作为父节点，修复只接受源码函数的旧假设。实际引用目标随创建表达式进入已有可执行依赖集合，供 MIR 取得完整外来 callable；未实例化的模板引用不成为机器根。本地默认值身份扫描只读取真实本地引用，导入引用沿原词法模板取得 materialization。实现规范和设计同步。
- 新增 15 组真实产物用例、17 份源码与 45 份 HIR/MIR/LIR golden，覆盖普通与泛型函数、局部与局部泛型函数捕获、对象与值类型接收者快照、接收者单次求值、泛型宿主成员、virtual/interface 动态派发、绑定与未绑定扩展、ZST 和 24 字节含引用值、lambda 内局部函数引用及局部函数返回引用。provider 与 consumer 发布后移走源码，下游仅凭产物用自身引用类型再次实例化、链接和运行；15 组在普通与移动 GC 模式下均返回 42，并核对初始化一次和实际外来初始化依赖。
- 新增 HIR 断言检查原 invoke identity、初始化 application、接收者身份、外来机器调用根，以及跨局部函数前置参数的同一捕获值身份。MIR golden 保留 virtual/interface 调用；大值内联进入函数引用环境，三个引用位置进入递归扫描表，ZST 沿现有 ABI 省略。全部新 golden 已检查，既有快照无需更新。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 及最新配套 `scoopc` 构建通过，无警告。关闭快照更新后运行 `cargo test --workspace --no-fail-fast`，5186 项通过、0 失败、0 忽略；其中 driver library 的 140 项全部通过，用时 368.83 秒。完整日志为 `/tmp/scoop-m23-7-delegate-references-workspace.log`，15 组用例的首次实际产物验证日志为 `/tmp/scoop-m23-7-delegate-references-driver.log`。
- 全仓通过后确认构建、测试进程结束及 `target` 无打开文件，通过 Cargo metadata 核对实际目录与缓存内容，恢复缺失的标准缓存标记后执行 `cargo clean --target-dir target`，删除 2508 个文件，回收 4.5 GiB。

本项完成上述 initializer 函数引用与捕获、派发组合。类型参数 bound 成员及派生相等目标、导入默认值中的生成实体和消费方源码直接引用外来函数仍属后续主线，M23-7 尚未完成。

## 2026-09-29：类型参数 bound 调用、函数引用与再次发布

- 导入模板中的 bound 调用和绑定成员引用保留实际 receiver type、接口/member/slot、完整函数签名及已选声明。具体化按实际 conformance 选择本地或外来实现；class override 保留 virtual dispatch，抽象与接口接收者保留接口槽，接口默认实现按其实际声明 owner 适配。值接收者可以直接调用自己的实现，未装箱值不再发布物理派发表；下游后来装箱时仍按真实需要生成表并合并共有定义。
- 源级本地与外来 class/interface 上界合并到同一结构，支持混合约束、继承接口及完整泛型应用，保留单 class、重复 interface、kind 互斥与 invariant 实参规则。源级类型参数的外来成员调用保留类型参数接收者，不制造提供方的本地 FunctionId。进入共有声明闭包且签名可物化的 private/internal 构造器随实际 MIR/LIR callable 导出，供下游再次实例化原模板；源码可见性保持，删除旧测试中把这些有效构造器重新拼装为非法导出的路径。
- 默认参数中的 bound receiver 保存完整类型 key，展开后的具体类型可以再次发布；正文与引用目录共用同一 bound target，并包含 receiver、接口和签名的类型引用。临时语句和控制节点使用消费调用的位置，内部表达式保留定义与求值双来源，`@NoGC` 错误定位到本次调用。HIR `cross-cone-interface` 升至 `/37`，移除专用 binder 对及其独立遍历，生产、reader、profile 和固定向量同步迁移。
- Int8/Int16/Int/Long、对应无符号整数、Boolean 和 String 的 bound 调用使用 core 的实际声明与接口实现。已具体化的 receiver 和模板内泛型转发的名义 bound 实参按需保留 primitive 声明；conformance 复用已有缓存，不在转发调用处重做语言约束检查。普通算术不会因此加载无关的 bound 方法。
- 修复两处真实的重复 ODR 定义冲突：代码生成复用 use/def 遍历，按正文首次使用或定义的顺序分配局部栈槽；同一 callable 的本地和依赖 relocation 在对象摘要中统一为实际 callable-body 身份。原普通依赖 runtime target tag 11 退役，Strong/ODR callable 分别使用既有 tag 1/14，非 callable shape 保持原规则。LIR `link-identity-closure` 升至 `/5`；三份 spec 与设计同步，旧产物和缓存重建，runtime C ABI 保持。
- 新增 27 份源码、13 组真实产物正例、39 份 HIR/MIR/LIR golden 和 11 份诊断 golden。覆盖 class/接口派发、泛型与继承接口默认实现、抽象接收者、本地 conformance、混合来源 bound、默认参数、原始类型及下游新增装箱；provider 与 consumer 发布后移走源码，由第三个 Cone 以自身引用类型再次实例化并链接运行，普通与移动 GC 模式均返回 42。反例验证实际调用或声明处的位置与诊断，包括不满足 class/interface、精确泛型接口、不匹配引用结果、默认值 effect、重复接口、多个 class 及 kind 混用。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和最新配套 `scoopc` 构建通过，无警告。关闭全部快照更新后运行 `cargo test --workspace --no-fail-fast`，5194 项通过、0 失败、0 忽略；driver library 的 144 项全部通过，用时 369.30 秒。既有快照差异已核对：22 份继承/属性 HIR 只改变会话内 callable 编号，core 与类型支持快照补齐实际构造器、移除未装箱值物理派发表并更新相应摘要。完整日志为 `/tmp/scoop-m23-7-bounds-workspace-verified.log`。
- 全仓通过后确认本仓库构建、测试和运行进程结束、`target` 无打开文件，通过 Cargo metadata 核对目录及缓存内容，恢复缺失的标准缓存标记后执行 `cargo clean --target-dir target`，删除 2954 个文件，回收 7.0 GiB。

本项完成上述 bound 调用、引用、默认参数和再次发布闭环。导入派生相等目标、默认值中的生成实体及消费方源码直接引用外来函数继续推进，M23-7 尚未完成。

## 2026-09-29：外来泛型值的派生相等与再次发布

- 外来泛型 struct、enum 从实际共有字段与 variant identity 生成完整 typed 比较正文；字段按静态类型使用本地、外来或 bound 的普通成员选择，保留调用和派发。struct/tuple 按字段顺序短路，enum 只读取当前 variant 的 payload；显式同类型 `equals` 替代派生，其他重载不屏蔽它。没有成员相等的引用、仅有扩展方法和静态类型为 `Any` 的字段继续在前端拒绝。
- 具名导入正文恢复其已声明的类型参数作用域，派生 helper 保留实际定义与求值双来源。helper 没有源码词法声明：reader 从当前物化产物取得 generated key，从位置所属产物验证文件、context 和 span，允许提供方模板尚未物化消费方的 exact helper。类型拥有的 helper 使用类型的真实访问域，避免首次需求所在文件限制后续合法调用；越界位置的反例保持拒绝。
- HIR 以完整 exact owner 生成已有 `DerivedEquality` identity，同一具体类型的直接比较和导入模板调用共用一个 helper。MIR 沿已有 `ExactOwnerRoot` 为泛型及结构 owner 发射 ODR callable，参数自由本地名义 owner 保持 Strong；相等 binding 与 reader 查询实际 MIR 定义和签名，不再统一假设 Strong。复用既有 group/member、产物格式与 runtime ABI，单次 reader 检查复用 Boolean 类型查询结果。
- 新增 16 份源码、8 组正例、6 组反例和 30 份 golden。正例覆盖 struct、泛型引用字段、enum、嵌套值与本地 conformance、显式重载、ZST/tuple、24 字节含引用值、字段顺序/短路和默认参数。提供方及消费方发布后移走源码，由第三个 Cone 仅消费产物，以自身引用类型再次实例化，同时重复已有 Int application；全部完成真实链接、普通运行与移动 GC，已有 helper 的 ODR 定义合并通过。反例逐一检查相等表达式的真实位置、诊断和不产生目标产物。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和最新配套 `scoopc` 构建通过，无警告。关闭全部快照更新后运行 `cargo test --workspace --no-fail-fast`，5198 项通过、0 失败、0 忽略；driver library 的 145 项全部通过，用时 418.07 秒。三阶段 golden 中的字段访问、短路分支、variant tag/payload、ZST 与含引用大值 ABI 已核对，既有快照无需更新；缺失相等 helper、源码记录和签名不匹配等原有 reader 反例继续通过。完整日志为 `/tmp/scoop-m23-7-equality-workspace.log`。
- 全仓通过后确认构建、测试及配套编译器进程结束、`target` 无打开文件，并用 Cargo metadata 核对目录和缓存内容；恢复缺失的标准缓存标记后执行 `cargo clean --target-dir target`，删除 45976 个构建文件，Cargo 报告总大小 43.8 GiB。

本项完成上述具名泛型正文、成员和默认参数中的派生相等闭环。参数自由外来值的 helper 消费、显式派生 `equals` 调用与函数引用、词法生成正文中的 bound 组合仍继续推进，M23-7 尚未完成。

## 2026-09-29：消费方源码函数引用与再次发布

- 消费方源码中的普通／泛型 `::name`、绑定成员和绑定／未绑定扩展引用共同收集同层本地与依赖候选，以完整函数签名、宿主实参和期望类型推断，并在同一类型环境中比较最具体候选。只有选中候选提交导入模板；函数引用保留全部参数，默认参数不会缩短签名。unsafe、suspend、kind bound、不可推导实参、歧义、结果类型、访问域以及不允许的构造器／未绑定成员形式继续给出源码诊断。
- 已选目标沿原 callable use 或完整 application 消费提供方实现，创建处只生成自己的 invoke。绑定接收者求值一次并保存其值，普通、泛型宿主、virtual/interface、protected 和类型参数 bound 的成员引用复用现有目标选择、捕获环境与派发。共有默认值的成员引用目录保留完整 owner type，支持下游解析实际 callable owner。
- core 原语成员引用保存实际声明与正规化 intrinsic kind，MIR invoke 复用普通运算降低；整数除零继续构造并抛出 ArithmeticException，带符号最小值除以／取余负一、移位计数和转换沿原语义处理。共有模板仍使用原成员引用，消费时从已解析声明恢复运算，不发布不存在的 Strong 函数；无新增 wire 字段或 runtime ABI。
- 默认值模板之间的代换保留原引用身份；展开到可执行正文时，invoke 使用创建点的词法 root、新路径和完整宿主实参。局部函数引用从实际目标函数的前置捕获参数解析原值身份，避免把 invoke 的上下文当作目标函数的捕获作用域。本地 lambda／匿名函数捕获读取保留首次引用的定义位置，求值位置使用创建点；共有 invoke key 与本地具体化共同选择最近词法 callable 父节点，嵌套函数再次发布后复用同一机器身份。
- 新增 30 份源码、13 组真实产物正例、13 组反例和 52 份 golden。正例覆盖混合来源重载和查找层、宿主与 callable 两组 binder、默认参数多次展开、局部函数／lambda／匿名函数捕获、原语运算与异常、动态派发、接收者快照、ZST 和 24 字节含三个引用的大值。反例逐一检查引用处的真实位置、诊断与不产生目标产物。另有挂起签名和泛型正文直接全局读写两份 HIR 用例，仅验证前端与共有接口生产，其运行闭环仍属后续工作。
- 提供方与消费方发布后移走源码，下游仅凭产物以自身引用类型再次实例化，并重复已有 Int application；13 组均完成链接、普通运行和移动 GC，重复 ODR 定义合并通过。复测同时覆盖原有 15 组 initializer 引用及默认值普通 reader。六份既有 golden 差异已逐项核对：两份 MIR 增加不同默认展开创建点的闭包与 invoke，四份嵌套引用 MIR/LIR 更新最近词法父节点对应的身份摘要；捕获 ABI、派发、布局和 GC 扫描保持实际程序语义。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和最新配套 `scoopc` 构建通过，无警告。关闭全部快照更新后运行 `cargo test --workspace --no-fail-fast`，5201 项通过、0 失败、0 忽略；HIR-lower 的 1278 项与 driver library 的 146 项全部通过，后者用时 379.88 秒。完整日志为 `/tmp/scoop-m23-7-source-references-workspace-verified.log`，受影响的三组产物复测日志为 `/tmp/scoop-m23-7-source-references-repaired-driver.log`。
- 全仓通过后确认构建、测试与配套编译器进程结束、`target` 无打开文件，并通过 Cargo metadata 核对目录与标准缓存标记；执行 `cargo clean --target-dir target`，删除 2755 个构建文件，Cargo 报告总大小 6.0 GiB。

本项完成上述消费方源码函数引用的候选选择、再次发布与运行闭环；M23-7 尚未完成。

## 2026-09-29：泛型正文共用普通顶层状态

- 普通顶层 stored property 在声明阶段统一生成真实 getter/setter，涵盖 private/internal 和静态初值。源码与导入模板调用同一原访问器，backing 读写、初始化 ensure、存储与 GC root 留在声明方；隐式访问器沿已有 hidden support 闭包发射，不新增全局存储导入表。泛型函数、局部函数／lambda、构造和成员、默认参数及 delegate initializer 在不同类型实参和再次发布后共用原状态，公开查找仍按原可见性处理。
- 纯存储、静态初值且完整 value type 为 GC-free 的隐式顶层访问器使用 NoGc 合同；runtime ensure、managed ref 和自定义正文保持原 effect 检查。签名直接复用已解析的属性类型，避免重复解析 `Ptr` type syntax 并对同一来源追加多份诊断。默认参数中访问器引用按源码属性类别诊断；前端测试改用已有真实声明查询，删除根据名义类型猜测 provider 的旧测试实现。
- 修复普通 ZST 属性的初始化登记：零字节值关联原属性拥有的 `StaticPlaceToken`，继续保留 initializer、ensure、cell 和 RHS 副作用，非零存储不得使用该角色。复用已计算的存储大小，不重放布局计算；新增反例拒绝非零字节 token，实际 ZST 初始化和赋值通过产物运行验证。
- 泛型 enum 状态用例暴露了类型描述符引用的 ODR 摘要差异：消费方与下游的重复 callable 对象字节完全相同，但本地／外来 Strong 描述符目标编码不同。对象摘要现统一编码实际 exact type 与 TypeDescriptor role，已有 ODR 描述符沿原 member 编码；普通依赖记录仍保留真实 provider 并承担原引用检查。`link-identity-closure` 从 `/5` 升至 `/6`，规范、profile、固定向量及旧版本拒绝测试同步，旧产物与缓存需重建，runtime C ABI 保持。
- 新增 23 份源码、11 组真实产物正例、9 组实际编译反例及 42 份 HIR/MIR/LIR／诊断 golden；另用同目录的取址反例在完整本地 core 前端检查错误位置和信息。正例覆盖静态与运行时初值、读改写和异常求值顺序、托管 String／引用、24 字节含三个引用的值、ZST、泛型 enum niche、词法捕获、成员／构造、默认值、普通委托和泛型委托成功／失败状态。提供方与消费方发布后移走源码，下游仅凭产物以自身引用类型再次实例化并重复 Int application，11 组均完成链接、普通运行和移动 GC。初始化用例先访问属性，再检查求值次数；完整多 image 启动仍属 M23-8。
- 原源码函数引用的 `global-state` 用例从 HIR 验证提升为真实产物闭环，新增三阶段 golden，完整套件的 14 组正例和 13 组反例通过。已核对旧 `members` 三阶段 golden：全局直接读写改为原访问器调用，追加 NoGc getter/setter，其他函数引用、捕获、派发及 GC 结构保持。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和最新配套 `scoopc` 构建通过，无警告。关闭全部快照更新开关后运行 `cargo test --workspace --no-fail-fast`，5204 项通过、0 失败、0 忽略；其中 HIR lowering 的 1279 项、LIR 的 466 项、slib 的 586 项和 driver library 的 147 项全部通过，driver 用时 380.82 秒。完整日志为 `/tmp/scoop-m23-7-global-state-workspace-verified.log`，两组定向实际产物测试日志为 `/tmp/scoop-m23-7-global-state-driver.log` 与 `/tmp/scoop-m23-7-global-state-references.log`。
- 既有快照差异已核对：普通全局读写改为原访问器调用，纯静态 GC-free 访问器移除 managed poll，相关 safepoint、stackmap、patch 和 symbol 计数相应更新；profile `/6`、对象引用编码与实际生成代码同步改变产物摘要。原对象、引用、初始化和损坏产物反例均在关闭更新开关的全仓回归中通过。
- 确认构建、测试及配套编译器进程全部结束、`target` 中没有打开的文件，并通过 Cargo metadata 核对实际目录与标准缓存标记后，执行 `cargo clean --target-dir target`，删除 2814 个构建文件，Cargo 报告总大小 5.4 GiB。

本项完成上述普通顶层状态的模板消费和再次发布路径。外来 core 的 `Option` 短写与变体构造、`addressOf` intrinsic 仍分别触发既有角色／能力缺口；本批的普通属性不可取址规则由独立前端反例验收，不把能力诊断算作该语言规则通过。其余主线继续推进，M23-7 尚未完成。

## 2026-09-29：外来 Option 与泛型 enum 变体

- `T?`、`T??` 和空安全运算通过既有 imported core protocol 查询实际 Option owner、variant 与 payload。默认导入从该 owner 的公共命名空间取得变体，裸 `Some`／`None` 与普通导入共用名称解析；保留查找层，使不适用的 prelude 变体可以进入既有 contextual 回退，词法值仍按原规则遮蔽。用户同名 Option 不改变语法角色，外来声明留在原 Cone。
- 泛型 enum 变体复用普通候选实参映射、约束求解、kind bound 和默认参数；从实际 enum 声明准备签名，推导后直接构造 variant，不为值构造补造函数正文。`E.V` 的泛型宿主作为声明命名空间，已应用的 typealias 保留原实参；嵌套构造的期望类型保留已解析的外层 binder，只排除当前候选尚未解决的变量。
- 导入默认值中的 Option 构造、分支和解包沿已有共享表达式展开；`!!` 按需选择依赖中真实的 UnwrapException 构造器。可变 Option 属性省略初值、显式及限定的 unit variant 初值保留常量 image；纯静态 GC-free 属性仍可由 NoGc 函数访问。修正 MIR 可选 String 转换的引用分支，成功时保留引用，值类型转换继续从 box 提取 payload；未修改 runtime ABI 或产物格式。
- 新增 23 份源码、9 组真实产物正例、11 组反例和 38 份 golden。正例覆盖限定／别名／star import 构造、嵌套 Option、安全调用的接收者与实参／默认值顺序、Elvis、解包异常、String／class／值类型转换、静态属性、闭包及局部函数、同名遮蔽、ZST、tuple 和 24 字节含引用值。提供方与消费方发布后移走源码，下游用自身引用类型及重复 Int application 再次实例化；9 组均完成正式产物读写、链接、普通运行与移动 GC。另将 core 的 Some／None 声明顺序调换并增加泛型 helper，重建且移走 core 源码后完成相同闭环；LIR niche 的 payload variant 随真实声明由 0 变为 1。
- 11 组反例核对源码位置、诊断和不产生目标产物，涵盖 None 缺少期望类型、unit 调用、位置／命名参数形态、类型实参数量、alias 固定实参、kind bound、不变性、嵌套 Option、词法遮蔽和不可变属性省略初值。定向产物测试 2 项通过，日志为 `/tmp/scoop-m23-7-options-driver.log`；三阶段 golden 已核对实际 variant、分支、异常调用、静态存储及 GC 表示。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 和最新配套 `scoopc` 构建通过，无警告。关闭快照更新后运行 `cargo test --workspace --no-fail-fast`，初轮 5207 项通过、0 忽略，唯一失败是一份旧 enum 诊断快照：限定构造保留实际 Choice 类型后，原返回 Other 的错误改由函数正文类型检查报告，错误位置及拒绝结果保持。仅更新这份诊断 golden 后，完整 enum 产物测试的 5 组正例和 18 组反例在关闭更新时复测通过，用时 40.34 秒；无后续代码修改。日志分别为 `/tmp/scoop-m23-7-options-workspace-verified.log` 和 `/tmp/scoop-m23-7-options-enum-verified.log`。
- 测试结束后通过 Cargo metadata 核对实际构建目录，确认 `target` 无打开文件、内容为编译与编辑器检查缓存。恢复缺失的标准 `CACHEDIR.TAG` 后执行 `cargo clean --target-dir target`，删除 2706 个构建文件，Cargo 报告总大小 5.0 GiB。

本项完成上述外来 Option 与泛型 enum 变体的源码、产物消费和再次发布闭环；`addressOf`、数组／vararg、其余 adapter 与 coroutine 等主线继续推进，M23-7 尚未完成。

## 外来指针与布局 intrinsic

- 外来 `addressOf`、`sizeOf`、`alignOf` 与 `Ptr<T>` 的 `load/store`（含元素偏移）、`toULong`、`cast<U>`、`plus/minus` 进入普通候选推断。签名从实际共有声明取得，owner 与方法 binder 分别求解，已选调用使用现有指针／布局 HIR 节点；不制造 intrinsic 正文或 Strong 函数。
- `addressOf` 保留调用处参数、局部值和方法 `this` 的原 place，命名参数与导入别名不改变取址对象。泛型 pointee 的 GC-free 条件沿现有 `Ptr` 条件传播；`sizeOf/alignOf` 仅要求值类型，支持含引用字段的值与泛型布局查询。新生成的 cast pointee 接入既有类型检查，避免结果立即转成整数时漏检，直接与泛型包装的反例均已覆盖。
- 共有正文和默认值消费既有 `AddressOf(Local)`、布局查询及指针节点。普通 `Ptr.equals` 保留真实 core 正文、结构 receiver 和实际 owner 类型实参，跨 Cone 再次发布时继续沿已有方法 application / ODR 路径；不要求补造本地 struct 声明。
- 新增 `m23-imported-pointers` 的 25 份源码及 40 份 golden：9 组实际产物正例、13 个诊断反例。覆盖泛型 provider / consumer / downstream、下游新值类型与重复 `Int` 实例、显式与推断 cast、实际 core 普通成员、参数写回、`this` 副本、ZST 独立地址与 offset 副作用、GC-bearing value 布局、默认值、别名和普通重载遮蔽。provider / consumer 源码移走后继续消费与链接，验证 HIR/MIR/LIR、共同 ODR member、普通运行和移动 GC。
- 专项验证：2 个前端测试通过；重建配套 `scoopc` 后，2 个 driver 测试包含上述全部实际产物与反例并通过（43.78 秒）。执行顺序为 `cargo fmt --all` → `cargo clippy --workspace` → 重建配套编译器 → 专项测试。
- 关闭 snapshot 更新后，使用实际配套 `scoopc` 完成 `cargo test --workspace`：**5212 passed、0 failed、0 ignored**，包含既有 Option、成员、函数引用、继承／接口、core 和产物视图回归。完整日志为 `/tmp/scoop-m23-7-pointers-workspace.log`，专项产物日志为 `/tmp/scoop-m23-7-pointers-driver.log`。
- 全部验证完成后，通过 Cargo metadata 确认 `target` 为实际构建目录，核对无符号链接、无打开文件且只含编译／编辑器检查缓存；恢复缺失的标准 `CACHEDIR.TAG` 并执行 `cargo clean --target-dir target`，删除 2622 个文件，Cargo 报告 6.1 GiB。

本项完成上述指针与布局调用闭环。显式 `Ptr<T>(raw)` 的外来构造见下一节；外来 raw storage 的取址和泛型组合、指针函数值适配仍需推进。

## 显式原始指针构造

- 普通名称查找得到实际 core `Ptr` 声明后，外来入口与当前 Cone 共用 pointee 推断、`raw: ULong` 参数和 GC-free 检查。显式类型实参、期望类型推断、`_`、位置／命名参数与导入别名均进入同一特殊构造候选，直接生成已有 `PtrFromNonZeroULong`；不补造本地 struct、普通 constructor identity、机器 callable 或新的 wire 分支。
- 固定 `Ptr<Int>` application 的本地／外来 typealias 作为非参数化候选，和同层普通函数统一比较。unsafe 和常量零错误只由已选入口提交，错误表达式随即结束；修复常量零被当作不适用、从而错误回退到较弱普通函数的情况。合法调用仍只求值一次 raw 实参，动态非零继续是既有 unsafe 前置条件，不增加 runtime 检查。
- `Ptr`／`FunPtr` 别名按结构化指针签名保留实际 component 类型引用，移除签名中不存在的 pointer family 名义 AliasTarget；直接 alias 边继续保留真实绑定。`FunPtr` 及其别名仍无整数构造器。构造中的泛型 GC-free 条件复用既有正文条件传播，结果立即转成整数的包装函数也不能绕过条件；已检查的具体 pointee 不再登记到后续重复检查列表。
- 默认值、局部函数和 lambda 直接消费现有指针节点。新增 `m23-imported-pointer-construction` 的 **32 份源码、45 份 golden**，含 8 组真实产物正例和 21 个反例：显式／推断构造、别名、普通重载与歧义、同名普通 struct、raw 求值顺序、泛型正文及默认值、捕获、ZST、嵌套指针和 Option niche；反例覆盖参数协议、类型与 kind、非零、unsafe、直接和泛型 GC-free 条件及无效 FunPtr 构造。
- provider／consumer 发布后移走源码，下游以自有值类型、Unit 及重复 Int application 再次消费和发布，核对 HIR/MIR/LIR、共同 ODR member、普通运行与移动 GC。泛型测试把重新编码的 raw 和局部函数的读取结果实际传给后续读取，保证这些路径影响最终结果。
- 专项验证依次完成 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets`、配套 `scoopc` 构建和测试，无 lint 警告。5 个前端测试通过（含 3 个既有构造测试）；最终 2 个集成测试覆盖上述 8 组正例及 21 个反例并通过，用时 46.70 秒。日志为 `/tmp/scoop-m23-7-pointer-construction-frontend.log` 和 `/tmp/scoop-m23-7-pointer-construction-driver.log`。
- 关闭 snapshot 更新，使用实际配套 `scoopc` 完成 `cargo test --workspace --no-fail-fast`：37 个测试组全部完成，**5216 passed、0 failed、0 ignored**，包含上述用例以及既有 core、属性初始化、继承／接口、函数引用、普通状态、Option 和指针调用回归。完整日志为 `/tmp/scoop-m23-7-pointer-construction-workspace.log`。
- 全部测试结束后，通过 Cargo metadata 核对实际 `target`，确认目录非符号链接、无打开文件且只含构建／编辑器缓存；恢复缺失的标准 `CACHEDIR.TAG` 后执行 `cargo clean --target-dir target`，删除 2731 个文件，Cargo 报告 **5.5 GiB**，清理后 `target` 不存在。

原始指针构造验证中发现的依赖 package 限定类型名缺口已由下一节修复；通过普通 import 使用的短名与限定类型路径现在进入相同的类型解析规则。

## 依赖包限定类型路径

- 当前 Cone 与 direct dependency 的公开包共同选择最长前缀；同包类型按实际 typed target 合并和去重，冲突在同一查找层诊断。仅含 value 的包仍占用其前缀，support provider 不增加源码可见包。导入收集生成一次不可变包类型索引，后续 lowering 和候选探测共享该索引，不按 FQN 扫描产物或补造实体。
- 路径选定包后沿实际静态 nominal owner 遍历。末段使用普通类型参数、kind／nominal bound、可见性和 alias 展开入口；泛型外层仅提供命名空间，不实例化缺失实参。固定泛型别名、本地转接别名、多层泛型外层和末段泛型均保留真实身份；alias 循环继续使用完整解析栈诊断。
- 修复静态嵌套类型的构造器导出遗漏：参数自由的 nested owner 按自身签名参与原机器绑定生产，不因词法外层含泛型类型而被过滤。既有组合 fixture 的 `Nested()` 导出由 false 改为 true，普通构造器选择由 16 增至 17；泛型嵌套类型仍走原 application／ODR 路径。
- 公开 alias 的目标由最终签名或直接 typed alias 边产生 `AliasTarget` 引用。移除别名专用的名称来源证明、解析栈中的来源传播，以及 core／普通类型解析中的重复来源收集；公开别名的类型、访问、目标归属、引用闭合和循环检查保留。经可见外层类型取得的嵌套目标无需补造直接包查找路径即可再次发布。共有 HIR 格式升至 **`/38`**，旧 `/37` 及更早产物与缓存重建；两份 profile 固定向量仅改变对应版本字节，SHA-256 指纹按既有规则同步更新，runtime ABI 与 GC 契约不变。
- 新增 `m23-qualified-types` 的 **28 份源码、34 份 golden**，覆盖 6 组运行正例和 16 组反例：普通与泛型类型、固定／指针别名、嵌套类型、泛型外层、约束、当前与依赖共包、两路同源转导出去重、不同直接依赖的重名、仅含 value 的最长包前缀及 support 包不可见。反例核对具体源码位置、诊断和不产生目标产物。
- 5 组 provider／consumer 在发布后移走源码，下游以自身 class 和重复 Int application 再次消费和发布；另以两份转导出产物访问同一个原始 provider，核对普通／泛型 alias 与嵌套类型的真实引用。6 组均通过 HIR/MIR/LIR 快照、正式产物读取、链接、普通运行、移动 GC 以及重叠 ODR 定义和 ABI 比较。
- `cargo fmt --all` 与 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过，无警告。HIR、HIR lowering 和 slib 的完整单元测试 **2745 passed、0 failed、0 ignored**；重建实际配套 `scoopc` 后，3 项集成测试全部通过，用时 32.23 秒。日志为 `/tmp/scoop-m23-7-qualified-types-unit.log` 和 `/tmp/scoop-m23-7-qualified-types-driver.log`。
- 关闭快照更新，使用实际配套 `scoopc` 执行 `cargo test --workspace --no-fail-fast`，37 个测试组全部结束：**5200 passed、21 failed、0 ignored**。21 处失败均是 core 布局产物快照仍保存 `/37` 的产物指纹；逐一比较断言内容，代码指纹、runtime 指纹及其余字段完全相同。上述新用例与其他回归全部通过，完整日志为 `/tmp/scoop-m23-7-qualified-types-workspace.log`。
- 同步 core 布局及形状依赖的 **43 份产物快照**，与提交前版本逐一比较，均仅改变第一行产物指纹。嵌套构造器修复另更新 4 份 HIR／MIR／LIR 快照：`Nested()` 进入导出表，callable 数量从 50 增至 51；实际对象、布局、描述符和注册表数量保持一致。
- 仅快照变更后，清除全部 `SCOOP_UPDATE_*` 与 `INSTA_UPDATE` 环境变量，复测受影响的 `layout_exports::core` 全组，**22 passed、0 failed、0 ignored**，耗时 283.52 秒，完整 core 产物链和属性初始化组合均通过。最终日志为 `/tmp/scoop-m23-7-qualified-types-layout-verified.log`；未重复运行代码未变化的其他测试组。
- 验证结束后，通过 Cargo metadata 确认实际 `target`，核对无编译／测试进程、无打开文件且目录仅含构建与编辑器检查缓存；恢复标准 `CACHEDIR.TAG` 并执行 `cargo clean --target-dir target`，删除 **2881 个文件、6.2 GiB**。清理后 `target` 不存在，日志为 `/tmp/scoop-m23-7-qualified-types-clean.log`。

上述验证中发现的显式嵌套 import 缺口由下一节修复：当外层类型经 facade 转导出且原 provider 仅作 support 时，类型位置、普通 receiver 构造与显式 import 均可沿真实静态 owner 解析。M23-7 的其余主线仍需继续完成。

## 经转导出类型的静态嵌套 import

- direct package index 继续决定源码可见包；选中实际 nominal owner 后，exact、star 和 public import 共用其真实 public 静态 namespace，不再把 support provider 的嵌套绑定过滤掉。终点保留原 provider、typed target、conflict key 及公开 binding 的既有转导出引用；同源 owner 经两路 facade 到达时合并为一个目标，嵌套终点不复制外层查找路径。
- 移除公开绑定及外部名称引用 reader 中重复的 direct-provider 资格检查，删除对应 trait 方法、错误分支及仅供该检查使用的 direct 输入表。保留实际 provider 与 binding 的存在、实体归属、typed target、namespace／role、引用闭合、转导出连续性及普通源码可见性检查。共有 HIR 格式升级为 **`/39`**，旧 `/38` 及更早产物与缓存重建；两份 profile 固定向量只改变版本字节，指纹按既有 SHA-256 规则更新，runtime ABI 与 GC 契约不变。
- 新增 `m23-reexported-namespaces` 的 **28 份源码、37 份 golden**，覆盖 8 组运行正例和 13 组诊断反例。正例包括 exact／star、导入改名、多层泛型外层、object、companion 转发、generic enum 变体，以及 exact／star 再次转导出。反例检查 support 包不可直接导入、private／internal／protected 及 instance 成员不可静态导入、private 成员不进入 star 或再次转导出、泛型实参数量、不同 origin 的 owner 歧义和最长包前缀遮蔽。
- provider、两份同源 facade 和 consumer 发布后移走源码；下游只凭产物，以自有引用类型、重复 Int application 和 Unit 再次消费与发布。泛型正文通过传入的读取函数实际使用值，验证泛型嵌套存储、函数值参数、ZST 和移动 GC；public import 的下游直接使用再次公开的嵌套类型。8 组均完成 HIR／MIR／LIR 快照、完整产物读取、链接、普通运行和移动 GC，复用既有 ODR 成员与 ABI 比较。
- `cargo fmt --all` 与 LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过，无警告；HIR、HIR lowering 和 slib 的完整单元测试 **2746 passed、0 failed、0 ignored**。重建实际配套 `scoopc` 后，专项集成测试覆盖上述全部正反例并通过，耗时 65.38 秒。日志为 `/tmp/scoop-m23-7-nested-imports-unit.log` 与 `/tmp/scoop-m23-7-nested-imports-driver.log`。
- 同步格式版本引起的 core 与 shape-dependency 产物快照，`layout_exports::core` 全组 **22 passed、0 failed、0 ignored**，耗时 277.49 秒。核对 43 份既有 `.artifact.snap` 均只改变首行产物指纹，后续正文记录完全相同，其余既有 golden 没有变化；日志为 `/tmp/scoop-m23-7-nested-imports-core-snapshots.log`。
- 清除全部 `SCOOP_UPDATE_*` 与 `INSTA_UPDATE` 环境变量，启用本次构建的实际配套 `scoopc` 执行 `cargo test --workspace --no-fail-fast`，完整回归 **5223 passed、0 failed、0 ignored**，无编译警告；driver 的 157 项测试全部通过，耗时 391.34 秒。新增静态嵌套 import、上一批限定类型路径、core 产物及属性初始化的快照均在关闭更新的条件下通过。日志为 `/tmp/scoop-m23-7-nested-imports-workspace.log`。
- 全部测试结束后，通过 Cargo metadata 确认实际 `target`，核对无编译／测试进程、无打开文件且目录仅含构建与编辑器检查缓存；恢复标准 `CACHEDIR.TAG` 后执行 `cargo clean --target-dir target`，删除 **2759 个文件、5.6 GiB**。清理后 `target` 不存在，日志为 `/tmp/scoop-m23-7-nested-imports-clean.log`。

## 数组模板与函数 vararg 的跨 Cone 消费

- 隐式数组从实际 core 声明取得 owner，与显式 `Array<T>`／`MutableArray<T>` 共用完整类型和普通 application；Export `ArrayAssembly` 保存完整 `TypeId`。共有泛型正文与默认值支持数组 literal／assembly、读写、长度和转换成员，不复制外来 nominal 到本地 arena，也不新增数组产物格式。
- 导入函数的 vararg 映射保留每个位置元素、spread 的源码索引和命名整数组；接收者与显式实参先按源码顺序执行，再按形参序执行默认值和数组物化。命名 `values = array`／`values = *array` 保留原 identity，位置 spread 和省略空数组产生新 identity；后续默认值可以读取已经物化的 vararg。普通函数引用仍按一个完整数组参数调用。
- 数组 intrinsic 经实际声明完成普通选择后正规化。越界沿实际 core 异常构造与既有 MIR throw／catch／finally 处理，删除 codegen 的重复 fatal bounds check。动态 assembly 的溢出消息复用 callable-owned CString，同文消息在函数内共享，随 Strong／ODR 函数发布；canonical instruction 新增 tag 57，旧 tag 47 退役，production section 的摘要记录格式与 runtime C ABI 保持。
- 数组 instance layout 关联 `ManagedObject` scan，TD inline scan 保留独立 `ArrayElement` 身份；producer 和 reader 均使用真实数组 class application 的接口派发表。递归 shape scan 的根与子程序放入同一常量对象，原 primary atom 覆盖全部字节，移除无归属的私有 `.element`／`.part` 全局及无调用 helper；保留真实对象边界与 relocation 校验。
- 新增 `m23-generic-arrays` 的 **16 份源码、24 份 golden**，覆盖 **5 组运行正例、9 组诊断反例**。provider 和 consumer 发布后移走源码，下游只凭产物再次实例化并发布，复用已有 ODR member／ABI 比较、真实链接、普通运行与移动 GC。组合覆盖求值顺序、默认参数、整数组／spread identity、复制独立性、含引用大 struct／enum／tuple、嵌套数组、自有引用类型、重复 Int application、Unit／ZST、越界异常与 finally、普通 iterator／next 派发、vararg 函数引用；反例锁定位置及消息，包含非数组／可变数组 spread、不变性、整数组类型、重复参数、下标类型、只读数组写入、空实参推断与函数值参数数量。
- LLVM 22.1 下 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过，无警告。重建实际配套 `scoopc` 后，专项集成测试通过，耗时 **46.13 秒**，日志为 `/tmp/scoop-m23-7-arrays-driver.log`。
- 清除全部 `SCOOP_UPDATE_*` 与 `INSTA_UPDATE` 后执行 `cargo test --workspace --no-fail-fast`：**5225 passed、1 failed、0 ignored**；唯一失败是 codegen 仍要求独立私有数组扫描子程序的旧断言。将该断言改为核对同一对象内的完整扫描字节和子程序指针，生产代码保持不变，再次格式化、lint 并完整重跑 `scoop-codegen`，**302 passed、0 failed、0 ignored**，耗时 **3.22 秒**。全仓其余 **4924 项** 已通过，其中 driver 的 **158 项** 全部通过、耗时 **393.11 秒**，包含本批关闭更新的正反例。两轮覆盖的 **5226 项不同测试** 均完成验证，无编译警告；日志为 `/tmp/scoop-m23-7-arrays-workspace.log` 与 `/tmp/scoop-m23-7-arrays-codegen.log`。
- 验证结束后通过 Cargo metadata 确认实际 `target`，检查无编译／测试进程、无打开文件且目录仅含构建和编辑器检查缓存；恢复标准 `CACHEDIR.TAG` 后执行 `cargo clean --target-dir target`，删除 **2798 个文件、6.1 GiB**。清理后 `target` 不存在，日志为 `/tmp/scoop-m23-7-arrays-clean.log`。

数组转换构造 `Array(m)`／`MutableArray(a)` 和 `pack(*[], 42)` 等需后续实参提供上下文的推断已由后续两节接通。本节迭代正例通过普通 `iterator()`／`next()` 调用执行；后续 for 专节已接通数组与泛型迭代协议，具体范围与剩余缺口见该节，M23-7 尚未完成。

## 数组转换构造

- 当前与导入数组从实际 nominal owner 建立完整转换候选，唯一必需参数 `source` 使用另一数组种类的相同元素 application。共享参数映射、宿主实参推断、期望类型和 MSC，支持显式实参、`_`、命名参数、空数组上下文、导入别名及本地／外来固定 typealias。固定 alias 的完整类型与声明参数在共同候选环境准备，避免候选私有 arena 的类型进入其他候选的比较。
- 转换与普通同名函数共同选择：具体普通函数胜过泛型构造，数组形状胜过宽泛参数，固定 alias 胜过等形状泛型函数；仍相同的泛型／固定签名正常诊断歧义。声明诊断按真实本地位置或外来 typed owner 排序，不以 FQN 重新识别数组，也不补造本地 class、源码 constructor 或机器 callable。
- 本地数组 view 直接包含完整 `source` 参数，去掉先留空再由调用方补齐的步骤及重复的结果／来源类型断言。共有 nominal 求解器沿实际导入 owner 物化 application，选中后生成已有 `ArrayClone`，复用泛型正文、默认值、布局、复制与 GC 路径；产物格式和 runtime C ABI 保持。
- 新增 `m23-array-construction` 的 **23 份源码、31 份 golden**，包含 **5 组运行正例、16 组诊断反例**。provider／consumer 发布后移走源码，下游用自有引用类型、重复 Int application 及 Unit 再次实例化和发布，复用真实链接、ODR member／ABI 比较、普通运行与移动 GC。正例还覆盖大 struct、带引用泛型 enum、嵌套数组、零大小元素、源表达式求值一次、默认值、局部函数与 lambda 捕获、快照独立性及普通同名 class；反例核对相同数组种类、非数组、元素冲突、引用元素不变性、参数名／数量／重复／spread、类型实参数量、固定 alias、空数组推断、期望类型冲突及歧义的具体位置与消息。
- LLVM 22.1 下 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 通过，无 lint 警告；重建实际配套 `scoopc` 后专项测试通过，耗时 **52.48 秒**。日志为 `/tmp/scoop-m23-7-array-construction-clippy.log`、`/tmp/scoop-m23-7-array-construction-build.log` 与 `/tmp/scoop-m23-7-array-construction-driver.log`。
- 清除全部 `SCOOP_UPDATE_*` 与 `INSTA_UPDATE`，使用实际配套 `scoopc` 完成 `cargo test --workspace --no-fail-fast`，37 个测试组全部结束：**5227 passed、0 failed、0 ignored**，无编译警告。其中 HIR lowering 的 **1288 项**、driver 的 **159 项** 全部通过，driver 耗时 **394.70 秒**；本批正反例、既有数组／vararg、函数引用、普通构造、别名和 core 产物回归均在关闭更新的条件下通过。完整日志为 `/tmp/scoop-m23-7-array-construction-workspace.log`。
- 全部验证结束后，通过 Cargo metadata 确认实际 `target`，核对无编译／测试进程、无打开文件、目录非符号链接且仅含构建／编辑器检查缓存；恢复标准 `CACHEDIR.TAG` 后执行 `cargo clean --target-dir target`，删除 **2536 个文件、4.8 GiB**。清理后 `target` 不存在，日志为 `/tmp/scoop-m23-7-array-construction-clean.log`。

本项接通上述数组转换构造。后续实参提供上下文的推断见下一节，外来 `for` 协议和以下主线继续推进，M23-7 尚未完成。

## 导入泛型调用的整组实参推断

- 导入泛型函数、构造和成员按实际形参映射收集整组约束，复用既有上下文分类、partial solver 和字面量默认类型顺序。先处理可独立确定类型的输入，再用完整 hint 检查待定实参；只有没有进展时才事务性尝试默认类型种子，失败不泄漏表达式、诊断或局部状态。宿主与 callable 参数保持分组，调用方已解析的外层 binder 仍可作为上下文；成功输出包含完整实参、类型与目标，不增加产物格式或 runtime ABI。
- 裸名称、导入改名和限定名称的外来泛型 unit variant 从实际枚举声明识别上下文需求；固定 typealias 的完整 application 直接使用其已知类型。普通词法／成员值继续遮蔽类型限定符，不按 `None` 等名称特判，也不补造本地 nominal 或来源资格。
- 每个实参及其 desugaring sink 按原源码索引保存，后续仍按源码序执行显式实参、按形参序物化 vararg 与展开默认值。`addressOf` 保留参数临时值生成前确定的实际 place；推断中改变检查顺序不改变运行期顺序。
- 泛型探测入口直接持有并返回已有的 boxed 候选状态，减少嵌套调用中按值传递和错误出口的整份栈副本，修复默认测试栈上的溢出。既有嵌套名义构造用例和前端全组在默认线程栈下通过。
- 新增 `m23-argument-inference` 的 **17 份源码、27 份 golden**，覆盖 **6 组运行正例、9 组诊断反例**。正例组合空数组／spread／命名整数组、signed／unsigned 默认宽度、lambda、泛型函数引用、空变体、构造、两组 binder、extension、MSC、默认值与求值顺序，以及含引用大 struct／enum、嵌套数组、Unit 和自有下游引用类型。provider／consumer 发布后移走源码，下游仅凭产物再次实例化和发布，复用 HIR／MIR／LIR 快照、共同 ODR member／ABI 比较、普通运行及移动 GC；反例锁定缺少推断输入、未约束 binder、默认参数不能补足推断、不变性、lambda 结果类型、kind bound、冲突元素类型和调用歧义的具体位置与消息。
- LLVM 22.1 下完成 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，无 lint 警告。重建实际配套 `scoopc`；前端完整单元测试在默认线程栈下 **1288 passed、0 failed、0 ignored**，耗时 **23.95 秒**。日志为 `/tmp/scoop-m23-7-argument-inference-clippy.log`、`/tmp/scoop-m23-7-argument-inference-build.log` 与 `/tmp/scoop-m23-7-argument-inference-frontend.log`。
- 清除全部 `SCOOP_UPDATE_*`、`INSTA_UPDATE` 与测试栈大小覆盖，使用实际配套 `scoopc` 完成 `cargo test --workspace --no-fail-fast`：37 个测试组全部结束，**5228 passed、0 failed、0 ignored**，无编译警告。driver 的 **160 项** 全部通过，耗时 **398.22 秒**；新增实参推断、既有嵌套泛型构造、函数引用、扩展属性、数组、默认值和完整 core 产物回归均通过。最终日志为 `/tmp/scoop-m23-7-argument-inference-workspace.log`。
- 全部测试结束后，通过 Cargo metadata 确认实际 `target`，检查无编译／测试进程、无打开文件、目录非符号链接且仅含构建／编辑器检查缓存；恢复标准 `CACHEDIR.TAG` 后执行 `cargo clean --target-dir target`，删除 **2701 个文件、5.3 GiB**。清理后 `target` 不存在，日志为 `/tmp/scoop-m23-7-argument-inference-clean.log`。

## for 在 Export HIR 前展开与跨产物泛型迭代

- HIR lowering 从普通成员／扩展候选取得 `iterator`，按实际 core owner 与完整 class／interface／bound 闭包选择唯一 exact `Iterator<T>`。source、iterator result 和适配后的接收者分别只求值一次；引用上行转换与值装箱沿普通节点进行，`next` 直接引用真实 core slot。每轮在 while condition setup 中调用 `next`，用 `IsSome` 与受真分支支配的 `Unwrap` 取得元素，binding 在同一前端事务中展开。typed LoopId、作用域、不可变叶子及每轮捕获沿普通语句和函数值路径保留。
- 删除 Export HIR 的 For 及其独立 concretization、默认值复制、效果／局部值遍历和 dump 分支，移除共有正文中的 portable For／binding-plan 编码与专用字段索引、验证和测试工厂。前端 val／lambda／for 继续共用必要的 typed binding plan，Export 后只运输已经解析的普通语句、类型与引用。HIR `cross-cone-interface` 升为 **`/40`**，statement tag **9** 退役且不复用；旧 `/39` 及更早产物与缓存重建，profile 固定向量与指纹同步，runtime C ABI 和 GC 契约保持。
- 修复再次实例化相同泛型值迭代器时的 MIR 重复类型错误。类型记录从实际 origin 与 payload exact type 保存 Strong／ODR 归属，类型、dispatch 及 section 闭包索引复用同一结果，只合并完整内容相同的 ODR 记录。普通名义类型和其有限装箱 helper 的重复 Strong 定义继续拒绝；不增加产物字段或物理定义规则。
- 新增 `m23-iteration` 的 **20 份源码、32 份 golden**，包含 **7 组运行正例、11 组诊断反例**。覆盖 Array／MutableArray、空数组、vararg、成员与扩展迭代器、泛型 class／struct／enum、class bound／interface bound／菱形继承、普通默认表达式中的循环、提供方解构与 component、每轮闭包捕获、Unit、大值和引用 payload，以及 source／iterator／next 次数、嵌套跳转、异常和 finally。provider／consumer 发布后移走源码，下游以自有引用类型、重复 Int application 和 Unit 再次实例化、发布、链接并运行；7 组在普通与移动 GC 模式均返回 42，复用既有 ODR member 与 ABI 比较。反例锁定 operator、协议、多个 exact application、可见性、refutable pattern、不可变 binding、跨闭包 break、suspend、用户同名接口不能替代 core 协议、NoGC 和解构元数的真实位置与消息。

- LLVM 22.1 下完成 `cargo fmt --all`、`cargo clippy --workspace --all-targets`，无 lint 警告，并重建实际配套 `scoopc`。日志为 `/tmp/scoop-m23-7-iteration-clippy.log` 与 `/tmp/scoop-m23-7-iteration-build.log`。
- 清除全部 `SCOOP_UPDATE_*`、`INSTA_UPDATE` 与 `RUST_MIN_STACK` 后执行 `cargo test --workspace --no-fail-fast`，37 个结果组全部结束：**5188 passed、21 failed、0 ignored**，无编译警告。21 项失败均来自 core 产物快照的格式指纹迁移，其余测试包括本批迭代正反例全部通过；driver 共 162 项，耗时 **309.19 秒**。完整日志为 `/tmp/scoop-m23-7-iteration-workspace.log`。随后仅打开 core layout 与 shape dependency 的更新开关，完整 core 组 **22 passed、0 failed、0 ignored**，耗时 **290.13 秒**；逐份核对 **41 份 core 与 2 份 shape dependency 快照**，均只改变首行 `artifact` 指纹，代码、runtime 与依赖图内容完全一致。更新与比对日志为 `/tmp/scoop-m23-7-iteration-core-update.log`、`/tmp/scoop-m23-7-iteration-core-snapshot-review.log`。

本项覆盖数组与上述泛型迭代路径，M23-7 尚未完成。真实组合验证还明确了三个后续缺口：普通参数自由宿主继承／实现封闭泛型父类型仍被旧的 materialization gate 排除，core 整数范围的普通 iterator 与泛型接口声明还会重复进入候选；消费者源码直接解构外来 struct／class 尚未接通共用 binding lowering；源码调用直接展开外来函数值默认参数仍受旧 gate 限制。这些问题随职责修订转入 [M23-6a](../stage6a/DESIGN.md) 的共同 HIR 迁移，不能把本项测试通过当成范围迭代或全部默认值／解构组合已经交付。

## 2026-10-01：跨 Cone 原生函数取址

- 源码与默认值／泛型正文的 `FunctionAddress` 共用实际选中的 callable target。当前与依赖函数按同一候选流程检查普通顶层、参数自由、NoGC、精确 C-safe 签名及 unsafe 规则；消费方不复制外来源码函数或 Strong 桥。
- 定义方为实际发射的合格源函数生成 storage bridge，并为公开与模板支持函数发布共有 callable binding。桥保留原函数的 typed identity 和语义签名，物理签名使用可选结果指针及各参数指针并返回 Unit；只有实际取址表达式才产生 C trampoline。自动属性 getter／setter 不生成源函数取址桥，未取址的 provider 函数也不产生 C 对象。
- 共有 MIR type bridge 升至 **`/5`**，storage bridge 使用 lowering role tag **12**。foundation 保存真实 storage ABI，消费方沿已有 generated identity 选择定义方桥；LIR 与 generated-C relocation 使用同一外部 callable 引用。profile 固定向量和指纹同步，旧产物与缓存重建，runtime ABI 与 GC 契约保持。
- 新增 `m23-native-addresses` 的 **14 份源码、23 份 golden**，覆盖首次外来取址、提供方已取址、重载、默认值、私有泛型正文支持、再次转导出、24-byte CLayout 大值与指针写回。provider／consumer 发布后移走源码，下游以自有引用类型和重复 Int application 再次实例化并发布；**7 组**真实产物完成 C callback 调用、普通运行和移动 GC。**5 组**反例核对 managed 函数、错误签名、泛型原生地址、unsafe 和 private 可见性的报错位置与内容。
- 新增实际产物检查确认 provider 只为 6 个源函数发布 storage bridge，公共属性访问器不混入；其唯一已取址函数产生 1 个 C trampoline，对每个桥核对定义方与实际 ABI。测试链接入口同时使用正式 reader 返回的 Scoop 对象与 generated-C 对象。
- 完成 LLVM 22.1 下的 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 和配套编译器构建，无警告。HIR lowering、MIR lowering 与 slib 的严格单元回归 **2038 passed、0 failed、0 ignored**；受影响的 driver 回归 **39 passed、0 failed、0 ignored**，耗时 **494.36 秒**。既有快照中 41 份只变化产物指纹，其余增量核对为合格源函数的实际桥、callable、对象和登记记录。
- 清除全部 `SCOOP_UPDATE_*`、`INSTA_UPDATE` 与 `RUST_MIN_STACK`，启用最新实际配套 `scoopc` 完成 `cargo test --workspace --no-fail-fast --target-dir target/m23-6a`：**37 个测试组、5313 passed、0 failed、0 ignored**，无编译警告；driver 的 **206 项** 全部通过，耗时 **703.46 秒**。包含实际编译执行的 collector、GC／stack-map、EH C 回归及关闭更新的全部 golden。日志与汇总为 `/tmp/scoop-m23-7-native-final-workspace.log` 和 `/tmp/scoop-m23-7-native-final-results.json`。
- 全部构建和测试结束后执行 `cargo clean --target-dir target/m23-6a`，删除 **1948 个文件、5.1 GiB**；日志为 `/tmp/scoop-m23-7-native-clean.log`。

## 2026-10-01：跨 Cone 函数适配器与动态调用

- 函数值适配共用普通 typed CFG，覆盖 `Ptr`／`FunPtr` 的装箱、引用上行、嵌套函数型变、lambda 结果与 generic delegate initializer。closure 的 parent 指向自身精确 FunctionShape 描述符，vtable 第 0 槽固定为同参数个数、同挂起性的 Any 动态 invoke；目标 adapter 负责参数装箱和结果还原。下游首次检查自己的类型时不改变提供方 TD 或 bridge，泛型继续单态化。
- runtime 按完整函数签名执行参数逆变、结果协变检查。共有类型登记以必需 field 29 保存 `Absent | Signature | Interface`，interface 直接父关系保留多继承；TD 固定前缀扩为 144 byte，尾部保存实际关系引用。cone-production 升为 **`/3`**、历史 strong-production 升为 **`/16`**、runtime metadata ABI 升为 **2**，固定向量、对象布局与指纹同步，旧产物和缓存重建。对象头、实例布局、GC 扫描及 C 调用 ABI 保持。
- 泛型引用实参消除保守装箱时保留原操作数类型和显式上行转换，捕获字段仍按真实类型读取；外来值的装箱沿定义方支持记录选择。实际 closure、adapter 和动态检查的签名中，tuple／嵌套函数引用的私有名义类型进入共有表示闭包，无关私有声明继续本地保存。
- 新增 `m23-function-adapters` 的 **16 份 Scoop 源码、34 份 golden** 和实际 C callback fixture。**6 项测试**覆盖 **10 组正例、4 组诊断反例**，包含首次下游动态检查、接口多继承、私有 tuple 签名、委托初始化、捕获对象和异常／finally。provider／consumer 发布后移走源码，下游以自有引用类型、Int 和 Unit 再次实例化和发布；正例全部通过实际对象链接、普通执行和强制移动 GC。
- 按职责新增动态 adapter、函数描述符与类型关系模块，并将 LIR dump 的 metadata 部分拆出；新实现模块均保持在约 300 行以内。完成 LLVM 22.1 下的 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 和实际配套 `scoopc` 构建，无警告。
- 清除全部快照更新开关和测试栈覆盖，执行 `cargo test --workspace --no-fail-fast --target-dir target/m23-6a`：**37 个测试组、5322 passed、0 failed、0 ignored**，无编译警告；driver 的 **212 项**全部通过，耗时 **715.49 秒**。既有函数引用组合、source-only 私有类型、三个 IR golden、core 产物、reader、对象、EH、stack-map 和 runtime 回归均通过。日志及汇总为 `/tmp/scoop-m23-7-adapter-strict-workspace.log` 与 `/tmp/scoop-m23-7-adapter-strict-results.json`。

- 函数适配器提交后清理 `target/m23-6a`，删除 **1588 个文件、3.5 GiB**；日志为 `/tmp/scoop-m23-7-adapter-clean.log`。

## 2026-10-01：跨 Cone 协程协议与有限启动支持

- HIR 具体化输出必需的 `CoroutineProtocol` 记录，源定义与依赖声明共用真实 core intrinsic、协议成员和普通请求队列。先固定有限 source shape 根，再按挂起 callable、函数型变与 interface 槽的实际结果类型闭合协议，不递归产生无穷 helper。
- 挂起 callable 保留语义签名与完整物理签名：隐藏参数为实际 `Continuation<R>`，物理结果为 `GeneratedCoroutineStep<R>`。状态机、函数 adapter 和虚／接口派发沿同一物理 ABI，resume、failure、register 与 run 使用实际成员槽；删除没有正文的 `ContinuationShell` 重复签名，generated callable tag 10 退役且不复用。
- 参数自由 source exact 的 `CoroutineStart<R>` 由定义 Cone 随有限 shape support 发布，消费方只引用其真实 Strong 目标；泛型 application 与结构结果仍按实际需求生成 ODR 定义。共有 MIR type bridge 升至 **`/6`**，启动 helper 使用 role tag **13**；共有 reader 只补查实际 core 协议角色与有限 start 的对应关系，复用已有签名、step、ABI 与归属验证。规范、固定向量、指纹与旧格式重建规则同步。
- 新增 `m23-coroutines` 的 **15 份 Scoop 源码、28 份 golden**。**8 组正例**覆盖立即完成、真实挂起、失败恢复与 finally、值类型 task、suspend 函数值与型变、同步恢复／重复恢复／注册失败、闭包和待定异常跨 finally 再次挂起，以及重建 core 后的实际槽身份。provider／consumer 发布后移走源码，第三个 Cone 以自有类型、Int 与 Unit 再次实例化和发布，全部完成普通与强制移动 GC 运行。
- 含引用的 **24-byte 大值**、泛型 enum payload 和跨挂起存活值在恢复前经历 128 次分配；completion 恰一次及失败路径由实际程序结果验证。**4 组语言反例**核对 task／completion 型变、普通函数中的挂起调用及函数 effect 不匹配的 span 和诊断；真实 core 产物删除 start 或交换协议参数时分别拒绝。
- 协程校验按 adapter、控制流、frame、函数与签名拆分，协议错误构造独立成模块，大段 MIR golden 移出 Rust 源码；新增 Rust 模块最长 **292 行**，没有占位实现。
- LLVM 22.1 下 `cargo fmt --all`、`cargo clippy --workspace --all-targets` 和实际配套 `scoopc` 构建通过，无警告。清除全部快照更新开关与测试栈覆盖后，`cargo test --workspace --no-fail-fast --target-dir target/m23-6a` 共 **37 个测试组、5330 passed、0 failed、0 ignored**；driver **216 项**全部通过，耗时 **1914.20 秒**。包含关闭更新的 HIR／MIR／LIR golden、正式 core／provider／consumer 产物、真实子进程缓存和 C runtime／GC／EH／stack-map 回归。日志与汇总为 `/tmp/scoop-m23-7-coroutine-final-workspace.log` 和 `/tmp/scoop-m23-7-coroutine-final-results.json`。

- 全部验证结束后执行 `cargo clean --target-dir target/m23-6a`，删除 **1818 个文件、4.4 GiB**；日志为 `/tmp/scoop-m23-7-coroutine-clean.log`。

## 2026-10-01：独立 adapter 成员并集与实际地址合并

- 新增 `m23-odr-member-unions` 的 **7 份 Scoop 源码、1 份 C 探针和 12 份 HIR／MIR／LIR golden**。两个 sibling 分别将不同源函数签名转换成同一目标函数类型，逐项确认各自独占的真实 adapter member、共同 StructuralType group 和包含双方候选的目标 FunctionShape TD；完整定义、ABI、对象与关联摘要继续按共同 member 比较，正反输入顺序均合并成功。
- 独立和组合两组均由真实 core、provider、左右 sibling 与 consumer 组成；发布后移走源码。组合同时执行公共泛型 try／catch／finally、捕获、分配和动态函数适配；真实 reader 返回的对象完成链接，普通与 `SCOOP_GC_STRESS_MOVE=1` 运行结果一致。
- C 探针使用实际 MIR／LIR 导出取得的函数符号、interface TD 与槽位，不猜测名字或槽号。两侧 `Marker<Int>` 的真实 TD 地址相等、`Marker<Long>` 的 TD 不同址，实际 interface target 非空且共享。分配调用留在已登记的入口栈帧中，跨下一次分配只保存静态 TD 指针。
- 运行辅助代码拆为读取和执行模块，复用同一份完整 reader 结果组织对象、root／immortal 登记和运行；原有 sibling 定义比较单独成模块。新增 Rust 模块均在 300 行以内。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 和配套 `scoopc` 构建通过，无警告。关闭全部快照更新开关后执行 `cargo test -p scoopc --lib siblings:: --target-dir target/m23-6a`：**6 passed、0 failed、0 ignored**，耗时 **89.95 秒**，涵盖原有 generic/string 合并、相同 member 的正文／字符串冲突、委托初始化／失败共享及本批独立成员和地址检查。日志为 `/tmp/scoop-m23-7-unions-strict-tests.log`；首次探针的 C 栈边界问题已修正，普通及移动 GC 两组均已严格复验。

## 2026-10-01：真实泛型依赖缓存

- 新增 `m23-generic-cache` 的 provider／consumer 源码 fixture，使用现有构建图、真实 core 和 `SCOOP_TEST_PAIRED_SCOOPC` 运行正式编译子进程。provider 包含公开泛型函数、私有泛型 helper 与默认参数，consumer 同时声明自有引用类型。
- 顺序修改泛型正文、私有 helper 正文、默认值、`ref` 约束和调用实参类型。前四类变化要求 provider 与 consumer 重新编译，最后一类只重编译 consumer；core 始终命中已有缓存。变化节点的实际产物 fingerprint 改变，未变节点保持字节一致。
- 初始构建和每次变化后都重复相同输入，共 **6 轮**确认所有节点为 `CacheHit`、编译调用列表为空且实际产物字节完全相同。验证复用现有缓存键和正式产物路径，没有新增缓存策略或发布入口。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 和配套编译器构建通过，无警告；真实专项 **1 passed、0 failed、0 ignored**，耗时 **98.93 秒**，包含上述五类变化和全部未变输入检查。日志为 `/tmp/scoop-m23-7-generic-cache-r1-tests.log`。

## 2026-10-01：共有发布路径与旧入口清理

- 删除没有实际生产调用、只被旧格式 fixture 使用的 `SingleConeStrongArtifactInputV1`、`AssembledSingleConeStrongArtifactV1` 及对应 writer 错误；同时删除无构造点的 driver Strong-profile 错误分支与 HIR `GenericOdrRequired` 错误项。现行 archive 继续由 Strong／ODR 共用的完整 layout writer 发布。
- 按 spec 先行同步实现规范和阶段 3／7 设计，移除旧专用发布入口与收窄条件的叙述；底层格式 fixture 复用已有 canonical archive 构造器，当前格式、fingerprint 和 runtime ABI 不变。
- 将原 1487 行 link reader 测试按归档构造、对象、生产记录及拒绝用例拆分，主模块约 260 行，其余模块均低于 400 行。逐项比对原 34 个函数／结构体，除旧 writer 分支外保留正文；原字节重现断言改为描述实际 canonical archive 的测试名。
- 本批前执行 `cargo clean --target-dir target/m23-6a`，删除 **1627 个文件、2.1 GiB**，日志为 `/tmp/scoop-m23-7-pre-final-clean.log`。之后使用 Rust `opt-level=1`，保留 debug assertions 与整数溢出检查，并关闭增量和调试符号，降低大型实际产物检查的运行成本。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets`、实际配套 `scoopc` 构建及 slib **584 项**全部通过，无警告。关闭全部更新开关和测试栈覆盖的 workspace 回归 **37 个测试组、5332 passed、0 failed、0 ignored**；driver **217 项**全部通过，耗时 **344.73 秒**。日志及汇总为 `/tmp/scoop-m23-7-final-r2-workspace.log`、`/tmp/scoop-m23-7-final-r2-workspace-results.json`，实际编译器摘要与构建环境保存在 `/tmp/scoop-m23-7-final-r2-compiler.json`。

## 剩余主线

[M23-6a 已验收](../stage6a/ACCEPTANCE.md)，普通宿主的封闭泛型父类型、整数范围、外来类型解构及函数值默认参数不再列为本阶段缺口。本阶段继续承担实际机器定义、ODR 和委托运行闭环，具体边界以修订后的 [设计](DESIGN.md) 为准。

1. 补齐设计第 12 节中直接修改 core 数组迭代正文并新增泛型声明的实际产物证据，再记录逐项最终验收。

验收始终以源码与实际产物为依据。最终必须逐项核对设计第 12、14 节，不能用局部单测替代跨 Cone 链接运行或宣布阶段完成。
