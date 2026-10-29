# 原生调试定向验收

先显式准备配套 sysroot 和 Python-enabled Scoop LLDB，再从仓库根目录构建两个程序。
这些脚本只消费现成产物，不隐式重建编译器或系统库。

```sh
scoop build tooling/vscode-scoop/test/debugger/program.scoop --debug-info full \
  --enable-gc-stress -o /tmp/scoop-debug-program
scoop build tooling/vscode-scoop/test/debugger/console.scoop --debug-info full \
  -o /tmp/scoop-debug-console

python3 tooling/vscode-scoop/test/debugger/dap.py --adapter /path/to/bin/lldb-dap \
  --program /tmp/scoop-debug-program --source tooling/vscode-scoop/test/debugger/program.scoop \
  --mode launch --source-map --log /tmp/scoop-debug-launch.jsonl
python3 tooling/vscode-scoop/test/debugger/dap.py --adapter /path/to/bin/lldb-dap \
  --program /tmp/scoop-debug-console --source tooling/vscode-scoop/test/debugger/console.scoop \
  --mode terminal --log /tmp/scoop-debug-terminal.jsonl
python3 tooling/vscode-scoop/test/debugger/dap.py --adapter /path/to/bin/lldb-dap \
  --program /tmp/scoop-debug-console --source tooling/vscode-scoop/test/debugger/console.scoop \
  --mode attach --log /tmp/scoop-debug-attach.jsonl
```

launch 覆盖源码断点、物理栈、单步、参数／局部／全局／TLS、UTF-8 和内嵌 NUL String、
数组分页、tagged／niche enum、对象／interface、构造后局部及闭包实际读取。
强制 moving GC 后检查存活引用的新地址与字段、失效变量不可用，以及调用者帧不显示旧引用。
`--source-map` 将源码复制到含空格的临时目录，并以实际 DAP sourceMap 查找断点和栈位置。
terminal 使用适配器的 runInTerminal 请求与真实控制 PTY；attach 在 Linux 通过
PR_SET_PTRACER 只允许该次适配器访问该次目标，不改系统 ptrace 配置。日志保留原始协议，
成功标记为 `SCOOP_DAP_OK`；超时会关闭本次适配器与目标。

`--mode initialize` 只检查打包后的 DAP 握手，不代表进程内调试验收。
`offline.py` 在 LLDB 中检查源码名与精确 linkage 名命中相同断点，并对真实 program DWARF
的 SBValue 检查 enum、空引用和数组布局：

```sh
lldb -b /tmp/scoop-debug-program \
  -o 'command script import tooling/vscode-scoop/debugger/scoop_lldb.py' \
  -o 'command script import tooling/vscode-scoop/test/debugger/offline.py' -o quit
```

完整变量求值、写入 managed heap 和协程逻辑栈不在本批验收范围。

Linux 静态 TLS 的原生定向用例覆盖 `.tdata`、`.tbss`、64-byte 对齐及两个线程的独立值。
它验证调试器的目标 TLS 地址解析，编译和运行只需一个小型 C 程序：

```sh
musl-gcc -std=c11 -g -O0 -static -pthread tooling/vscode-scoop/test/debugger/tls.c \
  -o /tmp/scoop-debug-tls
lldb -b /tmp/scoop-debug-tls \
  -o 'command script import tooling/vscode-scoop/test/debugger/tls.py' -o quit
```

成功标记为 `SCOOP_STATIC_TLS_OK`。此用例与 `program.scoop` 的实际 Scoop TLS 断言共同验收。
