// Adapted from GetSayAll/remote-mic-app-windows, statistics.rs (GPL-3.0).
use serde::Serialize;
const MAX_VOICE_SESSION_RANKING:usize=10;
const MAX_APPLICATION_NAME_CHARS:usize=64;
#[derive(Serialize)]
pub struct VoiceSessionUsageRecord {
    /// 结束时刻（Unix 毫秒）；用于“时长相同按更晚结束优先”的排序与界面展示。
    pub ended_at_unix_ms: i64,
    pub duration_seconds: f64,
    pub application_name: Option<String>,
}

pub fn normalized_voice_session_ranking(
    records: Vec<VoiceSessionUsageRecord>,
) -> Vec<VoiceSessionUsageRecord> {
    let mut ranking: Vec<VoiceSessionUsageRecord> = records
        .into_iter()
        .filter_map(|mut record| {
            record.duration_seconds = sanitize_duration(record.duration_seconds);
            if record.duration_seconds <= 0.0 {
                return None;
            }
            record.ended_at_unix_ms = record.ended_at_unix_ms.max(0);
            record.application_name = record
                .application_name
                .as_deref()
                .and_then(normalize_application_name);
            Some(record)
        })
        .collect();
    ranking.sort_by(|left, right| {
        right
            .duration_seconds
            .total_cmp(&left.duration_seconds)
            .then_with(|| right.ended_at_unix_ms.cmp(&left.ended_at_unix_ms))
            .then_with(|| left.application_name.cmp(&right.application_name))
    });
    ranking.dedup_by(|left, right| {
        left.ended_at_unix_ms == right.ended_at_unix_ms
            && left.duration_seconds == right.duration_seconds
            && left.application_name == right.application_name
    });
    ranking.truncate(MAX_VOICE_SESSION_RANKING);
    ranking
}

fn normalize_application_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_APPLICATION_NAME_CHARS).collect())
}

fn sanitize_duration(duration: f64) -> f64 {
    if duration.is_finite() {
        duration.max(0.0)
    } else {
        0.0
    }
}
