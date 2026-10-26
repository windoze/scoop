## 运行

在保存文件的目录执行：

```sh
scoop run --release files.scoop
```

## 预期结果

程序 stdout：

```text
Hello, Scoop!
Second line
bytes=26
notes.txt
```

## 关键说明

createTempDirectory 选择临时位置。Path.resolve 组合路径，writeText 的 append 参数明确决定是否追加。readText 返回文本；metadata().size 计数的是字节。entries 返回需清理的目录迭代资源，因此用 use 包住遍历。

目录条目的名称是 OsString，display 用于展示；本机路径不应被假定为任意平台上都有效的普通 Unicode 字符串。这里创建的目录只有一个文件，输出不依赖多条目排序。finally 保证清理；不写用户已有路径。

## 常见误用

不要省略 entries 的清理，也不要把文件字节大小当作字符数量。若改为处理现有目录，先明确所有权，不能照搬示例末尾的递归删除。
