# JSON Editor

使用 Rust + GPUI / GPUI Kit 实现的原生桌面 JSON 编辑器。

## 运行

安装 rustup。项目使用 stable 工具链，需要 Rust 1.95 或更新版本。Windows 还需要 Visual Studio Build Tools 的「使用 C++ 的桌面开发」工具链和 Windows SDK。

Cargo 已通过 `.cargo/config.toml` 使用阿里云稀疏索引。rustup 工具链镜像在 PowerShell 中设置：

```powershell
$env:RUSTUP_DIST_SERVER = "https://mirrors.aliyun.com/rustup"
$env:RUSTUP_UPDATE_ROOT = "https://mirrors.aliyun.com/rustup/rustup"
rustup update stable
```

```powershell
cargo run
```

首次编译需要下载 GPUI 依赖，耗时较长。

## 发布构建

`cargo run` 默认使用开发配置（`dev` / `unoptimized`），适合开发和调试。分发程序时使用：

```powershell
cargo build --release --locked
cargo run --release
```

Windows 发布程序位于 `target/release/json-editor.exe`。程序已使用 GUI 子系统，双击 EXE 启动时不会打开控制台。只分发这个 EXE，不需要分发整个 `target` 目录。

macOS 使用应用包启动，避免双击裸可执行文件时打开终端：

```sh
sh scripts/build-macos-app.sh
open "target/release/JSON Editor.app"
```

也可以在 Finder 中双击 `target/release/JSON Editor.app`，或将它拖到「应用程序」目录。分发时保留完整的 `.app` 应用包。脚本会构建 Release 程序、生成应用元数据并进行本地临时签名。

发布配置优先优化体积，启用完整 LTO、单代码生成单元和符号剥离。`panic = "abort"` 表示发生 Rust panic 时直接结束程序，不进行栈展开。首次发布编译比开发编译慢。

## 功能

- 自由输入、选中、复制、剪切、粘贴、撤销和重做。
- Tree-sitter JSON 语法高亮，区分字符串、数字、布尔值及 null。
- **格式化**：两空格缩进，保留对象字段顺序和高精度数字；无效 JSON 保留原文。格式化可以撤销。
- **验证**：底部显示结果；错误显示从 1 开始的行号、列号及解析原因。列号采用 JSON 解析器的字节位置。
- **折叠**：点击行号旁的箭头折叠或展开多行对象、数组，支持嵌套折叠。
- 编辑后清除旧验证结果，显示待验证状态。

| 快捷键 | 操作 |
| --- | --- |
| Ctrl+Shift+F | 格式化 |
| Ctrl+Enter | 验证 |
| Ctrl+Z / Ctrl+Y | 撤销 / 重做 |
| Ctrl+A / C / X / V | 全选 / 复制 / 剪切 / 粘贴 |

当前版本编辑内存中的文档；文件打开、保存暂未加入。

## 检查

```powershell
cargo fmt --check
cargo check --locked
cargo test --locked
```

JSON 处理测试覆盖字段顺序、高精度数字、中文、嵌套内容、错误位置、空文本及尾随内容。

源码：`src/main.rs` 为 GPUI 界面，`src/json.rs` 为格式化和验证逻辑。
