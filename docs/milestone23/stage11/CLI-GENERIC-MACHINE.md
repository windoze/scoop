# M23-11 泛型机器正文的正式 CLI 验收

`m23-cli-generic-machine` 保留原 provider 及 standalone、control-flow、combined 两个不同 consumer 坐标。四个用例经正式 CLI 发布、移走源码、独立链接，分别检查 primitive、try／finally、异常恢复、默认值、局部 struct 与泛型 relay 的原数值，并以普通和 moving GC 运行。原 9 份 HIR／MIR／LIR 快照逐字相同；只读验收为 4 个用例、48 次进程执行、36 份阶段／plan golden。

删除 `actual_generic_library_emits_shared_odr_objects` 的文件 golden 比较分支和已替代快照。这个测试继续承担普通 codegen／reader 单元测试职责：MIR／LIR exact type 对应、foundation wire 往返、对象及边界符号、ODR registration、关联 LSDA／EH frame／compact unwind／stackmap 的 fingerprint 影响，以及四份产物之间 1／4／4 个共享正文的精确记录相等。它不再维护文件快照或执行另一套程序链接路径；正式 CLI 用例负责源码发布、运行和 golden。
