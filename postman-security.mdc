---
description: API Security Standards
globs: *.json, *.md, *.py, *.js, *.ts, *.yml, *.yaml, *.env*, *.example, *.sample, *.postman_collection.json
alwaysApply: true
---
# API Security Standards

Standards for securing API credentials in all files, including tests, documentation, examples, and configurations.

<rule>
name: api_key_security
description: Prevents hardcoded API keys anywhere in the codebase
filters:
  # Match any of the common file types that might contain API keys
  - type: file_extension
    pattern: "\\.(json|md|py|js|ts|yml|yaml|env.*|example|sample)$"

actions:
  - type: reject
    conditions:
      # Match potential API keys in variables, config files, etc.
      - pattern: "(api[-_]?key|apikey|x[-_]?api[-_]?key|auth[-_]?token|bearer[-_]?token|access[-_]?token|secret[-_]?key)\\s*[=:]\\s*[\"']([a-zA-Z0-9_\\-\\.]{16,})[\"']"
        message: "API keys should not be hardcoded. Use environment variables or replace with 'REDACTED'."
      
      # Match potential hardcoded JWT or long tokens
      - pattern: "(eyJ[a-zA-Z0-9_-]{10,}\\.[a-zA-Z0-9_-]{10,}\\.[a-zA-Z0-9_-]{10,})"
        message: "JWTs should not be hardcoded. Use environment variables or replace with 'REDACTED'."
      
      # Match specific API key patterns for common services
      - pattern: "(sk-[a-zA-Z0-9]{20,}|pk-[a-zA-Z0-9]{20,}|ak-[a-zA-Z0-9]{20,})"
        message: "Service API keys should not be hardcoded. Use environment variables or replace with 'REDACTED'."
      
      # Match potential API keys in JSON format (like in Postman collections)
      - pattern: "\"key\":\\s*\"(api[-_]?key|apikey|x[-_]?api[-_]?key)\",[\\s\\n]*\"value\":\\s*\"(?!\\{\\{|REDACTED)[a-zA-Z0-9_\\-\\.]{16,}\""
        message: "API keys should not be hardcoded in configuration files. Use environment variables or replace with 'REDACTED'."

  - type: suggest
    message: |
      When handling API keys or sensitive credentials:
      
      1. Never include actual API keys or credentials in:
         - Code repositories
         - Documentation
         - Example files
         - Test files
         - Configuration templates
      
      2. Always use one of these approaches:
         - Environment variables: `API_KEY=${API_KEY}`
         - Template placeholders: `API_KEY={{apiKey}}`
         - The word "REDACTED": `API_KEY="REDACTED"`
      
      3. For documentation and examples:
         ```
         # Config example
         API_KEY="REDACTED"  # Replace with your actual API key
         
         # JSON example
         {
           "apiKey": "REDACTED"
         }
         ```
      
      4. For tests, use:
         - Environment variables
         - Test-specific dummy keys
         - Mock authentication services
      
      5. Never commit .env files containing real credentials
         - Add .env to .gitignore
         - Provide .env.example files with "REDACTED" values

examples:
  - input: |
      // Bad: Hardcoded API key in config
      const apiKey = "7a68992fc24e3a62b5ea0a60a07fc930e044c62f56ad4cbd08ef2cd77c998751";
      
      # Bad: Hardcoded API key in Python
      api_key = "7a68992fc24e3a62b5ea0a60a07fc930e044c62f56ad4cbd08ef2cd77c998751"
      
      # Bad: In markdown documentation
      Use your API key: `7a68992fc24e3a62b5ea0a60a07fc930e044c62f56ad4cbd08ef2cd77c998751`
      
      // Bad: In JSON config
      {
        "apiKey": "7a68992fc24e3a62b5ea0a60a07fc930e044c62f56ad4cbd08ef2cd77c998751"
      }
    output: |
      // Good: Using environment variable
      const apiKey = process.env.API_KEY;
      
      # Good: Using environment variable in Python
      api_key = os.environ.get("API_KEY")
      
      # Good: In markdown documentation
      Use your API key: `REDACTED`
      
      // Good: In JSON config
      {
        "apiKey": "REDACTED"
      }
      
      // Good: In .env.example file
      API_KEY=REDACTED

metadata:
  priority: high
  version: 1.0
</rule> 