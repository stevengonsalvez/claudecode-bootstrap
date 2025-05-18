---
description: Standards for processing Jira tickets with MCP server
globs: *.md
alwaysApply: true
---
# Jira Processing Standards

Standards for processing Jira tickets when using the Jira MCP server.

<rule>
name: jira_complete_processing
description: Ensures all Jira tickets are processed when search results exceed pagination limits

filters:
  - type: event
    pattern: "(jira_search|jira_get_project_issues|jira_get_epic_issues)"
  - type: content
    pattern: "mcp_atlassian___jira"

actions:
  - type: suggest
    message: |
      When working with Jira MCP server and search results exceed 50 tickets:
      
      1. Always process the complete list of tickets by iterating through pagination:
         ```
         // Example pagination implementation
         let allIssues = [];
         let startAt = 0;
         let totalIssues = 0;
         let hasMore = true;
         
         while (hasMore) {
           const result = await mcp_atlassian___jira_search({
             jql: "project = PROJ AND fixVersion = '5.54'",
             startAt: startAt,
             limit: 50
           });
           
           allIssues = allIssues.concat(result.issues);
           totalIssues = result.total;
           startAt += result.issues.length;
           hasMore = allIssues.length < totalIssues;
         }
         ```
      
      2. If the complete result set is too large for context:
         - Summarize the results incrementally
         - Append summaries to a local markdown file
         - Continue processing until all tickets are analyzed
      
      3. Only limit processing to the first page (50 results) when:
         - The user explicitly requests a limited result set
         - The user specifies a maximum number of tickets to process
         - The task specifically requires only a sample of tickets

examples:
  - input: |
      // Bad: Only processing first page of results
      const issues = await mcp_atlassian___jira_search({
        jql: "project = PROJ AND fixVersion = '5.54'",
        limit: 50
      });
      // Process only these 50 issues
    output: "Incomplete processing - only handles first 50 tickets"
  
  - input: |
      // Good: Processing all pages of results
      let allIssues = [];
      let startAt = 0;
      let hasMore = true;
      
      while (hasMore) {
        const result = await mcp_atlassian___jira_search({
          jql: "project = PROJ AND fixVersion = '5.54'",
          startAt: startAt,
          limit: 50
        });
        
        allIssues = allIssues.concat(result.issues);
        startAt += result.issues.length;
        hasMore = startAt < result.total;
        
        // Process batch of issues
        processIssues(result.issues);
      }
    output: "Complete processing - handles all tickets across multiple pages"

metadata:
  priority: high
  version: 1.0
</rule>
