use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsertionStatus {
    Success,
    Fallback,
    Failure,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InsertionRecord {
    pub text: String,
    pub status: InsertionStatus,
}

pub fn resolve_status(
    direct_result: Result<(), String>,
    fallback_enabled: bool,
    fallback_result: Result<(), String>,
) -> InsertionStatus {
    if direct_result.is_ok() {
        return InsertionStatus::Success;
    }
    if fallback_enabled && fallback_result.is_ok() {
        return InsertionStatus::Fallback;
    }
    InsertionStatus::Failure
}

pub fn append_recent(records: &mut Vec<InsertionRecord>, record: InsertionRecord, max: usize) {
    records.insert(0, record);
    records.truncate(max);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_direct_success() {
        let status = resolve_status(Ok(()), true, Ok(()));
        assert_eq!(status, InsertionStatus::Success);
    }

    #[test]
    fn uses_fallback_when_direct_fails() {
        let status = resolve_status(Err("direct failed".to_string()), true, Ok(()));
        assert_eq!(status, InsertionStatus::Fallback);
    }

    #[test]
    fn returns_failure_when_both_paths_fail() {
        let status = resolve_status(
            Err("direct failed".to_string()),
            true,
            Err("fallback failed".to_string()),
        );
        assert_eq!(status, InsertionStatus::Failure);
    }

    #[test]
    fn truncates_history_to_max_length() {
        let mut records = vec![
            InsertionRecord {
                text: "one".to_string(),
                status: InsertionStatus::Success,
            },
            InsertionRecord {
                text: "two".to_string(),
                status: InsertionStatus::Success,
            },
            InsertionRecord {
                text: "three".to_string(),
                status: InsertionStatus::Success,
            },
        ];
        append_recent(
            &mut records,
            InsertionRecord {
                text: "four".to_string(),
                status: InsertionStatus::Fallback,
            },
            3,
        );

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].text, "four");
        assert_eq!(records[1].text, "one");
        assert_eq!(records[2].text, "two");
    }
}
