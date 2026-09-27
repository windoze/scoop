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

## 剩余主线

1. 继续共用可移植节点，完成构造初始化和 delegate template 的生产、读取与实际消费；完成逐 member 内容摘要及完整 profile。
2. public generic function 经真实 provider `.slib`、消费方具体化、MIR/LIR、对象与单 image 运行形成闭环，包含 consumer-local struct。
3. 支持 hidden helper、默认值与 vararg、宿主和方法两组 binder、bound dispatch、局部函数及 capture。
4. 完成泛型名义类型、构造、继承、属性、dispatch、ZST/大值/引用 ABI 与扫描。
5. 完成 adapter、box、coroutine 与有限 shape support，验证共同 member 一致、独立 member 并集、EH/stackmap 和实际地址合并。
6. 泛型委托扩展属性接入完整 LazyAccess application、现有初始化协调、失败共享与移动 GC。
7. 切换 core、driver、reader/publisher、cache 与全部 fixture，删除无调用的旧路径，完成真实配套编译器和 runtime 的全仓验收。

验收始终以源码与实际产物为依据。最终必须逐项核对设计第 12、14 节，不能用局部单测替代跨 Cone 链接运行或宣布阶段完成。
