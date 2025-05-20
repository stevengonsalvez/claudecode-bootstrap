---
description: Pull Request (PR/MR) Review Standards and Best Practices
globs: [] # This rule doesn't apply to specific file globs, it's a process rule.
alwaysApply: true # This is a guideline for a process.
---
# Effective Pull Request (PR/MR) Review Practices

Guidelines for conducting constructive, thorough, and efficient code reviews to improve code quality, share knowledge, and foster a positive team environment.

<rule>
name: pr_review_provide_clear_summary
description: Reviewers should provide a concise summary of their overall feedback, including positive aspects and any major concerns.
filters: [] # Process rule
actions:
  - type: suggest # This is a behavioral guideline
    message: |
      Start your review with a high-level summary. This helps the author quickly understand the main takeaways.
      - **Acknowledge Positives:** Mention what was done well (e.g., "Great job on the clean refactor of X!", "Love the thorough test coverage here.").
      - **State Major Concerns (if any):** Clearly articulate any architectural issues, significant bugs, or major deviations from requirements that need addressing before merge.
      - **Overall Impression:** Briefly state if the PR is largely good to go pending minor changes, or if more substantial work is needed.

      **Platform Tip (GitHub/GitLab):** Use the main comment box for the PR/MR (not just inline comments) to provide this summary.
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: pr_review_inline_comments_for_specifics
description: Use inline comments for specific feedback tied directly to lines of code.
filters: [] # Process rule
actions:
  - type: suggest
    message: |
      For feedback related to specific lines or blocks of code:
      - Use inline comments directly on the diff.
      - Be precise: Refer to the exact lines or logic you're discussing.
      - Explain *why*: Don't just say "change this"; explain the reasoning behind your suggestion (e.g., "Consider using a `Set` here for better performance on lookups because...").
      - Offer solutions: If possible, suggest an alternative approach or provide a code snippet.

      **Platform Tip (GitHub/GitLab):** Leverage the "Start a review" or "Add to review" feature to batch inline comments before submitting them all at once. This prevents a flood of notifications for the author.
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: pr_review_use_conventional_comments
description: Encourages using Conventional Comments to prefix feedback, indicating severity and type.
filters: [] # Process rule
actions:
  - type: suggest
    message: |
      Prefix your comments with Conventional Comment labels to clearly indicate their nature and importance. This helps the author prioritize and understand the feedback. Common labels include:

      - `praise:` For highlighting something done well.
        Example: `praise: Excellent use of early returns here, makes the logic much clearer!`
      - `suggestion:` For optional improvements or alternative approaches.
        Example: `suggestion: We could potentially use a generator here to save memory if this list becomes very large. What do you think?`
      - `issue:` For problems that need to be addressed (bugs, logical errors).
        Example: `issue: This condition seems to miss the edge case where `count` is zero, leading to a division by zero error.`
      - `question:` When you need clarification or want to understand the author's reasoning.
        Example: `question: I'm not sure I follow the intent behind this variable name. Could you elaborate?`
      - `nitpick:` (or `nit:`) For minor, non-blocking issues (typos, minor style preferences).
        Example: `nit: Small typo here, "recieve" should be "receive".`
      - `thought:` For ideas or thoughts that might be out of scope for the current PR but worth considering later.
        Example: `thought: This component is getting quite complex. Perhaps we should consider splitting it in a future refactor?`

      Using these prefixes makes reviews more scannable and actionable.
metadata:
  priority: high
  version: 1.0
</rule>

<rule>
name: pr_review_nitpicks_for_minor_issues
description: Use "NIT" (Nitpick) or equivalent for minor, non-blocking issues.
filters: [] # Process rule
actions:
  - type: suggest
    message: |
      For minor issues that don't prevent the PR from being merged but could be improved (e.g., slight formatting inconsistencies, typos in comments, very minor stylistic preferences), prefix your comment with "NIT:" or "nitpick:".
      This signals to the author that the feedback is not a blocker.

      Example:
      `nit: There's an extra space here.`
      `nitpick: Could we rename this variable to `isEnabled` for consistency with other parts of the codebase? Not a blocker though.`

      This helps differentiate between critical feedback and minor suggestions, allowing the author to prioritize.
metadata:
  priority: medium
  version: 1.0
</rule>

<rule>
name: pr_review_be_constructive_and_respectful
description: All feedback should be constructive, objective, and respectful, focusing on the code, not the author.
filters: [] # Process rule
actions:
  - type: suggest
    message: |
      Code reviews are a collaborative process.
      - **Focus on the code:** Frame feedback around the code's behavior, structure, and adherence to standards, not personal opinions about the author's abilities.
      - **Be empathetic:** Understand that everyone makes mistakes. Assume good intent.
      - **Ask questions:** Instead of making accusatory statements (e.g., "This is wrong"), ask questions to understand the author's perspective (e.g., "Could you explain the reasoning behind this approach? I was wondering if X might be an alternative.").
      - **Be specific:** Vague comments like "this is confusing" are not helpful. Point out what specifically is confusing and why.
      - **Avoid sarcasm and overly critical language.** The goal is to improve the code and help the author grow.
metadata:
  priority: critical
  version: 1.0
</rule>
