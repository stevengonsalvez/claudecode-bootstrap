# OAuth Implementation Tests

This directory contains the test suite for the Node.js OAuth implementation for Claude Code authentication.

## Structure

- `package.json` - Test configuration and dependencies
- `test/` - Test files using Node.js built-in test runner
  - `oauth-start.test.js` - Tests for OAuth login initiation
  - `oauth-finish.test.js` - Tests for OAuth login completion

## Test-Driven Development Approach

The OAuth implementation follows strict TDD:

1. **Red**: Write failing tests that define the expected behavior
2. **Green**: Implement minimal code to make tests pass
3. **Refactor**: Improve code structure while keeping tests green

## Running Tests

```bash
# Run all tests
npm test

# Run tests in watch mode
npm run test:watch

# Run with coverage
npm run test:coverage

# Run specific test files
npm run test:oauth-start
npm run test:oauth-finish
```

## Implementation Status

- ✅ Stub implementations created with constants from Ruby version
- 🔄 Tests will be implemented first (TDD red phase)
- ⏳ Production code will be implemented after tests

## OAuth Constants

The implementation uses the same OAuth constants as the Ruby version:

- Client ID: `9d1c250a-e61b-44d9-88ed-5944d1962f5e`
- Authorization URL: `https://claude.ai/oauth/authorize`
- Token URL: `https://console.anthropic.com/v1/oauth/token`
- Redirect URI: `https://console.anthropic.com/oauth/code/callback`
- Scopes: `org:create_api_key user:profile user:inference`

## Security Features

- PKCE (Proof Key for Code Exchange) implementation
- Secure state parameter generation
- Time-based state expiration (10 minutes)
- Proper token storage and cleanup
