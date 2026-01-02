---
description: TypeScript Model Context Protocol (MCP) Server Development Standards
globs:
  - "**/*.ts" # Apply to all TypeScript files
  - "!**/*.d.ts" # Exclude declaration files
  - "!**/node_modules/**" # Exclude node_modules
alwaysApply: true
---
# TypeScript MCP Server Standards

Rules for building robust, secure, and compliant MCP servers in TypeScript.

<rule>
name: mcp_stdio_no_console_log
description: Prevents direct use of console.log/warn/error to avoid interference with MCP stdout communication.
filters:
  - type: file_extension
    pattern: "\\.ts$"
actions:
  - type: reject
    conditions:
      - pattern: "console\\.(log|warn|error|info|debug)\\("
        message: "MCP servers must not use console.log/warn/error/info/debug directly. MCP uses stdout for protocol messages. Use a dedicated logger configured to write to stderr or a file for debugging and operational logs."
  - type: suggest
    message: |
      For logging in an MCP server, use a dedicated logging library (e.g., Winston, Pino) and configure it to output to `stderr` or a log file.
      `stdout` is reserved for MCP request/response JSON messages.

      Example (conceptual, use a proper logger):
      ```typescript
      // Bad:
      // console.log("Processing request:", request);

      // Good (using a hypothetical logger):
      // import logger from './logger'; // configured to write to stderr
      // logger.info({ message: "Processing request", requestData: request });
      ```
examples:
  - input: |
      // file: tool-server.ts
      console.log("Server starting...");
      function handleRequest(req: any) {
        console.error("An error occurred:", req.error);
        process.stdout.write(JSON.stringify({ id: req.id, error: "..." }));
      }
    output: "Rejected: MCP servers must not use console.log/warn/error/info/debug directly... (for console.log and console.error)"
  - input: |
      // file: tool-server.ts
      // import logger from './custom-logger'; // Assume this logs to stderr
      // logger.info("Server starting...");
      function handleRequest(req: any) {
        // logger.error("An error occurred:", req.error);
        process.stdout.write(JSON.stringify({ id: req.id, error: "..." }));
      }
    output: "Accepted"
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: mcp_describe_method_implementation
description: Ensures tools provide a 'describe' method with comprehensive metadata, including input/output JSON schemas.
filters:
  - type: file_extension
    pattern: "\\.ts$"
  - type: content # Heuristic: look for files that might define tools or MCP handling
    pattern: "(handleRequest|processMessage|RpcHandler|ToolProvider|implements\\s+IMcpTool)"
actions:
  - type: reject
    conditions:
      # This is a heuristic. A more robust check would involve AST parsing.
      # Looks for a common pattern of handling requests without a clear describe method nearby or called.
      - pattern: "async\\s+(invoke|executeTool)\\s*\\([^)]*\\)\\s*:\\s*Promise<[^>]*ToolResponse[^>]*>\\s*{((?!\\.describe\\(|this\\.describe\\(|getToolDescription\\().)*$)"
        message: "MCP tools must expose metadata via a 'describe'-like mechanism. Ensure your server can respond to a 'describe' request detailing all available tools, their purpose, and their input/output JSON schemas."
  - type: suggest
    message: |
      A crucial part of MCP is the `describe` method (or equivalent). It should return a JSON structure detailing:
      - Each tool's `name` (unique identifier).
      - A clear `description` of what the tool does and when to use it (this helps the LLM formulate prompts).
      - `input_schema`: A JSON schema defining the expected input parameters.
      - `output_schema`: A JSON schema defining the structure of the tool's output.

      Example structure for `describe` response for a single tool:
      ```json
      {
        "name": "weather_lookup",
        "description": "Fetches the current weather for a given city. Use this when a user asks about weather conditions.",
        "input_schema": {
          "type": "object",
          "properties": {
            "city": { "type": "string", "description": "The name of the city." }
          },
          "required": ["city"]
        },
        "output_schema": {
          "type": "object",
          "properties": {
            "temperature": { "type": "number" },
            "condition": { "type": "string" },
            "unit": { "type": "string", "enum": ["celsius", "fahrenheit"] }
          }
        }
      }
      ```
      Consider using libraries like Zod to define schemas and automatically generate JSON schemas.
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: mcp_invoke_input_validation
description: Mandates strong input validation for 'invoke' method parameters against the defined input_schema to prevent errors and mitigate injection risks.
filters:
  - type: file_extension
    pattern: "\\.ts$"
  - type: content
    pattern: "async\\s+(invoke|executeTool)\\s*\\(" # Targets invoke/execute methods
actions:
  - type: reject
    conditions:
      # Heuristic: Looks for invoke methods that don't seem to call a validation function early on.
      # This is very hard to get right with regex alone.
      - pattern: "async\\s+(invoke|executeTool)\\s*\\((params|args)[^)]*\\)\\s*:\\s*Promise<[^>]*>\\s*{\\s*(?!.*(validateInput|schema\\.parse|ajv\\.validate)).*"
        message: "The 'invoke' method (or equivalent) must validate incoming parameters against the tool's 'input_schema' *before* processing. Use a JSON schema validator (e.g., Zod, AJV, typia)."
  - type: suggest
    message: |
      Within your tool's `invoke` method, the first step should be to validate the received parameters against the `input_schema` you defined in `describe`.
      This catches malformed requests, provides clear errors, and is a first line of defense against injection attempts.

      Example (using Zod):
      ```typescript
      import { z } from 'zod';

      const MyToolInputSchema = z.object({
        city: z.string().min(1),
        // ... other params
      });

      async function invokeMyTool(params: unknown): Promise<any> {
        try {
          const validatedParams = MyToolInputSchema.parse(params);
          // Now use validatedParams.city safely
          // ... tool logic ...
        } catch (error) {
          if (error instanceof z.ZodError) {
            // Return a structured MCP error
            return { error: { code: -32602, message: "Invalid params", data: error.format() }};
          }
          // Handle other errors
          return { error: { code: -32000, message: "Tool execution error" }};
        }
      }
      ```
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: mcp_tool_poisoning_injection_prevention
description: Warns against unsafe practices when handling tool inputs, such as direct shell execution or eval, to prevent tool poisoning or command injection.
filters:
  - type: file_extension
    pattern: "\\.ts$"
  - type: content
    pattern: "async\\s+(invoke|executeTool)\\s*\\("
actions:
  - type: reject
    conditions:
      - pattern: "(eval\\(|new Function\\(|child_process\\.(exec|execSync|spawn)\\(.*(params|args|input)\\w*\\))" # Simplified, checks for eval or exec with dynamic input
        message: "Avoid using 'eval()', 'new Function()', or directly passing unvalidated/unsanitized tool parameters to shell commands (e.g., child_process.exec) to prevent code/command injection."
      - pattern: "dangerouslySetInnerHTML" # If tools generate HTML
        message: "If tools generate HTML, avoid using 'dangerouslySetInnerHTML' or similar. Sanitize output or use safe templating."
  - type: suggest
    message: |
      Tool inputs from an LLM (or any external source) should be treated as untrusted.
      - **NEVER** pass raw inputs directly to `eval()`, `new Function()`, `child_process.exec()`, or into SQL queries without proper sanitization or parameterization.
      - Prefer using SDKs or well-defined APIs to interact with external services rather than constructing shell commands.
      - If a tool needs to operate on file paths, ensure paths are constrained and validated.
      - All inputs should be strictly validated against their schemas (see `mcp_invoke_input_validation`).
metadata:
  priority: critical
  version: 1.0
</rule>

<rule>
name: mcp_api_key_management
description: Prohibits hardcoding API keys or secrets; advocates for environment variables or secure secret management.
filters:
  - type: file_extension
    pattern: "\\.ts$"
actions:
  - type: reject
    conditions:
      # Common API key patterns. Add more specific ones if needed.
      - pattern: "(API_KEY|SECRET_KEY|ACCESS_TOKEN)\\s*[:=]\\s*['\"](sk-|rk_live|pk_test|ghp_|glpat-|[A-Za-z0-9\\-_\\.+]{20,})['\"]"
        message: "API keys or secrets must not be hardcoded. Use environment variables (e.g., `process.env.MY_API_KEY`) or a dedicated secrets management system."
  - type: suggest
    message: |
      Store API keys, database credentials, and other secrets in environment variables or a secure secrets manager (e.g., HashiCorp Vault, AWS Secrets Manager).
      Never commit them to version control. Access them in your code via `process.env`.

      Example:
      ```typescript
      // Good:
      const apiKey = process.env.THIRD_PARTY_API_KEY;
      if (!apiKey) {
        throw new Error("THIRD_PARTY_API_KEY environment variable not set.");
      }

      // Bad:
      // const apiKey = "your_hardcoded_api_key_here";
      ```
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: mcp_standardized_error_responses
description: Encourages MCP servers to return errors in a standardized JSON-RPC like format.
filters:
  - type: file_extension
    pattern: "\\.ts$"
  - type: content
    pattern: "(process\\.stdout\\.write\\(|return\\s*\\{\\s*error)" # Heuristic for sending responses
actions:
  - type: suggest # Hard to strictly enforce structure with regex
    conditions:
      - pattern: "throw new Error\\(" # When simple errors are thrown, they might not be caught and formatted correctly for MCP
        message: "Ensure all errors, including unexpected ones, are caught and transformed into the MCP JSON error response format before writing to stdout."
    message: |
      When an error occurs (e.g., invalid parameters, tool execution failure), the MCP server should respond with a JSON object containing an `error` field.
      This `error` object should ideally follow JSON-RPC error object conventions:
      - `code`: A number indicating the error type (e.g., -32602 for Invalid params, -32000 for server/tool error).
      - `message`: A string providing a short description of the error.
      - `data` (optional): A primitive or structured value containing additional information about the error.

      Example error response:
      ```json
      {
        "id": "request_id_123", // Should match the request ID
        "error": {
          "code": -32602,
          "message": "Invalid params: 'city' field is required.",
          "data": { "field": "city", "issue": "required" }
        }
      }
      ```
      Implement a global error handler or try/catch blocks in your request processing logic to ensure all responses adhere to this.
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: mcp_tool_description_for_llm_prompting
description: Tool descriptions in 'describe' should be clear to guide LLMs on tool usage and assist in formulating effective prompts/invocations.
filters:
  - type: file_extension
    pattern: "\\.ts$"
  - type: content
    pattern: "(describe|getToolDescription)\\s*\\(" # Look for describe methods
actions:
  - type: suggest
    message: |
      The `description` field for each tool (returned by the `describe` method) is critical for the LLM client. It's not just for humans; it's a "prompt" for the LLM to understand:
      1.  **What the tool does**: Be specific and outcome-oriented.
      2.  **When to use it**: Provide context or example scenarios. E.g., "Use this tool when the user asks for the current stock price of a publicly traded company."
      3.  **Key inputs (briefly)**: While the `input_schema` has details, a brief mention in the description helps. E.g., "...requires a company stock ticker symbol."
      4.  **What it returns (briefly)**: E.g., "...returns the current price and daily change."

      A well-crafted description significantly improves the LLM's ability to correctly choose and use your tool. Avoid vague or overly technical descriptions.
metadata:
  priority: medium
  version: 1.0
</rule>