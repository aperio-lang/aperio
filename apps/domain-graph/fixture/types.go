package main

// Each of these type declarations exercises one branch of the
// morpheme rewriter:
//
//   - Controller    → lookup hit → "controlling"
//   - Repository    → lookup hit → "carrying"
//   - Cache         → lookup hit → "remembering"
//   - Greeter       → -er suffix rule (not in lookup) → "greeting"
//   - OrderProcessor → CamelCase split + lookup → "ordering-processing"
//   - UserValidator  → CamelCase split + lookup → "using-checking"
//                     (User isn't in lookup → -er suffix? no →
//                      <unknown:User>; Validator → checking)
//   - Foobaz        → no suffix, no lookup → <unknown:Foobaz>

type Controller struct {
	current string
}

type Repository struct {
	items []string
}

type Cache struct {
	store map[string]string
}

type Greeter struct {
	prefix string
}

type OrderProcessor struct {
	queue chan int
}

type UserValidator struct {
	rules []string
}

type Foobaz struct {
	x int
}
