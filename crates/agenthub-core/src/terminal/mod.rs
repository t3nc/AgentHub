//! 终端会话历史：`~/.agenthub/logs/terminal/` 下的元数据与日志文件。
//!
//! 只放数据规则（保留多少个、文件名怎么清洗），不碰 PTY：
//! 真正的写入节奏由壳层的日志线程决定。

pub mod history;
