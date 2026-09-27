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
- `hir/cross-cone-interface` 升至 `/31`；required inventory、reader、profile 指纹及固定向量同步迁移，旧 `/30` 产物与缓存需要重建。后续构造、委托模板按设计分别增加 field 12、13，并使用 `/32`、`/33`。
- 增加独立、组合、private 支持三份真实源码 fixture 与 HIR golden；覆盖泛型函数、扩展、宿主和方法两组 binder、局部函数、嵌套闭包、私有类型与属性。实际 driver 发布三份 `.slib` 后，经正式 Compile/Link reader 验证并读取上述正文和支持声明。
- `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets` 通过。885 项 HIR、1239 项 HIR lowering、574 项 slib 测试及 1 项实际产物集成测试通过，共 2699 项；源码 golden 在关闭更新选项后另行复验通过。
- 确认构建与测试结束后执行 `cargo clean`，删除 2686 个构建文件，回收 5.0 GiB。

本项完成源码 → 共有 HIR → `.slib` 发布与读取。消费方具体化、构造初始化与 delegate template、逐 member ODR 定义和跨 Cone 链接运行仍在后续主线，当前 Strong profile 没有据此宣告泛型机器能力完成。

## 剩余主线

1. 继续共用可移植节点，完成构造初始化和 delegate template 的生产、读取与实际消费；完成逐 member 内容摘要及完整 profile。
2. public generic function 经真实 provider `.slib`、消费方具体化、MIR/LIR、对象与单 image 运行形成闭环，包含 consumer-local struct。
3. 支持 hidden helper、默认值与 vararg、宿主和方法两组 binder、bound dispatch、局部函数及 capture。
4. 完成泛型名义类型、构造、继承、属性、dispatch、ZST/大值/引用 ABI 与扫描。
5. 完成 adapter、box、coroutine 与有限 shape support，验证共同 member 一致、独立 member 并集、EH/stackmap 和实际地址合并。
6. 泛型委托扩展属性接入完整 LazyAccess application、现有初始化协调、失败共享与移动 GC。
7. 切换 core、driver、reader/publisher、cache 与全部 fixture，删除无调用的旧路径，完成真实配套编译器和 runtime 的全仓验收。

验收始终以源码与实际产物为依据。最终必须逐项核对设计第 12、14 节，不能用局部单测替代跨 Cone 链接运行或宣布阶段完成。
