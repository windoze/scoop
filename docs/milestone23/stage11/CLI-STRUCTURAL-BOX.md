# M23-11 结构类型跨 Any 调用与旧链接 helper 清理

`m23-cli-structural-box` 复制原 core 源码并加入原 `property-initialization-provider.scoop` 到 `src/layout_probe.scoop`，正式发布修改后的 core；原 structural-box 源码随后通过 core 中的 `relayAny` 传递 `(Long, String)` tuple，移走源码后由普通 executable 读取结果 42。独立 artifact-only link 和普通／moving GC 运行均通过，GC epoch 断言保留。

原 HIR／MIR／LIR 三份快照逐字相同。迁移时在实际 `.slib` 上执行原 typed 断言，确认 Any 参数的 boxed payload 是 Tuple，descriptor 为 OdrWeak；最终符号表中该 descriptor 恰好一份且不属于 strong symbols。只读验收为 1 个用例、11 次进程执行、5 份阶段／plan golden。

这消除了旧 `imported_classes/runtime` 的最后一个调用者。删除其 runtime archive 构建、合成 Scoop runner、C main/image 列表拼装、原始对象提取与手工链接代码，以及 property-initialization 的旧 odr 文件编排。三个已经没有调用者的 C main 模板一并删除；native-addresses 与 function-adapters 的有用 callback 函数继续由各自的 `native.c` 和已迁移 CLI 用例执行。原 Scoop 源码全部保留。
