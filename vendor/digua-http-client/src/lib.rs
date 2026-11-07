//! Kudi HTTP client SDK: service-to-service (client_credentials) + user Bearer passthrough.

// Install the rustls crypto provider once. Safe to call multiple times.
use std::sync::Once;
static CRYPTO_INIT: Once = Once::new();
fn ensure_crypto_provider() {
    // ai-pm vendored-snapshot patch (2026-11-07): upstream installs the
    // rustls aws_lc_rs provider because lmms's reqwest uses rustls. This
    // workspace pins reqwest to native-tls (schannel — zero C compilation
    // deps, see root Cargo.toml), so there is no rustls provider to install.
    CRYPTO_INIT.call_once(|| {});
}

pub mod cache;
pub mod client;
pub mod error;
pub mod fetcher;
pub mod models;
pub mod options;
pub mod transport;

/// Re-export of [`error::KudiHttpError`] under the canonical SDK name.
///
/// Service SDKs (iam/token/audit/...) should `use digua_http_client::SdkError`
/// for the error type so callers don't have to know the internal name.
/// Aligns with the multi-language SDK design spec
/// `docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md` §9.
pub use crate::error::KudiHttpError as SdkError;
