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

## 剩余主线

1. 继续共用可移植节点，完成构造初始化和 delegate template 的生产、读取与实际消费；补齐其他物理角色的内容摘要，接入已有成员合并入口，随实际 payload 同步升级正式 profile inventory。
2. 在已通过的泛型函数产物闭环上补齐 hidden helper、默认值与 vararg、宿主和方法两组 binder、bound dispatch、局部函数及 capture 的完整组合。
3. 完成泛型名义类型、构造、继承、属性、dispatch、ZST/大值/引用 ABI 与扫描。
4. 完成 adapter、box、coroutine 与有限 shape support，验证共同 member 一致、独立 member 并集、EH/stackmap 和实际地址合并。
5. 泛型委托扩展属性接入完整 LazyAccess application、现有初始化协调、失败共享与移动 GC。
6. 切换 core、driver、reader/publisher、cache 与全部 fixture，删除无调用的旧路径，完成真实配套编译器和 runtime 的全仓验收。

验收始终以源码与实际产物为依据。最终必须逐项核对设计第 12、14 节，不能用局部单测替代跨 Cone 链接运行或宣布阶段完成。
