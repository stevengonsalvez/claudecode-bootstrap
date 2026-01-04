#!/usr/bin/env node
// ABOUTME: OAuth login initiation script for Claude Code authentication
// Generates secure OAuth login URLs with PKCE for Claude AI authorization

const crypto = require('crypto');
const fs = require('fs').promises;
const path = require('path');

// OAuth constants from claude_oauth Ruby implementation
const OAUTH_CONSTANTS = {
  OAUTH_AUTHORIZE_URL: 'https://claude.ai/oauth/authorize',
  CLIENT_ID: '9d1c250a-e61b-44d9-88ed-5944d1962f5e',
  REDIRECT_URI: 'https://console.anthropic.com/oauth/code/callback',
  STATE_FILE: path.join(process.env.HOME || '', '.claude', '.claude_oauth_state.json'),
  SCOPES: 'org:create_api_key user:profile user:inference',
  STATE_EXPIRY_MINUTES: 10
};

/**
 * Generates a secure random string for OAuth state parameter
 * @returns {string} 64-character hex string
 */
function generateState() {
  return crypto.randomBytes(32).toString('hex');
}

/**
 * Generates PKCE code verifier and challenge
 * @returns {Object} Object containing codeVerifier and codeChallenge
 */
function generatePKCE() {
  // Generate random code verifier (32 bytes)
  const codeVerifier = crypto.randomBytes(32).toString('base64url');

  // Create code challenge (SHA256 hash of verifier, base64url encoded)
  const codeChallenge = crypto
    .createHash('sha256')
    .update(codeVerifier)
    .digest('base64url');

  return {
    codeVerifier,
    codeChallenge
  };
}

/**
 * Saves OAuth state to file for later verification
 * @param {string} state - OAuth state parameter
 * @param {string} codeVerifier - PKCE code verifier
 */
async function saveState(state, codeVerifier) {
  // Calculate expiration time
  const expiresAt = new Date();
  expiresAt.setMinutes(expiresAt.getMinutes() + OAUTH_CONSTANTS.STATE_EXPIRY_MINUTES);

  const stateData = {
    state,
    code_verifier: codeVerifier,
    expires_at: expiresAt.toISOString()
  };

  // Ensure directory exists
  const stateDir = path.dirname(OAUTH_CONSTANTS.STATE_FILE);
  await fs.mkdir(stateDir, { recursive: true });

  // Write state file
  await fs.writeFile(OAUTH_CONSTANTS.STATE_FILE, JSON.stringify(stateData, null, 2));
}

/**
 * Generates complete OAuth authorization URL
 * @returns {Promise<string>} OAuth authorization URL
 */
async function generateLoginUrl() {
  // Generate state and PKCE
  const state = generateState();
  const { codeVerifier, codeChallenge } = generatePKCE();

  // Save state for later verification
  await saveState(state, codeVerifier);

  // Build OAuth URL with parameters
  const params = new URLSearchParams({
    response_type: 'code',
    client_id: OAUTH_CONSTANTS.CLIENT_ID,
    redirect_uri: OAUTH_CONSTANTS.REDIRECT_URI,
    scope: OAUTH_CONSTANTS.SCOPES,
    state: state,
    code_challenge: codeChallenge,
    code_challenge_method: 'S256'
  });

  return `${OAUTH_CONSTANTS.OAUTH_AUTHORIZE_URL}?${params.toString()}`;
}

/**
 * Main function to initiate OAuth login process
 */
async function startOAuthLogin() {
  const loginUrl = await generateLoginUrl();
  console.log(loginUrl);
}

module.exports = {
  OAUTH_CONSTANTS,
  generateState,
  generatePKCE,
  saveState,
  generateLoginUrl,
  startOAuthLogin
};

// CLI execution when run directly
if (require.main === module) {
  const args = process.argv.slice(2);

  if (args.includes('--help') || args.includes('-h')) {
    console.log('Usage: node oauth-start.js');
    console.log('  Generates an OAuth login URL for Claude Code authentication');
    console.log('  --help, -h     Show this help message');
    process.exit(0);
  }

  startOAuthLogin()
    .then(() => process.exit(0))
    .catch(error => {
      console.error('OAuth start failed:', error.message);
      process.exit(1);
    });
}
