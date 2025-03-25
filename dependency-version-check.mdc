---
description: Prevent dependency version downgrades
globs: requirements*.txt, pyproject.toml, package.json,pyproject*.toml
alwaysApply: false
---
# Dependency Version Downgrade Prevention

This rule prevents downgrading dependencies in requirements files. Only version increases or new dependencies are allowed.

<rule>
name: prevent_dependency_downgrade
description: Prevents downgrading dependency versions in requirements files
filters:
  # Match requirements files
  - type: file_path
    pattern: "(requirements.*\\.txt|pyproject\\.toml|package\\.json)$"
  # Match file modification events
  - type: event
    pattern: "(file_modify|file_create)"

actions:
  - type: check
    conditions:
      # Look for version downgrades in requirements.txt files
      - pattern: "^([a-zA-Z0-9_\\-\\.]+)([><=~^]+)([0-9]+\\.[0-9]+\\.[0-9]+)"
        message: "Checking dependency versions in requirements files"
        
  - type: reject
    conditions:
      # Detect direct version downgrades like "package>=1.0.0" to "package>=0.9.0"
      - pattern: "(?<=[><=~^]+)([0-9]+\\.[0-9]+\\.[0-9]+).*[\\s\\S]*\\1.*(?<=[><=~^]+)([0-9]+\\.[0-9]+\\.[0-9]+)"
        where: "$2 < $1"
        message: "Dependency version downgrade detected: $2 is lower than previous version $1. Only version upgrades are allowed."

  - type: suggest
    message: |
      ⚠️ Dependency Version Management Guidelines ⚠️
      
      When updating dependencies:
      
      1. NEVER downgrade dependency versions
      2. You can:
         - Add new dependencies
         - Upgrade existing dependencies
         - Keep existing versions unchanged
      
      3. If a dependency conflicts, consider:
         - Finding a compatible version that satisfies all requirements
         - Refactoring code to work with newer versions
         - Adding detailed comments explaining version constraints

examples:
  - input: |
      # requirements.txt
      fastapi>=0.109.0
      uvicorn>=0.27.0
      pydantic>=2.0.0
    output: |
      # requirements.txt
      fastapi>=0.109.0
      uvicorn>=0.27.0
      pydantic>=2.0.0
      weasyprint>=55.0
  
  - input: |
      # requirements.txt
      fastapi>=0.109.0
      uvicorn>=0.27.0
      pydantic>=2.0.0
    output: |
      # requirements.txt
      fastapi>=0.110.0  # Upgrade is fine
      uvicorn>=0.27.0
      pydantic>=2.1.0  # Upgrade is fine
  
  - input: |
      # requirements.txt
      fastapi>=0.109.0
      uvicorn>=0.27.0
      pydantic>=2.0.0
    output: |
      # requirements.txt
      fastapi>=0.68.0  # ERROR: Downgrade detected
      uvicorn>=0.15.0  # ERROR: Downgrade detected
      pydantic>=1.8.0  # ERROR: Downgrade detected

metadata:
  priority: high
  version: 1.0
</rule> 