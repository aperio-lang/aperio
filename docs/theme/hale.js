// Custom highlight.js mode for Hale code blocks.
//
// mdbook 0.4.x / 0.5.x ships highlight.js 10.1.1, which:
//   * accepts keywords only as space-separated strings (NOT arrays)
//   * exposes `hljs.highlightBlock` (NOT v11's `highlightElement`)
// This file is written against v10 conventions and feature-detects
// the highlight fn so it keeps working if mdbook upgrades.
//
// Load-order workaround: mdbook's `additional-js` always loads AFTER
// book.js, which runs hljs.highlightBlock at top-level on every code
// block — so by the time this file runs, the initial highlight pass
// has already happened and ```hale blocks fell through (warning:
// "Could not find the language 'hale'"). After registering the
// language below, the trailing IIFE rehighlights every
// `code.language-hale` block: reset its textContent (collapses any
// stale spans), strip the `.hljs` marker class, then call
// highlightBlock again — now the language IS registered so it
// produces real output.
//
// Aliased to `ap` so ```ap fences also work.

hljs.registerLanguage('hale', function (hljs) {
  // v10 wants space-separated strings, one per keyword category.
  var KEYWORDS = {
    keyword:
      // Declaration
      'locus type perspective interface topic import const fn module main ' +
      // Locus members
      'params contract bus capacity mode closure bindings ' +
      // Lifecycle
      'birth accept run drain dissolve on_failure ' +
      // Statement / control flow
      'let mut if else match for in while return break continue yield as ' +
      // Contract
      'expose consume inferred ' +
      // Bus
      'subscribe publish of ' +
      // Mode
      'bulk harmonic resolution ' +
      // Projection
      'projection rich chunked recognition ' +
      'fixed_cell shared_slab spillover summary_only ' +
      // Schedule
      'schedule cooperative pinned ' +
      // Closure
      'epoch persists_through resets_on approx within ' +
      // Recovery
      'restart restart_in_place quarantine reorganize bubble ' +
      // Perspective
      'stable_when serialize_as ' +
      // Fallible
      'fallible fail or raise ' +
      // Capacity slot
      'pool heap indexed_by as_parent_for ' +
      // Reserved
      'trait impl async await macro where with tier self ' +
      // Transport (binding spec)
      'in_memory unix tcp nats listen connect',
    literal: 'true false nil',
    type:
      'Int Uint Float Decimal String Bool ' +
      'Time Duration Bytes ' +
      'Rich Chunked Recognition',
    built_in:
      'print println eprint eprintln ' +
      'len to_string min max abs ' +
      'sum prod ' +
      'starts_with contains'
  };

  return {
    name: 'Hale',
    aliases: ['ap'],
    keywords: KEYWORDS,
    contains: [
      hljs.C_LINE_COMMENT_MODE,
      hljs.C_BLOCK_COMMENT_MODE,

      // f-string with {expr} interpolation
      {
        className: 'string',
        begin: 'f"', end: '"',
        contains: [
          hljs.BACKSLASH_ESCAPE,
          {
            className: 'subst',
            begin: /\{/, end: /\}/,
            keywords: KEYWORDS
          }
        ]
      },
      // Raw string r"..."
      { className: 'string', begin: 'r"', end: '"' },
      // Triple-quoted string (multi-line)
      { className: 'string', begin: '"""', end: '"""' },
      // Bytes literal b"..."
      {
        className: 'string',
        begin: 'b"', end: '"',
        contains: [hljs.BACKSLASH_ESCAPE]
      },
      // Regular string "..."
      hljs.QUOTE_STRING_MODE,
      // Time literal: `2026-05-08T12:00:00Z`
      { className: 'string', begin: '`', end: '`' },

      // Decimal literal: 1.50d, 0.05d
      { className: 'number', begin: /\b\d[\d_]*\.\d+d\b/ },
      // Duration literal: 5s, 100ms, 1h30m, etc.
      { className: 'number', begin: /\b\d+(?:ns|us|ms|s|m|h|d)\b/ },
      // Hex / oct / bin / decimal / float / typed-suffix numbers
      {
        className: 'number',
        begin: /\b(?:0x[0-9a-fA-F_]+|0o[0-7_]+|0b[01_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?)(?:[iuf](?:8|16|32|64|128))?\b/
      },

      // Annotation: @form(vec), @projection, etc.
      { className: 'meta', begin: /@[a-zA-Z_][a-zA-Z0-9_]*/ },

      // Hale-specific operators
      { className: 'operator', begin: /<-|~~/ }
    ]
  };
});

// Re-highlight any hale code blocks that book.js processed before
// our language was registered. See file header.
(function rehighlightHaleBlocks() {
  if (typeof document === 'undefined' || typeof hljs === 'undefined') return;
  // Feature-detect: v10 uses highlightBlock; v11 uses highlightElement.
  var highlight = hljs.highlightElement || hljs.highlightBlock;
  if (!highlight) return;
  var blocks = document.querySelectorAll(
    'pre code.language-hale, pre code.language-ap'
  );
  Array.prototype.forEach.call(blocks, function (block) {
    // Reset to plain text: collapses any hljs-* spans from the
    // first pass.
    var src = block.textContent;
    block.textContent = src;
    // Strip the .hljs marker so highlightBlock processes again.
    block.classList.remove('hljs');
    // Strip any hljs-* state classes the first pass added.
    Array.prototype.slice.call(block.classList).forEach(function (c) {
      if (c.indexOf('hljs-') === 0) block.classList.remove(c);
    });
    try {
      highlight.call(hljs, block);
    } catch (e) {
      // Leave plain on failure.
    }
  });
})();
