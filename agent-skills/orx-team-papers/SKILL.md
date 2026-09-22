---
name: orx-team-papers
description: "Read and reason about team papers uploaded to the project. Use when the user refers to a shared paper, asks to align research with team references, migrate a scheme or writing style, or mine prior team work for topics or innovations."
---

Project members can upload PDFs under the project's **team papers** collection.
These papers are available to you as extracted plain text in the session
worktree or project directory. Read them before proposing research directions,
writing style, notation, or experimental schemes that should match team norms.

## Where the papers live

Each paper has its own directory:

```
.openresearch/team-papers/<paperId>/
  paper.pdf   # original PDF
  paper.txt   # extracted plain text
```

If the session worktree cannot reach the project directory directly, the
`.openresearch/team-papers/` tree is staged into the worktree at the same
relative path. The playbook lists uploaded paper titles and their relative
paths under the current working directory.

## What to use team papers for

- **Topic discovery** — identify what the team cares about and anchor new
  hypotheses in existing work.
- **Innovation mining** — note techniques, metrics, ablations, or baselines the
  team has already explored.
- **Writing style and scheme migration** — adopt the team's notation, section
  structure, figure conventions, and citation style when drafting a paper.
- **Background** — cite team papers as prior work when relevant, just as you
  would cite external literature.

## Reading a paper

Always read the extracted text rather than trying to parse the PDF directly:

```sh
cat .openresearch/team-papers/<paperId>/paper.txt
```

If `paper.txt` is missing, the PDF was stored but extraction failed (the
`pdftotext` tool was unavailable). In that case you can still reason from the
filename, title, and metadata, but you cannot read the full text.

## Do not treat team papers as external literature

Cite them as internal references or prior team work, not as independently
published sources. Use the file tag contract from the session playbook when
referring to a team paper in chat.

## Uploading and editing are user-facing dashboard actions

Do not attempt to create, move, or delete team-paper files yourself. The user
adds papers through the project dashboard or API. Your role is to read and
reason about the ones that are already there.
