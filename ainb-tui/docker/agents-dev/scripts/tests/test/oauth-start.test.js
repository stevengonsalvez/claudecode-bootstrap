// ABOUTME: Test suite for OAuth login initiation functionality
// Tests OAuth URL generation, state management, and PKCE implementation

const { describe, it, before, after } = require('node:test');
const assert = require('node:assert');
const path = require('path');
const fs = require('fs').promises;

// Import the module under test
const oauthStart = require('../../oauth-start.js');

describe('OAuth Start', () => {
  describe('Constants', () => {
    it('should have correct OAuth constants', () => {
      assert.strictEqual(oauthStart.OAUTH_CONSTANTS.OAUTH_AUTHORIZE_URL, 'https://claude.ai/oauth/authorize');
      assert.strictEqual(oauthStart.OAUTH_CONSTANTS.CLIENT_ID, '9d1c250a-e61b-44d9-88ed-5944d1962f5e');
      assert.strictEqual(oauthStart.OAUTH_CONSTANTS.REDIRECT_URI, 'https://console.anthropic.com/oauth/code/callback');
      assert.strictEqual(oauthStart.OAUTH_CONSTANTS.SCOPES, 'org:create_api_key user:profile user:inference');
      assert.strictEqual(oauthStart.OAUTH_CONSTANTS.STATE_EXPIRY_MINUTES, 10);
      assert.ok(oauthStart.OAUTH_CONSTANTS.STATE_FILE.includes('.claude_oauth_state.json'));
    });
  });

  describe('generateState', () => {
    it('should generate a secure random state', () => {
      const state = oauthStart.generateState();

      // Should be a string
      assert.strictEqual(typeof state, 'string');

      // Should be exactly 64 characters (32 bytes → 64 hex chars)
      assert.strictEqual(state.length, 64);

      // Should only contain hexadecimal characters
      assert.match(state, /^[0-9a-f]+$/);

      // Should generate different values each time
      const state2 = oauthStart.generateState();
      assert.notStrictEqual(state, state2);
    });
  });

  describe('generatePKCE', () => {
    it('should generate PKCE code verifier and challenge', () => {
      const pkce = oauthStart.generatePKCE();

      // Should return an object with both verifier and challenge
      assert.strictEqual(typeof pkce, 'object');
      assert.ok(pkce.codeVerifier);
      assert.ok(pkce.codeChallenge);

      // Code verifier should be base64url encoded (43 chars from 32 bytes)
      assert.strictEqual(typeof pkce.codeVerifier, 'string');
      assert.strictEqual(pkce.codeVerifier.length, 43);
      assert.match(pkce.codeVerifier, /^[A-Za-z0-9_-]+$/);

      // Code challenge should be base64url encoded SHA256 (43 chars from 32 bytes)
      assert.strictEqual(typeof pkce.codeChallenge, 'string');
      assert.strictEqual(pkce.codeChallenge.length, 43);
      assert.match(pkce.codeChallenge, /^[A-Za-z0-9_-]+$/);

      // Should generate different values each time
      const pkce2 = oauthStart.generatePKCE();
      assert.notStrictEqual(pkce.codeVerifier, pkce2.codeVerifier);
      assert.notStrictEqual(pkce.codeChallenge, pkce2.codeChallenge);
    });
  });

  describe('saveState', () => {
    const testStateFile = '/tmp/test-oauth-state.json';

    after(async () => {
      // Clean up test file
      try {
        await fs.unlink(testStateFile);
      } catch (err) {
        // File might not exist, that's fine
      }
    });

    it('should save state to file with expiration', async () => {
      const testState = 'test-state-12345';
      const testCodeVerifier = 'test-code-verifier-abcdef';

      // Override the STATE_FILE constant for testing
      const originalStateFile = oauthStart.OAUTH_CONSTANTS.STATE_FILE;
      oauthStart.OAUTH_CONSTANTS.STATE_FILE = testStateFile;

      try {
        await oauthStart.saveState(testState, testCodeVerifier);

        // Verify file was created and contains correct data
        const fileContent = await fs.readFile(testStateFile, 'utf8');
        const savedData = JSON.parse(fileContent);

        assert.strictEqual(savedData.state, testState);
        assert.strictEqual(savedData.code_verifier, testCodeVerifier);
        assert.ok(savedData.expires_at);

        // Verify expiration is set correctly (should be ~10 minutes from now)
        const expiresAt = new Date(savedData.expires_at);
        const now = new Date();
        const timeDiff = (expiresAt - now) / (1000 * 60); // Convert to minutes

        assert.ok(timeDiff > 9 && timeDiff < 11); // Should be around 10 minutes

      } finally {
        // Restore original STATE_FILE
        oauthStart.OAUTH_CONSTANTS.STATE_FILE = originalStateFile;
      }
    });
  });

  describe('generateLoginUrl', () => {
    it('should generate complete OAuth authorization URL', async () => {
      const url = await oauthStart.generateLoginUrl();

      // Should be a valid URL
      assert.ok(url);
      assert.strictEqual(typeof url, 'string');

      const parsedUrl = new URL(url);

      // Should use the correct base URL
      assert.strictEqual(parsedUrl.origin + parsedUrl.pathname, oauthStart.OAUTH_CONSTANTS.OAUTH_AUTHORIZE_URL);

      // Should have all required query parameters
      const params = parsedUrl.searchParams;
      assert.strictEqual(params.get('response_type'), 'code');
      assert.strictEqual(params.get('client_id'), oauthStart.OAUTH_CONSTANTS.CLIENT_ID);
      assert.strictEqual(params.get('redirect_uri'), oauthStart.OAUTH_CONSTANTS.REDIRECT_URI);
      assert.strictEqual(params.get('scope'), oauthStart.OAUTH_CONSTANTS.SCOPES);
      assert.strictEqual(params.get('code_challenge_method'), 'S256');

      // Should have state and code_challenge parameters
      assert.ok(params.get('state'));
      assert.ok(params.get('code_challenge'));

      // State should be 64 hex characters
      const state = params.get('state');
      assert.strictEqual(state.length, 64);
      assert.match(state, /^[0-9a-f]+$/);

      // Code challenge should be 43 base64url characters
      const codeChallenge = params.get('code_challenge');
      assert.strictEqual(codeChallenge.length, 43);
      assert.match(codeChallenge, /^[A-Za-z0-9_-]+$/);
    });
  });

  describe('startOAuthLogin', () => {
    // Mock console.log to capture output
    let originalConsoleLog;
    let logOutput;

    before(() => {
      originalConsoleLog = console.log;
      logOutput = [];
      console.log = (...args) => {
        logOutput.push(args.join(' '));
      };
    });

    after(() => {
      console.log = originalConsoleLog;
    });

    it('should orchestrate the OAuth login initiation', async () => {
      logOutput = []; // Reset log output

      await oauthStart.startOAuthLogin();

      // Should have logged the OAuth URL
      assert.strictEqual(logOutput.length, 1);
      const outputUrl = logOutput[0];

      // Should be a valid OAuth URL
      assert.ok(outputUrl.startsWith('https://claude.ai/oauth/authorize?'));

      // Parse and verify the URL structure
      const parsedUrl = new URL(outputUrl);
      const params = parsedUrl.searchParams;

      // Should have all required parameters
      assert.strictEqual(params.get('response_type'), 'code');
      assert.strictEqual(params.get('client_id'), oauthStart.OAUTH_CONSTANTS.CLIENT_ID);
      assert.ok(params.get('state'));
      assert.ok(params.get('code_challenge'));
    });
  });
});
