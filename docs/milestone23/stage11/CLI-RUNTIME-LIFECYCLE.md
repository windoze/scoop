# M23-11 runtime 生命周期与 image 输入边界

全部用例经公开 build、移走源码、独立 link，再以普通和 moving GC 环境运行；保留完整阶段 golden、正式 startup 的 link plan、空 stdout、SIGABRT 与完整错误消息。没有另一套 main 生成或对象提取／链接流程。

`restart` 从原 empty 源码精确加入普通 native 注册调用，`atexit` 在正式程序完成 runtime shutdown 后再次调用启动入口；`reentry` 保留原 eager initializer，并为 native 声明补上逻辑库名。两者都断言 `scoop startup: program runtime may only be started once`。二次调用传入空参数，验证生命周期检查发生在任何 metadata 读取前。

| 原功能 | 用例 | 只读进程／golden |
| --- | --- | --- |
| `program_lifetime_rejects_restart_and_initializer_reentry` | `restart`、`reentry` | 16／18 |

空 image 和缺失 core image 两例保留原 eager-failure／empty-provider 源码与真实产物。TOML 复制 runtime 到私有目录，只精确替换 `startup.c` 传给 `scoop_image_collect` 的数量或列表起点，分别传入空列表和去掉 core 的列表；其余 reader、启动逻辑及 descriptor 均保持原实现。正式链接仍生成包含全部三 image 的入口，测试在 runtime 消费边界注入错误输入，不放宽 native 自动初始化段限制。独立 link 前移走私有 runtime 源码。两例分别检查 `empty input closure` 与 `image dependency or root owner is absent`，且 abort 前没有 eager 的 11 输出。

删除旧 Rust lifecycle 文件编排、support helper 和注册，以及失去调用者的 C main/image 模板。

| 原功能 | 用例 | 只读进程／golden |
| --- | --- | --- |
| `incomplete_real_image_inputs_fail_before_any_eager_code` | `empty-images`、`missing-image` | 16／18 |
