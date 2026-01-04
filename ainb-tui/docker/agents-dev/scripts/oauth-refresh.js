#!/usr/bin/env node
// ABOUTME: OAuth token refresh script for Claude Code authentication
// Refreshes expired access tokens using refresh token

const https = require('https');
const fs = require('fs').promises;
const path = require('path');

// OAuth constants
const OAUTH_CONSTANTS = {
  OAUTH_TOKEN_URL: 'https://console.anthropic.com/v1/oauth/token',
  CLIENT_ID: '9d1c250a-e61b-44d9-88ed-5944d1962f5e',
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
 * Loads existing credentials
 * @returns {Promise<Object|null>} Credentials object or null if not found
 */
async function loadCredentials() {
  try {
    const credentialsData = await fs.readFile(OAUTH_CONSTANTS.CREDENTIALS_PATH, 'utf-8');
    return JSON.parse(credentialsData);
  } catch (error) {
    if (process.env.DEBUG) {
      console.error('[DEBUG] Failed to load credentials:', error.message);
    }
    return null;
  }
}

/**
 * Checks if token needs refresh
 * @param {Object} credentials - Credentials object
 * @returns {boolean} True if token needs refresh
 */
function needsRefresh(credentials) {
  if (!credentials?.claudeAiOauth?.expiresAt) {
    return true;
  }

  const now = Date.now();
  const expiresAt = credentials.claudeAiOauth.expiresAt;

  // Refresh if token expires in less than 30 minutes
  const bufferTime = 30 * 60 * 1000; // 30 minutes

  if (process.env.DEBUG) {
    const timeUntilExpiry = (expiresAt - now) / 1000 / 60; // minutes
    console.error(`[DEBUG] Token expires in ${timeUntilExpiry.toFixed(1)} minutes`);
  }

  return now >= (expiresAt - bufferTime);
}

/**
 * Refreshes OAuth tokens using refresh token
 * @param {string} refreshToken - Refresh token
 * @returns {Promise<Object|null>} New tokens or null on error
 */
async function refreshTokens(refreshToken) {
  const tokenRequestBody = {
    grant_type: 'refresh_token',
    refresh_token: refreshToken,
    client_id: OAUTH_CONSTANTS.CLIENT_ID
  };

  if (process.env.DEBUG) {
    console.error('[DEBUG] Refreshing tokens...');
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

      res.on('data', (chunk) => {
        data += chunk;
      });

      res.on('end', () => {
        try {
          const response = JSON.parse(data);

          if (res.statusCode === 200) {
            if (process.env.DEBUG) {
              console.error('[DEBUG] Token refresh successful');
            }
            resolve(response);
          } else {
            console.error('Token refresh failed:', res.statusCode);
            if (process.env.DEBUG) {
              console.error('[DEBUG] Response:', JSON.stringify(response, null, 2));
            }
            resolve(null);
          }
        } catch (error) {
          console.error('Failed to parse refresh response:', error);
          resolve(null);
        }
      });
    });

    req.on('error', (error) => {
      console.error('Token refresh request failed:', error);
      resolve(null);
    });

    req.write(postData);
    req.end();
  });
}

/**
 * Saves updated credentials
 * @param {Object} credentials - Existing credentials
 * @param {Object} tokens - New tokens from refresh
 */
async function saveUpdatedCredentials(credentials, tokens) {
  // Update the OAuth section with new tokens
  credentials.claudeAiOauth = {
    ...credentials.claudeAiOauth,
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token || credentials.claudeAiOauth.refreshToken,
    expiresAt: Date.now() + (tokens.expires_in * 1000),
    lastRefreshed: new Date().toISOString()
  };

  // Save to file
  await fs.writeFile(
    OAUTH_CONSTANTS.CREDENTIALS_PATH,
    JSON.stringify(credentials, null, 2),
    'utf-8'
  );

  if (process.env.DEBUG) {
    console.error('[DEBUG] Credentials updated successfully');
  }
}

/**
 * Main refresh function with retry logic
 * @param {boolean} force - Force refresh even if token is still valid
 * @param {number} maxRetries - Maximum number of retry attempts
 * @returns {Promise<boolean>} Success status
 */
async function performRefresh(force = false, maxRetries = 3) {
  try {
    // Load existing credentials
    const credentials = await loadCredentials();
    if (!credentials) {
      console.error('No credentials found to refresh');
      return false;
    }

    // Check if refresh is needed (unless forced)
    if (!force && !needsRefresh(credentials)) {
      console.log('Token is still valid, no refresh needed');
      return true;
    }

    if (force) {
      console.log('Forcing token refresh as requested');
    }

    // Get refresh token
    const refreshToken = credentials.claudeAiOauth?.refreshToken;
    if (!refreshToken) {
      console.error('No refresh token available');
      return false;
    }

    // Refresh the tokens with retry logic
    let newTokens = null;
    let lastError = null;

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      if (process.env.DEBUG) {
        console.error(`[DEBUG] Refresh attempt ${attempt} of ${maxRetries}`);
      }

      newTokens = await refreshTokens(refreshToken);

      if (newTokens) {
        break; // Success, exit retry loop
      }

      lastError = `Attempt ${attempt} failed`;

      if (attempt < maxRetries) {
        // Wait before retrying (exponential backoff)
        const delay = Math.min(1000 * Math.pow(2, attempt - 1), 5000);
        if (process.env.DEBUG) {
          console.error(`[DEBUG] Waiting ${delay}ms before retry`);
        }
        await new Promise(resolve => setTimeout(resolve, delay));
      }
    }

    if (!newTokens) {
      console.error(`Failed to refresh tokens after ${maxRetries} attempts`);
      return false;
    }

    // Save updated credentials
    await saveUpdatedCredentials(credentials, newTokens);

    console.log('Token refreshed successfully');
    console.log(`New token expires at: ${new Date(credentials.claudeAiOauth.expiresAt).toISOString()}`);

    return true;
  } catch (error) {
    console.error('Error during token refresh:', error.message);
    return false;
  }
}

// CLI interface
if (require.main === module) {
  if (process.argv.includes('--help') || process.argv.includes('-h')) {
    console.log('Usage: oauth-refresh.js [--force]');
    console.log('  --force    Force refresh even if token is still valid');
    console.log('  --help     Show this help message');
    console.log('');
    console.log('Refreshes OAuth access token using refresh token');
    process.exit(0);
  }

  const forceRefresh = process.argv.includes('--force');

  performRefresh(forceRefresh).then((success) => {
    process.exit(success ? 0 : 1);
  });
}

// Export for use as module
module.exports = {
  loadCredentials,
  needsRefresh,
  refreshTokens,
  saveUpdatedCredentials,
  performRefresh
};
