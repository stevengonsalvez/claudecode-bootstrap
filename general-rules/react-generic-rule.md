---
description: React and Next.js Development Standards for Robust and Modern Applications
globs:
  - "**/*.{js,jsx,ts,tsx}"
  - "!**/*.d.ts"
  - "!**/node_modules/**"
  - "!**/*.config.{js,ts}" # Exclude config files usually
  - "!**/generated/**" # Exclude auto-generated files
alwaysApply: true
---
# React/Next.js Core Development Standards

Rules promoting maintainable, performant, and secure React/Next.js applications.

<rule>
name: react_component_file_structure
description: Enforces clear separation of concerns by preventing page-level logic/routing directly within general UI components.
filters:
  - type: file_extension
    pattern: "\\.(j|t)sx?$"
  - type: file_path # Target files within a 'components' or 'ui' directory
    pattern: "(components|ui|shared|features)/"
actions:
  - type: reject
    conditions:
      # Looks for Next.js page/route specific exports or hooks in general component files
      - pattern: "export\\s+async\\s+function\\s+(getServerSideProps|getStaticProps|generateStaticParams|generateMetadata)"
        message: "Page-specific data fetching (getServerSideProps, getStaticProps) or metadata generation should be in 'app/' or 'pages/' route files, not general components."
      - pattern: "import\\s+.*\\s+from\\s+['\"]next/(navigation|router)['\"]" # Discourage direct router usage in general UI components
        message: "Avoid direct usage of 'next/navigation' or 'next/router' in generic UI components. Pass navigation handlers as props or use context for shared navigation concerns if absolutely necessary."
  - type: suggest
    message: |
      General UI components (those in `components/`, `ui/`, `shared/`, etc.) should be presentation-focused and reusable.
      They should not contain page-level routing logic, Next.js specific data fetching functions (`getServerSideProps`, `getStaticProps`), or route handlers.
      - Page/Route specific logic belongs in files within the `app/` directory (e.g., `app/dashboard/page.tsx`) or `pages/` directory.
      - Pass necessary data and event handlers (like navigation functions) as props to your UI components.
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: react_state_management_use_react_hooks
description: Recommends using React's built-in hooks (useState, useReducer, useContext) for state management over more complex libraries like Redux for most cases.
filters:
  - type: file_extension
    pattern: "\\.(j|t)sx?$"
actions:
  - type: reject # Strong discouragement of Redux unless truly justified
    conditions:
      - pattern: "import\\s+.*\\s+from\\s+['\"](redux|react-redux)['\"]"
        message: "For most state management needs, prefer React's built-in hooks (useState, useReducer, useContext) or simpler state libraries (Zustand, Jotai). Redux often introduces unnecessary complexity unless managing very large, complex global state."
  - type: suggest
    message: |
      Start with React's built-in state management solutions:
      - `useState` for local component state.
      - `useReducer` for more complex local state logic.
      - `useContext` for sharing state across components without prop drilling.

      For more global or shared state needs, consider lightweight libraries like Zustand or Jotai before reaching for Redux.
      Redux can be powerful but often adds significant boilerplate and complexity for many applications.
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: react_data_fetching_with_tanstack_query
description: Recommends TanStack Query (React Query) for server-state management, caching, and data synchronization.
filters:
  - type: file_extension
    pattern: "\\.(j|t)sx?$"
  - type: content # Only if not already using TanStack Query
    pattern: "(fetch\\(|axios\\.(get|post)|useEffect\\s*\\(\\s*async\\s*\\(\\))((?!useQuery|useMutation|TanStack|@tanstack).)*$"
actions:
  - type: suggest
    conditions:
      - pattern: "useEffect\\s*\\(\\s*async\\s*\\(\\)\\s*=>\\s*{\\s*(await\\s+)?(fetch\\(|axios\\.)" # Basic data fetching in useEffect
        message: "For client-side data fetching, caching, and synchronization, consider using TanStack Query (React Query) via `useQuery` or `useMutation` instead of manual `useEffect` and `fetch/axios` calls. It simplifies state management around server data."
    message: |
      TanStack Query (formerly React Query) is a powerful library for managing server state in React applications. It handles:
      - Data fetching and background updates.
      - Caching and stale-while-revalidate strategies.
      - Optimistic updates.
      - Pagination and infinite scrolling.
      - Error handling and retries.

      Using `useQuery` and `useMutation` significantly reduces boilerplate for common data fetching patterns compared to manual `useEffect` and `useState` combinations.

      Example:
      ```tsx
      // Instead of:
      // useEffect(() => {
      //   const fetchData = async () => { /* ... fetch logic with setLoading, setData, setError ... */ };
      //   fetchData();
      // }, []);

      // Use TanStack Query:
      // import { useQuery } from '@tanstack/react-query';
      // const { data, isLoading, error } = useQuery({ queryKey: ['todos'], queryFn: fetchTodos });
      ```
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: react_dependency_upgrade_caution
description: Advises caution when performing major version upgrades of dependencies, emphasizing checking for breaking changes and usage.
filters:
  - type: file_path # Typically relevant when package.json or lock files change
    pattern: "(package\\.json|pnpm-lock\\.yaml|yarn\\.lock|package-lock\\.json)$"
  - type: event # Apply on file modification
    pattern: "file_modify"
actions:
  - type: suggest # This is a process/awareness rule
    message: |
      When upgrading dependencies, especially major versions (e.g., v1.x.x to v2.x.x):
      1.  **Review Changelogs:** Carefully read the release notes and changelogs for breaking changes, deprecated features, and migration guides.
      2.  **Assess Impact:** Identify where your project uses the upgraded library and how breaking changes might affect your codebase.
      3.  **Test Thoroughly:** Perform comprehensive testing (unit, integration, E2E) after upgrading to catch regressions.
      4.  **Incremental Upgrades:** If upgrading multiple major versions, consider doing it incrementally (e.g., v1 to v2, then v2 to v3) to isolate issues.
      Blindly upgrading dependencies can introduce subtle bugs or break your application.
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: react_no_direct_dom_manipulation
description: Prohibits direct DOM manipulation, encouraging the use of React's state and props to manage UI updates.
filters:
  - type: file_extension
    pattern: "\\.(j|t)sx?$"
actions:
  - type: reject
    conditions:
      - pattern: "document\\.getElementById\\(|document\\.querySelector\\(|\\.innerHTML\\s*=|\\.appendChild\\(|\\.removeChild\\("
        message: "Avoid direct DOM manipulation (e.g., `document.getElementById`, `innerHTML`). Manage UI updates declaratively through React's state, props, and refs (for imperative access when necessary)."
  - type: suggest
    message: |
      React operates on a Virtual DOM and efficiently updates the actual DOM. Direct manipulation bypasses React's rendering lifecycle,
      can lead to inconsistencies, and makes components harder to reason about.
      - Use component state (`useState`) and props to drive UI changes.
      - For cases where direct DOM access is unavoidable (e.g., managing focus, media playback, integrating with third-party DOM libraries), use `useRef`.
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: react_api_keys_server_side_only
description: Prevents exposure of API keys or secrets in client-side code.
filters:
  - type: file_extension
    pattern: "\\.(j|t)sx?$"
  - type: file_path # Don't apply to files explicitly in server-only paths
    pattern: "^(?!.*(server|api|pages/api|app/api|app/.*route\\.tsx?)).*$"
actions:
  - type: reject
    conditions:
      - pattern: "(NEXT_PUBLIC_)?(API_KEY|SECRET_KEY|ACCESS_TOKEN|_TOKEN|_SECRET)\\s*[:=]\\s*['\"](sk-|rk_live|pk_test|ghp_|glpat-|[A-Za-z0-9\\-_\\.+]{20,})['\"]"
        message: "API keys or secrets (unless explicitly prefixed with `NEXT_PUBLIC_` for public keys) must not be hardcoded or exposed in client-side bundles. Access them via server-side API routes or server components."
      - pattern: "process\\.env\\.(?!NEXT_PUBLIC_)\\w+(API_KEY|SECRET_KEY|ACCESS_TOKEN|_TOKEN|_SECRET)" # Accessing non-public env vars on client
        message: "Environment variables without `NEXT_PUBLIC_` prefix are not available in the client-side bundle. Access sensitive credentials via server-side API routes or server components."
  - type: suggest
    message: |
      Sensitive credentials like API keys must NEVER be included in client-side JavaScript bundles, as they can be easily extracted.
      - Store secrets in environment variables on your server/build environment.
      - In Next.js, only environment variables prefixed with `NEXT_PUBLIC_` are exposed to the browser. Use these only for non-sensitive, public keys.
      - Access sensitive APIs by creating a backend API route (e.g., in `app/api/` or `pages/api/`) that makes the call on the server and then call this route from your client-side code.
      - Utilize Server Components in Next.js App Router to perform data fetching with secrets on the server.
metadata:
  priority: critical
  version: 1.0
</rule>

<rule>
name: react_useeffect_dependency_array_explicit
description: Enforces explicit and correct dependency arrays for `useEffect`, `useCallback`, and `useMemo` to prevent stale closures or infinite loops.
filters:
  - type: file_extension
    pattern: "\\.(j|t)sx?$"
actions:
  - type: reject # ESLint plugin 'eslint-plugin-react-hooks' handles this best, but a regex can catch obvious omissions.
    conditions:
      - pattern: "useEffect\\s*\\(\\s*\\([^)]*\\)\\s*=>\\s*{[^}]*}\\s*\\)\\s*;" # useEffect without dependency array
        message: "`useEffect` calls must have a dependency array. An empty array `[]` means the effect runs only on mount/unmount. Omit it only if you truly understand the implications (rare)."
      - pattern: "(useCallback|useMemo)\\s*\\(\\s*\\([^)]*\\)\\s*=>\\s*{[^}]*}\\s*\\)\\s*;" # useCallback/useMemo without dependency array
        message: "`useCallback` and `useMemo` calls must have a dependency array to correctly memoize values/functions."
  - type: suggest
    message: |
      The dependency array for `useEffect`, `useCallback`, and `useMemo` is crucial for correct behavior:
      - `useEffect`: Determines when the effect re-runs. If it uses any value from the component scope (props, state, functions), that value should be in the array.
        - `[]`: Runs once on mount and cleans up on unmount.
        - No array (omitted): Runs after every render (usually undesirable).
      - `useCallback`: Memoizes a callback function. Include all dependencies the callback closes over.
      - `useMemo`: Memoizes a computed value. Include all dependencies used in the computation.

      Incorrect dependency arrays can lead to stale data, infinite loops, or missed updates.
      The `eslint-plugin-react-hooks` (specifically the `exhaustive-deps` rule) is highly recommended to automatically check this.
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: nextjs_image_component_usage
description: Recommends using `next/image` for optimized image handling over the native `<img>` tag.
filters:
  - type: file_extension
    pattern: "\\.(j|t)sx?$"
actions:
  - type: suggest
    conditions:
      - pattern: "<img\\s+[^>]*src=" # Finds native <img> tags
        message: "For images in Next.js, prefer using the `<Image>` component from `next/image` over the native `<img>` tag. It provides automatic optimization (resizing, format conversion, lazy loading)."
    message: |
      The `next/image` component offers several benefits for image handling in Next.js applications:
      - Automatic image optimization: Serves images in modern formats (like WebP) and sizes appropriate for the device.
      - Lazy loading: Defers loading of offscreen images.
      - Prevents layout shift: Reserves space for the image before it loads.
      - Easy responsiveness.

      Example:
      ```tsx
      import Image from 'next/image';

      // Instead of:
      // <img src="/my-image.jpg" alt="Description" width="500" height="300" />

      // Use:
      // <Image src="/my-image.jpg" alt="Description" width={500} height={300} />
      ```
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: react_avoid_index_as_key_for_dynamic_lists
description: Discourages using array index as `key` prop for lists of components if the list order can change or items can be added/removed.
filters:
  - type: file_extension
    pattern: "\\.(j|t)sx?$"
actions:
  - type: reject
    conditions:
      # Heuristic: .map((item, index) => <Component key={index} />)
      - pattern: "\\.map\\s*\\(\\s*\\([^,)]+,\\s*index\\)\\s*=>\\s*[^>]*key\\s*=\\s*{\\s*index\\s*}"
        message: "Avoid using array index as `key` for dynamic lists where items can be reordered, added, or removed. Use a stable, unique ID from your data instead. Using index as key can lead to incorrect component state and UI issues."
  - type: suggest
    message: |
      The `key` prop helps React identify which items have changed, are added, or are removed.
      Using the array index as a key is problematic if:
      - The order of items can change (e.g., sorting).
      - Items can be inserted or deleted from the middle of the list.
      This can lead to React reusing component instances with incorrect state or props.
      Always use a stable, unique identifier from your data items as the key.

      Example:
      ```tsx
      // Bad (if items can change order or be added/deleted):
      // items.map((item, index) => <MyComponent key={index} data={item} />)

      // Good (assuming item.id is unique and stable):
      // items.map(item => <MyComponent key={item.id} data={item} />)
      ```
      Using index as key is only safe if the list is static and will never change.
metadata:
  priority: high
  version: 1.0
</rule>