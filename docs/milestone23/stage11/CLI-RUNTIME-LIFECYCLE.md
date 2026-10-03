# M23-11 runtime 生命周期与 image 输入边界

全部用例经公开 build、移走源码、独立 link，再以普通和 moving GC 环境运行；保留完整阶段 golden、正式 startup 的 link plan、空 stdout、SIGABRT 与完整错误消息。没有另一套 main 生成或对象提取／链接流程。

`restart` 从原 empty 源码精确加入普通 native 注册调用，`atexit` 在正式程序完成 runtime shutdown 后再次调用启动入口；`reentry` 保留原 eager initializer，并为 native 声明补上逻辑库名。两者都断言 `scoop startup: program runtime may only be started once`。二次调用传入空参数，验证生命周期检查发生在任何 metadata 读取前。

| 原功能 | 用例 | 只读进程／golden |
| --- | --- | --- |
| `program_lifetime_rejects_restart_and_initializer_reentry` | `restart`、`reentry` | 16／18 |
