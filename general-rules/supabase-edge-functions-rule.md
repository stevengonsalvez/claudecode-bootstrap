---
description: Rules for deploying Supabase Edge Functions
globs: supabase/functions/**/*.ts
---
# Supabase Edge Functions Deployment

This rule provides guidance on how to deploy Supabase Edge Functions to your remote project.

## Deployment Command

To deploy a Supabase Edge Function to your remote project, use the following command format:

```bash
supabase functions deploy <function-name> --project-ref <project-ref>
```

Where:
- `<function-name>` is the name of the function to deploy (e.g., parse-resume)
- `<project-ref>` is your Supabase project reference ID

## Examples

Deploy a specific function:
```bash
supabase functions deploy parse-resume --project-ref abcdefghijklmnopqrst
```

Deploy all functions:
```bash
supabase functions deploy --project-ref abcdefghijklmnopqrst
```

## Environment Variables

To include environment variables when deploying:

```bash
supabase functions deploy <function-name> --project-ref <project-ref> --env-file ./path/to/.env
```

## Important Notes

1. You must have the Supabase CLI installed and be logged in
2. Make sure you're in the project root directory when running the command
3. The project reference ID can be found in your Supabase dashboard under Project Settings
4. Functions are deployed from the `supabase/functions` directory

<rule>
name: supabase_edge_functions_deployment
description: Standards for deploying Supabase Edge Functions
filters:
  - type: file_path
    pattern: "supabase/functions/.+\\.ts$"

metadata:
  priority: medium
  version: 1.0
</rule> 