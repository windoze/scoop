# M23-11 native 函数值 CLI 迁移

本记录随各功能迁移更新；总验收仍以统一 Python 入口的完整只读运行结果为准。

## 函数值适配与 ODR winner

`native_function_values_adapt_after_external_address_selection` 迁为 `m23-cli-native-functions/function-adapters-native-value`。保留原 provider、consumer、downstream 及其坐标，逐层移走源码，只消费前一步的真实产物。consumer 与 downstream 都物化 `exercise<Int>`，同时覆盖不同类型的泛型实例、函数值型变、动态检查、FunPtr 装箱及 C 回调。

原 C companion 的 `m23_call_scalar` 正文移至普通 `native.c`，consumer 的 extern 明确声明 `m23_native_calls` 逻辑库，按 Stage 11 7.4 经 typed requirement 和 `--library-path` 链接。没有手工 startup。旧 HIR／MIR 各只有 extern 声明增加 `lib`；LIR 同一声明的 C bridge identity 随真实 native contract 改变，其余阶段字节相同。三个 library 各保存一次编译的完整四阶段，真实 executable 检查原返回值和 GC，并在移走源码／配套 compiler 后重新独立链接，普通与 moving GC 均通过。

这一组合暴露了最终引用校验的问题：两个 Cone 中兼容的同一 ODR 正文分别引用本 Cone 的 C trampoline；系统 linker 合并正文后，原校验仍把被合并候选的 relocation 套用到保留正文，误报 relaxed GOT target 不一致。按实现规范 2.8 先修订文档，再让现有 link map 保留实际 Cone／member。对具有多个候选且有受控引用的 ODR atom，只选择 map 中唯一的保留对象，核对候选归属与最终符号地址，然后检查 winner 的原 relocation。单一候选、Strong、native、startup 和 stackmap 的检查保持，既有 ODR 内容兼容性结果直接复用。

新增普通 Rust 单元测试覆盖不同 candidate 顺序、缺失／重复 map 记录、错误 Cone／member、地址偏移，以及保留行与 dead-stripped 行的区别。已有 binary corruption 测试继续覆盖真实入口、导入、指针、调用、ordinal、版本和 rpath；其固定 fixture 的已知符号范围补齐 ODR owner，原未损坏 binary 仍先通过，再逐项核对原拒绝条件。正式路径始终使用实际 ld map，不使用测试中的固定对象顺序。

原 Rust 用例及三份旧阶段快照删除。格式化与全 workspace lint 后，退役状态的只读验收为 1 个用例、1 个变体、14 次进程、13 次 golden 比较；linker 的 11 项 Rust 测试全部通过。既有 37 个 program-link 用例与 64 个 native-link 用例也全量只读通过，合计 144 个变体、791 次进程、938 次 golden 比较。ODR winner 逻辑独立为 83 行模块，没有新增产物格式、callback 身份或 runtime ABI。
