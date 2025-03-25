---
description: Security-first implementation guidelines
globs: src/**/*.{ts,tsx,js,jsx}
---
# Security-First Guidelines

Rules for ensuring that security is never bypassed in favor of functionality.

<rule>
name: security_first
description: Enforces strict security practices when working with user data and authentication
filters:
  # Match any TypeScript/JavaScript files
  - type: file_extension
    pattern: "\\.(ts|tsx|js|jsx)$"
  # Match Supabase client operations
  - type: content
    pattern: "(?s)supabase\\.(from|auth|storage|rpc)"

actions:
  - type: warn
    conditions:
      - pattern: "(?s)supabase\\.auth\\.user\\(\\)"
        message: "Using deprecated auth.user() method. Use auth.getSession() or auth.getUser() instead"

  - type: reject
    conditions:
      - pattern: "(?s)supabase\\.rpc\\('postgres'\\)"
        message: "Direct database access bypassing RLS is not allowed"
      - pattern: "(?s)supabase\\.auth\\.setSession\\("
        message: "Manual session manipulation is a security risk"
      - pattern: "(?s)\\.serviceRole"
        message: "Using service role client bypasses RLS and is not allowed"

  - type: suggest
    message: |
      When working with user data and authentication:

      ## Core Security Principles

      1. **Never bypass Row-Level Security (RLS)** - All data access must respect RLS policies.
      2. **Always use authenticated clients** - When accessing user data, always use the authenticated Supabase client.
      3. **Verify user identity** - Ensure users can only access their own data.
      4. **Handle sensitive data appropriately** - Never expose sensitive information.
      5. **Validate all inputs** - Always validate and sanitize inputs.

      ## Implementation Practices

      1. **For Supabase queries**:
         - Use the authenticated client from `useSupabaseClient()` hook
         - Ensure correct RLS policies are set up
         - Test with different user accounts

      2. **For APIs and endpoints**:
         - Always validate authentication
         - Implement proper authorization checks

      3. **For client-side code**:
         - Never store sensitive information in local storage
         - Implement proper session management

      ## Always Choose Security Over Functionality

examples:
  - input: |
      // Bad: Using deprecated method
      const user = supabase.auth.user();
      
      // Bad: Bypassing RLS
      const { data } = await supabase.rpc('postgres', { sql: 'SELECT * FROM profiles' });
      
      // Good: Using recommended approach
      const { data: { user } } = await supabase.auth.getUser();
      
      // Good: Proper query with RLS
      const { data } = await supabase.from('profiles').select('*').eq('user_id', user.id);
    output: "Correctly implemented security practices for Supabase"

metadata:
  priority: critical
  version: 1.0
</rule> 