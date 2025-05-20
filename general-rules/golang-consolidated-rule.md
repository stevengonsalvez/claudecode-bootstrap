---
description: Opinionated Go Development Standards for Robust and Idiomatic Code
globs:
  - "**/*.go"
  - "!**/*_test.go" # Some rules might differ or be less strict for test files
alwaysApply: true
---
# Go Development: Opinionated Standards

These rules promote idiomatic, maintainable, and robust Go code, focusing on conventions and patterns that enhance clarity and prevent common pitfalls. Assumes `gofmt` and basic `golint`/`staticcheck` usage.

<rule>
name: go_error_handling_wrap_or_return_as_is
description: Errors returned from other functions should either be returned as-is or wrapped with `fmt.Errorf` using `%w` for context, preserving the original error. Avoid shadowing or losing original error types.
filters:
  - type: file_extension
    pattern: "\\.go$"
  - type: content # Looks for error handling
    pattern: "if\\s+err\\s*!=\\s*nil\\s*{"
actions:
  - type: reject
    conditions:
      # Rejects creating a new error without wrapping when an error already exists
      - pattern: "if\\s+(\\w+,\\s*)?err\\s*!=\\s*nil\\s*{\\s*return\\s+([^%w]*fmt\\.Errorf\\([^%]*%[^w][^)]*\\)|errors\\.New\\([^)]*\\))\\s*(, err\\b)?"
        message: "When handling a non-nil error `err`, either return `err` directly or wrap it using `fmt.Errorf(\"context: %w\", err)`. Do not create a new error with `errors.New` or `fmt.Errorf` without `%w` if `err` is available."
  - type: suggest
    message: |
      Proper error handling is crucial in Go. When an error is returned from a called function:
      1. **Return as-is:** If no additional context is needed at this level, return the error directly: `return nil, err`
      2. **Wrap with context:** If you need to add context, use `fmt.Errorf` with the `%w` verb to wrap the original error: `return nil, fmt.Errorf("my_operation: failed to call X: %w", err)`
      This allows `errors.Is` and `errors.As` to work correctly up the call stack. Avoid simply returning `errors.New("something went wrong")` when you have an underlying error.
examples:
  - input: |
      // if err != nil { return errors.New("failed") }
    output: "Rejected: When handling a non-nil error `err`, either return `err` directly or wrap it using `fmt.Errorf(\"context: %w\", err)`..."
  - input: |
      // if err != nil { return fmt.Errorf("failed: %v", err) } // %v loses type
    output: "Rejected: When handling a non-nil error `err`, either return `err` directly or wrap it using `fmt.Errorf(\"context: %w\", err)`..."
  - input: |
      // if err != nil { return fmt.Errorf("context: %w", err) }
    output: "Accepted"
metadata:
  priority: critical
  version: 1.0
</rule>

<rule>
name: go_receiver_names_consistency_and_shortness
description: Receiver names for a given type should be consistent, short (often one or two letters), and preferably the first letter(s) of the type.
filters:
  - type: file_extension
    pattern: "\\.go$"
  - type: content
    pattern: "func\\s*\\(\\s*\\w+\\s+\\*?\\w+\\s*\\)" # Matches method definitions
actions:
  - type: suggest # Hard to enforce perfect consistency with regex alone across a whole type
    conditions:
      # Looks for receiver names like 'this', 'self', or overly long names.
      - pattern: "func\\s*\\(\\s*(this|self|me|current|instance|[a-zA-Z]{3,})\\s+\\*?(\\w+)\\s*\\)"
        message: "Receiver names should be short (e.g., 's' for 'MyStruct S', 'c' for 'Client C') and consistent for all methods of a given type. Avoid 'this', 'self', or overly descriptive receiver names."
    message: |
      In Go, receiver names for methods should be:
      - **Short:** Often just one or two letters. For `type MyStruct struct{}`, `(s *MyStruct)` or `(ms *MyStruct)` is common.
      - **Consistent:** Use the same receiver name for all methods on a given type.
      - **Not `this` or `self`:** These are not idiomatic Go.
      The name should be representative of the type, typically derived from the type name itself.
examples:
  - input: |
      // func (myService *MyService) DoWork() {}
    output: "Suggested: Receiver names should be short... (e.g., 's' for 'MyStruct S')..."
  - input: |
      // type User struct{}; func (u *User) GetName() string; func (usr *User) SetName(n string) {}
    output: "Suggested: Receiver names should be ... consistent for all methods of a given type. (Found 'u' and 'usr' for User)"
  - input: |
      // type Client struct{}; func (c *Client) Fetch() {}
    output: "Accepted"
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: go_interface_naming_er_suffix
description: Interface names should typically end with "er" (e.g., `Reader`, `Writer`, `Logger`) if they define a single primary method. For broader interfaces, choose a descriptive noun.
filters:
  - type: file_extension
    pattern: "\\.go$"
  - type: content
    pattern: "type\\s+\\w+\\s+interface\\s*{"
actions:
  - type: suggest
    conditions:
      # Interface with one method not ending in 'er' or related.
      - pattern: "type\\s+(\\w+)(?<!er|or|able|Interface)\\s+interface\\s*{\\s*\\w+\\([^)]*\\)(\\s*\\w+)?\\s*}"
        message: "Single-method interfaces are often named by the method name plus an 'er' suffix (e.g., `type Reader interface { Read(p []byte) (n int, err error) }`). If not single-method or 'er' doesn't fit, use a descriptive noun."
    message: |
      Go interface naming conventions:
      - For interfaces with a single primary method, the name is often the method name with an "er" suffix (e.g., `Reader` for `Read`, `Writer` for `Write`, `Stringer` for `String`).
      - For interfaces with multiple methods or where "er" doesn't make sense, use a descriptive noun (e.g., `http.Handler`, `sql.DB`).
      - Avoid prefixing with `I` (e.g., `IUserService`).
examples:
  - input: |
      // type PerformAction interface { DoAction() }
    output: "Suggested: Single-method interfaces are often named by the method name plus an 'er' suffix (e.g., `type ActionPerformer interface { DoAction() }` or `type Actor interface { Act() }`)"
  - input: |
      // type DataProcessor interface { Process([]byte) error; Validate([]byte) bool }
    output: "Accepted (Multi-method, descriptive noun)"
  - input: |
      // type StringMaker interface { MakeString() string }
    output: "Accepted"
metadata:
  priority: low # Style preference, but strong Go idiom
  version: 1.0
</rule>

<rule>
name: go_avoid_naked_returns_in_longer_functions
description: Avoid using naked returns (returns without explicit variable names) in functions longer than a few lines, as they can harm readability.
filters:
  - type: file_extension
    pattern: "\\.go$"
  - type: content
    pattern: "func\\s*\\w*\\s*\\([^)]*\\)\\s*\\(([^)]+,\\s*)*[^)]+\\)\\s*{" # Matches named return parameters
actions:
  - type: reject
    conditions:
      # Matches a function with named returns AND a `return` statement without arguments AND more than ~5 lines.
      # Line count is a heuristic; more complex parsing would be needed for accuracy.
      - pattern: "(func\\s*\\w*\\s*\\([^)]*\\)\\s*\\(([^)]+,\\s*)*[^)]+\\)\\s*{(\\s*\\S+[^\n]*\n){5,}[^}]*return\\s*(\n|//|})[^}]*})"
        message: "Avoid naked returns in functions longer than a few lines. Explicitly returning named result parameters improves clarity. E.g., `return result, err` instead of just `return`."
  - type: suggest
    message: |
      While Go allows named return parameters and "naked" returns (e.g., `func foo() (s string, err error) { s = "bar"; return }`),
      using naked returns in functions longer than a few lines can make it harder to see what values are actually being returned.
      For clarity, prefer explicitly stating the return values: `return s, err`.
      Naked returns are acceptable for very short functions where the association is obvious.
examples:
  - input: |
      // func process() (out string, err error) {
      //   // 5 lines of code...
      //   if problem { err = errors.New("oops"); return }
      //   out = "done"
      //   return
      // }
    output: "Rejected: Avoid naked returns in functions longer than a few lines..."
  - input: |
      // func short() (s string) { s = "hi"; return }
    output: "Accepted (very short function)"
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: go_pass_pointers_for_large_structs_or_modification
description: Pass structs by pointer if they are large or if the function needs to modify them. Pass by value for small, immutable structs.
filters:
  - type: file_extension
    pattern: "\\.go$"
  - type: content
    pattern: "func\\s*\\w*\\s*\\([^)]*\\w+\\s+(?!\\*\\w|map|chan|func|interface|[]byte)\\w+[^)]*\\)" # Heuristic: func with non-pointer, non-primitive struct arg
actions:
  - type: suggest # Size is subjective and hard to check with regex
    conditions:
      # This is a very rough heuristic. It tries to find functions taking a struct by value
      # where the struct name doesn't suggest it's a very small, simple type.
      - pattern: "func\\s+\\w+\\s*\\([^)]*\\w+\\s+(?!\\*\\w|Config|Options|Params|Key|ID|bool|int|string|float)(?!(map|chan|func|interface{.*}|[]byte)\\b)\\b([A-Z]\\w+)\\b[^)]*\\)"
        message: "Consider passing large structs or structs that need modification by pointer (e.g., `func process(s *MyStruct)`). Small, immutable structs can be passed by value. Evaluate based on size and mutability needs."
    message: |
      When passing structs to functions:
      - **Pass by Pointer (`*MyStruct`):**
        - If the struct is large, to avoid copying costs.
        - If the function needs to modify the original struct.
      - **Pass by Value (`MyStruct`):**
        - For small structs where copying is cheap.
        - If the function should operate on a copy and not affect the original (immutability).
      Slices, maps, channels, functions, and interface values are reference types and typically don't need to be passed as pointers to be modified (though pointers to slices/maps are used for specific reasons like re-slicing or replacing the map itself).
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: go_context_first_parameter
description: Functions that accept a `context.Context` should accept it as their first parameter, conventionally named `ctx`.
filters:
  - type: file_extension
    pattern: "\\.go$"
  - type: content
    pattern: "context\\.Context"
actions:
  - type: reject
    conditions:
      # Matches func(..., ctx context.Context) or func(..., c context.Context)
      - pattern: "func\\s*\\w*\\s*\\([^)]*,\\s*\\w+\\s+context\\.Context\\s*\\)"
        message: "`context.Context` should be the first parameter to a function, conventionally named `ctx`. E.g., `func DoSomething(ctx context.Context, arg1 string)`."
      # Matches func(firstArg string, myContext context.Context) -- wrong name
      - pattern: "func\\s*\\w*\\s*\\(\\s*(?!ctx\\b)\\w+\\s+context\\.Context"
        message: "When `context.Context` is the first parameter, it should be conventionally named `ctx`. E.g., `func DoSomething(ctx context.Context, ...)`."
  - type: suggest
    message: |
      If a function needs to be cancellable, carry deadlines, or pass request-scoped values, it should accept a `context.Context`.
      By convention, the `context.Context` should be:
      - The **first** parameter of the function.
      - Named `ctx`.
      Example: `func ProcessData(ctx context.Context, userID string) error`
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: go_avoid_init_functions_for_complex_setup
description: Avoid using `init()` functions for complex logic, error handling, or setup that might fail. Prefer explicit initialization functions or dependency injection.
filters:
  - type: file_extension
    pattern: "\\.go$"
  - type: content
    pattern: "func\\s+init\\s*\\(\\s*\\)\\s*{"
actions:
  - type: suggest # `init` has valid uses, but complex logic is an anti-pattern
    conditions:
      # Heuristic: init() with more than a few lines or calls to functions that might return errors.
      - pattern: "func\\s+init\\s*\\(\\s*\\)\\s*{(\\s*\\S+[^\n]*\n){3,}|err\\s*:=\\s*|panic\\("
        message: "Avoid complex logic, error handling, or fallible operations within `init()` functions. `init()` cannot return errors and panics are hard to recover from. Prefer explicit `New...()` or setup functions that can return errors."
    message: |
      `init()` functions are executed automatically when a package is initialized. While useful for simple setup (e.g., initializing package-level variables with constants), avoid:
      - Complex logic or lengthy computations.
      - Operations that can fail and require error handling (since `init()` cannot return an error). Relying on `panic` in `init` is generally discouraged for library code.
      - Dependencies on external state or services that might not be ready.
      For more involved setup, create an explicit initialization function (e.g., `NewMyService() (*MyService, error)`) that can return an error and be called when appropriate.
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: go_package_naming_short_lowercase_singleword
description: Package names should be short, lowercase, and ideally single words. Avoid underscores or camelCase.
filters:
  - type: file_path # This rule applies to directory names which are package names
    pattern: ".*/[^/]*(_|([A-Z][a-z]))[^/]*$" # Matches directories with underscores or camelCase
  - type: file_extension # Only check actual Go package directories
    pattern: ".*\\.go$"
actions:
  - type: reject # This is a strong convention
    message: "Package names (directory names containing .go files) should be short, lowercase, single words (e.g., 'http', 'customjson', 'user'). Avoid underscores (`_`) or camelCase (`myPackage`). The path part, not the filename."
  - type: suggest
    message: |
      Go package names are derived from the directory they reside in. They should be:
      - **Short:** Easy to type and remember.
      - **Lowercase:** All lowercase letters.
      - **Single Word (ideally):** If multiple words are needed, concatenate them (e.g., `customjson`) rather than using underscores or camel case.
      - **Descriptive:** Clearly indicate the package's purpose.
      Example: A package for utility functions related to HTTP might be in a directory named `httputil`.
metadata:
  priority: high
  version: 1.0
  note: "This rule is best enforced by a linter that checks directory names against package declarations, as regex on file_path is a heuristic."
</rule>