use std::io;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库操作失败：{0}")]
    Database(#[from] rusqlite::Error),
    #[error("文件操作失败：{0}")]
    Io(#[from] io::Error),
    #[error("JSON 数据无效：{0}")]
    Json(#[from] serde_json::Error),
    #[error("压缩包操作失败：{0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Message(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
