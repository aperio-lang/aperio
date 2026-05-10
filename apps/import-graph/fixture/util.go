package main

// No imports here — exercises the "file with package_clause but no
// import_declaration" path through the extractor.

const greeting = "hi"

func defaultName() string {
	return "world"
}
