# M23-11 泛型正文与 native storage 的正式产物读取

`m23-cli-generic-reader` 保留原 provider 源码与坐标，正式 CLI 一次发布完整四阶段输出，在移走 provider 源码后由普通 consumer 调用其公开函数，再移走消费源码并独立链接。新程序检查返回 42，并让一个普通对象跨越调用保持有效；普通与 moving GC 运行均通过，GC epoch 检查确认 stress 路径实际发生收集。

迁移时直接读取同批 CLI 生成的 `.slib` 执行原 typed reader 断言。完整 provider／consumer golden 和归档成员列表锁定对应结构；concrete-support 的原 MIR／LIR 两份快照逐字相同。未把 provider 源码复制进 Rust，也未使用另一套编译或链接入口。

| 原功能 | 用例 | 只读进程／golden | 原 typed 断言 |
| --- | --- | --- | --- |
| `ordinary_reader_retains_generic_bodies_from_actual_published_libraries` | `standalone`、`combined`、`support`、`concrete-support` | 48／36 | 全部保留 generic function 正文；standalone 正文 3、callable support 2；combined 有 capture；support／concrete-support 各有 1 个 nominal support 与 public binding；concrete-support 正文和 representation support 各为 1；删除旧文件编排、注册和已替代快照 |
