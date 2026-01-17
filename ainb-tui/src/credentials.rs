// ABOUTME: Secure credential storage using system keychain
// Uses keyring crate for cross-platform support (macOS Keychain, Linux Secret Service)

use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "agents-in-a-box";

/// Credential keys for different secrets
pub enum CredentialKey {
    AnthropicApiKey,
    OpenAiApiKey,
    GeminiApiKey,
    GithubPat,
}

impl CredentialKey {
    fn as_str(&self) -> &'static str {
        match self {
            CredentialKey::AnthropicApiKey => "anthropic_api_key",
            CredentialKey::OpenAiApiKey => "openai_api_key",
            CredentialKey::GeminiApiKey => "gemini_api_key",
            CredentialKey::GithubPat => "github_pat",
        }
    }
}

/// Store a credential in the system keychain
pub fn store_credential(key: CredentialKey, value: &str) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, key.as_str())
        .context("Failed to create keyring entry")?;

    entry
        .set_password(value)
        .context("Failed to store credential in keychain")?;

    tracing::info!("Stored credential: {}", key.as_str());
    Ok(())
}

/// Retrieve a credential from the system keychain
pub fn get_credential(key: CredentialKey) -> Result<Option<String>> {
    let entry = Entry::new(SERVICE_NAME, key.as_str())
        .context("Failed to create keyring entry")?;

    match entry.get_password() {
        Ok(password) => {
            tracing::debug!("Retrieved credential: {}", key.as_str());
            Ok(Some(password))
        }
        Err(keyring::Error::NoEntry) => {
            tracing::debug!("No credential found for: {}", key.as_str());
            Ok(None)
        }
        Err(e) => {
            tracing::warn!("Failed to retrieve credential {}: {}", key.as_str(), e);
            Err(anyhow::anyhow!("Failed to retrieve credential: {}", e))
        }
    }
}

/// Delete a credential from the system keychain
pub fn delete_credential(key: CredentialKey) -> Result<()> {
    let entry = Entry::new(SERVICE_NAME, key.as_str())
        .context("Failed to create keyring entry")?;

    match entry.delete_credential() {
        Ok(()) => {
            tracing::info!("Deleted credential: {}", key.as_str());
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            // Already doesn't exist, that's fine
            Ok(())
        }
        Err(e) => {
            Err(anyhow::anyhow!("Failed to delete credential: {}", e))
        }
    }
}

/// Check if a credential exists in the system keychain
pub fn has_credential(key: CredentialKey) -> bool {
    get_credential(key).map(|opt| opt.is_some()).unwrap_or(false)
}

/// Store Anthropic API key
pub fn store_anthropic_api_key(api_key: &str) -> Result<()> {
    // Basic validation
    if api_key.is_empty() {
        return Err(anyhow::anyhow!("API key cannot be empty"));
    }
    if !api_key.starts_with("sk-ant-") {
        tracing::warn!("API key doesn't start with 'sk-ant-' - may be invalid");
    }

    store_credential(CredentialKey::AnthropicApiKey, api_key)
}

/// Get Anthropic API key
pub fn get_anthropic_api_key() -> Result<Option<String>> {
    get_credential(CredentialKey::AnthropicApiKey)
}

/// Check if Anthropic API key is configured
pub fn has_anthropic_api_key() -> bool {
    has_credential(CredentialKey::AnthropicApiKey)
}

/// Delete Anthropic API key
pub fn delete_anthropic_api_key() -> Result<()> {
    delete_credential(CredentialKey::AnthropicApiKey)
}

/// Get masked display of API key (for UI)
pub fn get_anthropic_api_key_masked() -> String {
    match get_anthropic_api_key() {
        Ok(Some(key)) => {
            if key.len() > 12 {
                format!("{}••••••••", &key[..12])
            } else {
                "••••••••".to_string()
            }
        }
        _ => "Not configured".to_string(),
    }
}

// =============================================================================
// OpenAI API Key Functions
// =============================================================================

/// Store OpenAI API key
pub fn store_openai_api_key(api_key: &str) -> Result<()> {
    if api_key.is_empty() {
        return Err(anyhow::anyhow!("API key cannot be empty"));
    }
    if !api_key.starts_with("sk-") {
        tracing::warn!("API key doesn't start with 'sk-' - may be invalid");
    }
    store_credential(CredentialKey::OpenAiApiKey, api_key)
}

/// Get OpenAI API key
pub fn get_openai_api_key() -> Result<Option<String>> {
    get_credential(CredentialKey::OpenAiApiKey)
}

/// Check if OpenAI API key is configured
pub fn has_openai_api_key() -> bool {
    has_credential(CredentialKey::OpenAiApiKey)
}

/// Delete OpenAI API key
pub fn delete_openai_api_key() -> Result<()> {
    delete_credential(CredentialKey::OpenAiApiKey)
}

/// Get masked display of OpenAI API key (for UI)
pub fn get_openai_api_key_masked() -> String {
    match get_openai_api_key() {
        Ok(Some(key)) => {
            if key.len() > 8 {
                format!("{}••••••••", &key[..8])
            } else {
                "••••••••".to_string()
            }
        }
        _ => "Not configured".to_string(),
    }
}

// =============================================================================
// Gemini API Key Functions
// =============================================================================

/// Store Gemini API key
pub fn store_gemini_api_key(api_key: &str) -> Result<()> {
    if api_key.is_empty() {
        return Err(anyhow::anyhow!("API key cannot be empty"));
    }
    // Gemini keys don't have a strict prefix requirement
    store_credential(CredentialKey::GeminiApiKey, api_key)
}

/// Get Gemini API key
pub fn get_gemini_api_key() -> Result<Option<String>> {
    get_credential(CredentialKey::GeminiApiKey)
}

/// Check if Gemini API key is configured
pub fn has_gemini_api_key() -> bool {
    has_credential(CredentialKey::GeminiApiKey)
}

/// Delete Gemini API key
pub fn delete_gemini_api_key() -> Result<()> {
    delete_credential(CredentialKey::GeminiApiKey)
}

/// Get masked display of Gemini API key (for UI)
pub fn get_gemini_api_key_masked() -> String {
    match get_gemini_api_key() {
        Ok(Some(key)) => {
            if key.len() > 8 {
                format!("{}••••••••", &key[..8])
            } else {
                "••••••••".to_string()
            }
        }
        _ => "Not configured".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests interact with the real system keychain
    // They should be run manually, not in CI

    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_store_and_retrieve() {
        let test_key = "sk-ant-test-key-12345";

        // Store
        store_anthropic_api_key(test_key).expect("Failed to store");

        // Retrieve
        let retrieved = get_anthropic_api_key().expect("Failed to get");
        assert_eq!(retrieved, Some(test_key.to_string()));

        // Check exists
        assert!(has_anthropic_api_key());

        // Delete
        delete_anthropic_api_key().expect("Failed to delete");

        // Verify deleted
        assert!(!has_anthropic_api_key());
    }
}
