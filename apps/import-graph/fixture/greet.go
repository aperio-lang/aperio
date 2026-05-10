package main

import (
	"fmt"
	"strings"
)

func greet(name string) string {
	if strings.TrimSpace(name) == "" {
		return "hello"
	}
	return fmt.Sprintf("hello, %s", name)
}
