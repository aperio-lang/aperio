# Capacity & storage

> **α** — How does a locus declare what it holds, and how does
> that commitment shape its lowering?

Covers:

- **Capacity slots**: `pool X of T;` (cell-recycling) and
  `heap Y of T;` (growable, locus-bounded lifetime).
- **Projection classes** (`rich` / `chunked` / `recognition`)
  — how a locus declares the resolution at which it serves
  observations of its children, and how that drives the
  allocator strategy underneath.
- **Forms** (`@form(vec)`, `@form(hashmap)`,
  `@form(ring_buffer)`): the storage substrate the application
  layer uses, and why Aperio surfaces them as form annotations
  rather than parametric `Vec<T>` / `Map<K,V>` / etc. types.
- Choosing between forms: contiguous-growable (vec), keyed
  intrusive (hashmap), bounded FIFO (ring_buffer).

*This chapter is under construction. The
[`spec/forms.md`](https://github.com/aperio-lang/aperio/blob/main/spec/forms.md)
and [`spec/memory.md`](https://github.com/aperio-lang/aperio/blob/main/spec/memory.md)
files are the canonical references in the meantime.*
