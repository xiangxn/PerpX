# FuturesTicker

一个基于 Rust 的期货行情监控工具，用于实时监控期货行情并触发价格波动警报。

## 功能特性

1. **实时行情监控**：通过 WebSocket 连接获取期货行情数据。
2. **价格波动警报**：当价格波动超过预设阈值时，触发警报。
3. **多时间周期分析**：支持 5分钟、15分钟、1小时、4小时等时间周期的价格分析。
4. **多线程处理**：使用 Tokio 异步运行时，支持多线程处理行情数据。
5. **配置灵活**：通过 `config.toml` 文件配置数据库、服务器和代理设置。

## 快速开始

### 依赖安装

确保已安装 Rust 和 Cargo。运行以下命令安装依赖：

```bash
cargo build
```

### 配置文件

在项目根目录下创建 `config.toml` 文件，内容如下：

```toml
[redis]
host = "localhost"
port = 5432
user = "postgres"
password = "password"

[server]
worker_count = 4
max_kline_count = 100

[proxy]
addr = "http://proxy.example.com"

[logging]
level = "debug"
```

### 运行程序

```bash
cargo run
```

## 代码结构

- `src/main.rs`：主程序入口，包含行情数据处理逻辑和警报触发逻辑。
- `src/config.rs`：配置文件加载模块。
- `config.toml`：配置文件。

## 未来计划

1. 支持更多期货交易所的行情数据。
2. 增加 Telegram 或 Webhook 通知功能。
3. 优化性能，支持更高频率的行情数据处理。

## 贡献

欢迎提交 Issue 或 Pull Request。