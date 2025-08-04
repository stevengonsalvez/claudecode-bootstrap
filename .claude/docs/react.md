--- START OF FILE react.md ---

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
  │   │   ├── Button.tsx
  │   │   ├── Button.module.css
  │   │   └── Button.test.tsx
  │   ├── Input/
  │   │   ├── Input.tsx
  │   │   ├── Input.module.css
  │   │   └── Input.test.tsx
  │   └── ...
  ├── contexts/
  │   ├── AuthContext.tsx
  │   └── ThemeContext.tsx
  ├── hooks/
  │   ├── useAuth.ts
  │   └── useTheme.ts
  ├── pages/
  │   ├── Home.tsx
  │   ├── About.tsx
  │   └── ...
  ├── services/
  │   ├── api.ts
  │   └── auth.ts
  ├── utils/
  │   ├── helpers.ts
  │   └── validators.ts
  ├── App.tsx
  ├── index.tsx
  └── ...


-   **`components/`**: Reusable UI components.
    -   Each component has its own directory containing the component file, associated styles (using CSS modules), and tests.
-   **`contexts/`**: React context providers.
-   **`hooks/`**: Custom React hooks.
-   **`pages/`**: Top-level components representing different routes or views.
-   **`services/`**: API interaction logic.
-   **`utils/`**: Utility functions.

### 1.2 Naming Conventions

-   **Files**:
    -   **React Components**: Use PascalCase (e.g., `MyComponent.tsx`).
    -   **Hooks**: Use camelCase prefixed with `use` (e.g., `useMyHook.ts`).
    -   **Contexts**: Use PascalCase suffixed with `Context` (e.g., `MyContext.tsx`).
    -   **General TypeScript Files**: Use kebab-case for other modules like services or utils (e.g., `api-service.ts`, `string-utils.ts`).
    -   **Test Files**: Use `*.test.ts`, `*.spec.ts`, or `*.test.tsx` (e.g., `Button.test.tsx`).
    -   **CSS Modules**: Use `.module.css` or `.module.scss` (e.g., `Button.module.css`).
-   **Code**:
    -   **Types/Interfaces**: Use `PascalCase` (e.g., `PaymentRequest`, `UserProfile`).
    -   **Functions**: Use `camelCase` and make them verb-based (e.g., `calculateTotal`, `validatePayment`).
    -   **Variables**: Use `camelCase`.
    -   **Constants**: Use `UPPER_SNAKE_CASE` for true, immutable constants (e.g., `PREMIUM_DISCOUNT_MULTIPLIER`). Use `camelCase` for module-level configuration that doesn't change at runtime.

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

## 2. TypeScript Guidelines

### 2.1 Strict Mode Requirements

```json
// tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "strictFunctionTypes": true,
    "strictBindCallApply": true,
    "strictPropertyInitialization": true,
    "noImplicitThis": true,
    "alwaysStrict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": true
  }
}
```

- **No `any`** - ever. Use `unknown` if type is truly unknown.
- **No type assertions** (`as SomeType`) unless absolutely necessary with clear justification.
- **No `@ts-ignore`** or `@ts-expect-error` without explicit explanation.
- These rules apply to test code as well as production code.

### 2.2 Type Definitions

- **Prefer `type` over `interface`** in all cases.
- Use explicit typing where it aids clarity, but leverage inference where appropriate.
- Utilize utility types effectively (`Pick`, `Omit`, `Partial`, `Required`, etc.).
- Create domain-specific types (e.g., `UserId`, `PaymentId`) for type safety.
- Use Zod or any other [Standard Schema](https://standardschema.dev/) compliant schema library to create types, by creating schemas first.

```typescript
// Good
type UserId = string & { readonly brand: unique symbol };
type PaymentAmount = number & { readonly brand: unique symbol };

// Avoid
type UserId = string;
type PaymentAmount = number;
```

#### 2.2.1 Schema-First Development with Zod

Always define your schemas first, then derive types from them:

```typescript
import { z } from "zod";

// Define schemas first - these provide runtime validation
const AddressDetailsSchema = z.object({
  houseNumber: z.string(),
  houseName: z.string().optional(),
  addressLine1: z.string().min(1),
  addressLine2: z.string().optional(),
  city: z.string().min(1),
  postcode: z.string().regex(/^[A-Z]{1,2}\d[A-Z\d]? ?\d[A-Z]{2}$/i),
});

const PayingCardDetailsSchema = z.object({
  cvv: z.string().regex(/^\d{3,4}$/),
  token: z.string().min(1),
});

const PostPaymentsRequestV3Schema = z.object({
  cardAccountId: z.string().length(16),
  amount: z.number().positive(),
  source: z.enum(["Web", "Mobile", "API"]),
  accountStatus: z.enum(["Normal", "Restricted", "Closed"]),
  lastName: z.string().min(1),
  dateOfBirth: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  payingCardDetails: PayingCardDetailsSchema,
  addressDetails: AddressDetailsSchema,
  brand: z.enum(["Visa", "Mastercard", "Amex"]),
});

// Derive types from schemas
type AddressDetails = z.infer<typeof AddressDetailsSchema>;
type PayingCardDetails = z.infer<typeof PayingCardDetailsSchema>;
type PostPaymentsRequestV3 = z.infer<typeof PostPaymentsRequestV3Schema>;

// Use schemas at runtime boundaries
export const parsePaymentRequest = (data: unknown): PostPaymentsRequestV3 => {
  return PostPaymentsRequestV3Schema.parse(data);
};

// Example of schema composition for complex domains
const BaseEntitySchema = z.object({
  id: z.string().uuid(),
  createdAt: z.date(),
  updatedAt: z.date(),
});

const CustomerSchema = BaseEntitySchema.extend({
  email: z.string().email(),
  tier: z.enum(["standard", "premium", "enterprise"]),
  creditLimit: z.number().positive(),
});

type Customer = z.infer<typeof CustomerSchema>;
```

### 2.3 Schema Usage in Tests

**CRITICAL**: Tests must use real schemas and types from the main project, not redefine their own.

```typescript
// ❌ WRONG - Defining schemas in test files
const ProjectSchema = z.object({
  id: z.string(),
  workspaceId: z.string(),
  ownerId: z.string().nullable(),
  name: z.string(),
  createdAt: z.coerce.date(),
  updatedAt: z.coerce.date(),
});

// ✅ CORRECT - Import schemas from the shared schema package
import { ProjectSchema, type Project } from "@your-org/schemas";
```

**Why this matters:**

-   **Type Safety**: Ensures tests use the same types as production code
-   **Consistency**: Changes to schemas automatically propagate to tests
-   **Maintainability**: Single source of truth for data structures
-   **Prevents Drift**: Tests can't accidentally diverge from real schemas

**Implementation:**

-   All domain schemas should be exported from a shared schema package or module
-   Test files should import schemas from the shared location
-   If a schema isn't exported yet, add it to the exports rather than duplicating it
-   Mock data factories should use the real types derived from real schemas

```typescript
// ✅ CORRECT - Test factories using real schemas
import { ProjectSchema, type Project } from "@your-org/schemas";

const getMockProject = (overrides?: Partial<Project>): Project => {
  const baseProject = {
    id: "proj_123",
    workspaceId: "ws_456",
    ownerId: "user_789",
    name: "Test Project",
    createdAt: new Date(),
    updatedAt: new Date(),
  };

  const projectData = { ...baseProject, ...overrides };

  // Validate against real schema to catch type mismatches
  return ProjectSchema.parse(projectData);
};
```

## 3. Code Style

### 3.1 Functional Programming

We follow a "functional light" approach:

-   **No data mutation** - work with immutable data structures.
-   **Pure functions** wherever possible.
-   **Composition** as the primary mechanism for code reuse.
-   Avoid heavy FP abstractions (no need for complex monads or pipe/compose patterns) unless there is a clear advantage to using them.
-   Use array methods (`map`, `filter`, `reduce`) over imperative loops.

#### Examples of Functional Patterns

```typescript
// Good - Pure function with immutable updates
const applyDiscount = (order: Order, discountPercent: number): Order => {
  return {
    ...order,
    items: order.items.map((item) => ({
      ...item,
      price: item.price * (1 - discountPercent / 100),
    })),
    totalPrice: order.items.reduce(
      (sum, item) => sum + item.price * (1 - discountPercent / 100),
      0
    ),
  };
};

// Good - Composition over complex logic
const processOrder = (order: Order): ProcessedOrder => {
  return pipe(
    order,
    validateOrder,
    applyPromotions,
    calculateTax,
    assignWarehouse
  );
};

// When heavy FP abstractions ARE appropriate:
// - Complex async flows that benefit from Task/IO types
// - Error handling chains that benefit from Result/Either types
// Example with Result type for complex error handling:
type Result<T, E = Error> =
  | { success: true; data: T }
  | { success: false; error: E };

const chainPaymentOperations = (
  payment: Payment
): Result<Receipt, PaymentError> => {
  return pipe(
    validatePayment(payment),
    chain(authorizePayment),
    chain(capturePayment),
    map(generateReceipt)
  );
};
```

### 3.2 Code Structure

-   **No nested if/else statements** - use early returns, guard clauses, or composition.
-   **Avoid deep nesting** in general (max 2 levels).
-   Keep functions small and focused on a single responsibility.
-   Prefer flat, readable code over clever abstractions.

### 3.3 No Comments in Code

Code should be self-documenting through clear naming and structure. Comments indicate that the code itself is not clear enough.

```typescript
// Avoid: Comments explaining what the code does
const calculateDiscount = (price: number, customer: Customer): number => {
  // Check if customer is premium
  if (customer.tier === "premium") {
    // Apply 20% discount for premium customers
    return price * 0.8;
  }
  // Regular customers get 10% discount
  return price * 0.9;
};

// Good: Self-documenting code with clear names
const PREMIUM_DISCOUNT_MULTIPLIER = 0.8;
const STANDARD_DISCOUNT_MULTIPLIER = 0.9;

const isPremiumCustomer = (customer: Customer): boolean => {
  return customer.tier === "premium";
};

const calculateDiscount = (price: number, customer: Customer): number => {
  const discountMultiplier = isPremiumCustomer(customer)
    ? PREMIUM_DISCOUNT_MULTIPLIER
    : STANDARD_DISCOUNT_MULTIPLIER;

  return price * discountMultiplier;
};

// Avoid: Complex logic with comments
const processPayment = (payment: Payment): ProcessedPayment => {
  // First validate the payment
  if (!validatePayment(payment)) {
    throw new Error("Invalid payment");
  }

  // Check if we need to apply 3D secure
  if (payment.amount > 100 && payment.card.type === "credit") {
    // Apply 3D secure for credit cards over £100
    const securePayment = apply3DSecure(payment);
    // Process the secure payment
    return executePayment(securePayment);
  }

  // Process the payment
  return executePayment(payment);
};

// Good: Extract to well-named functions
const requires3DSecure = (payment: Payment): boolean => {
  const SECURE_PAYMENT_THRESHOLD = 100;
  return (
    payment.amount > SECURE_PAYMENT_THRESHOLD && payment.card.type === "credit"
  );
};

const processPayment = (payment: Payment): ProcessedPayment => {
  if (!validatePayment(payment)) {
    throw new PaymentValidationError("Invalid payment");
  }

  const securedPayment = requires3DSecure(payment)
    ? apply3DSecure(payment)
    : payment;

  return executePayment(securedPayment);
};
```

**Exception**: JSDoc comments for public APIs are acceptable when generating documentation, but the code should still be self-explanatory without them.

### 3.4 Prefer Options Objects

Use options objects for function parameters as the default pattern. Only use positional parameters when there's a clear, compelling reason (e.g., single-parameter pure functions, well-established conventions like `map(item => item.value)`).

```typescript
// Avoid: Multiple positional parameters
const createPayment = (
  amount: number,
  currency: string,
  cardId: string,
  customerId: string,
  description?: string,
  metadata?: Record<string, unknown>,
  idempotencyKey?: string
): Payment => {
  // implementation
};

// Calling it is unclear
const payment = createPayment(
  100,
  "GBP",
  "card_123",
  "cust_456",
  undefined,
  { orderId: "order_789" },
  "key_123"
);

// Good: Options object with clear property names
type CreatePaymentOptions = {
  amount: number;
  currency: string;
  cardId: string;
  customerId: string;
  description?: string;
  metadata?: Record<string, unknown>;
  idempotencyKey?: string;
};

const createPayment = (options: CreatePaymentOptions): Payment => {
  const {
    amount,
    currency,
    cardId,
    customerId,
    description,
    metadata,
    idempotencyKey,
  } = options;

  // implementation
};

// Clear and readable at call site
const payment = createPayment({
  amount: 100,
  currency: "GBP",
  cardId: "card_123",
  customerId: "cust_456",
  metadata: { orderId: "order_789" },
  idempotencyKey: "key_123",
});

// Avoid: Boolean flags as parameters
const fetchCustomers = (
  includeInactive: boolean,
  includePending: boolean,
  includeDeleted: boolean,
  sortByDate: boolean
): Customer[] => {
  // implementation
};

// Confusing at call site
const customers = fetchCustomers(true, false, false, true);

// Good: Options object with clear intent
type FetchCustomersOptions = {
  includeInactive?: boolean;
  includePending?: boolean;
  includeDeleted?: boolean;
  sortBy?: "date" | "name" | "value";
};

const fetchCustomers = (options: FetchCustomersOptions = {}): Customer[] => {
  const {
    includeInactive = false,
    includePending = false,
    includeDeleted = false,
    sortBy = "name",
  } = options;

  // implementation
};

// Self-documenting at call site
const customers = fetchCustomers({
  includeInactive: true,
  sortBy: "date",
});
```

**Guidelines for options objects:**

-   Default to options objects unless there's a specific reason not to.
-   Always use for functions with optional parameters.
-   Destructure options at the start of the function for clarity.
-   Provide sensible defaults using destructuring.
-   Keep related options grouped (e.g., all shipping options together).

**When positional parameters are acceptable:**

-   Single-parameter pure functions (`const double = (n: number): number => n * 2;`).
-   Well-established functional patterns (map, filter, reduce callbacks).
-   Mathematical operations where order is conventional.

## 4. Routing and Authentication

### 4.1 Route Categories

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

### 4.2 Protected Route Pattern

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

### 4.3 Route Organization in App.tsx

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

### 4.4 Navigation Patterns

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

## 5. Styling with Material-UI (MUI)

### 5.1 Styled Components Pattern (Preferred)

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

### 5.2 sx Prop Pattern

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

### 5.3 Theme Usage

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

### 5.4 Responsive Design Patterns

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

### 5.5 FORBIDDEN Styling Patterns

**NEVER USE**:
- Inline styles: `style={{ color: 'red' }}`
- Hardcoded colors: `sx={{ color: '#ff0000' }}`
- CSS classes for MUI components
- styled-components library (use MUI's styled instead)

## 6. Testing Approaches

### 6.1 Testing Strategy Overview

-   **Unit Testing**: Test individual components in isolation using Vitest + React Testing Library
-   **Integration Testing**: Test component interactions and API integrations
-   **End-to-End Testing**: Test complete user flows using Playwright

### 6.2 Unit Test Structure (Vitest + React Testing Library)

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

### 6.3 E2E Test Structure (Playwright)

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

### 6.4 Mocking Standards

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

### 6.5 Test Organization

- **Co-locate Tests**: Keep test files close to the components they test
- **Descriptive Names**: Use descriptive names for test files and test cases
- **Test Suites**: Organize tests into logical suites
- **Data Isolation**: Each test uses fresh data
- **Clean Up**: Clean up after tests

## 7. Common Patterns and Anti-patterns

### 7.1 Design Patterns

-   **Higher-Order Components (HOCs)**: Reusable logic that wraps components (use with caution; prefer hooks).
-   **Render Props**: Sharing code using a prop whose value is a function.
-   **Compound Components**: Components that work together implicitly (e.g., `Tabs`, `Tab`).
-   **Hooks**: Reusable stateful logic that can be shared across functional components.

### 7.2 Recommended Approaches

-   **Form Handling**: Use controlled components with local state or a form library like Formik or React Hook Form.
-   **API Calls**: Use `useEffect` hook to make API calls and manage loading states.
-   **Conditional Rendering**: Use short-circuit evaluation (`&&`) or ternary operators for simple conditions; use separate components for complex scenarios.
-   **List Rendering**: Always provide a unique and stable `key` prop when rendering lists.

### 7.3 Anti-patterns and Code Smells

-   **Direct DOM Manipulation**: Avoid directly manipulating the DOM; let React handle updates.
-   **Mutating State Directly**: Always use `setState` or the state updater function to modify state.
-   **Inline Styles**: Use CSS modules or styled-components for maintainable styles.
-   **Over-Engineering**: Avoid using complex solutions for simple problems.
-   **Prop Drilling**: Passing props through multiple levels of components without them being used.

### 7.4 State Management Best Practices

-   **Local State**: Use `useState` for component-specific state.
-   **Context API**: Use `useContext` for global state accessible to many components, but avoid for very frequently updated data.
-   **TanStack Query**: Use for server state management and data fetching.
-   **Redux/Mobx**: Use these libraries for complex state management in large applications.
-   **Recoil/Zustand**: Lightweight alternatives to Redux, often easier to set up and use.
-   **Immutable Data**: Treat state as immutable to prevent unexpected side effects.

### 7.5 Error Handling Patterns

-   **Error Boundaries**: Wrap components with error boundaries to catch errors during rendering and prevent crashes.
-   **Try-Catch Blocks**: Use try-catch blocks for handling errors in asynchronous operations and event handlers.
-   **Centralized Error Logging**: Implement a centralized error logging service to track errors and improve application stability.

## 8. Performance Considerations

### 8.1 Optimization Techniques

-   **Memoization**: Use `React.memo`, `useMemo`, and `useCallback` to prevent unnecessary re-renders and recalculations.
-   **Virtualization**: Use libraries like `react-window` or `react-virtualized` to efficiently render large lists or tables.
-   **Debouncing/Throttling**: Limit the rate at which functions are executed (e.g., in input fields).
-   **Code Splitting**: Load code on demand using `React.lazy` and `Suspense`.

### 8.2 Memory Management

-   **Avoid Memory Leaks**: Clean up event listeners, timers, and subscriptions in `useEffect`'s cleanup function.
-   **Release Unused Objects**: Avoid holding onto large objects in memory when they are no longer needed.
-   **Garbage Collection**: Understand how JavaScript's garbage collection works and avoid creating unnecessary objects.

### 8.3 Rendering Optimization

-   **Minimize State Updates**: Avoid unnecessary state updates that trigger re-renders.
-   **Batch Updates**: Batch multiple state updates into a single update using `ReactDOM.unstable_batchedUpdates`.
-   **Keys**: Ensure that keys are unique and consistent across renders.

### 8.4 Bundle Size Optimization

-   **Tree Shaking**: Remove unused code during the build process.
-   **Minification**: Reduce the size of JavaScript and CSS files.
-   **Image Optimization**: Compress and optimize images to reduce file size.
-   **Dependency Analysis**: Use tools like `webpack-bundle-analyzer` to identify large dependencies.

### 8.5 Lazy Loading Strategies

-   **Route-Based Lazy Loading**: Load components when a user navigates to a specific route.
-   **Component-Based Lazy Loading**: Load components when they are about to be rendered.
-   **Intersection Observer**: Load components when they become visible in the viewport.

## 9. Security Best Practices

### 9.1 Common Vulnerabilities and Prevention

-   **Cross-Site Scripting (XSS)**: Sanitize user input to prevent malicious code injection.
-   **Cross-Site Request Forgery (CSRF)**: Use anti-CSRF tokens to protect against unauthorized requests.
-   **Denial of Service (DoS)**: Implement rate limiting and request validation to prevent abuse.
-   **Injection Attacks**: Avoid directly embedding user input into database queries or system commands.

### 9.2 Input Validation

-   **Client-Side Validation**: Validate user input in the browser to provide immediate feedback.
-   **Server-Side Validation**: Always validate user input on the server to prevent malicious data.
-   **Sanitize Input**: Sanitize user input to remove potentially harmful characters or code.

### 9.3 Authentication and Authorization

-   **Secure Authentication**: Use secure authentication mechanisms like OAuth 2.0 or JWT.
-   **Role-Based Access Control (RBAC)**: Implement RBAC to control access to resources based on user roles.
-   **Multi-Factor Authentication (MFA)**: Enable MFA to add an extra layer of security.

### 9.4 Data Protection Strategies

-   **Encryption**: Encrypt sensitive data at rest and in transit.
-   **Data Masking**: Mask sensitive data in logs and UI displays.
-   **Regular Backups**: Create regular backups of application data.

### 9.5 Secure API Communication

-   **HTTPS**: Use HTTPS to encrypt communication between the client and the server.
-   **API Keys**: Protect API keys and secrets.
-   **CORS**: Configure Cross-Origin Resource Sharing (CORS) to prevent unauthorized access to APIs.

## 10. Common Pitfalls and Gotchas

### 10.1 Frequent Mistakes

-   **Ignoring Keys in Lists**: Forgetting to provide unique and stable `key` props when rendering lists.
-   **Incorrect State Updates**: Mutating state directly instead of using `setState` or the state updater function.
-   **Missing Dependencies in `useEffect`**: Not including all dependencies in the dependency array of the `useEffect` hook.
-   **Over-Using State**: Storing derived data in state instead of calculating it on demand.

### 10.2 Edge Cases

-   **Asynchronous State Updates**: Handling state updates in asynchronous operations.
-   **Race Conditions**: Preventing race conditions when making multiple API calls.
-   **Handling Errors in Event Handlers**: Properly handling errors in event handlers to prevent crashes.

### 10.3 Version-Specific Issues

-   **React 16 vs. React 17/18**: Understanding differences in lifecycle methods, error handling, and concurrent mode.
-   **Deprecated Features**: Being aware of deprecated features and using recommended alternatives.

### 10.4 Compatibility Concerns

-   **Browser Compatibility**: Ensuring compatibility with different browsers and devices.
-   **Library Compatibility**: Ensuring compatibility between React and other libraries.

### 10.5 Debugging Strategies

-   **React DevTools**: Use React DevTools to inspect component hierarchies, props, and state.
-   **Console Logging**: Use console logging to debug code and track variables.
-   **Breakpoints**: Set breakpoints in the code to step through execution and inspect variables.

## 11. Tooling and Environment

### 11.1 Recommended Development Tools

-   **VS Code**: A popular code editor with excellent React support.
-   **Create React App**: A tool for quickly setting up a new React project.
-   **React DevTools**: A browser extension for inspecting React components.
-   **ESLint**: A linter for enforcing code style and preventing errors.
-   **Prettier**: A code formatter for automatically formatting code.
-   **husky**: for pre-commit hooks for all git development practice
-   **nvmrc**: for node version management
-   **depcheck**: for unused dependencies


### 11.2 Build Configuration

-   **Vite**: Vite to bundle and optimize code.
-   **Environment Variables**: Use environment variables to configure different environments.

### 11.3 Linting and Formatting

-   **ESLint**: Configure ESLint with recommended React rules.
-   **Prettier**: Configure Prettier to automatically format code.
-   **Husky/lint-staged**: Use Husky and lint-staged to run linters and formatters before committing code.

By following these best practices, React developers can build high-quality, maintainable, and scalable applications that meet the demands of modern web development. Continual education and adaptation to emerging trends in the React ecosystem are crucial for sustained success.