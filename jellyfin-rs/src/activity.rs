use super::err::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::sha::Sha256;
use crate::Authed;
use crate::JellyfinClient;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ActivityLogEntry {
    pub id: u32,
    pub name: String,
    pub overview: Option<String>,
    pub short_overview: Option<String>,
    pub r#type: String,
    pub item_id: Option<String>,
    pub date: String,
    pub user_id: String,
    pub severity: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ActivityLogEntries {
    pub items: Vec<ActivityLogEntry>,
    pub total_record_count: u32,
    pub start_index: u32,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActivityLogEntriesQuery<'s> {
    start_index: Option<u32>,
    limit: Option<u32>,
    min_date: Option<&'s str>,
    has_user_id: bool,
}

impl<Auth: Authed, Sha: Sha256> JellyfinClient<Auth, Sha> {
    pub async fn get_activity_log_entries(
        &self,
        start_index: Option<u32>,
        limit: Option<u32>,
        min_date: Option<&str>,
        has_user_id: bool,
    ) -> Result<ActivityLogEntries> {
        let req = self
            .get(format!("{}System/ActivityLog/Entries", self.url,))
            .query(&GetActivityLogEntriesQuery {
                start_index,
                limit,
                min_date,
                has_user_id,
            })
            .send()
            .await?;
        Ok(req.json().await?)
    }
}
