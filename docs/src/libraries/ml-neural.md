<!-- Synced from aperio-lang/pond/ml/neural/README.md by tools/sync-pond-docs.sh — do not edit here. -->

# pond/ml/neural — toy NN trainer with dense layers + autograd-lite

Suggested alias: `nn`.

```aperio
import "vendor/pond/math/matrix" as mat;
import "vendor/pond/ml/neural"   as nn;

let mx = mat::Mat { };

let model = nn::Model {
    name:       "xor",
    last_error: nn::NnError { },
    rng_state:  42,
};
model.add_dense(2, 3, "sigmoid");
model.add_dense(3, 1, "sigmoid");

let xs = mx.from_rows(4, 2, "0,0, 0,1, 1,0, 1,1");
let ys = mx.from_rows(4, 1, "0, 1, 1, 0");

let trainer = nn::Trainer {
    lr:         0.5,
    batch_size: 4,
    last_error: nn::NnError { },
};
trainer.fit(model, xs, ys, 5000);

let x = mx.from_rows(2, 1, "1.0, 0.0");
let p = model.forward(x);                 // 3x1 → 1x1 col
println(p.get(0) or 0.0);                  // ~0.98
```

## Scope — what "autograd-lite" means here

This is a *toy* trainer designed for tiny supervised problems
(XOR, the canonical "smallest non-linear-separable" benchmark;
the MNIST-scale problems CONTRACTS.md mentions are aspirational
at v1 — you can train a 784-128-10 MLP with this surface but the
single-sample SGD inner loop will be uncompetitive with a
production framework).

What you get:

- **Dense layers** — `add_dense(input_dim, output_dim, activation)`
  appends one fully-connected layer to the model. Activations:
  `"sigmoid" | "relu" | "tanh" | "linear"`.
- **No graph, no tape.** Backward is hand-coded against the
  forward pass inside `Model.train_step`. The four supported
  activations have closed-form derivatives wired into
  `NnOps.backprop_activation`. New activations require editing
  `activate_one` + `activation_deriv` together.
- **SGD only.** One sample, one update; no Adam / momentum /
  weight-decay. Per-sample order is the dataset's row order
  (no shuffling at v1).
- **MSE loss.** Mean-squared-error over the final layer's output
  vs. target. Cross-entropy / NLL are not wired in.
- **Per-epoch reporting.** `Trainer.fit` publishes one
  `TrainStepEvent` on the bus per epoch with `loss / epoch_idx /
  step`. The example wires a `TrainLogger` subscriber.
- **MOA owner.** `Model` satisfies the moa::MoaOwner shape —
  `version()`, `snapshot_bytes()`, `apply_delta(bytes)`. Snapshot
  encodes meta_csv + the full params buffer as ASCII; any
  consumer with std::str can round-trip.

## Surface

### `nn::Layer` — pure-data record

```aperio
type Layer {
    name:           String;
    input_dim:      Int;
    output_dim:     Int;
    activation:     String;        // "sigmoid" | "relu" | "tanh" | "linear"
    weights_offset: Int;           // window into Model.params
    weights_count:  Int;
    biases_offset:  Int;
    biases_count:   Int;
}
```

CONTRACTS.md declares `weights: Matrix; biases: Matrix;` —
deviated to (offset, count) Int windows because v1 type records
can't carry locus refs (FRICTION.md "layer-stores-windows-not-matrices").
The Matrix surface is preserved everywhere it matters: the
read-side helpers `Model.load_weights(layer)` and
`Model.load_biases(layer)` rebuild a fresh Matrix per call.

### `nn::Model` — owner of weights + biases (MoaOwner)

```aperio
@form(vec)        // params buffer: heap of Float
locus Model {
    params {
        name:        String;
        meta_csv:    String;       // "in,out,act;in,out,act;..."
        layer_count: Int;
        params_len:  Int;
        gen:         Int;
        era:         Int;
        last_error:  NnError;
        rng_state:   Int;          // LCG seed for Xavier init
    }
    fn add_dense(input_dim: Int, output_dim: Int, activation: String);
    fn forward(x: Matrix) -> Matrix;          // shape mismatch → error sentinel
    fn train_step(x: Matrix, y: Matrix, lr: Float) -> Float;   // NaN on shape err
    fn layer_at(i: Int) -> Layer;
    fn load_weights(layer: Layer) -> Matrix;
    fn load_biases(layer: Layer) -> Matrix;

    // MoaOwner satisfaction (non-fallible per two-channel rule;
    // errors land on `self.last_error`).
    fn version() -> NnVersion;
    fn snapshot_bytes() -> Bytes;
    fn apply_delta(d: Bytes);
}
```

`forward(x)` returns the matrix error sentinel
(`mat::Mat.is_error(out)`) on shape mismatch and stashes a
populated `last_error`. `train_step(x, y, lr)` does forward +
backward + per-sample SGD update; returns the sample's MSE loss
(or `mat::Mat.nan_sentinel()` on shape mismatch).

### `nn::Trainer` — service locus, publishes TrainStepEvent

```aperio
locus Trainer {
    params { lr: Float = 0.01; batch_size: Int = 32; last_error: NnError; }
    bus { publish "nn.TrainStep" of type TrainStep; }
    fn fit(model: Model, xs: Matrix, ys: Matrix, epochs: Int);
}
```

`fit` walks the dataset epoch-by-epoch and per-sample. After each
epoch publishes a `TrainStepEvent` carrying the epoch's mean
loss. `batch_size` is accepted for forward-compat but the v1
implementation is per-sample SGD (effective batch_size=1) —
see FRICTION.md "batch-size-stub".

CONTRACTS.md declares `params { model: Model; ... }` and
`fit(xs, ys, epochs) -> () fallible(NnError)`. Both deviated:
`model` moved to a fit-arg (locus refs can't sit in another
locus's params), and the fallible(E) marker dropped (locus
methods can't carry fallible per the two-channel rule). Full
context in FRICTION.md.

### `nn::TrainStep` + `nn::TrainStepEvent` — per-epoch metric

```aperio
type TrainStep {
    loss:      Float;
    epoch_idx: Int;          // 1-indexed. Spelled epoch_idx not epoch
                             //   because `epoch` is reserved (closure
                             //   cadence clause).
    step:      Int;
}
topic TrainStepEvent { payload: TrainStep; subject: "nn.TrainStep"; }
```

### `nn::NnError` — single error payload

```aperio
type NnError { kind: String; detail: String; }
// kinds: "shape_mismatch" | "unknown_activation" | "empty_model" |
//        "decode_failed" | "snapshot_decode_failed"
```

## Internals

| File          | What it holds                                            |
|---------------|----------------------------------------------------------|
| `errors.ap`   | `NnError` payload type.                                  |
| `metrics.ap`  | `TrainStep` per-epoch metric record.                     |
| `topics.ap`   | `TrainStepEvent` bus topic with explicit `nn.TrainStep` subject (KNOWN_GOTCHAS G1 workaround). |
| `layer.ap`    | `Layer` shape type carrying (offset, count) windows.     |
| `model.ap`    | `Model` locus (`@form(vec)` of Float), `NnOps` namespace lotus (Matrix helpers — G3 workaround), `FwdCache` + `OffsetTable` per-train_step buffers, `NnVersion` shape type. |
| `trainer.ap`  | `Trainer` service locus.                                 |

`FwdCache` and `OffsetTable` are `@form(vec)` sibling loci
let-bound inside `Model.train_step` for the forward-pass
activation cache. Both dissolve at `train_step`'s scope exit.

## Example

```bash
$ aperio build \
      pond/ml/neural/examples/xor-trainer/
$ pond/ml/neural/examples/xor-trainer/xor-trainer
[xor] training 2-2-1 MLP for 5000 epochs ...
[xor]   epoch=1 loss=0.286837
[xor]   epoch=500 loss=0.0171117
[xor]   epoch=1000 loss=0.00245043
[xor]   epoch=2000 loss=0.000824435
[xor]   epoch=3000 loss=0.000486876
[xor]   epoch=4000 loss=0.000343538
[xor]   epoch=5000 loss=0.000264721
ok   trainer.last_error empty
ok   final loss 0.000264721 < 0.05
ok   XOR(0,0) = 0
ok   XOR(0,1) = 1
ok   XOR(1,0) = 1
ok   XOR(1,1) = 0
[xor] sigmoid outputs:
  f(0,0) = 0.0077656
  f(0,1) = 0.983393
  f(1,0) = 0.983591
  f(1,1) = 0.0212664
ok   snapshot round-trip clean
ok   copy XOR(0,1) = 1
ok   copy XOR(1,1) = 0
[xor] xor-trainer demo complete
```

The 2-3-1 topology trains reliably across seeds in a few
thousand epochs; a 2-2-1 net falls into the classic
local-minimum failure mode for most random initializations
(see FRICTION.md "xor-2-2-1-init-sensitive").

## See also

- `pond/math/matrix/README.md` — `Matrix` + `Mat` substrate that
  this lib stands on top of.
- `pond/moa/README.md` — the MoaOwner / MoaConsumer interfaces
  the Model satisfies.
- `pond/CONTRACTS.md § pond/ml/neural/` — the binding contract.
- `pond/ml/neural/FRICTION.md` — deviations from the contract,
  language gaps, duplication suspicions.
