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

## GitHub Actions 构建和发布

在 GitHub 的 **Actions → Build and release → Run workflow** 中手动运行，成功后可以在该次运行的 Artifacts 中下载 Windows x64、macOS Intel 和 macOS Apple Silicon 的 ZIP。产物保留 14 天；Windows ZIP 包含 EXE，macOS ZIP 包含完整的 `JSON Editor.app`。

推送 `v*` 标签会运行相同的检查和构建，并在全部平台成功后自动创建 GitHub Release，上传三个 ZIP 和 `SHA256SUMS.txt`。例如先将 `Cargo.toml` 的版本和对应的 `Cargo.lock` 更新并提交，再执行：

```sh
git tag v0.1.0
git push origin v0.1.0
```

工作流使用仓库自带的 `GITHUB_TOKEN`，不需要额外配置发布密钥；仓库需允许 GitHub Actions 运行。重复运行同一标签会替换该 Release 的同名附件。macOS 应用仅做临时签名，没有 Apple Developer ID 签名或公证，下载后可能受到 Gatekeeper 限制。

## 功能

- 自由输入、选中、复制、剪切、粘贴、撤销和重做。
- Tree-sitter JSON 语法高亮，区分字符串、数字、布尔值及 null。
- **格式化**：两空格缩进，保留对象字段顺序和高精度数字；无效 JSON 保留原文。格式化可以撤销。
- **验证**：底部显示结果；错误显示从 1 开始的行号、列号及解析原因。列号采用 JSON 解析器的字节位置。
- **折叠**：点击行号旁的箭头折叠或展开多行对象、数组，支持嵌套折叠。
- 编辑后清除旧验证结果，显示待验证状态。
- 右上角 **复制** 将整份文档复制到剪贴板；**从剪贴板粘贴** 用剪贴板文本替换整份文档，保留原始文本并支持撤销。剪贴板为空、不可读取或无文本时不修改文档。

| 操作 | Windows / Linux | macOS |
| --- | --- | --- |
| 保存 / 首次创建文档 | Ctrl+S | Cmd+S |
| 复制整份文档 | Ctrl+Shift+C | Cmd+Shift+C |
| 从剪贴板替换整份文档 | Ctrl+Shift+V | Cmd+Shift+V |
| 格式化 | Ctrl+Shift+F | Cmd+Shift+F |
| 验证 | Ctrl+Enter | Cmd+Enter |
| 撤销 / 重做 | Ctrl+Z / Ctrl+Y | Cmd+Z / Cmd+Shift+Z |
| 全选 / 复制选区 / 剪切选区 / 光标处粘贴 | Ctrl+A / C / X / V | Cmd+A / C / X / V |

工具栏按钮悬停时会显示对应平台的快捷键。

## 本地文档历史

启动时默认打开空白的新文档，并在用户主目录下创建 `.json-editor/documents` 数据目录（Windows 通常为 `C:\Users\用户名\.json-editor\documents`，macOS 为 `/Users/用户名/.json-editor/documents`，Linux 为 `/home/用户名/.json-editor/documents`）。

- 点击 **保存** 或按 **Ctrl+S**（Windows/Linux）、**Cmd+S**（macOS）保存。首次保存以本机时间命名为 `yyyy-mm-dd hh-mm-ss.json`；同一秒内重名时追加 `-1`、`-2` 等序号。后续保存更新当前文件。
- 左侧 **文档历史** 显示数据目录中的 JSON 文档，最近保存的在前。点击文档恢复编辑，点击 **刷新** 可重新扫描目录；重启后历史仍保留，编辑区依然从空白文档开始。
- 点击 **新建文档** 开始另一份空白文档。切换历史或新建前如有未保存内容，会提供 **保存并继续**、**放弃改动并继续** 和 **取消**；保存失败时保留当前内容。
- 已保存的文档可以点击 **重命名当前文档**，输入新名称后确认。可省略 `.json` 扩展名；不会覆盖同名文件。重命名保留正在编辑的未保存内容，后续保存写入新名称。
- 保存保留原始文本，不自动格式化或验证，因此空白、无效 JSON 和未完成草稿也可以保存。保存使用同目录临时文件替换，避免写入失败直接截断原文件。

内容有变化时文件名旁显示 **未保存**。关闭程序前请保存需要保留的内容。

## 检查

```powershell
cargo fmt --check
cargo check --locked
cargo test --locked
```

JSON 处理测试覆盖字段顺序、高精度数字、中文、嵌套内容、错误位置、空文本及尾随内容。

文档测试覆盖时间命名、同秒重名、保存和重新读取、重命名不覆盖、非法名称、历史筛选，以及界面保存快捷键、未保存内容切换提示和保存失败时保留内容。

源码：`src/main.rs` 为 GPUI 界面，`src/json.rs` 为格式化和验证逻辑，`src/documents.rs` 为本地文档存储。
