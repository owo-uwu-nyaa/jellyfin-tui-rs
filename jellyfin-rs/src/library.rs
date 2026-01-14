use crate::{
    connect::JsonResponse,
    request::{NoQuery, RequestBuilderExt},
    Authed, JellyfinClient,
};

use color_eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Library {
    pub name: String,
    pub item_id: String,
    pub refresh_progress: Option<f64>,
}


impl<Auth: Authed> JellyfinClient<Auth> {
    pub async fn get_libraries(&self) -> Result<JsonResponse<Vec<Library>>> {
        self.send_request_json(self.get("/Library/VirtualFolders", NoQuery)?.empty_body()?)
            .await
    }
}
