---
description: Comprehensive guide to React best practices, covering code organization, performance, security, testing, and common pitfalls. Adhering to these guidelines helps developers build maintainable, scalable, and high-performing React applications.
globs: *.js,*.jsx,*.ts,*.tsx
---
# React Best Practices: A Comprehensive Guide

This document outlines the best practices for developing React applications, covering various aspects from code organization to security and testing. Following these guidelines leads to more maintainable, scalable, and performant applications.

## 1. Code Organization and Structure

### 1.1 Directory Structure

A well-defined directory structure is crucial for maintainability. Here's a recommended structure:


src/
  ├── components/
  │   ├── Button/
  │   │   ├── Button.jsx
  │   │   ├── Button.module.css
  │   │   └── Button.test.jsx
  │   ├── Input/
  │   │   ├── Input.jsx
  │   │   ├── Input.module.css
  │   │   └── Input.test.jsx
  │   └── ...
  ├── contexts/
  │   ├── AuthContext.jsx
  │   └── ThemeContext.jsx
  ├── hooks/
  │   ├── useAuth.js
  │   └── useTheme.js
  ├── pages/
  │   ├── Home.jsx
  │   ├── About.jsx
  │   └── ...
  ├── services/
  │   ├── api.js
  │   └── auth.js
  ├── utils/
  │   ├── helpers.js
  │   └── validators.js
  ├── App.jsx
  ├── index.jsx
  └── ...


-   **`components/`**: Reusable UI components.
    -   Each component has its own directory containing the component file, associated styles (using CSS modules), and tests.
-   **`contexts/`**: React context providers.
-   **`hooks/`**: Custom React hooks.
-   **`pages/`**: Top-level components representing different routes or views.
-   **`services/`**: API interaction logic.
-   **`utils/`**: Utility functions.

### 1.2 File Naming Conventions

-   **Components**: Use PascalCase (e.g., `MyComponent.jsx`).
-   **Hooks**: Use camelCase prefixed with `use` (e.g., `useMyHook.js`).
-   **Contexts**: Use PascalCase suffixed with `Context` (e.g., `MyContext.jsx`).
-   **Services/Utils**: Use camelCase (e.g., `apiService.js`, `stringUtils.js`).
-   **CSS Modules**: Use `.module.css` or `.module.scss` (e.g., `Button.module.css`).

### 1.3 Module Organization

-   **Co-location**: Keep related files (component, styles, tests) together in the same directory.
-   **Single Responsibility**: Each module should have a clear and specific purpose.
-   **Avoid Circular Dependencies**: Ensure modules don't depend on each other in a circular manner.

### 1.4 Component Architecture

-   **Atomic Design**: Consider using Atomic Design principles (Atoms, Molecules, Organisms, Templates, Pages) to structure components.
-   **Composition over Inheritance**: Favor component composition to reuse code and functionality.
-   **Presentational and Container Components**: Separate UI rendering (presentational) from state management and logic (container).

### 1.5 Component Organization Rules

**CRITICAL RULES**:
- **Pages**: Only route-level components go in `/pages`
- **Components**: Reusable UI components go in `/components`
- **Specialized Components**: Group related components in subdirectories:
  - `/components/event-creation/` - Event creation specific
  - `/components/seat-selection/` - Seat selection specific
- **Tests**: Co-locate test files with components
- **NO Page components in `/components`** - They belong in `/pages`

### 1.6 Code Splitting Strategies

-   **Route-Based Splitting**: Use `React.lazy` and `Suspense` to load components only when a specific route is accessed.  This is very common and improves initial load time.
-   **Component-Based Splitting**: Split large components into smaller chunks that can be loaded on demand.
-   **Bundle Analyzer**: Use a tool like `webpack-bundle-analyzer` to identify large dependencies and optimize bundle size.

## 2. Routing and Authentication

### 2.1 Route Categories

**CRITICAL**: Follow these exact route patterns:

```typescript
// Public Routes (No Authentication Required)
// - /home - Landing page
// - /login - User authentication
// - /register - User registration
// - /verify - Email verification
// - /:pageUrl - Dynamic organization pages

// Protected Routes (Authentication Required)
// - /profile - User profile management
// - /account - Account settings
// - /plans - Subscription plans
// - /organiser/dashboard - Organiser dashboard
// - /organiser/events - Event management

// Organiser-Only Routes (Organiser Role Required)
// - /organiser/events/create - Create new events
// - /organiser/events/:id/edit - Edit existing events
// - /organisation/* - Organization management
```

### 2.2 Protected Route Pattern

**MANDATORY**: Use this exact pattern for protected routes:

```typescript
const ProtectedRoute: React.FC<{
  component: React.ComponentType<any>;
  path: string;
  exact?: boolean;
}> = ({ component: Component, ...rest }) => {
  const { user, loading } = useUserContext();
  const history = useHistory();

  useEffect(() => {
    if (!loading && !user) {
      console.log('Protected route: User not authenticated, redirecting to login');
      history.push('/login');
    }
  }, [user, loading, history]);

  return (
    <Route
      {...rest}
      render={props =>
        loading ? (
          <Box sx={{ minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <CircularProgress />
          </Box>
        ) : user ? (
          <Component {...props} />
        ) : null
      }
    />
  );
};
```

### 2.3 Route Organization in App.tsx

**MANDATORY**: Organize routes in this exact order:

```typescript
// System Routes (always accessible)
<Route exact path="/verify" component={EmailVerification} />
<Route exact path="/home" component={Home} />

// Protected Routes (require authentication)
<ProtectedRoute exact path="/profile" component={ProfilePage} />
<ProtectedRoute exact path="/account" component={Account} />

// Organiser Routes (require organiser role)
<OrganiserProtectedRoute exact path="/organiser/events/create" component={EventCreationWizard} />

// Dynamic Organization Pages (must be before catch-all)
<Route exact path="/:pageUrl" render={({ match }) => {
  // Validation logic here
  return <OrganisationPage />;
}} />

// Catch-all route (must be last)
<Route path="*">
  <Redirect to="/home" />
</Route>
```

### 2.4 Navigation Patterns

**FORBIDDEN**: Never use `window.location.href`
**REQUIRED**: Always use React Router's history:

```typescript
// Use history for programmatic navigation
const history = useHistory();

const handleNavigation = (path: string) => {
  history.push(path);
};

// Use Redirect for declarative navigation
<Route exact path="/">
  <Redirect to="/organiser/dashboard" />
</Route>
```

## 3. Styling with Material-UI (MUI)

### 3.1 Styled Components Pattern (Preferred)

**PREFERRED**: Use for reusable styles:

```typescript
import { styled } from '@mui/material/styles';
import { Box, Button } from '@mui/material';

const StyledContainer = styled(Box)(({ theme }) => ({
  padding: theme.spacing(2),
  backgroundColor: theme.palette.background.paper,
  borderRadius: theme.shape.borderRadius,
  [theme.breakpoints.down('sm')]: {
    padding: theme.spacing(1),
  },
}));

const StyledButton = styled(Button)(({ theme }) => ({
  textTransform: 'none',
  fontWeight: theme.typography.fontWeightMedium,
}));
```

### 3.2 sx Prop Pattern

**ACCEPTABLE**: For one-off styles:

```typescript
<Box
  sx={{
    p: 2,
    bgcolor: 'background.paper',
    borderRadius: 1,
    display: 'flex',
    flexDirection: { xs: 'column', sm: 'row' },
    gap: 2,
  }}
>
  <Button
    sx={{
      textTransform: 'none',
      fontWeight: 'medium',
      '&:hover': {
        bgcolor: 'primary.dark',
      },
    }}
  >
    Click me
  </Button>
</Box>
```

### 3.3 Theme Usage

```typescript
import { useTheme } from '@mui/material/styles';

const MyComponent = () => {
  const theme = useTheme();
  
  return (
    <Box
      sx={{
        color: theme.palette.primary.main,
        [theme.breakpoints.up('md')]: {
          fontSize: theme.typography.h4.fontSize,
        },
      }}
    >
      Content
    </Box>
  );
};
```

### 3.4 Responsive Design Patterns

```typescript
// Breakpoint-based responsive values
<Box
  sx={{
    width: { xs: '100%', sm: '50%', md: '33%' },
    p: { xs: 1, sm: 2, md: 3 },
    display: { xs: 'block', md: 'flex' },
  }}
/>

// Theme breakpoints in styled components
const ResponsiveBox = styled(Box)(({ theme }) => ({
  [theme.breakpoints.down('sm')]: {
    flexDirection: 'column',
  },
  [theme.breakpoints.up('md')]: {
    flexDirection: 'row',
  },
}));
```

### 3.5 FORBIDDEN Styling Patterns

**NEVER USE**:
- Inline styles: `style={{ color: 'red' }}`
- Hardcoded colors: `sx={{ color: '#ff0000' }}`
- CSS classes for MUI components
- styled-components library (use MUI's styled instead)

## 4. Testing Approaches

### 4.1 Testing Strategy Overview

-   **Unit Testing**: Test individual components in isolation using Vitest + React Testing Library
-   **Integration Testing**: Test component interactions and API integrations
-   **End-to-End Testing**: Test complete user flows using Playwright

### 4.2 Unit Test Structure (Vitest + React Testing Library)

**TEMPLATE**: Use this exact structure:

```typescript
// ABOUTME: Unit tests for ComponentName - covers user interactions and edge cases

import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ComponentName } from './ComponentName';

const createTestQueryClient = () => new QueryClient({
  defaultOptions: { queries: { retry: false } }
});

describe('ComponentName', () => {
  test('renders with required props', () => {
    const queryClient = createTestQueryClient();
    
    render(
      <QueryClientProvider client={queryClient}>
        <ComponentName requiredProp="test" />
      </QueryClientProvider>
    );
    
    expect(screen.getByText('Expected Text')).toBeInTheDocument();
  });
  
  test('handles user interactions', async () => {
    const user = userEvent.setup();
    const mockHandler = vi.fn();
    
    render(<ComponentName onClick={mockHandler} />);
    
    await user.click(screen.getByRole('button'));
    expect(mockHandler).toHaveBeenCalledTimes(1);
  });
});
```

### 4.3 E2E Test Structure (Playwright)

**TEMPLATE**: Use this exact structure:

```typescript
// ABOUTME: E2E tests for user workflow - covers complete user journeys

import { test, expect } from '@playwright/test';

test.describe('User Authentication Flow', () => {
  test('user can login successfully', async ({ page }) => {
    await page.goto('/login');
    
    await page.fill('[data-testid="email-input"]', 'test@example.com');
    await page.fill('[data-testid="password-input"]', 'password123');
    await page.click('[data-testid="login-button"]');
    
    await expect(page).toHaveURL('/organiser/dashboard');
    await expect(page.getByText('Welcome')).toBeVisible();
  });
});
```

### 4.4 Mocking Standards

**REQUIRED**: Use these exact mocking patterns:

```typescript
// Mock Supabase client
vi.mock('../utils/supabaseClient', () => ({
  supabase: {
    auth: {
      signIn: vi.fn(),
      signOut: vi.fn(),
    },
    from: vi.fn(() => ({
      select: vi.fn().mockReturnThis(),
      insert: vi.fn().mockReturnThis(),
    })),
  },
}));

// Mock React Query
const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } }
});
```

### 4.5 Test Organization

- **Co-locate Tests**: Keep test files close to the components they test
- **Descriptive Names**: Use descriptive names for test files and test cases
- **Test Suites**: Organize tests into logical suites
- **Data Isolation**: Each test uses fresh data
- **Clean Up**: Clean up after tests

## 5. Common Patterns and Anti-patterns

### 5.1 Design Patterns

-   **Higher-Order Components (HOCs)**: Reusable logic that wraps components (use with caution; prefer hooks).
-   **Render Props**: Sharing code using a prop whose value is a function.
-   **Compound Components**: Components that work together implicitly (e.g., `Tabs`, `Tab`).
-   **Hooks**: Reusable stateful logic that can be shared across functional components.

### 5.2 Recommended Approaches

-   **Form Handling**: Use controlled components with local state or a form library like Formik or React Hook Form.
-   **API Calls**: Use `useEffect` hook to make API calls and manage loading states.
-   **Conditional Rendering**: Use short-circuit evaluation (`&&`) or ternary operators for simple conditions; use separate components for complex scenarios.
-   **List Rendering**: Always provide a unique and stable `key` prop when rendering lists.

### 5.3 Anti-patterns and Code Smells

-   **Direct DOM Manipulation**: Avoid directly manipulating the DOM; let React handle updates.
-   **Mutating State Directly**: Always use `setState` or the state updater function to modify state.
-   **Inline Styles**: Use CSS modules or styled-components for maintainable styles.
-   **Over-Engineering**: Avoid using complex solutions for simple problems.
-   **Prop Drilling**: Passing props through multiple levels of components without them being used.

### 5.4 State Management Best Practices

-   **Local State**: Use `useState` for component-specific state.
-   **Context API**: Use `useContext` for global state accessible to many components, but avoid for very frequently updated data.
-   **TanStack Query**: Use for server state management and data fetching.
-   **Redux/Mobx**: Use these libraries for complex state management in large applications.
-   **Recoil/Zustand**: Lightweight alternatives to Redux, often easier to set up and use.
-   **Immutable Data**: Treat state as immutable to prevent unexpected side effects.

### 5.5 Error Handling Patterns

-   **Error Boundaries**: Wrap components with error boundaries to catch errors during rendering and prevent crashes.
-   **Try-Catch Blocks**: Use try-catch blocks for handling errors in asynchronous operations and event handlers.
-   **Centralized Error Logging**: Implement a centralized error logging service to track errors and improve application stability.

## 6. Performance Considerations

### 6.1 Optimization Techniques

-   **Memoization**: Use `React.memo`, `useMemo`, and `useCallback` to prevent unnecessary re-renders and recalculations.
-   **Virtualization**: Use libraries like `react-window` or `react-virtualized` to efficiently render large lists or tables.
-   **Debouncing/Throttling**: Limit the rate at which functions are executed (e.g., in input fields).
-   **Code Splitting**: Load code on demand using `React.lazy` and `Suspense`.

### 6.2 Memory Management

-   **Avoid Memory Leaks**: Clean up event listeners, timers, and subscriptions in `useEffect`'s cleanup function.
-   **Release Unused Objects**: Avoid holding onto large objects in memory when they are no longer needed.
-   **Garbage Collection**: Understand how JavaScript's garbage collection works and avoid creating unnecessary objects.

### 6.3 Rendering Optimization

-   **Minimize State Updates**: Avoid unnecessary state updates that trigger re-renders.
-   **Batch Updates**: Batch multiple state updates into a single update using `ReactDOM.unstable_batchedUpdates`.
-   **Keys**: Ensure that keys are unique and consistent across renders.

### 6.4 Bundle Size Optimization

-   **Tree Shaking**: Remove unused code during the build process.
-   **Minification**: Reduce the size of JavaScript and CSS files.
-   **Image Optimization**: Compress and optimize images to reduce file size.
-   **Dependency Analysis**: Use tools like `webpack-bundle-analyzer` to identify large dependencies.

### 6.5 Lazy Loading Strategies

-   **Route-Based Lazy Loading**: Load components when a user navigates to a specific route.
-   **Component-Based Lazy Loading**: Load components when they are about to be rendered.
-   **Intersection Observer**: Load components when they become visible in the viewport.

## 7. Security Best Practices

### 7.1 Common Vulnerabilities and Prevention

-   **Cross-Site Scripting (XSS)**: Sanitize user input to prevent malicious code injection.
-   **Cross-Site Request Forgery (CSRF)**: Use anti-CSRF tokens to protect against unauthorized requests.
-   **Denial of Service (DoS)**: Implement rate limiting and request validation to prevent abuse.
-   **Injection Attacks**: Avoid directly embedding user input into database queries or system commands.

### 7.2 Input Validation

-   **Client-Side Validation**: Validate user input in the browser to provide immediate feedback.
-   **Server-Side Validation**: Always validate user input on the server to prevent malicious data.
-   **Sanitize Input**: Sanitize user input to remove potentially harmful characters or code.

### 7.3 Authentication and Authorization

-   **Secure Authentication**: Use secure authentication mechanisms like OAuth 2.0 or JWT.
-   **Role-Based Access Control (RBAC)**: Implement RBAC to control access to resources based on user roles.
-   **Multi-Factor Authentication (MFA)**: Enable MFA to add an extra layer of security.

### 7.4 Data Protection Strategies

-   **Encryption**: Encrypt sensitive data at rest and in transit.
-   **Data Masking**: Mask sensitive data in logs and UI displays.
-   **Regular Backups**: Create regular backups of application data.

### 7.5 Secure API Communication

-   **HTTPS**: Use HTTPS to encrypt communication between the client and the server.
-   **API Keys**: Protect API keys and secrets.
-   **CORS**: Configure Cross-Origin Resource Sharing (CORS) to prevent unauthorized access to APIs.

## 8. Common Pitfalls and Gotchas

### 8.1 Frequent Mistakes

-   **Ignoring Keys in Lists**: Forgetting to provide unique and stable `key` props when rendering lists.
-   **Incorrect State Updates**: Mutating state directly instead of using `setState` or the state updater function.
-   **Missing Dependencies in `useEffect`**: Not including all dependencies in the dependency array of the `useEffect` hook.
-   **Over-Using State**: Storing derived data in state instead of calculating it on demand.

### 8.2 Edge Cases

-   **Asynchronous State Updates**: Handling state updates in asynchronous operations.
-   **Race Conditions**: Preventing race conditions when making multiple API calls.
-   **Handling Errors in Event Handlers**: Properly handling errors in event handlers to prevent crashes.

### 8.3 Version-Specific Issues

-   **React 16 vs. React 17/18**: Understanding differences in lifecycle methods, error handling, and concurrent mode.
-   **Deprecated Features**: Being aware of deprecated features and using recommended alternatives.

### 8.4 Compatibility Concerns

-   **Browser Compatibility**: Ensuring compatibility with different browsers and devices.
-   **Library Compatibility**: Ensuring compatibility between React and other libraries.

### 8.5 Debugging Strategies

-   **React DevTools**: Use React DevTools to inspect component hierarchies, props, and state.
-   **Console Logging**: Use console logging to debug code and track variables.
-   **Breakpoints**: Set breakpoints in the code to step through execution and inspect variables.

## 9. Tooling and Environment

### 9.1 Recommended Development Tools

-   **VS Code**: A popular code editor with excellent React support.
-   **Create React App**: A tool for quickly setting up a new React project.
-   **React DevTools**: A browser extension for inspecting React components.
-   **ESLint**: A linter for enforcing code style and preventing errors.
-   **Prettier**: A code formatter for automatically formatting code.
-   **husky**: for pre-commit hooks for all git development practice
-   **nvmrc**: for node version management
-   **depcheck**: for unused dependencies


### 9.2 Build Configuration

-   **Vite**: Vite to bundle and optimize code.
-   **Environment Variables**: Use environment variables to configure different environments.

### 9.3 Linting and Formatting

-   **ESLint**: Configure ESLint with recommended React rules.
-   **Prettier**: Configure Prettier to automatically format code.
-   **Husky/lint-staged**: Use Husky and lint-staged to run linters and formatters before committing code.

By following these best practices, React developers can build high-quality, maintainable, and scalable applications that meet the demands of modern web development. Continual education and adaptation to emerging trends in the React ecosystem are crucial for sustained success.