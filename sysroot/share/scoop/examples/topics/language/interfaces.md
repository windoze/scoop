## 运行

在保存文件的目录执行：

```sh
scoop run --release main.scoop
```

## 预期结果

程序 stdout：

```text
Ada
Scoop
```

## 关键说明

Named 声明一个可调用能力；Person 和 Label 都显式列出接口并使用 public override 实现。只拥有同名方法不构成 conformance。接口中的 public 方法与实现方法的可见性要一致。

`person: Named` 保存接口视图；Label 是 struct，传给接口参数时通过值装箱提供该视图。修改这里的类型时，可以增加实例字段，但调用者仍只依赖 displayName 合同。

## 常见误用

不要假设 public interface 会自动公开所有成员，或省略实现方法的 override/public。public class 本身也不会自动公开构造器；跨 Cone 示例见相关主题。
