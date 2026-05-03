//! Solid pod publishing — integration instructions.
//!
//! The implementation of `cmd_publish_solid` lives in `publish.rs` alongside
//! the other publish targets (Download, Github, Openshift).
//!
//! ## Wiring into main.rs
//!
//! To wire this into the CLI, the agent that owns `main.rs` needs to:
//!
//! 1. Add a `Solid` variant to the `PublishTarget` enum:
//!
//! ```ignore
//! /// Publish the built site to a Solid pod
//! Solid {
//!     /// Solid pod URL (e.g., https://paa.pub/ldary/)
//!     #[arg(long)]
//!     pod: String,
//!     /// Bearer token for authentication (or set SOLID_TOKEN env var)
//!     #[arg(long)]
//!     token: Option<String>,
//! },
//! ```
//!
//! 2. Add the match arm in `main()`:
//!
//! ```ignore
//! PublishTarget::Solid { pod, token } => {
//!     cmd_publish_solid(&path, &pod, token.as_deref(), v).await
//! }
//! ```
//!
//! 3. Add `cmd_publish_solid` to the import from `publish`:
//!
//! ```ignore
//! use publish::{cmd_publish_download, cmd_publish_github, cmd_publish_openshift, cmd_publish_solid};
//! ```
