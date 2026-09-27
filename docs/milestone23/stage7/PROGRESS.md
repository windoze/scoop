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

## 剩余主线

1. 继续共用可移植节点，完成构造初始化和 delegate template 的生产、读取与实际消费；完成逐 member 内容摘要及完整 profile。
2. public generic function 经真实 provider `.slib`、消费方具体化、MIR/LIR、对象与单 image 运行形成闭环，包含 consumer-local struct。
3. 支持 hidden helper、默认值与 vararg、宿主和方法两组 binder、bound dispatch、局部函数及 capture。
4. 完成泛型名义类型、构造、继承、属性、dispatch、ZST/大值/引用 ABI 与扫描。
5. 完成 adapter、box、coroutine 与有限 shape support，验证共同 member 一致、独立 member 并集、EH/stackmap 和实际地址合并。
6. 泛型委托扩展属性接入完整 LazyAccess application、现有初始化协调、失败共享与移动 GC。
7. 切换 core、driver、reader/publisher、cache 与全部 fixture，删除无调用的旧路径，完成真实配套编译器和 runtime 的全仓验收。

验收始终以源码与实际产物为依据。最终必须逐项核对设计第 12、14 节，不能用局部单测替代跨 Cone 链接运行或宣布阶段完成。
