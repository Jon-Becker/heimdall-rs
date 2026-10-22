//! The `orcarouter` command group.
//!
//! Two authentication choices are exposed, and they are deliberately separate:
//!
//! * `heimdall config orcarouter_api_key <KEY>` stores a key the user already has.
//! * `heimdall orcarouter login` authorizes this machine with an OrcaRouter account
//!   (OAuth 2.0 + PKCE, loopback redirect) and stores the key that comes back.
//!
//! Both write the same ordinary `sk-orca-…` key into the configuration file this project already
//! uses. Neither prints it.

use std::sync::Arc;

use crate::args::{
    OrcaRouterAction, OrcaRouterArgs, OrcaRouterLoginArgs, OrcaRouterModelsArgs,
};
use eyre::{eyre, Result};
use heimdall_common::resources::orcarouter::{
    catalog::{CatalogStatus, ModelInfo},
    mask_key, CredentialSource, OrcaRouterBases, PkceAdapter, AUTH_TIMEOUT_DEFAULT,
    KEY_DASHBOARD_URL,
};
use heimdall_config::Configuration;
use tracing::info;

/// Dispatches an `orcarouter` subcommand.
pub(crate) async fn orcarouter(args: OrcaRouterArgs) -> Result<()> {
    match args.action {
        OrcaRouterAction::Login(args) => login(args).await,
        OrcaRouterAction::Status => status(),
        OrcaRouterAction::Logout => logout(),
        OrcaRouterAction::Models(args) => models(args).await,
    }
}

/// Runs the PKCE connect flow and persists the resulting key.
async fn login(args: OrcaRouterLoginArgs) -> Result<()> {
    let bases = OrcaRouterBases::with_overrides(
        Some(args.auth_base.as_str()),
        Some(args.api_base.as_str()),
    );

    let no_browser = args.no_browser;
    let opener = Arc::new(move |url: &str| {
        println!("\nAuthorize heimdall in your browser:\n\n  {url}\n");
        if no_browser {
            return;
        }
        if !open_browser(url) {
            println!("Could not open a browser automatically. Open the URL above by hand.\n");
        }
    });

    let timeout = if args.timeout == 0 { AUTH_TIMEOUT_DEFAULT } else {
        std::time::Duration::from_secs(args.timeout)
    };
    let auth_origin = bases.auth.clone();
    let adapter = PkceAdapter::new(bases, "Heimdall", opener, timeout);

    println!("Waiting for authorization on {auth_origin} (Ctrl-C to cancel)...");

    let credential = tokio::task::spawn_blocking(move || adapter.acquire())
        .await
        .map_err(|e| eyre!("login task failed: {e}"))??;

    let mut config = Configuration::load().map_err(|e| eyre!("failed to load configuration: {e}"))?;
    config
        .store_orcarouter_credential(&credential)
        .map_err(|e| eyre!("failed to save the OrcaRouter key: {e}"))?;

    println!(
        "Connected as {} (scope '{}'). Stored {} in the heimdall configuration.\nManage or revoke this key at {KEY_DASHBOARD_URL}",
        if credential.account.is_empty() { "an OrcaRouter account" } else { &credential.account },
        credential.scope,
        credential.masked_key(),
    );

    Ok(())
}

/// Prints the configured credential's provenance without revealing the key.
fn status() -> Result<()> {
    let config = Configuration::load().map_err(|e| eyre!("failed to load configuration: {e}"))?;

    if !config.has_orcarouter_key() {
        println!("OrcaRouter is not configured.");
        println!("  - paste a key:  heimdall config orcarouter_api_key <KEY>");
        println!("  - sign in:      heimdall orcarouter login");
        return Ok(());
    }

    let method = match config.orcarouter_auth_method.as_str() {
        "pkce" => "OrcaRouter - Auth (OAuth 2.0 + PKCE)",
        "api-key" | "" => "OrcaRouter - API (pasted key)",
        other => other,
    };

    println!("OrcaRouter is configured.");
    println!("  key:    {}", mask_key(&config.orcarouter_api_key));
    println!("  method: {method}");
    println!(
        "  model:  {}",
        if config.orcarouter_model.is_empty() {
            "auto (chosen from the live catalog)".to_string()
        } else {
            config.orcarouter_model.clone()
        }
    );
    println!("  revoke: {KEY_DASHBOARD_URL}");

    Ok(())
}

/// Removes the stored key.
fn logout() -> Result<()> {
    let mut config = Configuration::load().map_err(|e| eyre!("failed to load configuration: {e}"))?;
    if !config.has_orcarouter_key() {
        println!("OrcaRouter is not configured; nothing to remove.");
        return Ok(());
    }
    config
        .clear_orcarouter_credential()
        .map_err(|e| eyre!("failed to update configuration: {e}"))?;
    info!("removed the stored OrcaRouter key. Any key issued by `heimdall orcarouter login` can also be revoked at {KEY_DASHBOARD_URL}");
    Ok(())
}

/// Lists the live catalog for one capability.
async fn models(args: OrcaRouterModelsArgs) -> Result<()> {
    let config = Configuration::load().map_err(|e| eyre!("failed to load configuration: {e}"))?;
    if !config.has_orcarouter_key() {
        return Err(eyre!(
            "no OrcaRouter credential configured. run `heimdall orcarouter login` or set `orcarouter_api_key`."
        ));
    }

    let bases = OrcaRouterBases::with_overrides(None, Some(args.api_base.as_str()));
    let capability = args.capability.into();
    let api_key = config.orcarouter_api_key.clone();

    let (models, status) = tokio::task::spawn_blocking(move || {
        heimdall_common::resources::orcarouter::catalog::resolve_catalog(
            &bases, &api_key, capability,
        )
    })
    .await
    .map_err(|e| eyre!("catalog task failed: {e}"))?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&model_rows(&models, status))?);
        return Ok(());
    }

    if status.is_degraded() {
        println!("Using the {}: {}", status.describe(), fallback_note(status));
    } else {
        println!("{}: {} model(s)", status.describe(), models.len());
    }

    for model in &models {
        let context = model
            .context_length
            .map(|length| format!("  {length:>10} ctx"))
            .unwrap_or_else(|| "               ".to_string());
        let modalities = if model.input_modalities.is_empty() {
            String::new()
        } else {
            format!("  [{}]", model.input_modalities.join(","))
        };
        println!("  {}{context}{modalities}", model.id);
    }

    Ok(())
}

fn fallback_note(status: CatalogStatus) -> &'static str {
    match status {
        CatalogStatus::Live => "live catalog",
        CatalogStatus::SeedFallback => "verified offline seed",
        CatalogStatus::LastKnownGood => "last known good catalog",
    }
}

fn model_rows(models: &[ModelInfo], status: CatalogStatus) -> serde_json::Value {
    serde_json::json!({
        "catalog_status": format!("{status:?}").to_lowercase(),
        "model_count": models.len(),
        "models": models,
    })
}

/// Opens a URL in the platform browser. Returns whether a launcher was found.
fn open_browser(url: &str) -> bool {
    let (program, args) = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", "", url])
    } else {
        ("xdg-open", vec![url])
    };

    std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}
