use crate::{error::AppResult, models::*, session_indexer};
use std::path::Path;

pub fn scan(db_path: &Path, settings: &AppSettings) -> AppResult<Vec<SessionSummary>> {
    session_indexer::full_scan(db_path, settings)
}
