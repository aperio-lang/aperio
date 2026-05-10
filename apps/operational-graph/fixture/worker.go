package main

import (
	"log"
	"time"
)

// backgroundWorker is the entry point for a long-lived goroutine
// spawned from main. The infinite for { } loop is the operational
// signal: this is a `run()` body equivalent, not a one-shot
// helper. The extractor should pick up both the spawn site
// (`go backgroundWorker()` in main) and the long-loop here.
func backgroundWorker() {
	tick := time.NewTicker(1 * time.Second)
	defer tick.Stop()
	for {
		select {
		case <-tick.C:
			log.Println("tick")
			go fanout()
		}
	}
}

func fanout() {
	// Anonymous-function goroutine — exercises the
	// `go func() { ... }()` shape, distinct from
	// `go someFunc()`.
	go func() {
		log.Println("fanned out")
	}()
}
