# asyncio 原生测试辅助库

这个 Cone 只提供原生测试符号，Scoop 文件用于声明包，没有公开 Scoop API。native/fence_work.c 占用真实平台后端的工作槽，native/work.c 提供可控工作、结果和释放计数；导出函数旁说明调用及生命周期要求。

这是嵌套测试／辅助 Cone，不属于发行内置库清单，也不进入 SYSROOT/docs 的生产库站点。
