# M23-7 实现进展

目标：完成 [M23-7 设计](DESIGN.md) 的全部能力与完成门；每个功能通过验证后提交，定期清理构建目录。当前阶段为实现中，本文件不替代最终验收。

## 2026-09-27：ODR member 摘要身份

- `DigestOwnerAndRoleKey` 使用 `OdrMemberDefinition(OdrMemberId)`，owner tag 为 11；`DigestKind::OdrDefinition` 仍为 8。旧 group owner tag 8 在读取时拒绝。
- digest reader 查询实际 member 身份，不能用同字节的 group 身份满足引用；同组不同 member 产生不同 node 身份。
- LIR identity-foundation 升至 `/2`，现有完整 profile 的必需 section、兼容摘要和固定向量同步更新，旧 `/1` 产物与缓存需要重建。现有 Strong profile 仍拒绝 ODR 定义，完整 generic profile 在实际模板与对象生产接通后切换。
- 已执行 `cargo fmt --all`、LLVM 22.1 下的 `cargo clippy --workspace --all-targets`，均通过。
- `cargo test -q -p scoop-identity -p scoop-lir -p scoop-slib` 通过：333 项 identity、449 项 LIR、574 项 slib，以及 1 项 doc-test，共 1357 项，无失败或忽略。

本项完成的是摘要的 typed owner 与编码迁移。逐 member ABI/definition 内容计算、ODR 对象发射与合并仍须在后续实现中完成，不能据此认定 ODR 能力已经可用。

## 剩余主线

1. 共用默认值的可移植节点，生产、读取泛型正文、构造初始化和 delegate template；完成逐 member 内容摘要及完整 profile。
2. public generic function 经真实 provider `.slib`、消费方具体化、MIR/LIR、对象与单 image 运行形成闭环，包含 consumer-local struct。
3. 支持 hidden helper、默认值与 vararg、宿主和方法两组 binder、bound dispatch、局部函数及 capture。
4. 完成泛型名义类型、构造、继承、属性、dispatch、ZST/大值/引用 ABI 与扫描。
5. 完成 adapter、box、coroutine 与有限 shape support，验证共同 member 一致、独立 member 并集、EH/stackmap 和实际地址合并。
6. 泛型委托扩展属性接入完整 LazyAccess application、现有初始化协调、失败共享与移动 GC。
7. 切换 core、driver、reader/publisher、cache 与全部 fixture，删除无调用的旧路径，完成真实配套编译器和 runtime 的全仓验收。

验收始终以源码与实际产物为依据。最终必须逐项核对设计第 12、14 节，不能用局部单测替代跨 Cone 链接运行或宣布阶段完成。
