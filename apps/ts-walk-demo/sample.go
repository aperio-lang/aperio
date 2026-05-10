package main

import "fmt"

type Greeter struct {
	name string
}

func (g *Greeter) Hello() string {
	return fmt.Sprintf("hello, %s", g.name)
}

func main() {
	g := &Greeter{name: "world"}
	fmt.Println(g.Hello())
}
