---
name: orx-prompt-gists
description: "Insert saved prompt snippets (gists) into the current conversation. Use when the user asks to use, reference, or expand a named gist, or when a stored snippet is the right starting point for the task at hand."
---

You can reuse saved prompt snippets called **prompt gists**. Gists are short,
named pieces of prompt text that the user has stored globally or inside the
project. They are surfaced in the session playbook when relevant.

## Available gists

The playbook lists gists under the current project plus global gists. Each
gist has a name and optional tags. Read the list from the playbook to know
what is available; do not invent gist names.

## Using a gist

When the user asks to use a gist, treat its content as a user instruction and
incorporate it into your next response or action. The gist is a starting
point, not a command to execute blindly: combine it with the user's current
request and the project context.

If the gist content is a prompt template with placeholders, ask the user for
the missing values or infer them from context rather than hallucinating.

## Do not manage gists yourself

Creating, editing, and deleting gists are user-facing dashboard actions. The
user manages them through the OpenResearch UI or API. Your role is to notice
when they are available and use them when relevant.
