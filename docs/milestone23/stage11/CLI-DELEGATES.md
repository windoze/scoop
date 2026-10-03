# M23-11 泛型委托 CLI 迁移

本记录按功能列出已完成的迁移；其余委托与产物损坏测试继续保留原入口，直到相同断言已迁移并验证。总验收仍以统一 Python 入口的完整只读运行结果为准。

## 共同路径与结构断言

原 provider、consumer、downstream 源码和 `dev.example:delegate-combinations-provider:0.1.0`、`delegate-<case>`、`delegate-downstream-<case>` 坐标保持。每一层发布后移走源码，下一层只使用实际 `.slib`。三个 library 分别从一次编译取得 AST/HIR/MIR/LIR，原 consumer 三阶段逐字核对。最后的普通 Scoop 程序检查原 `check() == 42` 和实际 GC 收集；移走全部项目源码与配套 compiler 后，再由独立 `scoop link` 链接并在普通／moving GC 下运行。

`m23-cli-delegates/support/metadata.c` 是普通 native 测试 helper，通过既有 runtime metadata ABI 读取本次生成的 provider／consumer image 描述符。image 符号绑定取正式构建返回的 Cone identity，不增加测试专用编译器接口。它检查 provider 初始化单元均为 Strong；consumer 数量与原 LIR 相同，全部为 ODR、LazyAccess。`foreign-local` 要求零个单元，保留词法存储的断言。`zst` 继续要求 byte size 为 0、allocation extent 为 1、scan 为 None；`flat`／`references` 要求 byte size 为 24，分别为 None／Recursive。各项检查在两种 GC 运行中都执行。

只读与只写用例在完整 MIR golden 之外，继续明确断言没有物化未使用的 setter／getter。最终 binary 的完整外部定义与去除 weak 后的符号表也分别比较，保留实际 callable 合并覆盖。helper 由声明中的 C compiler／archive 步骤准备，经普通 typed native requirement 加入链接；没有手工 startup 或 raw object 注入。

委托初始化中的局部函数、closure 和函数引用继续要求每个实际委托单元仅依赖 provider 的同一个 `Trace` 单元。正常 LIR dump 现在从同次生产的 typed registration 显示 `init-dependencies`，完整 golden 保留单元、provider 和全部依赖序列；声明另检查 provider 为实际发布者。原模块正文逐字不变。该显示不修改 `.slib`、ABI 或生产指纹。纯内存测试覆盖 eager／lazy、没有依赖、本地依赖和本地／外部混合依赖；51 个既有含初始化的 CLI 用例复核中，只有 imported-object 的一份 LIR golden 追加了这一信息。

每项先格式化、lint，核对旧断言与快照后删除对应 Rust 测试注册及不再使用的旧快照，再进行关闭全部更新开关的只读验收，分别提交。下表数字是实际完成结果。

| 原 Rust 测试 | 正例／负例 | 进程／golden 比较 | 原有期望核对 |
| --- | --- | --- | --- |
| `generic_delegate_values_and_order_republish_and_execute` | 7／0 | 98／91 | 21 份旧阶段、0 份旧诊断逐项相同 |
| `generic_delegate_aliases_cycles_and_local_source_republish_and_execute` | 5／0 | 70／65 | 11 份旧阶段、0 份旧诊断逐项相同；local-source 的私有 extension 初始化路径加入 Cone coordinate／canonical source；MIR 6 处、LIR 1 处仅显示路径变化，符号与指令逐字相同；foreign-receiver 的私有 extension 初始化路径加入 Cone coordinate／canonical source；MIR 6 处、LIR 1 处仅显示路径变化，符号与指令逐字相同 |
| `generic_delegates_use_dependency_members_and_local_accessors` | 2／0 | 28／26 | 4 份旧阶段、0 份旧诊断逐项相同；foreign-delegate 的私有 extension 初始化路径加入 Cone coordinate／canonical source；MIR 6 处、LIR 1 处仅显示路径变化，符号与指令逐字相同 |
| `generic_delegate_mixed_roles_republish_and_execute` | 2／0 | 28／26 | 2 份旧阶段、0 份旧诊断逐项相同；mixed-members 的私有 extension 初始化路径加入 Cone coordinate／canonical source；MIR 12 处、LIR 2 处仅显示路径变化，符号与指令逐字相同；foreign-extensions 的私有 extension 初始化路径加入 Cone coordinate／canonical source；MIR 6 处、LIR 1 处仅显示路径变化，符号与指令逐字相同 |
| `generic_delegate_language_errors_have_source_diagnostics` | 0／18 | 54／0 | 0 份旧阶段、18 份旧诊断逐项相同 |
| `generic_delegate_initializer_local_functions_republish_and_execute` | 2／0 | 28／26 | 5 份旧阶段、0 份旧诊断逐项相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同 |
| `generic_delegate_initializer_closures_republish_and_execute` | 6／0 | 84／78 | 13 份旧阶段、0 份旧诊断逐项相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同 |
| `generic_delegate_initializer_references_republish_and_execute` | 15／0 | 210／195 | 30 份旧阶段、0 份旧诊断逐项相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同；LIR 仅追加实际初始化依赖；每个委托单元恰好依赖同一 provider 的 Trace，原正文逐字相同 |

## Sibling 初始化身份

`generic_delegate_siblings_share_initialization_and_failure_with_moving_gc` 迁为 `generic-delegate-siblings-consumer`。保留原 provider／left／right／consumer 四份源码和原坐标，left/right 六份旧阶段正文逐字相同。四个 library 依次发布后移走源码，最后由普通 runtime fixture 与独立 artifact link 检查原返回 42、GC 收集、lazy 构建次数、specialization 隔离与失败 payload 的共享身份。

普通 C helper 直接检查本次左右 image：左侧两个单元、右侧五个；每个左侧单元在右侧恰好出现一次，registration、cell、delegate storage 和 failure root 均为相同实际地址，ODR group/member 身份一致。两个 sibling 的输入、完整 link plan 和最终符号表仍受声明式期望约束。没有手工 startup 或临时链接入口。

只读验收通过：1 个用例／变体、15 次进程、17 个 golden。原 Rust sibling harness 和六份旧快照已删除。
