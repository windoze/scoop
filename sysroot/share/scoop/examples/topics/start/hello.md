## 运行

把文件保存为 `main.scoop`，在所在目录执行：

```sh
scoop check main.scoop
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
Hello, Scoop!
```

## 关键说明

`.scoop` 是源文件扩展名。普通 `main` 是程序入口，`println` 追加换行。`check` 检查程序，`run` 构建可执行文件并运行。这里没有异步操作，不需要事件循环。

单文件入口默认提供配套版本的 core 和 std。程序可放在普通可写目录；不要求创建 Cone.toml。默认构建输出在当前目录的 `target/`。

## 常见误用

不要把其他语言的 `System.out`、`console.log` 或 `printf` 当作 Scoop 内置 API。复制这里的完整文件即可；`example` 只展示内容，不会替你运行。
