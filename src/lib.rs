//! GCP auth provides authentication using service accounts Google Cloud Platform (GCP)
//!
//! The library looks for authentication methods in the following order:
//!
//! 1. Path to service account JSON configuration file using GOOGLE_APPLICATION_CREDENTIALS environment
//! variable. The service account configuration file can be downloaded in the IAM service when displaying service account detail.
//! The downloaded JSON file should be provided without any further modification.
//! 2. Invoking the library inside GCP environment fetches the default service account for the service and
//! the application is authenticated using that particular account
//! 3. Application default credentials. Local user authetincation for development purposes created using `gcloud auth` application.
//! 4. If none of the above can be used an error occurs
//!
//! The tokens are single-use and as such they shouldn't be cached and for each use a new token should be requested.
//! Library handles token caching for their lifetime and so it won't make a request if a token with appropriate scope
//! is available.
//!
//! # Default service account
//!
//! When running inside GCP the library can be asked directly without any further configuration to provide a Bearer token
//! for the current service account of the service.
//!
//! ```async
//! let authentication_manager = gcp_auth::init().await?;
//! let token = authentication_manager.get_token().await?;
//! ```
//!
//! # Custom service account
//!
//! When running outside of GCP e.g on development laptop to allow finer granularity for permission a
//! custom service account can be used. To use a custom service account a configuration file containing key
//! has to be downloaded in IAM service for the service account you intend to use. The configuration file has to
//! be available to the application at run time. The path to the configuration file is specified by
//! `GOOGLE_APPLICATION_CREDENTIALS` environment variable.
//!
//! ```async
//! // GOOGLE_APPLICATION_CREDENTIALS environtment variable is set-up
//! let authentication_manager = gcp_auth::init().await?;
//! let token = authentication_manager.get_token().await?;
//! ```
//! # Local user authentication
//! This authentication method allows developers to authenticate again GCP services when developign locally.
//! The method is intended only for development. Credentials can be set-up using `gcloud auth` utility.
//! Credentials are read from file `~/.config/gcloud/application_default_credentials.json`.
//!
//! # FAQ
//!
//! ## Does library support windows?
//!
//! No

#![deny(missing_docs)]
#![deny(warnings)]
#![allow(clippy::pedantic)]

mod authentication_manager;
mod custom_service_account;
mod default_authorized_user;
mod default_service_account;
mod error;
mod jwt;
mod types;
mod util;
mod prelude {
    pub(crate) use {
        crate::error::Error, crate::types::HyperClient, crate::types::Token, crate::util::HyperExt,
        async_trait::async_trait, hyper::Request, serde::Deserialize, serde::Serialize,
        std::collections::HashMap, std::path::Path,
    };
}
pub use authentication_manager::AuthenticationManager;
pub use error::Error;
pub use types::Token;

use hyper::Client;
use hyper_rustls::HttpsConnector;

/// Initialize GCP authentication
///
/// Returns `AuthenticationManager` which can be used to obtain tokens
pub async fn init(cred: Option<String>) -> Result<AuthenticationManager, Error> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);

    if let Some(credentials) = cred {
        return custom_service_account::CustomServiceAccount::new_from_cred(credentials)
            .await
            .map(|sa| AuthenticationManager {
                client,
                service_account: Box::new(sa),
            });
    }

    if let Ok(_) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        return custom_service_account::CustomServiceAccount::new()
            .await
            .map(|sa| AuthenticationManager {
                client,
                service_account: Box::new(sa),
            });
    }

    let default = default_service_account::DefaultServiceAccount::new(&client).await;
    if let Ok(service_account) = default {
        return Ok(AuthenticationManager {
            client: client.clone(),
            service_account: Box::new(service_account),
        });
    }
    let user = default_authorized_user::DefaultAuthorizedUser::new(&client).await;
    if let Ok(user_account) = user {
        return Ok(AuthenticationManager {
            client,
            service_account: Box::new(user_account),
        });
    }
    Err(Error::NoAuthMethod(
        Box::new(default.unwrap_err()),
        Box::new(user.unwrap_err()),
    ))
}
