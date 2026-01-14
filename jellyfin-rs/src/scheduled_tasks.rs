use serde::{Deserialize, Serialize};

use crate::{
    connect::JsonResponse,
    request::{NoQuery, RequestBuilderExt},
    Authed, JellyfinClient,
};
use color_eyre::Result;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum TaskState {
    Idle,
    Cancelling,
    Running,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Hash, Eq)]
pub enum ExecutionResultStatus {
    Completed,
    Failed,
    Cancelled,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ExecutionResult {
    pub start_time_utc: String,
    pub end_time_utc: String,
    pub status: ExecutionResultStatus,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ScheduledTask {
    pub name: String,
    pub state: TaskState,
    pub id: String,
    pub last_execution_result: Option<ExecutionResult>,
    pub description: Option<String>,
    pub key: String,
    pub current_progress_percentage: f64,
}

pub mod known_keys {
    pub const REFRESH_LIBRARY: &str = "RefreshLibrary";
}

impl<Auth: Authed> JellyfinClient<Auth> {
    pub async fn get_scheduled_tasks(&self) -> Result<JsonResponse<Vec<ScheduledTask>>> {
        self.send_request_json(
            self.get("/ScheduledTasks?isHidden=false", NoQuery)?
                .empty_body()?,
        )
        .await
    }
    pub async fn get_scheduled_task(&self, id: &str) -> Result<JsonResponse<ScheduledTask>> {
        self.send_request_json(
            self.get(
                |base: &mut String| {
                    base.push_str("/ScheduledTasks/");
                    base.push_str(id);
                },
                NoQuery,
            )?
            .empty_body()?,
        )
        .await
    }
    pub async fn start_scheduled_task(&self, id: &str) -> Result<()> {
        self.send_request(
            self.post(
                |base: &mut String| {
                    base.push_str("/ScheduledTasks/Running/");
                    base.push_str(id);
                },
                NoQuery,
            )?
            .empty_body()?,
        )
        .await?;
        Ok(())
    }
    pub async fn stop_scheduled_task(&self, id: &str) -> Result<()> {
        self.send_request(
            self.delete(
                |base: &mut String| {
                    base.push_str("/ScheduledTasks/Running/");
                    base.push_str(id);
                },
                NoQuery,
            )?
            .empty_body()?,
        )
        .await?;
        Ok(())
    }
}
