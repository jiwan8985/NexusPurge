use crate::commands::s3::FileItem;

pub struct ListResult {
    pub files:      Vec<FileItem>,
    pub next_token: Option<String>,
    pub is_truncated: bool,
}
