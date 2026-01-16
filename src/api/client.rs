use anyhow::{Context, Result};
use reqwest::Client;

use super::models::{
    CollectionDetail, CollectionDetailInfo, CollectionDetailResponse, CollectionInfo,
    CollectionsResponse, EnvironmentDetail, EnvironmentDetailResponse, EnvironmentInfo,
    EnvironmentsResponse, ExecutedResponse, Item, Request, Variable, WorkspaceInfo,
    WorkspacesResponse,
};

const BASE_URL: &str = "https://api.getpostman.com";

#[derive(Clone)]
pub struct PostmanClient {
    client: Client,
    api_key: String,
}

impl PostmanClient {
    pub fn new(api_key: String) -> Self {
        PostmanClient {
            client: Client::new(),
            api_key,
        }
    }

    pub async fn list_workspaces(&self) -> Result<Vec<WorkspaceInfo>> {
        let url = format!("{}/workspaces", BASE_URL);
        let response: WorkspacesResponse = self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context("Failed to fetch workspaces")?
            .json()
            .await
            .context("Failed to parse workspaces response")?;

        Ok(response.workspaces)
    }

    pub async fn list_collections(&self, workspace_id: Option<&str>) -> Result<Vec<CollectionInfo>> {
        let mut url = format!("{}/collections", BASE_URL);
        if let Some(ws_id) = workspace_id {
            url = format!("{}?workspace={}", url, ws_id);
        }
        let response: CollectionsResponse = self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context("Failed to fetch collections")?
            .json()
            .await
            .context("Failed to parse collections response")?;

        Ok(response.collections)
    }

    pub async fn get_collection(&self, collection_uid: &str) -> Result<CollectionDetail> {
        let url = format!("{}/collections/{}", BASE_URL, collection_uid);
        let response_text = self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context("Failed to fetch collection details")?
            .text()
            .await
            .context("Failed to read collection response")?;

        let response: CollectionDetailResponse = serde_json::from_str(&response_text)
            .with_context(|| {
                format!(
                    "Failed to parse collection details. Response preview: {}",
                    &response_text[..response_text.len().min(500)]
                )
            })?;

        Ok(response.collection)
    }

    pub async fn execute_request(&self, request: &Request) -> Result<ExecutedResponse> {
        let url = request.url.to_string();

        let mut req_builder = match request.method.to_uppercase().as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            "HEAD" => self.client.head(&url),
            _ => self.client.get(&url),
        };

        for header in &request.header {
            // Skip disabled headers and headers with empty keys
            if header.disabled.unwrap_or(false) || header.key.trim().is_empty() {
                continue;
            }
            req_builder = req_builder.header(&header.key, &header.value);
        }

        if let Some(body) = &request.body {
            if let Some(raw) = &body.raw {
                req_builder = req_builder.body(raw.clone());
            }
        }

        let response = req_builder
            .send()
            .await
            .with_context(|| format!("Failed to execute request to {}", url))?;

        let status = response.status().as_u16();
        let status_text = response.status().to_string();

        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let body = response
            .text()
            .await
            .context("Failed to read response body")?;

        Ok(ExecutedResponse {
            status,
            status_text,
            headers,
            body,
        })
    }

    pub async fn update_collection(
        &self,
        collection_uid: &str,
        info: &CollectionDetailInfo,
        items: &[Item],
    ) -> Result<()> {
        let url = format!("{}/collections/{}", BASE_URL, collection_uid);

        let body = serde_json::json!({
            "collection": {
                "info": {
                    "_postman_id": info.postman_id,
                    "name": info.name,
                    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
                },
                "item": items
            }
        });

        let response = self
            .client
            .put(&url)
            .header("X-Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to update collection")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!("API returned error {}: {}", status, error_body);
        }

        Ok(())
    }

    /// Update a single request using the individual request endpoint
    /// This avoids validation errors from unrelated requests in the collection
    pub async fn update_request(
        &self,
        collection_uid: &str,
        request_id: &str,
        name: &str,
        request: &Request,
    ) -> Result<()> {
        let url = format!("{}/collections/{}/requests/{}", BASE_URL, collection_uid, request_id);

        let body = serde_json::json!({
            "name": name,
            "request": request
        });

        let response = self
            .client
            .put(&url)
            .header("X-Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to update request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!("API returned error {}: {}", status, error_body);
        }

        Ok(())
    }

    pub async fn list_environments(&self, workspace_id: Option<&str>) -> Result<Vec<EnvironmentInfo>> {
        let mut url = format!("{}/environments", BASE_URL);
        if let Some(ws_id) = workspace_id {
            url = format!("{}?workspace={}", url, ws_id);
        }
        let response: EnvironmentsResponse = self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context("Failed to fetch environments")?
            .json()
            .await
            .context("Failed to parse environments response")?;

        Ok(response.environments)
    }

    pub async fn get_environment(&self, environment_uid: &str) -> Result<EnvironmentDetail> {
        let url = format!("{}/environments/{}", BASE_URL, environment_uid);
        let response: EnvironmentDetailResponse = self
            .client
            .get(&url)
            .header("X-Api-Key", &self.api_key)
            .send()
            .await
            .context("Failed to fetch environment details")?
            .json()
            .await
            .context("Failed to parse environment response")?;

        Ok(response.environment)
    }

    pub async fn update_environment(
        &self,
        environment_uid: &str,
        name: &str,
        values: &[Variable],
    ) -> Result<()> {
        let url = format!("{}/environments/{}", BASE_URL, environment_uid);

        let body = serde_json::json!({
            "environment": {
                "name": name,
                "values": values
            }
        });

        let response = self
            .client
            .put(&url)
            .header("X-Api-Key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to update environment")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            anyhow::bail!("API returned error {}: {}", status, error_body);
        }

        Ok(())
    }
}
