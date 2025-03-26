---
description: Guidelines for safely handling environment variables
globs: ".env*,.env.local,.env.production,.env
alwaysApply: false
---
# Environment Variables Safety Guidelines

<rule>
name: environment_variables_safety
description: Rules for safely handling environment variables in the codebase

filters:
  - type: file_name
    pattern: "^\\.env.*|.*\\.env\\.(js|ts)$"
  - type: event
    pattern: "file_edit"

actions:
  - type: warn
    conditions:
      - pattern: ".*"
        message: |
          ⚠️ You are modifying environment variables.
          
          Please verify the following before making changes:
          
          1. Check where each variable is used in the codebase
          2. Never delete variables without verifying they're unused
          3. Consider the impact on different environments (dev, staging, prod)
          4. Document new variables you're adding
          5. Ensure sensitive values are properly masked when appropriate
          
          Use tools like grep or codebase search to find all usages before modifying.

  - type: suggest
    message: |
      When modifying environment variables:
      
      ✅ DO:
      - Add comments explaining what each variable is used for
      - Keep related variables grouped together
      - Add new variables only when necessary
      - Document required variables in README or documentation
      - Consider providing example values in .env.example
      
      ❌ DON'T:
      - Remove existing variables without verification
      - Commit real secrets (use placeholders or .env.example)
      - Duplicate variables with different names
      - Use ambiguous variable names

examples:
  - input: |
      # Database Configuration
      - DB_HOST=localhost
      - DB_PORT=5432
      - DB_USER=admin
      
      # Adding API key
      + API_KEY=actual-secret-key
    output: |
      # Database Configuration
      DB_HOST=localhost
      DB_PORT=5432
      DB_USER=admin
      
      # API Configuration
      API_KEY=your-api-key-here # Replace with your actual key
      
metadata:
  priority: high
  version: 1.0
</rule> 