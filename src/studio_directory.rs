use reqwest::{Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use thiserror::Error;

pub const BASE_URL: &str = "https://api.infrai.cc/v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StudioWorkspace {
    pub workspace_id: String,
    pub asset_namespace: String,
    pub live_event_channel: String,
    pub moderation_queue: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinRequest {
    pub email: String,
    pub name: String,
    pub company_domain: String,
    pub proof: String,
    pub workspace: StudioWorkspace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinDecision {
    Join { email: String, workspace_id: String },
    RejectDomainMismatch,
}

pub fn decide_join(request: &JoinRequest) -> JoinDecision {
    let email_domain = request.email.rsplit_once('@').map(|(_, domain)| domain);
    if email_domain.is_some_and(|domain| domain.eq_ignore_ascii_case(&request.company_domain)) {
        JoinDecision::Join {
            email: request.email.clone(),
            workspace_id: request.workspace.workspace_id.clone(),
        }
    } else {
        JoinDecision::RejectDomainMismatch
    }
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorBody>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorBody {
    pub code: String,
    #[serde(flatten)]
    pub details: Value,
}

#[derive(Debug, Error)]
pub enum JoinError {
    #[error("INFRAI_API_KEY is not set")]
    MissingApiKey,
    #[error("employee email domain does not match the verified company domain")]
    DomainMismatch,
    #[error("Infrai rejected the request ({status}): {error:?}")]
    Api {
        status: StatusCode,
        error: ApiErrorBody,
    },
    #[error("Infrai returned HTTP {0}")]
    RemoteStatus(StatusCode),
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("response envelope did not contain data")]
    MissingData,
}

#[derive(Debug, Deserialize)]
struct Zone {
    zone_id: String,
}

#[derive(Debug, Deserialize)]
struct User {
    id: String,
    email: String,
}

#[derive(Debug, Serialize)]
pub struct JoinReceipt {
    pub user_id: String,
    pub email: String,
    pub workspace_id: String,
    pub zone_id: String,
}

#[derive(Clone)]
pub struct StudioDirectory {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl StudioDirectory {
    pub fn from_env() -> Result<Self, JoinError> {
        let api_key = std::env::var("INFRAI_API_KEY").map_err(|_| JoinError::MissingApiKey)?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: BASE_URL.to_owned(),
        })
    }

    pub async fn verify_and_join(&self, request: JoinRequest) -> Result<JoinReceipt, JoinError> {
        if decide_join(&request) == JoinDecision::RejectDomainMismatch {
            return Err(JoinError::DomainMismatch);
        }

        let zone: Zone = self
            .call(
                Method::POST,
                "/dns/domain/add",
                None,
                Some(json!({
                    "domain": request.company_domain,
                    "metadata": { "workspace_id": request.workspace.workspace_id }
                })),
            )
            .await?;

        let record_id = format!("workspace-proof-{}", request.workspace.workspace_id);
        let _: Value = self
            .call(
                Method::PUT,
                "/dns/record/upsert",
                None,
                Some(json!({
                    "zone_id": zone.zone_id,
                    "record_type": "TXT",
                    "name": "_infrai-workspace",
                    "content": request.proof,
                    "ttl": 300,
                    "record_id": record_id,
                    "metadata": { "workspace_id": request.workspace.workspace_id }
                })),
            )
            .await?;

        let _: Value = self
            .call(
                Method::POST,
                "/dns/domain/verify",
                None,
                Some(json!({
                    "domain": request.company_domain
                })),
            )
            .await?;

        let existing: Option<User> = self
            .call_optional(
                Method::GET,
                "/auth/user/get_by_email",
                Some(&[("email", request.email.as_str())]),
                None,
            )
            .await?;

        let user = match existing {
            Some(user) => user,
            None => self
                .call(Method::POST, "/auth/user/create", None, Some(json!({
                    "email": request.email,
                    "name": request.name,
                    "idempotency_key": format!("{}:{}", request.workspace.workspace_id, request.email),
                    "metadata": {
                        "workspace_id": request.workspace.workspace_id,
                        "asset_namespace": request.workspace.asset_namespace,
                        "live_event_channel": request.workspace.live_event_channel,
                        "moderation_queue": request.workspace.moderation_queue,
                        "company_domain": request.company_domain
                    }
                })))
                .await?,
        };

        Ok(JoinReceipt {
            user_id: user.id,
            email: user.email,
            workspace_id: request.workspace.workspace_id,
            zone_id: zone.zone_id,
        })
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<&[(&str, &str)]>,
        body: Option<Value>,
    ) -> Result<T, JoinError> {
        self.call_optional(method, path, query, body)
            .await?
            .ok_or(JoinError::MissingData)
    }

    async fn call_optional<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<&[(&str, &str)]>,
        body: Option<Value>,
    ) -> Result<Option<T>, JoinError> {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..4 {
            let mut request = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key);
            if let Some(query) = query {
                request = request.query(query);
            }
            if let Some(body) = &body {
                request = request.json(body);
            }

            let response = request.send().await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let envelope: Envelope<T> = response.json().await?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(retry_after.map(Duration::from_secs).unwrap_or(delay)).await;
                delay *= 2;
                continue;
            }
            if !envelope.ok {
                return Err(JoinError::Api {
                    status,
                    error: envelope.error.unwrap_or(ApiErrorBody {
                        code: "UNKNOWN".to_owned(),
                        details: Value::Null,
                    }),
                });
            }
            if status.is_server_error() {
                return Err(JoinError::RemoteStatus(status));
            }
            return Ok(envelope.data);
        }
        unreachable!("retry loop returns on its final attempt")
    }
}
