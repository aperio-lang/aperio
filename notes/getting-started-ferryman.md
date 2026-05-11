# Getting started with ferryman

> Captured 2026-05-11. This is the end-to-end walkthrough for
> a developer pointing ferryman at their codebase for the first
> time. No prior Aperio knowledge required; the three-tower
> framing is introduced as you go.
>
> Companion docs:
> - `notes/agent-onboarding/ferryman-enrichment-protocol.md` —
>   the protocol an LLM agent follows for the enrichment stage.
> - `notes/aperio-types-vs-loci.md` — the axiom the three-tower
>   recognition derives from.

## What ferryman does, in one paragraph

You have a codebase you want to *recognize* — understand its
operational shape, find its backbone, see which binaries do
what, without reading every file. Ferryman runs a static pass
that emits structured data, then hands off to an LLM agent
which fills in the meaning. The result is a partner-readable
recognition report. The whole pipeline is one binary plus a
few hundred lines of yaml the agent can edit by hand.

## Step 0 — prerequisites

- The `ferryman` binary. In the lotus-lang repo it lives at
  `apps/ferryman/ferryman` (built with
  `aperio build apps/ferryman/`). Symlink onto your PATH or
  use the absolute path.
- A Go codebase to recognize. (Other flavors will land later;
  Go is the v0 target.) We'll use `~/code/grease` for examples.
- File-read access to the codebase. If you're running this
  through Claude Code or a similar agent, the agent does the
  reading; if you're driving by hand, your editor does.

That's it. No config files, no setup, no service to run.

## Step 1 — point ferryman at the codebase

```
ferryman ~/code/grease
```

Pass `--lang=go` if you don't have a `go.mod` at the repo root
(unusual; ferryman auto-detects via manifest files otherwise).

You'll see:

```
============================================================
  Aperio recognition: /home/riley/code/grease
  flavor: go
  output: /home/riley/code/grease/.ferryman
============================================================

[stage 0] filesystem lotus...
           /home/riley/code/grease/.ferryman/00-tree.yaml (76797 bytes)
[stage 4] writing agent prompt...
prompt written: /home/riley/code/grease/.ferryman/PROMPT.md

Next steps:
  ...
```

Ferryman just created `<repo>/.ferryman/` with two files:

| File | What it is |
|---|---|
| `00-tree.yaml` | The **filesystem lotus** — every dir and source-relevant file, classified by role |
| `PROMPT.md` | Instructions the agent reads to do the enrichment work |

This is the cheap, scale-safe stage. It runs in under a second
even on multi-binary monorepos. Nothing expensive happens yet.

## Step 2 — read the filesystem lotus

```
less ~/code/grease/.ferryman/00-tree.yaml
```

The yaml has a flat structure: a list of `dirs` and a list of
`files`. Each file carries a `role`:

- `entrypoint` — files with `func main()` (Go) / `fn main()`
  (Rust) etc. These are the binaries you might want to deep-dive.
- `manifest` — `go.mod`, `package.json`, `Cargo.toml`, etc.
- `test` — `*_test.go`, `test_*.py`, etc.
- `source` — flavor source files.
- `other` — everything else that wasn't filtered out (k8s
  yamls, configs, docs).

For grease this is the survey-level shape:

```
156 dirs, 1153 files
   40 entrypoints     binaries you could deep-dive
    5 manifests
   72 tests
  630 source
  406 other
```

Forty binaries means the codebase is a multi-binary monorepo
with shared code somewhere (likely a `src/` or `lib/` dir, which
you'll see in the dirs list). You're not going to deep-dive
forty things — typical practice is 2-4 representative picks,
plus the survey reading of the tree.

## Step 3 — read the agent prompt

```
less ~/code/grease/.ferryman/PROMPT.md
```

This is what an LLM agent reads to know what to do. The key
ideas — paraphrased so you can decide whether to read it:

- Pick 2-4 binaries to deep-dive. Survey the rest from the FS
  lotus alone.
- For each deep-dive: run `ferryman skeleton` on the binary,
  read the resulting yaml + source, write enrichment cells back
  to a new yaml.
- Use the **three-tower-agreement rule** to identify loci (see
  step 5 below).
- Log honest **unknowns** instead of guessing.

If you're driving this with Claude Code or a similar agent,
hand the agent the PROMPT.md file and step back. If you're
driving by hand, the steps below are the practical version.

## Step 4 — pick a binary to deep-dive

Look at the entrypoints in `00-tree.yaml`. Pick one that looks
central. For grease, `cmd/quoter` was a good first pick because
the name suggests "core trading operation"; `cmd/investigator`
was a good second pick because it's clearly a small utility
(one file, no subcommands), useful as the easy demo.

Look at the source first so you know what you'd be deep-diving:

```
ls ~/code/grease/cmd/quoter/
cat ~/code/grease/cmd/quoter/main.go
```

## Step 5 — extract the three towers for that binary

```
ferryman skeleton \
  ~/code/grease/cmd/quoter \
  ~/code/grease/.ferryman/01-quoter-skel.yaml
```

This is where the **three-tower lotus framing** first lands.
The skeleton yaml is the same source seen from three angles:

| Tower | Skeleton yaml section | What it shows |
|---|---|---|
| **Operational** (resolution) | `outward_tower` | What main() does at runtime — the call tree, who calls whom, builtin / package / method classification per node |
| **Harmonic** (substrate) | `inward_tower` | What this binary's package transitively needs — file-to-file imports with stdlib / local / external classification |
| **Domain** (bulk) | (skeleton only — agent fills the rest) | Names of types, functions, files, and what they signal about meaning |

The three towers project the same code from three orthogonal
epistemic angles. Every source-level node lives in at least
one tower. A node that appears in **two or more** towers with
coherent roles is a **locus** (per
`notes/aperio-types-vs-loci.md`). One-tower presence is
structural artifact, not its own locus.

You can read the skeleton yaml directly to see what was
extracted. Tree-sitter ran; receiver-preserving extraction
classified each call as `builtin` / `internal` / `external` /
`method`; the inward tower aggregated transitive imports.
Anywhere the skeleton hedged (`kind: "method"` with a
lowercase receiver — ambiguous between package call and local
var) is space for the agent to resolve.

## Step 6 — the agent enriches the skeleton

Open the skeleton yaml and the source files. The enrichment
cells you (or the agent) write are:

**Per binary:**

| Cell | Shape | Purpose |
|---|---|---|
| `summary` | scalar | 2-4 sentences of what the binary does at runtime |
| `loci` | list | source-level entities that pass the three-tower-agreement rule |
| `unknowns` | list | things you tried to resolve and couldn't, with why |

**Per file (under `files[j]`):**

| Cell | Shape | Purpose |
|---|---|---|
| `classification` | scalar | a phrase: "http transport", "command dispatch" |
| `contributes_to` | scalar | which locus this file participates in |

**`loci` entries:**

| Field | Required | Purpose |
|---|---|---|
| `name` | yes | source identifier |
| `verdict` | yes | `"locus"` / `"type"` / `"unknown"` |
| `agreement` | optional | how many towers agreed: `"2"` or `"3"` |
| `shape` | optional | which Aperio pattern (`"service"`, `"namespace"`, `"interface"`, `"shape-type"`, `"spawned-child"`, `"subscriber"`) |
| `motion` | optional | present-participle motion form — Agent / Entity nouns ONLY, never for Shape nouns |
| `prose` | optional | one-line domain reading of what the locus means |

Save the result as `04-<binary>-enriched.yaml` in the
`.ferryman/` directory.

If you skip this step, the renderer still produces a usable
report — just less polished. Enrichment is additive.

## Step 7 — render the recognition report

```
ferryman render \
  ~/code/grease/.ferryman/04-quoter-enriched.yaml \
  > ~/code/grease/.ferryman/recognition-quoter.txt
```

Or, if you skipped enrichment:

```
ferryman render \
  ~/code/grease/.ferryman/01-quoter-skel.yaml \
  > ~/code/grease/.ferryman/recognition-quoter.txt
```

Open the result. The report has:

- The binary's `summary` paragraph (if you enriched)
- The `Loci (cross-tower agreement)` block (domain-tower
  verdicts — currently rendered with this label; future passes
  will surface the three-tower frame explicitly here)
- Per-file summary lines with `classification` and
  `contributes_to`
- The **outward tower** section — operational view
- The **inward tower** section — harmonic view
- The **unknowns** block — agent-actionable next-pass work

## Step 8 — repeat for the other picks

Steps 4-7 repeat for each binary you picked. Two-to-four deep-
dives plus a survey-level reading of the tree is the typical
partner-facing shape.

## What you have at the end

In `<repo>/.ferryman/`:

```
00-tree.yaml                  whole codebase, filesystem lotus
PROMPT.md                     the agent instructions
01-<binary>-skel.yaml         per-binary three-tower data (static)
04-<binary>-enriched.yaml     per-binary recognition (agent)
recognition-<binary>.txt      the rendered report
```

The `.ferryman/` directory is meant to be gitignored
(ferryman's convention; add `.ferryman/` to your repo's
`.gitignore`). Re-run any step at any time; outputs overwrite.

## Where the three-tower frame lives

To pull together what the walkthrough surfaced:

- The **filesystem lotus** is its own thing — the outermost
  shape, emitted at stage 0. Everything below is per-binary.
- The **operational tower** (`outward_tower`) is what runs.
  Call trees, spawns, handlers, lifecycle owners.
- The **harmonic tower** (`inward_tower`) is what coordinates
  the running. Imports, package dependencies, shared modules.
- The **domain tower** is named meanings. The skeleton emits
  names + classification candidates; the agent applies the
  three-tower-agreement rule to assign locus / type / unknown
  verdicts.

A node is a locus iff ≥2 of the three towers point at it with
coherent roles. This is the rule the agent applies during
enrichment, and it's the rule the rendered "Loci" section
surfaces in the report.

## Known rough edges

- The render output currently uses "Outward tower" / "Inward
  tower" / "Loci (cross-tower agreement)" as section headers.
  The three-tower framing (operational / harmonic / domain)
  is only named in this doc and in PROMPT.md, not in the
  report itself. A small rename pass is queued.
- Yaml string escaping leaks into the rendered output — a
  `summary` containing `\"foo\"` shows the literal backslash.
  Cosmetic; tracked.
- The inward tower can be verbose on large binaries because it
  lists every transitively-reachable file. For partner-facing
  reports this could be collapsed; for deep-dive reports the
  full detail is useful.
- The single-binary skeleton stage still hits an O(N²) yaml-
  build limit on very large binaries (see
  `notes/aperio-friction.md` —
  `reader-list_item-quadratic-concat`). Picking
  representative binaries (rather than running skeleton on
  the whole monorepo) sidesteps this.

## Cross-references

- `notes/agent-onboarding/ferryman-enrichment-protocol.md` —
  long-form version of step 6, with worked examples
- `notes/aperio-types-vs-loci.md` — the three-tower-agreement
  rule sourced as an axiom
- `notes/onboarding-shape-rules.md` — Agent / Entity / Shape
  noun categories and motion-form derivation rules
- `notes/codebase-onboarder-progress.md` — overall project state
- `apps/ferryman/` — the implementation
