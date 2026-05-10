package main

import "sync"

// RequestCache remembers responses keyed by URL — exercises the
// domain extractor's lookup hit (Cache → remembering) plus the
// CamelCase split (Request → unknown, Cache → remembering).
type RequestCache struct {
	mu    sync.Mutex
	store map[string]string
}

// SessionManager owns long-lived auth state for the running
// server — Manager → managing via lookup table.
type SessionManager struct {
	cache *RequestCache
}

// AuditLogger writes to disk asynchronously — Logger isn't in
// the seed lookup, so this falls to the suffix rule
// (-er → strip + -ing → "logging") with the "Audit" morpheme
// also passing min-stem (5 chars; n<6 is true, so it falls to
// unknown — the honest output).
type AuditLogger struct {
	queue chan string
}
