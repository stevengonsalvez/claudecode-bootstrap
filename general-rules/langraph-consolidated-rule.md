---
description: LangGraph Development Standards for Structure and Security
globs:
  - "**/*.py" # Apply to all Python files; specific rules will filter further by content
alwaysApply: true
---
# LangGraph Core Development Standards

This ruleset enforces structural integrity and security best practices for LangGraph applications.

<rule>
name: langgraph_typed_state_definition
description: Ensures LangGraph agent state is defined using typed data structures (e.g., Pydantic models or TypedDict) for clarity and safety.
filters:
  - type: file_extension
    pattern: "\\.py$"
  - type: content # Only apply if StateGraph is likely used
    pattern: "StateGraph\\("
actions:
  - type: reject
    conditions:
      - pattern: "StateGraph\\(dict\\)" # Catches direct use of dict for state
        message: "Agent state for StateGraph should be a Pydantic BaseModel or TypedDict, not a raw 'dict'. Define a specific state class."
  - type: suggest
    message: |
      Define agent state using Pydantic BaseModel or typing.TypedDict for clarity, type safety, and easier validation.
      This makes your graph's data flow explicit and robust.

      Example:
      ```python
      from typing import TypedDict
      from pydantic import BaseModel

      # Option 1: TypedDict
      class AgentState(TypedDict):
          input: str
          messages: list
          # ... other state fields

      # Option 2: Pydantic BaseModel
      class AgentStateModel(BaseModel):
          input: str
          messages: list = []
          # ... other state fields

      # Then use it:
      # graph = StateGraph(AgentState) 
      # or
      # graph = StateGraph(AgentStateModel)
      ```
examples:
  - input: |
      # file: my_agent.py
      from langgraph.graph import StateGraph
      graph = StateGraph(dict)
    output: "Rejected: Agent state for StateGraph should be a Pydantic BaseModel or TypedDict, not a raw 'dict'. Define a specific state class."
  - input: |
      # file: my_agent.py
      from langgraph.graph import StateGraph
      from typing import TypedDict
      class MyState(TypedDict):
          query: str
      graph = StateGraph(MyState)
    output: "Accepted"
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: langgraph_node_function_typing
description: Encourages type hints for LangGraph node functions, especially for the state argument and return type.
filters:
  - type: file_extension
    pattern: "\\.py$"
  - type: content # Only apply if add_node is likely used
    pattern: "\\.add_node\\("
actions:
  - type: suggest # Hard to strictly reject without complex AST parsing
    conditions:
      - pattern: "def\\s+\\w+\\s*\\(\\s*\\w+\\s*\\):" # Matches func(state): without type hint for state
        message: "Node functions should have type hints for their state argument and return value for clarity. E.g., `def my_node(state: AgentState) -> dict:` or `async def my_node(state: AgentState) -> AgentState:`."
      - pattern: "def\\s+\\w+\\s*\\(\\s*\\w+\\s*:\\s*\\w+\\s*\\)\\s*:" # Matches func(state: Type): without return type hint
        message: "Node functions should specify a return type hint. E.g., `-> dict` or `-> AgentState` (matching your defined state type)."
    message: |
      Node functions define the core logic of your graph. Using type hints for the state argument (matching your `AgentState` type)
      and the return value (typically a dictionary of state updates or the full state type) significantly improves readability and maintainability.

      Example:
      ```python
      from typing import TypedDict

      class AgentState(TypedDict):
          input: str
          result: str

      def process_data(state: AgentState) -> dict: # Or AgentState if returning full state
          # ... logic ...
          return {"result": "processed " + state["input"]}
      
      # graph.add_node("processor", process_data)
      ```
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: langgraph_conditional_edge_function
description: Recommends using named functions for conditional edge logic instead of complex lambdas for better readability.
filters:
  - type: file_extension
    pattern: "\\.py$"
  - type: content
    pattern: "\\.add_conditional_edges\\("
actions:
  - type: reject
    conditions:
      # This regex looks for lambdas with more than one operation (e.g., multiple `and`/`or`, or complex expressions)
      # It's a heuristic and might need refinement.
      - pattern: "\\.add_conditional_edges\\([^,]+,\\s*lambda[^:]+:\\s*.*(and|or|if.+else|\\{|\\}).*,"
        message: "Complex conditional logic in `add_conditional_edges` should be encapsulated in a named function for readability, not a complex lambda."
  - type: suggest
    message: |
      For `add_conditional_edges`, use a clear, named function to determine the next node.
      This makes the graph's routing logic easier to understand and debug.

      Example:
      ```python
      class AgentState(TypedDict):
          # ...
          next_step: str

      def determine_next_node(state: AgentState) -> str:
          if state["next_step"] == "A":
              return "node_a"
          elif state["next_step"] == "B":
              return "node_b"
          else:
              return END # or another node

      # graph.add_conditional_edges(
      #     "source_node",
      #     determine_next_node,
      #     {"node_a": "node_a", "node_b": "node_b", END: END}
      # )
      ```
examples:
  - input: |
      # file: my_router.py
      # graph.add_conditional_edges("entry", lambda x: "tool_node" if x.get("tool_call") else "llm_node", ...)
    output: "Accepted (simple lambda is okay)" # Assuming this simple lambda passes
  - input: |
      # file: my_router.py
      # graph.add_conditional_edges("entry", lambda x: "tool" if x.get("foo") and x.get("bar") else "end", ...)
    output: "Rejected: Complex conditional logic in `add_conditional_edges` should be encapsulated in a named function for readability, not a complex lambda."
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: langgraph_api_key_security
description: Prevents hardcoding of API keys or sensitive credentials directly in the source code.
filters:
  - type: file_extension
    pattern: "\\.py$"
actions:
  - type: reject
    conditions:
      # Looks for common API key patterns (OpenAI, Anthropic, general "KEY = ...")
      # This is a heuristic and might generate false positives/negatives.
      - pattern: "(OPENAI_API_KEY|ANTHROPIC_API_KEY|_API_KEY|_SECRET)\\s*=\\s*['\"](sk-|ak-|[A-Za-z0-9\\-_\\.+]{20,})['\"]"
        message: "API keys or secrets should not be hardcoded. Use environment variables (e.g., `os.getenv('MY_API_KEY')`) or a secrets management system."
  - type: suggest
    message: |
      Never hardcode API keys, passwords, or other secrets directly in your Python files.
      Instead, load them from environment variables or a dedicated secrets management tool.
      This is critical for security, especially when code is shared or version controlled.

      Example:
      ```python
      import os
      # Good:
      api_key = os.getenv("OPENAI_API_KEY")
      if not api_key:
          raise ValueError("OPENAI_API_KEY environment variable not set.")

      # Bad:
      # OPENAI_API_KEY = "sk-thisIsAFakeKeyDoNotUseItReally"
      ```
examples:
  - input: |
      # my_service.py
      OPENAI_API_KEY = "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
    output: "Rejected: API keys or secrets should not be hardcoded. Use environment variables (e.g., `os.getenv('MY_API_KEY')`) or a secrets management system."
  - input: |
      # my_service.py
      import os
      MY_SECRET = os.getenv("MY_APP_SECRET")
    output: "Accepted"
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: langgraph_llm_input_consideration
description: Reminds developers to consider input validation/sanitization when passing external data to Large Language Models (LLMs) to mitigate prompt injection risks.
filters:
  - type: file_extension
    pattern: "\\.py$"
  - type: content # Heuristic for LLM invocation
    pattern: "(ChatOpenAI|OpenAI|Bedrock|Cohere|LLMChain|invoke|stream|batch)\\("
actions:
  - type: suggest # Actual validation is context-specific, so this is an awareness rule
    message: |
      When passing user-provided or external data directly into LLM prompts (e.g., as part of a query or context),
      be mindful of potential prompt injection vulnerabilities. Consider:
      1. Validating input length and character sets.
      2. Using structured input formats where possible.
      3. Clearly delimiting user input within the prompt.
      4. If applicable, use specific modes or parameters on the LLM API that are designed for instruction-following rather than free-form chat if user input is part of an instruction.
      This is a complex area; ensure you understand the risks associated with your LLM usage pattern.
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: langgraph_configurable_llm_parameters
description: Promotes using configuration for LLM parameters like model names and temperature, rather than hardcoding.
filters:
  - type: file_extension
    pattern: "\\.py$"
  - type: content # Heuristic for LLM instantiation
    pattern: "(ChatOpenAI|OpenAI|Bedrock|Cohere)\\(.*(model_name|temperature).*\\)"
actions:
  - type: reject
    conditions:
      - pattern: "(model_name|model)\\s*=\\s*['\"](gpt-3\\.5-turbo|gpt-4|claude-|text-davinci-) # Add more models
        message: "LLM model names should be configurable (e.g., via environment variables or config files), not hardcoded. This allows easier model switching and updates."
      - pattern: "temperature\\s*=\\s*(0\\.[0-9]+|[01]\\b)" # Matches temperature = 0.x or 0 or 1
        message: "LLM temperature should be configurable, not hardcoded. This allows tuning creativity/determinism without code changes."
  - type: suggest
    message: |
      Hardcoding LLM parameters like model names or temperature makes it difficult to adapt or experiment.
      Load these from environment variables or a configuration file.

      Example:
      ```python
      import os
      from langchain_openai import ChatOpenAI

      llm_model_name = os.getenv("LANGGRAPH_LLM_MODEL", "gpt-3.5-turbo")
      llm_temperature = float(os.getenv("LANGGRAPH_LLM_TEMPERATURE", 0.7))

      llm = ChatOpenAI(model_name=llm_model_name, temperature=llm_temperature)
      ```
examples:
  - input: |
      # file: llm_setup.py
      # from langchain_openai import ChatOpenAI
      # llm = ChatOpenAI(model_name="gpt-4", temperature=0.5)
    output: "Rejected: LLM model names should be configurable... AND Rejected: LLM temperature should be configurable..." # Assuming linter shows both
  - input: |
      # file: llm_setup.py
      # import os
      # from langchain_openai import ChatOpenAI
      # llm = ChatOpenAI(model_name=os.getenv("MODEL"), temperature=float(os.getenv("TEMP")))
    output: "Accepted"
metadata:
  priority: medium
  version: 1.0
</rule>