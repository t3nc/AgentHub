use std::path::Path;

use crate::l10n::{Msg, text};

/// 领域错误。
///
/// 文案不写死在这里：`Display` 按当前界面语言取词，CLI 与 GUI 共用同一份词典。
/// 也正因为要带 locale，`Display` 是手写的而不是 `thiserror` 生成的 ——
/// derive 的 `#[error("...")]` 只能写死字面量。
#[derive(Debug)]
pub enum Error {
    Io {
        path: String,
        source: std::io::Error,
    },
    Db(rusqlite::Error),
    Json(serde_json::Error),
    Toml(String),
    Yaml(String),
    Unsupported(String),
    UnsafePath(String),
    NotFound(String),
    Conflict(String),
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rendered = match self {
            Error::Io { path, source } => text(
                Msg::ErrIo,
                &[("path", path), ("source", &source.to_string())],
            ),
            Error::Db(source) => text(Msg::ErrDb, &[("source", &source.to_string())]),
            Error::Json(source) => text(Msg::ErrJson, &[("source", &source.to_string())]),
            Error::Toml(source) => text(Msg::ErrToml, &[("source", source)]),
            Error::Yaml(source) => text(Msg::ErrYaml, &[("source", source)]),
            Error::Unsupported(detail) => text(Msg::ErrUnsupported, &[("detail", detail)]),
            Error::UnsafePath(path) => text(Msg::ErrUnsafePath, &[("path", path)]),
            Error::NotFound(what) => text(Msg::ErrNotFound, &[("what", what)]),
            Error::Conflict(detail) => text(Msg::ErrConflict, &[("detail", detail)]),
            Error::Other(detail) => text(Msg::ErrOther, &[("detail", detail)]),
        };
        f.write_str(&rendered)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            Error::Db(source) => Some(source),
            Error::Json(source) => Some(source),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(source: rusqlite::Error) -> Self {
        Error::Db(source)
    }
}

impl From<serde_json::Error> for Error {
    fn from(source: serde_json::Error) -> Self {
        Error::Json(source)
    }
}

impl Error {
    pub fn io(path: impl AsRef<Path>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.as_ref().display().to_string(),
            source,
        }
    }

    pub fn msg(text: impl Into<String>) -> Self {
        Error::Other(text.into())
    }
}

/// 便捷宏：`io_try!(fs::read(path))` 自动把 io 错误附上路径。
#[macro_export]
macro_rules! io_try {
    ($expr:expr) => {
        $expr.map_err(|e| $crate::error::Error::io(file!(), e))?
    };
}
