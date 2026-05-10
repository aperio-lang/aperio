# Codebase-onboarder — shape rules (for agents)

> Captured 2026-05-10. Scope: the description of the
> categorical shapes that nouns in a foreign codebase fall
> into when the codebase-onboarder maps them to motion-forms,
> locus identity, or types.
>
> This document is **load-bearing for the agentic pipeline**.
> The codebase-onboarder is not run by a human in isolation;
> it is run inside an agentic session where an LLM agent reads
> source, applies these shape rules, and fills in
> classifications the static extractor can't justify on its
> own. The lookup table is a seed of universally-clear
> mappings; the *rules* are what scale.

## The thesis

A codebase's nouns fall into one of **three categorical
shapes**. Each shape has a different relationship to motion
(the present-participle verb form) and to locus identity:

| Shape           | Motion?   | Locus?       | Examples                   |
|-----------------|-----------|--------------|----------------------------|
| **Agent noun**  | yes       | yes          | Listener, Worker, Server   |
| **Entity noun** | yes       | yes          | Cache, Bus, Pool, Pipeline |
| **Shape noun**  | NO        | usually no   | Request, User, Token, Cell |

The codebase-onboarder's job is to detect which category each
noun belongs to and emit the appropriate Aperio construct.

## Category 1 — Agent nouns

**Definition.** A noun that names an actor *by what it does*.
Surface signal: `-er` / `-or` / `-ar` suffix on a verb root.

**Motion form.** Strip the suffix, add `-ing`.

| Name      | Stem    | Motion     |
|-----------|---------|------------|
| Listener  | listen  | listening  |
| Worker    | work    | working    |
| Server    | serv    | serving    |
| Logger    | logg    | logging    |
| Handler   | handl   | handling   |

**Locus identity.** Almost always a locus. Agent nouns name
flow by definition.

**Min-stem guard.** Stem must be ≥ 4 chars (full morpheme ≥ 6
chars) or the rule mis-fires:

| Name   | Stem | Reason                                    |
|--------|------|-------------------------------------------|
| Order  | Ord  | not a verb root → mark `<unknown:Order>`  |
| User   | Us   | not a verb root → mark `<unknown:User>`   |
| Filter | Filt | "filting" is wrong; correct is "filtering" → use lookup override |

**Edge cases the static rule misses** (agent should fill in):

- Acronyms inside compounds: `HTTPClient` doesn't split into
  `HTTP` + `Client` because the morpheme splitter is
  capital-letter-based. Agent reads context, recognizes
  `HTTPClient` as one entity, classifies as Agent noun
  (Client → requesting).
- Past-participle naming: `Cached`, `Logged`, `Stored` aren't
  agent nouns even though they look adjacent to one. Agent
  marks them as shape nouns (the data they describe).

## Category 2 — Entity nouns

**Definition.** A noun that names a *thing-as-instrument*. The
thing is the tool that enables a particular flow. Common
infrastructure-ish vocabulary.

**Motion form.** The present-participle verb of the action the
tool serves. Comes from a small lookup table because there is
no morphological rule that derives it from the noun.

| Name        | Motion       | Why                                  |
|-------------|--------------|--------------------------------------|
| Cache       | remembering  | a cache remembers values             |
| Bus         | routing      | a bus routes messages                |
| Pipeline    | flowing      | a pipeline flows data through stages |
| Pool        | pooling      | a pool pools instances               |
| Repository  | carrying     | a repository carries persisted data  |

**Locus identity.** Almost always a locus. Entity nouns name
the flow by naming what enables it.

**Lookup-table policy.** Keep small. The seed in
`std::lang::Lang.lookup_morpheme` covers ~15 universally-clear
entries (Controller, Processor, Manager, Handler, Listener,
Validator, Builder, Parser, Repository, Cache, Bus, Queue,
Pool, Pipeline, Service). Per-codebase extensions happen at
the agent layer, not by hand-extending the seed.

**Edge cases the static rule misses** (agent should fill in):

- Domain-specific vocabulary that LOOKS like Entity but means
  something specific (`SessionManager` is Manager-shaped but
  could be tracking, scoping, or guarding depending on what
  the implementation does — agent reads).
- Synonyms: a project that uses `Stash` instead of `Cache`
  needs the agent to recognize the equivalence by reading
  the type's methods + usage.

## Category 3 — Shape nouns

**Definition.** A noun that names *pure data*. No flow. The
thing IS, doesn't DO. Surface signal: short noun, no agent
suffix, names a record / entity / record-type.

**Motion form.** **None.** Shape nouns have no verb form.
Forcing one fabricates meaning that isn't in the source.

| Name     | Why no motion                                    |
|----------|--------------------------------------------------|
| Request  | a request is data describing a request          |
| Response | data describing a response                      |
| User     | a record describing a person                    |
| Order    | a record describing an order                    |
| Token    | a record carrying authorization data            |
| Session  | a record carrying session state                 |
| Config   | a record carrying configuration                 |
| Schema   | a record describing data shape                  |
| Cell     | a record describing one position in a structure |

**Locus identity.** Usually a `type` declaration in the
absorbed source, not a `locus`. Per the cross-tower agreement
rule (`notes/aperio-types-vs-loci.md`):

- Domain-tower presence only (a type declaration with no
  operational role and no harmonic role) → emit as `type`.
- Cross-tower agreement (the shape noun ALSO appears in
  operational signal — e.g., `Session` is both a struct AND
  the receiver of methods that have lifecycle) → may emit
  as a *locus that holds shape data*. The locus's data
  fields are typed by the shape.

**Output shape for shape nouns** in the tower-join:

```json
{
  "name": "Request",
  "category": "shape",
  "motion": null,
  "verdict": "type",
  "agent_note": "Pure data record. If this struct also has methods that mutate state or perform I/O, an agent should reclassify this as a locus and provide a motion-form."
}
```

The `agent_note` field is the prompt for the LLM driving the
session. It tells the agent **what to look for** to upgrade or
override the static classification.

## How an agent applies these rules

The codebase-onboarder runs as the first stage of an agentic
session. The agent's loop is roughly:

1. **Run the static extractors** (m97 / m100 / m102) and the
   tower-join. Emit per-file JSON with classifications +
   unknown markers.
2. **For each unknown or low-confidence classification,
   read the source.** The agent opens the file, looks at the
   type's methods, fields, and usage in other files.
3. **Apply the shape rules** above to decide:
   - Agent noun? Apply suffix rule, propose motion.
   - Entity noun? Look at what the type does; pick a verb
     for the action it enables; propose motion.
   - Shape noun? Mark as type, no motion.
4. **Optionally extend the per-codebase lookup**. The agent
   may emit a `codebase-overrides.json` next to the tower
   output that the next pass merges in. This is how
   per-codebase vocabulary builds up *without* hand-editing
   the stdlib's seed.
5. **Re-run the tower-join with overrides applied** to
   confirm classifications are consistent across files.
6. **Emit the polished recognition report** (next milestone)
   with all unknowns resolved or honestly flagged.

The static tool stays small and honest. The agent provides
context. The lookup table seed is universal, not per-codebase
exhaustive.

## Why this matters strategically

The codebase-onboarder is **agent-first, not human-first**.
Human-first would mean curating exhaustive vocabulary tables
ahead of time so the static tool produces clean output for
human consumption. That's the wrong shape for an LLM-driven
pipeline:

- **Agents read source faster than humans.** A 1500-LOC Go
  module that would take a human 30 minutes to fully
  internalize is a sub-minute pass for a competent agent.
  The agent can resolve unknowns by reading; pre-curated
  tables are speculative work.
- **Per-codebase vocabulary varies wildly.** No seed table
  covers what a specific company's domain language looks
  like. Hand-curation against a corpus would still miss
  every novel codebase.
- **Static unknowns ARE the agent's prompt.** The static
  tool's `<unknown:X>` markers tell the agent *exactly
  where to look*. Without them, the agent has to scan the
  whole output for things that look wrong; with them, the
  agent has a focused work queue.

The implication for substrate design: the static tools
optimize for **honest classification with rich context**, not
for low unknown rates. A lower unknown rate at the cost of
fabrication is *worse* than an honest unknown — the agent
wastes time second-guessing a wrong "confident" answer.

## What this changes in the codebase

- **`std::lang::Morpheme`** (this round) inherits the 15-entry
  seed unchanged. We do NOT spend time on Go-vocabulary
  curation. The entry count grows organically from
  cross-codebase confirmed-mapping evidence, not from
  speculation.
- **The polished recognition report** (next round) surfaces
  unknowns prominently with file paths + agent_notes, so an
  agent reading the report has a focused work queue.
- **A future "agent-resolved overrides" merge step** lets
  per-codebase agent decisions carry forward across runs
  without contaminating the universal seed.
