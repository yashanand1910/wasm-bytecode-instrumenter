# [WIP] WebAssembly Bytecode Instrumenter

Injects instrumentation bytecode directly into Wasm bytecode to perform some simple dynamic analyses. Built using [Walrus](https://github.com/rustwasm/walrus), a Wasm transformation library.

This was implemented as part of an experiment comparing various instrumentation implementations with the
instrumentation capabilities offered by the [Wizard Engine](https://github.com/titzer/wizard-engine).

### Monitors

- **Hotness monitor**: Inserts counting bytecode at every instruction and then produces a summary of hot execution paths.

- **Branch monitor**: Instruments all `if`, `br_if` and `br_table` instructions in the program and uses the top-of-stack to predict the direction each branch will take.

### Usage

```bash
./wasm-bytecode-instrumenter <monitor> <filename>
```

This should generate a new Wasm program injected with instrumentation bytecode that you can run using
any Wasm engine that supports [multi-memory](https://github.com/WebAssembly/multi-memory) as it uses a separate memory region to store counts.

### WIP
- Print captured info
- Loop monitor

### Paper

WIP
