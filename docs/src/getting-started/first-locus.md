# Your first locus

Save the following as `hello.ap`:

```aperio
locus Greeter {
    params { name: String = "world"; }
    birth() { println("hello, ", self.name); }
}

fn main() {
    Greeter { };
    Greeter { name: "Aperio" };
}
```

Run it interpreted:

```sh
aperio run hello.ap
```

You should see:

```
hello, world
hello, Aperio
```

## What just happened

`Greeter` is a **locus**: a typed unit with a lifecycle.
`params` declares its configurable state with defaults;
`birth()` is the lifecycle method that runs when an instance is
constructed.

`Greeter { }` constructs an instance using the default `name`;
`Greeter { name: "Aperio" }` overrides it. Both instances run
their `birth()` body to completion, then dissolve at the end of
the surrounding statement.

That's the smallest possible Aperio program: a locus with one
field and one lifecycle method, instantiated twice at statement
position. Every program is built out of compositions of this
same primitive — `locus` declarations with `params`, lifecycle
methods, and (as you'll see next) bus interfaces and methods.

## Next

Continue to [A small program with shape](./a-small-program.md)
to see two loci communicating across the typed bus. After that,
the **Concepts** chapters walk through the structural model in
depth, and the **How-tos** show common recipes — HTTP server,
CLI parsing, file I/O, multi-binary deployment, and more.
