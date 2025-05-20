---
description: Rules for running Supabase migrations and SQL statements
globs: *.sql
alwaysApply: true
---

# Supabase SQL Command Rule

<rule>
name: supabase_sql_command_format_suggestion
description: Suggests the correct psql command format for Supabase when interacting with SQL files or statements.

filters:
  - type: file_extension
    pattern: "\\.sql$"

actions:
  - type: suggest
    message: |
      When asked to run SQL files or execute SQL statements for Supabase:

      ## For SQL files
      Use this command format:
      ```
      psql "postgres://postgres:postgres@localhost:54322/postgres" -f <file_path>
      ```

      ## For SQL statements
      Use this command format:
      ```
      psql "postgres://postgres:postgres@localhost:54322/postgres" -c "<sql_statement>"
      ```

examples:
  - input: |
      User prompt: "How do I execute 'SELECT * FROM users;' on Supabase?"
    output: |
      Suggested command:
      psql "postgres://postgres:postgres@localhost:54322/postgres" -c "SELECT * FROM users;"
  - input: |
      User prompt: "Run the migration file apply_schemas.sql for Supabase."
    output: |
      Suggested command:
      psql "postgres://postgres:postgres@localhost:54322/postgres" -f apply_schemas.sql

metadata:
  priority: medium
  version: 1.0
</rule>