# asyncio 原生测试辅助库

这个 Cone 只提供原生测试符号，Scoop 文件用于声明包，没有公开 Scoop API。native/fence_work.c 占用真实平台后端的工作槽，native/work.c 提供可控工作、结果和释放计数；导出函数旁说明调用及生命周期要求。

这是嵌套测试／辅助 Cone，不属于发行内置库清单，也不进入 SYSROOT/docs 的生产库站点。

Windows TTY 测试需要在独立、隐藏的 console 中启动 `scoop test`，允许其测试子进程继承该 console；标准流仍可重定向。测试辅助库直接打开 `CONIN$`／`CONOUT$`，不会创建可见窗口。外部 runner 用 `CREATE_NEW_CONSOLE` 和 `STARTF_USESHOWWINDOW`／`SW_HIDE` 启动；断开测试用 `FreeConsole`，结束后重新附着父进程的测试 console。POSIX 的 PTY hangup 和 controlling-terminal 检查只在 Unix 目标编译。
