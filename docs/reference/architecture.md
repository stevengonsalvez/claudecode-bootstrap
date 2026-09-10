---
title: "Architecture deep-dive"
---

Where to find the architecture documentation, by depth.

## Start here

[**Whole-system architecture**](/product/architecture) is the page you
probably want. It carries the ecosystem diagram, the box diagram, the data and
control flow walkthroughs, the on-disk layout and the boundary contracts
between components.

## By component

| Component | Page |
|---|---|
| The TUI host: crates, threading, render loop | [TUI architecture](/tui/architecture) |
| The v2 plugin ABI: wire format, capabilities, conformance | [Plugin spec v2](/plugins/spec-v2) |
| Hangar: the managed-agents control plane | [Hangar architecture](/hangar/architecture) |
| Knowledge capture and recall | [Knowledge system overview](/knowledge/overview) |
| Which repo holds what | [Repositories](/reference/repositories) |

## Diagrams

The two system diagrams are generated, not drawn by hand, because the
hand-drawn originals kept outliving the code they described. Regenerate them
after any structural change:

```bash
python3 docs/assets/diagrams/generate-diagrams.py
```

The script reads crate count, workspace version, staged plugin ids, daemon
kinds, screen and CLI command counts, tool adapters and the Hangar RPC registry
straight from the source, and prints every figure it used so a reviewer can
check them against the rendered diagram.

## See also

- [Docs hub](/readme)
- [Glossary](/reference/glossary)
