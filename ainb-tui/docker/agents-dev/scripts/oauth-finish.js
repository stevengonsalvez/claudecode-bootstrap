#!/usr/bin/env node
// ABOUTME: OAuth login completion script for Claude Code authentication
// Exchanges authorization code for access tokens using PKCE verification

const https = require('https');
const fs = require('fs').promises;
const path = require('path');

// OAuth constants from claude_oauth Ruby implementation
const OAUTH_CONSTANTS = {
  OAUTH_TOKEN_URL: 'https://console.anthropic.com/v1/oauth/token',
  CLIENT_ID: '9d1c250a-e61b-44d9-88ed-5944d1962f5e',
  REDIRECT_URI: 'https://console.anthropic.com/oauth/code/callback',
  STATE_FILE: path.join(process.env.HOME || '', '.claude', '.claude_oauth_state.json'),
  CREDENTIALS_PATH: path.join(process.env.HOME || '', '.claude', '.credentials.json'),
  REQUEST_HEADERS: {
    'Content-Type': 'application/json',
    'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36',
    'Accept': 'application/json, text/plain, */*',
    'Accept-Language': 'en-US,en;q=0.9',
    'Referer': 'https://claude.ai/',
    'Origin': 'https://claude.ai'
  }
};

/**
 * Cleans authorization code from URL fragments and parameters
 * @param {string} authorizationCode - Raw authorization code from OAuth callback
 * @returns {string} Cleaned authorization code
 */
function cleanAuthorizationCode(authorizationCode) {
  if (!authorizationCode || typeof authorizationCode !== 'string') {
    return authorizationCode;
  }

  if (process.env.DEBUG) {
    console.error('[DEBUG] Raw authorization code:', authorizationCode);
  }

  // If it looks like a URL, try to parse it and get the 'code' parameter
  if (authorizationCode.includes('?') || authorizationCode.includes('://')) {
    try {
      // Use a dummy base URL if the provided string is just params
      const url = new URL(authorizationCode, 'https://dummy.base');
      if (url.searchParams.has('code')) {
        const code = url.searchParams.get('code');
        if (process.env.DEBUG) {
          console.error('[DEBUG] Extracted code from URL:', code);
        }
        return code;
      }
    } catch (e) {
      // Not a valid URL, fall through to simple splitting
    }
  }

  // Fallback for plain codes or malformed URLs
  const cleaned = authorizationCode.split('#')[0].split('?')[0].split('&')[0];
  if (process.env.DEBUG) {
    console.error('[DEBUG] Final cleaned code:', cleaned);
  }

  return cleaned;
}

/**
 * Verifies OAuth state is valid and not expired
 * @returns {Promise<Object|null>} State data if valid, null if invalid or expired
 */
async function verifyState() {
  try {
    const stateData = await fs.readFile(OAUTH_CONSTANTS.STATE_FILE, 'utf-8');
    const state = JSON.parse(stateData);

    if (process.env.DEBUG) {
      console.error('[DEBUG] Loaded state from file:', {
        state: state.state,
        has_code_verifier: !!state.code_verifier,
        expires_at: state.expires_at
      });
    }

    // Check if state has expired
    const expiresAt = new Date(state.expires_at);
    const now = new Date();

    if (now >= expiresAt) {
      if (process.env.DEBUG) {
        console.error('[DEBUG] State has expired');
      }
      return null; // Expired
    }

    return state;
  } catch (error) {
    if (process.env.DEBUG) {
      console.error('[DEBUG] Failed to load state:', error.message);
    }
    // File doesn't exist or is invalid JSON
    return null;
  }
}

/**
 * Exchanges authorization code for OAuth tokens
 * @param {string} authorizationCode - Authorization code from OAuth callback
 * @returns {Promise<Object|null>} Token response or null if failed
 */
async function exchangeCodeForTokens(authorizationCode) {
  // Validate authorization code
  if (!authorizationCode || authorizationCode.trim() === '') {
    throw new Error('Authorization code is required');
  }

  // Clean the authorization code
  const cleanedCode = cleanAuthorizationCode(authorizationCode);

  // Verify state and get code_verifier
  const state = await verifyState();
  if (!state) {
    return null; // State is invalid or expired
  }

  const tokenRequestBody = {
    grant_type: 'authorization_code',
    client_id: OAUTH_CONSTANTS.CLIENT_ID,
    code: cleanedCode,
    redirect_uri: OAUTH_CONSTANTS.REDIRECT_URI,
    code_verifier: state.code_verifier,
    state: state.state
  };

  // Log in debug mode if enabled
  if (process.env.DEBUG) {
    console.error('[DEBUG] Token request body:', JSON.stringify(tokenRequestBody, null, 2));
    console.error('[DEBUG] Request URL:', OAUTH_CONSTANTS.OAUTH_TOKEN_URL);
  }

  return new Promise((resolve, reject) => {
    const url = new URL(OAUTH_CONSTANTS.OAUTH_TOKEN_URL);
    const postData = JSON.stringify(tokenRequestBody);

    const options = {
      hostname: url.hostname,
      port: url.port || 443,
      path: url.pathname,
      method: 'POST',
      headers: {
        ...OAUTH_CONSTANTS.REQUEST_HEADERS,
        'Content-Length': Buffer.byteLength(postData)
      }
    };

    const req = https.request(options, (res) => {
      let data = '';

      if (process.env.DEBUG) {
        console.error('[DEBUG] Response status:', res.statusCode);
        console.error('[DEBUG] Response headers:', res.headers);
      }

      res.on('data', (chunk) => {
        data += chunk;
      });

      res.on('end', () => {
        if (process.env.DEBUG) {
          console.error('[DEBUG] Raw response data:', data);
        }
        try {
          const response = JSON.parse(data);

          if (res.statusCode === 200) {
            resolve(response);
          } else {
            console.error('OAuth token exchange failed:');
            console.error('Status code:', res.statusCode);
            console.error('Response:', JSON.stringify(response, null, 2));
            resolve(null);
          }
        } catch (error) {
          console.error('Failed to parse token response:', error);
          resolve(null);
        }
      });
    });

    req.on('error', (error) => {
      console.error('OAuth token request failed:', error);
      resolve(null);
    });

    req.write(postData);
    req.end();
  });
}

/**
 * Saves OAuth credentials to file
 * @param {Object} tokens - Token response from OAuth exchange
 */
async function saveCredentials(tokens) {
  if (!tokens) {
    throw new Error('Tokens are required');
  }

  // Create credentials object in the format expected by Claude CLI
  const credentials = {
    claudeAiOauth: {
      accessToken: tokens.access_token,
      refreshToken: tokens.refresh_token,
      expiresAt: Date.now() + (tokens.expires_in * 1000),
      scopes: tokens.scope ? tokens.scope.split(' ') : ['user:inference', 'user:profile'],
      isMax: true
    }
  };

  // Ensure the directory exists
  const credentialsDir = path.dirname(OAUTH_CONSTANTS.CREDENTIALS_PATH);
  try {
    await fs.mkdir(credentialsDir, { recursive: true });
  } catch (error) {
    // Directory might already exist, ignore error
  }

  // Save credentials to file
  await fs.writeFile(
    OAUTH_CONSTANTS.CREDENTIALS_PATH,
    JSON.stringify(credentials, null, 2),
    'utf-8'
  );

  // Also create a minimal .claude.json file for TUI validation
  const claudeJsonPath = path.join(credentialsDir, '.claude.json');
  const claudeJson = {
    installMethod: "claude-in-a-box",
    autoUpdates: false,
    hasCompletedOnboarding: true,
    hasTrustDialogAccepted: true,
    firstStartTime: new Date().toISOString()
  };

  await fs.writeFile(claudeJsonPath, JSON.stringify(claudeJson, null, 2), 'utf-8');
  if (process.env.DEBUG) {
    console.error('[DEBUG] Created .claude.json for TUI validation at:', claudeJsonPath);
  }
}

/**
 * Cleans up temporary OAuth state file
 */
async function cleanupState() {
  try {
    await fs.unlink(OAUTH_CONSTANTS.STATE_FILE);
  } catch (error) {
    // File might not exist, which is fine for cleanup
    if (error.code !== 'ENOENT') {
      throw error;
    }
  }
}

/**
 * Completes OAuth login process
 * @param {string} authorizationCode - Authorization code from OAuth callback
 * @returns {Promise<boolean>} True if login successful
 */
async function completeLogin(authorizationCode) {
  try {
    console.log('Starting OAuth login completion...');

    // Step 1: Exchange authorization code for tokens
    const tokens = await exchangeCodeForTokens(authorizationCode);
    if (!tokens) {
      console.log('Failed to exchange authorization code for tokens');
      return false;
    }

    console.log('Successfully received OAuth tokens');

    // Step 2: Save credentials to file
    await saveCredentials(tokens);
    console.log('OAuth credentials saved successfully');

    // Step 3: Clean up state file
    await cleanupState();
    console.log('OAuth state cleaned up');

    console.log('OAuth login completed successfully!');
    return true;

  } catch (error) {
    console.error('OAuth login completion failed:', error.message);

    // Try to clean up state file even if other steps failed
    try {
      await cleanupState();
    } catch (cleanupError) {
      // Ignore cleanup errors
    }

    return false;
  }
}

module.exports = {
  OAUTH_CONSTANTS,
  cleanAuthorizationCode,
  verifyState,
  exchangeCodeForTokens,
  saveCredentials,
  cleanupState,
  completeLogin
};

// CLI execution when run directly
if (require.main === module) {
  const args = process.argv.slice(2);

  if (args.includes('--help') || args.includes('-h') || args.length === 0) {
    console.log('Usage: node oauth-finish.js <authorization_code>');
    console.log('  Completes OAuth login WITHOUT creating API key');
    console.log('  authorization_code: The code received from the OAuth callback');
    console.log('  --help, -h        Show this help message');
    process.exit(args.length === 0 ? 1 : 0);
  }

  const authorizationCode = args[0];

  completeLogin(authorizationCode)
    .then(success => {
      process.exit(success ? 0 : 1);
    })
    .catch(error => {
      console.error('OAuth completion failed:', error.message);
      process.exit(1);
    });
}
