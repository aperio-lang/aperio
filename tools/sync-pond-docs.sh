#!/usr/bin/env bash
# Sync pond/* library READMEs into docs/src/libraries/ for the
# mdbook build.
#
# Pond lives in a separate repo (github.com/aperio-lang/pond).
# Each library is its own seed and ships an authoritative
# README.md. This script flattens those into the lotus-lang
# mdbook's libraries chapter, rewriting relative links to the
# upstream GitHub view (FRICTION.md, examples/, ../CONTRACTS.md,
# etc., none of which the book ships).
#
# Pattern: commit-the-sync. Run this, commit the diff to
# docs/src/libraries/, push. CI deploys what's committed.
# Re-run on meaningful pond changes.
#
# Usage:
#   tools/sync-pond-docs.sh              # default pond path: ../aperio-lang/pond
#   POND_REPO=/path/to/pond tools/sync-pond-docs.sh
#   tools/sync-pond-docs.sh --dry-run    # print actions, write to /tmp/pond-docs-dry/

set -euo pipefail

POND_REPO="${POND_REPO:-../aperio-lang/pond}"
DOCS_REPO="${DOCS_REPO:-$(cd "$(dirname "$0")/.." && pwd)}"

# Resolve to absolute paths.
POND_REPO="$(cd "$POND_REPO" && pwd)"

DEST_DEFAULT="$DOCS_REPO/docs/src/libraries"
DEST="${DEST:-$DEST_DEFAULT}"

DRY_RUN=0
if [[ "${1:-}" == "--dry-run" ]]; then
    DRY_RUN=1
    DEST="/tmp/pond-docs-dry"
fi

# Libraries to exclude from the book TOC entirely. _util/* is
# internal infrastructure; moa is superseded by lake/tower.
EXCLUDE_RE='^(_util(/|$)|moa$)'

# GitHub upstream for link rewriting.
POND_GH="https://github.com/aperio-lang/pond/blob/main"
POND_GH_TREE="https://github.com/aperio-lang/pond/tree/main"

# --- helpers ---------------------------------------------------------------

log() { printf '%s\n' "$*" >&2; }

slug_for() {
    # "trade/orderbook" -> "trade-orderbook"
    # "tower"           -> "tower"
    printf '%s' "${1//\//-}"
}

rewrite_links() {
    # $1 = path to .md file (in-place edit)
    # $2 = lib path under pond (e.g. "trade/orderbook" or "tower"
    #      or "" for the top index.md)
    local file="$1" lib="$2"

    # FRICTION.md / examples/ / ../CONTRACTS.md / ../../ → upstream.
    # The lib-relative forms ($lib != "") cover per-lib pages;
    # the lib-empty form covers the top index.md.
    if [[ -n "$lib" ]]; then
        sed -i \
            -e "s|](\./FRICTION\.md|](${POND_GH}/${lib}/FRICTION.md|g" \
            -e "s|](FRICTION\.md|](${POND_GH}/${lib}/FRICTION.md|g" \
            -e "s|](\./examples/|](${POND_GH_TREE}/${lib}/examples/|g" \
            -e "s|](examples/|](${POND_GH_TREE}/${lib}/examples/|g" \
            -e "s|](\.\./CONTRACTS\.md|](${POND_GH}/CONTRACTS.md|g" \
            -e "s|](\.\./README\.md|](${POND_GH}/README.md|g" \
            -e "s|](\.\./\.\./|](${POND_GH_TREE}/|g" \
            "$file"
    else
        # Top index — relative paths target peers in the pond
        # repo root.
        sed -i \
            -e "s|](\./CONTRACTS\.md|](${POND_GH}/CONTRACTS.md|g" \
            -e "s|](CONTRACTS\.md|](${POND_GH}/CONTRACTS.md|g" \
            -e "s|](\./SUMMARY\.md|](${POND_GH}/SUMMARY.md|g" \
            -e "s|](\./KNOWN_GOTCHAS\.md|](${POND_GH}/KNOWN_GOTCHAS.md|g" \
            "$file"
    fi

    # Cross-lib sibling references like `[../crypto/]` → in-book
    # peer page `./crypto.md`. Multi-segment libs flatten via slug
    # rule (../math/matrix/ → ./math-matrix.md). Trailing slash is
    # consumed.
    #
    # We need one substitution per known sibling slug; loop them.
    # This runs on every file but only matches actual occurrences.
    for sib_slug in "${EMITTED_SLUGS[@]:-}"; do
        [[ -z "$sib_slug" ]] && continue
        # Recover the lib path from the slug: math-matrix → math/matrix
        # We can't perfectly invert (router → router is ambiguous from agent-tools → agent/tools).
        # The reliable approach: explicit substitutions for each emitted lib path tracked alongside.
        :
    done
    # Use the lib_paths array (kept in sync with EMITTED) for accurate rewrites.
    for sib_lib in "${LIB_PATHS[@]:-}"; do
        [[ -z "$sib_lib" ]] && continue
        [[ "$sib_lib" == "$lib" ]] && continue
        local sib_slug
        sib_slug="$(slug_for "$sib_lib")"
        # `[(text)](../<sib_lib>/)` → `[(text)](./<sib_slug>.md)`
        # `[(text)](../<sib_lib>)`  → `[(text)](./<sib_slug>.md)`
        sed -i \
            -e "s|](\.\./${sib_lib}/)|](./${sib_slug}.md)|g" \
            -e "s|](\.\./${sib_lib})|](./${sib_slug}.md)|g" \
            "$file"
    done
}

# Prepend an autogen banner so casual readers know the canonical
# source is the upstream README, not the synced copy here.
autogen_banner() {
    local lib="$1"
    local src_path
    if [[ -n "$lib" ]]; then
        src_path="aperio-lang/pond/${lib}/README.md"
    else
        src_path="aperio-lang/pond/README.md"
    fi
    cat <<EOF
<!-- Synced from ${src_path} by tools/sync-pond-docs.sh — do not edit here. -->

EOF
}

# --- main ------------------------------------------------------------------

mkdir -p "$DEST"

# Wipe stale entries (keeps the dir reproducible run-to-run).
# We leave anything not produced by sync alone — but anything
# ending in .md inside DEST that isn't index.md or one of the
# emitted slugs gets removed at the end. Tracked here:
declare -a EMITTED=()
declare -a LIB_PATHS=()
declare -a EMIT_LIBS=()   # parallel: lib path per emitted file ("" for index)

# pond/README.md becomes the catalog overview page.
log "  -> libraries/index.md (from pond/README.md)"
if [[ "$DRY_RUN" -eq 0 ]]; then
    autogen_banner "" > "$DEST/index.md"
    cat "$POND_REPO/README.md" >> "$DEST/index.md"
    rewrite_links "$DEST/index.md" ""
    # The index lives one level up from per-lib pages; its
    # references to per-lib paths need adjusting too.
    sed -i \
        -e 's|vendor/pond/\([a-zA-Z0-9_/-]*\)|libraries/\1|g' \
        "$DEST/index.md"
fi
EMITTED+=("index.md")
EMIT_LIBS+=("")

# Walk pond/<lib>/README.md and pond/<group>/<lib>/README.md.
while IFS= read -r src; do
    rel="${src#$POND_REPO/}"
    lib="${rel%/README.md}"

    # Skip the top README and excluded paths.
    if [[ "$lib" == "$rel" ]]; then continue; fi
    if [[ "$lib" =~ $EXCLUDE_RE ]]; then
        log "  -- skip $lib (excluded)"
        continue
    fi

    slug="$(slug_for "$lib")"
    dst="$DEST/${slug}.md"
    log "  -> libraries/${slug}.md (from pond/${lib}/README.md)"

    if [[ "$DRY_RUN" -eq 0 ]]; then
        autogen_banner "$lib" > "$dst"
        cat "$src" >> "$dst"
        rewrite_links "$dst" "$lib"
    fi
    EMITTED+=("${slug}.md")
    EMIT_LIBS+=("$lib")
    LIB_PATHS+=("$lib")
done < <(find "$POND_REPO" -mindepth 2 -maxdepth 3 -name README.md \
    -not -path "*/examples/*" -not -path "*/_util/*" \
    -not -path "*/vendor/*" \
    | sort)

# Now that LIB_PATHS is fully populated, do a second pass to
# rewrite cross-lib sibling links in every emitted file. The
# first pass only knew about earlier-walked libs. Pass the
# correct lib (parallel-array EMIT_LIBS) so the first-pass
# rewrites stay idempotent.
if [[ "$DRY_RUN" -eq 0 ]]; then
    for i in "${!EMITTED[@]}"; do
        rewrite_links "$DEST/${EMITTED[$i]}" "${EMIT_LIBS[$i]}"
    done
fi

# Clean up stale generated files.
if [[ "$DRY_RUN" -eq 0 ]]; then
    for existing in "$DEST"/*.md; do
        [[ -f "$existing" ]] || continue
        name="$(basename "$existing")"
        keep=0
        for emit in "${EMITTED[@]}"; do
            if [[ "$emit" == "$name" ]]; then keep=1; break; fi
        done
        if [[ "$keep" -eq 0 ]]; then
            log "  -- stale: removing $name"
            rm "$existing"
        fi
    done
fi

log ""
log "Synced ${#EMITTED[@]} files into ${DEST}"
if [[ "$DRY_RUN" -eq 1 ]]; then
    log "(dry run — files written to /tmp/pond-docs-dry/)"
fi

# Print SUMMARY.md verification list for sanity.
log ""
log "SUMMARY.md should reference these slugs:"
for emit in $(printf '%s\n' "${EMITTED[@]}" | sort); do
    [[ "$emit" == "index.md" ]] && continue
    slug="${emit%.md}"
    log "  - [${slug}](./libraries/${emit})"
done
