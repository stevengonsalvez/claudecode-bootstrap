---
description: Agent Standard for Prioritizing `fep_docs` ChromaDB Collection via Tools
globs: [] # This rule applies to agent behavior, not specific file content
alwaysApply: true # This is a core operational principle for the agent
---
# Agent Standard: Mandatory `fep_docs` Search via ChromaDB Tools

This rule mandates that an AI agent, when tasked with information retrieval, MUST first use its available ChromaDB tools to query the `fep_docs` collection before resorting to general knowledge. All output must be in actionable steps with proper citation for non-document sources.

<rule>
name: agent_must_use_chroma_tools_for_fep_docs_first
description: "MANDATES: (1) Agent uses available ChromaDB tools (esp. `chroma___chroma_query_documents`) to search the `fep_docs` collection as the VERY FIRST STEP for information. (2) All answers presented as actionable, numbered steps. (3) Information NOT from `fep_docs` MUST be cited."
filters:
  - type: user_query_intent # Triggered by information-seeking queries
    pattern: "(?i)^(how to|what is|what's|explain|tell me about|steps for|documentation for|show me how|find docs on|deploy my app)"

actions:
  - type: suggest # Core instructions for the LLM/agent's system prompt
    message: |
      **LLM/Agent Information Retrieval Protocol:**

      You have access to the following ChromaDB tools for interacting with document collections:
      `chroma___chroma_add_documents`, `chroma___chroma_delete_collection`, `chroma___chroma_get_collection_info`, `chroma___chroma_update_documents`, `chroma___chroma_peek_collection`, `chroma___chroma_delete_documents`, `chroma___chroma_get_documents`, `chroma___chroma_list_collections`, `chroma___chroma_query_documents`, `chroma___chroma_get_collection_count`, `chroma___chroma_create_collection`, `chroma___chroma_modify_collection`.

      **1. MANDATORY FIRST ACTION - Query `fep_docs`:**
         *   For ANY user query seeking information or documentation, your IMMEDIATE and FIRST action is to use the `chroma___chroma_query_documents` tool to search the ChromaDB collection named `fep_docs`.
         *   Use other Chroma tools like `chroma___chroma_get_collection_info` or `chroma___chroma_get_documents` if needed to understand or verify the context of `fep_docs` results.

      **2. RESPONSE FORMAT - Actionable Steps:**
         *   ALL information provided to the user MUST be formatted as a series of clear, actionable, numbered steps.

      **3. CITATION & HANDLING `fep_docs` RESULTS:**
         *   **If `fep_docs` provides the answer:** Synthesize the information from the retrieved documents into numbered steps. No citation is needed for this content.
         *   **If `fep_docs` is insufficient or yields no relevant results:**
             a. Explicitly state this (e.g., "Information not found in `fep_docs` via ChromaDB query.").
             b. You may then use your general knowledge to answer.
             c. CRITICAL: Any step or piece of information from your general knowledge (i.e., NOT from `fep_docs` query results) MUST be clearly cited as `[Source: General Knowledge]`.

      **Example Flow:**
      User: "How to deploy FEP app?"
      You (Internal Thought Process & Actions):
          1. Action: Call `chroma___chroma_query_documents` with query "deploy FEP app" on collection `fep_docs`.
          2. Analyze results from `fep_docs`.
          3. If results are good: Formulate numbered steps based on `fep_docs` content.
          4. If results are not good: State "Info not in `fep_docs`." Then formulate numbered steps from general knowledge, citing each step.

  - type: reject # Rejects agent output that clearly uses general knowledge without any indication of trying fep_docs first via tools.
    conditions:
      # Heuristic: If the response jumps to general knowledge and lacks any mention of fep_docs or chroma tool usage attempt.
      - pattern: "^(?!.*(fep_docs|ChromaDB|chroma___chroma_query_documents|queried the documentation)).*(\\[Source: General Knowledge\\]|\\bbased on general practice\\b)"
        message: "Agent Output Violation: You MUST indicate an attempt to query `fep_docs` using ChromaDB tools BEFORE providing information from general knowledge. State the outcome of the `fep_docs` query."
  - type: reject # Rejects agent output that fails to cite non-fep_docs information when fep_docs was clearly insufficient.
    conditions:
      - pattern: "(?s)(Information not found in `fep_docs`|`fep_docs` did not yield|couldn't find this in the documentation).*?(?<!\\[Source: General Knowledge\\])$"
        message: "Agent Output Violation: If `fep_docs` was insufficient and general knowledge is used, that general knowledge portion MUST be cited with `[Source: General Knowledge]`."
  - type: reject # Rejects agent output that isn't primarily in step format.
    conditions:
      - pattern: "^(?!(\\s*\\d+\\.\\s)).{50,}|(\\n\\n(?!(\\s*\\d+\\.\\s)).{50,})"
        message: "Agent Output Violation: Response must be primarily structured as numbered, actionable steps."

examples:
  - input: |
      User Query: "How do I deploy my app (FEP is my frontend framework) ."
    output: |
      # Scenario 1: Agent Output - `fep_docs` (via chroma___chroma_query_documents) HAS relevant deployment info:
      Okay, based on a query to the `fep_docs` collection using ChromaDB tools, here are the steps to deploy your FEP application:
      1. Build your FEP application for production using `fep build --prod`.
      2. Deploy the contents of the `dist/` directory to your chosen static hosting provider (e.g., Vercel, Netlify, AWS S3).
      3. (Further steps specific to FEP deployment as found in `fep_docs`...)

      # Scenario 2: Agent Output - `fep_docs` (via chroma___chroma_query_documents) LACKS specific deployment info:
      I queried the `fep_docs` collection using `chroma___chroma_query_documents` for "FEP app deployment" but did not find specific instructions.
      However, here are general steps for deploying most frontend applications:
      1. Build your application for production (e.g., `npm run build`). [Source: General Knowledge]
      2. Choose a static hosting provider. [Source: General Knowledge]
      3. Upload your build output (e.g., `dist/` or `build/` folder) to the provider. [Source: General Knowledge]
      4. Configure DNS if using a custom domain. [Source: General Knowledge]

metadata:
  priority: critical
  version: 2.0 # Major revision for conciseness and tool emphasis
  target_agent_capability: "Tool-Augmented Documentation-Aware Question Answering"
  required_tools:
    - "chroma___chroma_query_documents (primary)"
    - "Other chroma___* tools (for context/verification if needed)"
  required_data_source: "ChromaDB collection: fep_docs"
</rule>   