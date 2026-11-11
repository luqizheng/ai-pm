//! `ArtifactStore` 的 file-service 适配（TD-01 §4.4 / TD-04 §2：复用
//! file-service，docproc 不直连 S3/RustFS）。
//!
//! 信任模型与 pm-api files.rs 相同（contracts/http-conventions.md）：
//! - 出站 per-tenant 服务 JWT（tid = upstream_tenant_key 镜像；"-" = dev
//!   映射租户 1）；
//! - 归属：上传 user_id 由本层覆写为服务身份（worker 场景无终端主体）。
//!
//! `from_env` 缺配置（裸机 dev 无 SERVICE_BOOTSTRAP_TOKEN）→ None：路由
//! 503、worker 跳过 docproc 循环——诚实降级，不假成功。

use std::sync::Arc;
use std::time::Duration;

use digua_common_jwt::{ServiceJwtCache, ServiceJwtConfig, ServiceJwtError};
use digua_file_sdk::{FileClient, SdkError};

use pm_application::docproc_service::{ArtifactStore, StoreError};

pub struct FileServiceStore {
    file_service_url: String,
    service_jwt: Arc<ServiceJwtCache>,
    /// 进程身份（服务名，用于 JWT caller 与 user_id 归属声明）
    caller_service: &'static str,
}

impl FileServiceStore {
    /// pm-api（用户面）与 pm-worker（产物写面）共用同一构造。
    pub fn from_env(caller_service: &'static str) -> Option<Arc<Self>> {
        let bootstrap_token = match std::env::var("SERVICE_BOOTSTRAP_TOKEN") {
            Ok(t) if !t.trim().is_empty() => t,
            _ => {
                tracing::warn!(
                    service = caller_service,
                    "docproc store disabled: no SERVICE_BOOTSTRAP_TOKEN"
                );
                return None;
            }
        };
        let file_service_url =
            std::env::var("FILE_SERVICE_URL").unwrap_or_else(|_| "http://file-service:8082".into());
        let token_service_url = std::env::var("AI_PM_TOKEN_SERVICE_URL")
            .unwrap_or_else(|_| "http://token-service:8082".into());
        let service_jwt = Arc::new(ServiceJwtCache::new(
            ServiceJwtConfig {
                token_service_url,
                bootstrap_token,
                caller_service_name: caller_service.to_string(),
                caller_tenant: 1,
                refresh_skew: Duration::from_secs(60),
                request_timeout: Duration::from_secs(10),
            },
            reqwest::Client::builder().no_proxy().build().ok()?,
            String::new(),
        ));
        Some(Arc::new(Self {
            file_service_url,
            service_jwt,
            caller_service,
        }))
    }

    /// upstream_tenant_key → 数字 tid（"-" = dev 映射租户 1，同 files.rs）。
    fn tid_of(&self, tenant_key: &str) -> Result<i64, StoreError> {
        let raw = tenant_key.trim();
        if raw == "-" {
            return Ok(1);
        }
        raw.parse()
            .map_err(|_| StoreError::Rejected("upstream tenant key is not numeric".into()))
    }

    async fn token(&self, tenant_key: &str) -> Result<String, StoreError> {
        let tid = self.tid_of(tenant_key)?;
        self.service_jwt
            .get_for_tenant("file-service", tid, false)
            .await
            .map_err(svc_jwt_err)
    }

    fn client_for(&self, token: String) -> FileClient {
        FileClient::new(self.file_service_url.clone(), move || Some(token.clone()))
    }
}

fn svc_jwt_err(e: ServiceJwtError) -> StoreError {
    StoreError::Unavailable(format!("service-jwt mint failed: {e}"))
}

fn sdk_err(e: SdkError) -> StoreError {
    match &e {
        SdkError::Network(m) => StoreError::Unavailable(m.clone()),
        other => StoreError::Rejected(format!("{other}")),
    }
}

/// 预签时效：短时效直取（TD-04 §2，不代理字节流）。
const PRESIGN_TTL_SECS: i64 = 600;

#[async_trait::async_trait]
impl ArtifactStore for FileServiceStore {
    async fn put(
        &self,
        tenant_key: &str,
        filename: &str,
        data: Vec<u8>,
    ) -> Result<String, StoreError> {
        let token = self.token(tenant_key).await?;
        let client = self.client_for(token);
        let meta = digua_file_sdk::UploadFileMeta {
            filename: filename.to_string(),
            content_type: Some("application/octet-stream".to_string()),
            user_id: Some(self.caller_service.to_string()),
            is_temporary: Some(false),
            expires_in_seconds: None,
        };
        let info = client
            .upload_file(&meta, filename, data)
            .await
            .map_err(sdk_err)?;
        Ok(info.id)
    }

    async fn get(&self, tenant_key: &str, storage_key: &str) -> Result<Vec<u8>, StoreError> {
        let token = self.token(tenant_key).await?;
        let client = self.client_for(token);
        let downloaded = client.download_file(storage_key).await.map_err(sdk_err)?;
        Ok(downloaded.bytes)
    }

    async fn presign(&self, tenant_key: &str, storage_key: &str) -> Result<String, StoreError> {
        let token = self.token(tenant_key).await?;
        let client = self.client_for(token);
        let req = digua_file_sdk::CreatePresignUrl {
            file_id: storage_key.to_string(),
            operation: "download".to_string(),
            expires_in_seconds: Some(PRESIGN_TTL_SECS),
        };
        let resp = client.create_presign(&req).await.map_err(sdk_err)?;
        Ok(resp.url)
    }
}
