package main

import (
	"fmt"
	"net/http"
)

func helloHandler(w http.ResponseWriter, r *http.Request) {
	fmt.Fprintln(w, "hello")
}

func statusHandler(w http.ResponseWriter, r *http.Request) {
	fmt.Fprintln(w, "ok")
}
