package main

import (
	"log"
	"net/http"
)

func init() {
	log.SetFlags(log.LstdFlags | log.Lmicroseconds)
}

func main() {
	mux := http.NewServeMux()
	mux.HandleFunc("/hello", helloHandler)
	mux.HandleFunc("/status", statusHandler)

	go backgroundWorker()

	log.Fatal(http.ListenAndServe(":8080", mux))
}
