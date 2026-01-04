// ABOUTME: Test suite for OAuth login completion functionality
// Tests authorization code exchange, token handling, and credential storage

const { describe, it, before, after } = require('node:test');
const assert = require('node:assert');
const path = require('path');
const fs = require('fs').promises;
const os = require('os');

// Import the module under test
const oauthFinish = require('../../oauth-finish.js');

describe('OAuth Finish', () => {
  describe('Constants', () => {
    it('should have correct OAuth token exchange constants', () => {
      assert.strictEqual(oauthFinish.OAUTH_CONSTANTS.OAUTH_TOKEN_URL, 'https://console.anthropic.com/v1/oauth/token');
      assert.strictEqual(oauthFinish.OAUTH_CONSTANTS.CLIENT_ID, '9d1c250a-e61b-44d9-88ed-5944d1962f5e');
      assert.strictEqual(oauthFinish.OAUTH_CONSTANTS.REDIRECT_URI, 'https://console.anthropic.com/oauth/code/callback');
      assert.ok(oauthFinish.OAUTH_CONSTANTS.STATE_FILE.endsWith(path.join('.claude', '.claude_oauth_state.json')), 'STATE_FILE path should be constructed correctly');
      assert.ok(oauthFinish.OAUTH_CONSTANTS.CREDENTIALS_PATH.includes('.credentials.json'));
      assert.strictEqual(oauthFinish.OAUTH_CONSTANTS.REQUEST_HEADERS['Content-Type'], 'application/json');
    });
  });

  describe('cleanAuthorizationCode', () => {
    it('should clean authorization code from URL fragments', () => {
      const testCases = [
        {
          input: 'abc123def456',
          expected: 'abc123def456',
          description: 'plain code without fragments'
        },
        {
          input: 'abc123def456#state=xyz&scope=read',
          expected: 'abc123def456',
          description: 'code with URL fragment'
        },
        {
          input: 'abc123def456?state=xyz&scope=read',
          expected: 'abc123def456',
          description: 'code with query parameters'
        },
        {
          input: 'abc123def456?state=xyz#fragment=data',
          expected: 'abc123def456',
          description: 'code with both query and fragment'
        }
      ];

      testCases.forEach(({ input, expected, description }) => {
        const result = oauthFinish.cleanAuthorizationCode(input);
        assert.strictEqual(result, expected, `Failed for ${description}: ${input}`);
      });
    });
  });

  describe('verifyState', () => {
    const testStateFile = path.join(os.tmpdir(), 'test-oauth-state.json');

    after(async () => {
      try {
        await fs.unlink(testStateFile);
      } catch (err) {
        // Ignore if file doesn't exist
      }
    });

    it('should return valid state data when file exists and is not expired', async () => {
      // Create valid state file with future expiration
      const validState = {
        state: 'test-state-123',
        code_verifier: 'test-code-verifier',
        expires_at: new Date(Date.now() + 10 * 60 * 1000).toISOString() // 10 minutes from now
      };

      await fs.writeFile(testStateFile, JSON.stringify(validState));

      // Override the STATE_FILE constant for this test
      const originalStateFile = oauthFinish.OAUTH_CONSTANTS.STATE_FILE;
      oauthFinish.OAUTH_CONSTANTS.STATE_FILE = testStateFile;

      try {
        const result = await oauthFinish.verifyState();
        assert.strictEqual(result.state, 'test-state-123');
        assert.strictEqual(result.code_verifier, 'test-code-verifier');
        assert.ok(result.expires_at);
      } finally {
        // Restore original constant
        oauthFinish.OAUTH_CONSTANTS.STATE_FILE = originalStateFile;
      }
    });

    it('should return null when state file does not exist', async () => {
      // Override STATE_FILE to non-existent file
      const nonExistentFile = path.join(os.tmpdir(), 'non-existent-oauth-state.json');
      const originalStateFile = oauthFinish.OAUTH_CONSTANTS.STATE_FILE;
      oauthFinish.OAUTH_CONSTANTS.STATE_FILE = nonExistentFile;

      try {
        const result = await oauthFinish.verifyState();
        assert.strictEqual(result, null);
      } finally {
        oauthFinish.OAUTH_CONSTANTS.STATE_FILE = originalStateFile;
      }
    });

    it('should return null when state has expired', async () => {
      // Create expired state file
      const expiredState = {
        state: 'test-state-456',
        code_verifier: 'test-code-verifier-expired',
        expires_at: new Date(Date.now() - 60 * 1000).toISOString() // 1 minute ago
      };

      await fs.writeFile(testStateFile, JSON.stringify(expiredState));

      const originalStateFile = oauthFinish.OAUTH_CONSTANTS.STATE_FILE;
      oauthFinish.OAUTH_CONSTANTS.STATE_FILE = testStateFile;

      try {
        const result = await oauthFinish.verifyState();
        assert.strictEqual(result, null);
      } finally {
        oauthFinish.OAUTH_CONSTANTS.STATE_FILE = originalStateFile;
      }
    });
  });

  describe('exchangeCodeForTokens', () => {
    it('should exchange authorization code for OAuth tokens', async () => {
      // This is a more complex function that requires mocking HTTP requests
      // For now, let's test the basic structure and error handling

      // Test with invalid authorization code (empty/null)
      try {
        await oauthFinish.exchangeCodeForTokens('');
        assert.fail('Should have thrown error for empty code');
      } catch (error) {
        assert.ok(error.message.includes('Authorization code is required'));
      }

      try {
        await oauthFinish.exchangeCodeForTokens(null);
        assert.fail('Should have thrown error for null code');
      } catch (error) {
        assert.ok(error.message.includes('Authorization code is required'));
      }
    });

    it('should handle missing state file', async () => {
      // Override STATE_FILE to non-existent file
      const nonExistentFile = path.join(os.tmpdir(), 'missing-oauth-state.json');
      const originalStateFile = oauthFinish.OAUTH_CONSTANTS.STATE_FILE;
      oauthFinish.OAUTH_CONSTANTS.STATE_FILE = nonExistentFile;

      try {
        const result = await oauthFinish.exchangeCodeForTokens('valid-code-123');
        assert.strictEqual(result, null);
      } finally {
        oauthFinish.OAUTH_CONSTANTS.STATE_FILE = originalStateFile;
      }
    });
  });

  describe('saveCredentials', () => {
    const testCredentialsFile = path.join(os.tmpdir(), 'test-credentials.json');

    after(async () => {
      try {
        await fs.unlink(testCredentialsFile);
      } catch (err) {
        // Ignore if file doesn't exist
      }
    });

    it('should save OAuth credentials to file', async () => {
      const mockTokens = {
        access_token: 'test-access-token-123',
        token_type: 'Bearer',
        expires_in: 3600,
        refresh_token: 'test-refresh-token-456',
        scope: 'org:create_api_key user:profile user:inference'
      };

      // Override the CREDENTIALS_PATH constant for this test
      const originalCredentialsPath = oauthFinish.OAUTH_CONSTANTS.CREDENTIALS_PATH;
      oauthFinish.OAUTH_CONSTANTS.CREDENTIALS_PATH = testCredentialsFile;

      try {
        await oauthFinish.saveCredentials(mockTokens);

        // Verify file was created and has correct content
        const fileContent = await fs.readFile(testCredentialsFile, 'utf-8');
        const savedCredentials = JSON.parse(fileContent);

        const oauthCreds = savedCredentials.claudeAiOauth;
        assert.ok(oauthCreds, 'claudeAiOauth object should exist');
        assert.strictEqual(oauthCreds.accessToken, 'test-access-token-123');
        assert.strictEqual(oauthCreds.refreshToken, 'test-refresh-token-456');
        assert.ok(oauthCreds.expiresAt, 'expiresAt should exist');
        assert.deepStrictEqual(oauthCreds.scopes, ['org:create_api_key', 'user:profile', 'user:inference']);
        assert.strictEqual(oauthCreds.isMax, true);
      } finally {
        // Restore original constant
        oauthFinish.OAUTH_CONSTANTS.CREDENTIALS_PATH = originalCredentialsPath;
      }
    });

    it('should create directory if it does not exist', async () => {
      const testDir = path.join(os.tmpdir(), 'test-oauth-dir');
      const testCredentialsPath = path.join(testDir, 'credentials.json');

      const mockTokens = {
        access_token: 'test-token',
        token_type: 'Bearer'
      };

      const originalCredentialsPath = oauthFinish.OAUTH_CONSTANTS.CREDENTIALS_PATH;
      oauthFinish.OAUTH_CONSTANTS.CREDENTIALS_PATH = testCredentialsPath;

      try {
        await oauthFinish.saveCredentials(mockTokens);

        // Verify directory and file were created
        const stats = await fs.stat(testCredentialsPath);
        assert.ok(stats.isFile());

        const fileContent = await fs.readFile(testCredentialsPath, 'utf-8');
        const savedCredentials = JSON.parse(fileContent);
        assert.strictEqual(savedCredentials.claudeAiOauth.accessToken, 'test-token');
      } finally {
        // Clean up
        try {
          await fs.unlink(testCredentialsPath);
          await fs.rmdir(testDir);
        } catch (err) {
          // Ignore cleanup errors
        }
        oauthFinish.OAUTH_CONSTANTS.CREDENTIALS_PATH = originalCredentialsPath;
      }
    });
  });

  describe('cleanupState', () => {
    it('should clean up temporary OAuth state file', async () => {
      const testStateFile = path.join(os.tmpdir(), 'cleanup-test-oauth-state.json');

      // Create a state file to clean up
      const stateData = {
        state: 'test-state',
        code_verifier: 'test-verifier',
        expires_at: new Date().toISOString()
      };
      await fs.writeFile(testStateFile, JSON.stringify(stateData));

      // Verify file exists
      let stats = await fs.stat(testStateFile);
      assert.ok(stats.isFile());

      // Override the STATE_FILE constant for this test
      const originalStateFile = oauthFinish.OAUTH_CONSTANTS.STATE_FILE;
      oauthFinish.OAUTH_CONSTANTS.STATE_FILE = testStateFile;

      try {
        await oauthFinish.cleanupState();

        // Verify file has been deleted
        try {
          await fs.stat(testStateFile);
          assert.fail('State file should have been deleted');
        } catch (error) {
          // File should not exist - this is expected
          assert.strictEqual(error.code, 'ENOENT');
        }
      } finally {
        // Restore original constant
        oauthFinish.OAUTH_CONSTANTS.STATE_FILE = originalStateFile;
      }
    });

    it('should handle non-existent state file gracefully', async () => {
      const nonExistentFile = path.join(os.tmpdir(), 'non-existent-oauth-state.json');

      const originalStateFile = oauthFinish.OAUTH_CONSTANTS.STATE_FILE;
      oauthFinish.OAUTH_CONSTANTS.STATE_FILE = nonExistentFile;

      try {
        // Should not throw error when file doesn't exist
        await oauthFinish.cleanupState();
        // If we get here, the function handled the missing file gracefully
        assert.ok(true);
      } finally {
        oauthFinish.OAUTH_CONSTANTS.STATE_FILE = originalStateFile;
      }
    });
  });

  describe('completeLogin', () => {
    it('should orchestrate the OAuth login completion', async () => {
      // This is an integration test that will test the error flow
      // since we can't easily mock the HTTP request in this test

      // Test with invalid authorization code (empty)
      try {
        const result = await oauthFinish.completeLogin('');
        assert.strictEqual(result, false);
      } catch (error) {
        assert.ok(error.message.includes('Authorization code is required'));
      }

      // Test with null authorization code
      try {
        const result = await oauthFinish.completeLogin(null);
        assert.strictEqual(result, false);
      } catch (error) {
        assert.ok(error.message.includes('Authorization code is required'));
      }
    });

    it('should return false when state file is missing', async () => {
      // Override STATE_FILE to non-existent file
      const nonExistentFile = path.join(os.tmpdir(), 'missing-complete-oauth-state.json');
      const originalStateFile = oauthFinish.OAUTH_CONSTANTS.STATE_FILE;
      oauthFinish.OAUTH_CONSTANTS.STATE_FILE = nonExistentFile;

      try {
        const result = await oauthFinish.completeLogin('valid-code-123');
        assert.strictEqual(result, false);
      } finally {
        oauthFinish.OAUTH_CONSTANTS.STATE_FILE = originalStateFile;
      }
    });
  });
});
