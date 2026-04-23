#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum InvalidationStatus {
    InProgress,
    Completed,
    Failed(String),
}
