//! Thin Tauri command handlers for Google login. All OAuth/keychain logic
//! lives in `downloadhub_core::auth`; this module just wires it to IPC.

use crate::state::AppState;
use downloadhub_core::auth::{self, LoginFlow, UserInfo};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

fn to_string_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Opens the system browser for the Google consent screen and waits for the
/// loopback redirect. Returns the signed-in user's profile on success.
#[tauri::command]
pub async fn auth_login<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<UserInfo, String> {
    let config = state.auth_config.as_ref().ok_or_else(|| {
        "Google OAuth is not configured. Set GOOGLE_OAUTH_CLIENT_ID and \
         GOOGLE_OAUTH_CLIENT_SECRET (see README) and restart the app."
            .to_string()
    })?;

    let flow = LoginFlow::begin(config).await.map_err(to_string_err)?;
    app.opener()
        .open_url(flow.authorize_url.to_string(), None::<String>)
        .map_err(to_string_err)?;

    let tokens = flow.complete().await.map_err(to_string_err)?;
    auth::store_tokens(&tokens).map_err(to_string_err)?;

    let info = auth::fetch_user_info(&tokens.access_token)
        .await
        .map_err(to_string_err)?;
    Ok(info)
}

#[tauri::command]
pub async fn auth_logout() -> Result<(), String> {
    auth::clear_tokens().map_err(to_string_err)
}

/// Returns the signed-in user's profile if a valid (or refreshable) session
/// exists, refreshing and re-persisting the token if it's near expiry.
///
/// Only a refresh failure (expired with no refresh token, revoked, etc.)
/// clears the stored session — that means the user genuinely needs to log
/// in again. A `fetch_user_info` failure is surfaced as an error instead of
/// clearing tokens, since it may just be a transient network issue and the
/// token itself could still be valid.
#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> Result<Option<UserInfo>, String> {
    let Some(tokens) = auth::load_tokens().map_err(to_string_err)? else {
        return Ok(None);
    };

    let tokens = match &state.auth_config {
        Some(config) => auth::ensure_fresh(config, tokens).await,
        None => Ok(tokens),
    };

    let tokens = match tokens {
        Ok(t) => t,
        Err(_) => {
            let _ = auth::clear_tokens();
            return Ok(None);
        }
    };

    let info = auth::fetch_user_info(&tokens.access_token)
        .await
        .map_err(to_string_err)?;
    Ok(Some(info))
}
