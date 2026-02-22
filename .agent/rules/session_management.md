---
description: enforce session boundary management for AI agents (starting new conversations per sprint)
---

# Session Management & Context Boundaries

To maintain optimal performance, prevent context window overload, and ensure the AI's "working memory" remains razor-sharp for the task at hand, Project Mimir enforces the following session boundary rules:

1. **One Sprint = One Conversation**: When a Sprint is officially closed and a new Sprint or Phase begins, the AI Agent MUST inform the user to start a **New Conversation** (New Chat Session) in their interface.
2. **Context Offloading**: The AI should rely on the automated `Knowledge Items (KIs)` summaries generated between sessions to retain high-level architectural knowledge, rather than trying to keep the entire project history in a single continuous chat window.
3. **Clean Slate**: Starting a new conversation acts as a "Clean Desk Policy", clearing out old terminal logs, unrelated file buffers, and past compilation errors, allowing the AI to focus 100% on the new Sprint's `Implementation_Plan`.
4. **Agent Action**: If the user asks to begin a completely new Phase or Sprint in an already long-running conversation, politely remind them of this rule and ask them to open a new chat.
