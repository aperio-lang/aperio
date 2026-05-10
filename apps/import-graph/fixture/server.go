package main

import (
	"net/http"
	"strconv"
)

func serve(port int) error {
	addr := ":" + strconv.Itoa(port)
	return http.ListenAndServe(addr, nil)
}
