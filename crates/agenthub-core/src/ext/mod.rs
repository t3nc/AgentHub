//! Phase 2/3/4/5 的预留扩展点。
//!
//! 本期只落地 trait 与 DTO：主链路（扫描 / 计划 / 落盘）不依赖它们，
//! 后续接入只需在对应目录新增实现并注册命令，不改动核心流程。

pub mod catalog;
pub mod gateway;
pub mod i18n;
pub mod installer;
pub mod pricing;
pub mod provider;
pub mod supplier;
pub mod usage;
