# Lev

Lev is an experimental programming language and compiler written in Rust.

The language is designed as a mix of ideas from **C#** and **TypeScript**, with a statically typed syntax and a compiler pipeline that lowers source code through several intermediate representations before generating x86-64 assembly.

> Lev is currently an early-stage compiler project. The architecture is being developed incrementally, and many language and backend features are not implemented yet.

## Current Pipeline

```text
Lev source
    ↓
Lexer
    ↓
Parser
    ↓
AST
    ↓
Semantic analysis
    ↓
HIR
    ↓
IR
    ↓
SSA construction
    ↓
IR validation
    ↓
x86-64 code generation
    ↓
Assembly
```

## Project Structure

```text
lev/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs
│   │
│   ├── compiler/
│   │   ├── mod.rs
│   │   ├── source.rs
│   │   └── diagnostics.rs
│   │
│   ├── lexer/
│   │   ├── mod.rs
│   │   ├── lexer.rs
│   │   └── token.rs
│   │
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── parser.rs
│   │   └── ast.rs
│   │
│   ├── semantic/
│   │   ├── mod.rs
│   │   ├── analyzer.rs
│   │   ├── hir.rs
│   │   ├── scope.rs
│   │   ├── symbols.rs
│   │   └── types.rs
│   │
│   ├── ir/
│   │   ├── mod.rs
│   │   ├── ir.rs
│   │   ├── lower.rs
│   │   ├── validate.rs
│   │   └── ssa.rs
│   │
│   └── codegen/
│       ├── mod.rs
│       └── codegen.rs
│
└── examples/
    └── hello.lev
```

## Requirements

* Rust
* Cargo
* An x86-64 assembler and linker if you want to build generated assembly into an executable

Install Rust through the official Rust toolchain if it is not already installed.

## Building

Build the compiler with Cargo:

```bash
cargo build
```

For a release build:

```bash
cargo build --release
```

## Running

The compiler accepts a `.lev` source file:

```bash
cargo run -- examples/hello.lev
```

The current compiler prints the generated x86-64 assembly to standard output.

You can also run the compiled binary directly:

```bash
cargo run --release -- examples/hello.lev
```

## Example Program

```lev
fn main(): int {
    let x: int = 10;
    x = 20;
    return x;
}
```

A control-flow example:

```lev
fn main(): int {
    let x: int = 10;

    while (x == 10) {
        if (x == 10) {
            break;
        } else {
            continue;
        }
    }

    x = 20;
    return x;
}
```

The compiler currently supports the following general language constructs:

* Functions
* Integer values
* String values
* Boolean values
* Local variable declarations
* Assignment
* Return statements
* Expression statements
* `if` / `else`
* `while`
* `break`
* `continue`
* Basic binary operations
* Function and variable symbols
* Static type checking

## Supported Syntax

### Function

```lev
fn main(): int {
    return 42;
}
```

### Variable Declaration

```lev
let value: int = 10;
```

### Assignment

```lev
let value: int = 10;
value = 20;
```

### Boolean Values

```lev
let enabled: bool = true;
let disabled: bool = false;
```

### Conditional

```lev
if (value == 10) {
    return 1;
} else {
    return 0;
}
```

### While Loop

```lev
while (value == 10) {
    value = 20;
}
```

### Break and Continue

```lev
while (true) {
    if (condition) {
        break;
    } else {
        continue;
    }
}
```

### Binary Operations

Currently defined binary operations include:

```text
+
-
*
/
==
```

## Compiler Architecture

### Lexer

The lexer converts source text into tokens.

Examples of tokens include:

* Identifiers
* Integer literals
* String literals
* Keywords
* Operators
* Punctuation
* Type names

### Parser

The parser converts tokens into an abstract syntax tree, or AST.

The AST represents source-level constructs such as:

* Functions
* Blocks
* Variable declarations
* Assignments
* Expressions
* Conditions
* Loops
* Returns

### Semantic Analysis

Semantic analysis checks the AST and converts it into a higher-level intermediate representation called HIR.

This stage is responsible for:

* Resolving identifiers
* Resolving functions
* Checking variable declarations
* Checking assignment types
* Checking expression types
* Checking return types
* Tracking scopes
* Assigning local and symbol IDs

### HIR

HIR removes some source-level details and represents resolved variables and functions using IDs.

For example, a source-level variable:

```lev
let x: int = 10;
```

becomes a HIR local with a `LocalId`.

### IR

The IR represents control flow using basic blocks.

An IR function contains:

* Parameters
* Locals
* Basic blocks
* Instructions
* A terminator for each block

Supported terminators include:

```text
Jump
Branch
Return
Unreachable
```

This makes structured source code such as `if`, `while`, `break`, and `continue` explicit as control-flow edges.

### SSA

The compiler converts the IR into Static Single Assignment form.

In SSA form:

* Each value is assigned once
* Values are represented by `ValueId`
* Control-flow merges may use `Phi` instructions
* Local loads and stores can be removed during SSA construction

For example:

```text
x = 10
x = 20
return x
```

can become a sequence of SSA values where the returned value refers directly to the second definition.

### IR Validation

The compiler validates IR before and after SSA construction.

Validation checks include:

* Valid block references
* Valid local references
* Valid value references
* Correct function calls
* Correct return types
* Control-flow consistency
* SSA-specific constraints

### Code Generation

The current backend generates readable x86-64 assembly using Intel syntax.

The backend currently uses a simple stack-slot strategy:

```text
ValueId(0) → [rbp-8]
ValueId(1) → [rbp-16]
ValueId(2) → [rbp-24]
```

This approach is intentionally simple and avoids implementing register allocation at the current stage.

The backend currently supports:

* Integer constants
* Boolean constants
* Integer arithmetic
* Integer division
* Equality comparisons
* Basic blocks
* Conditional branches
* Unconditional jumps
* Return instructions
* Basic phi elimination through edge copies

The backend does not yet implement all IR instructions.

## Generated Assembly

For a simple function such as:

```lev
fn main(): int {
    let x: int = 10;
    x = 20;
    return x;
}
```

the generated assembly follows this general structure:

```asm
.intel_syntax noprefix
.text
.globl main

main:
    push rbp
    mov rbp, rsp

    ; generated instructions

    leave
    ret
```

The generated assembly is currently printed rather than automatically assembled and linked.

## Phi Elimination

SSA `Phi` instructions do not directly correspond to x86-64 instructions.

The current backend handles them by converting each incoming phi value into a copy on the corresponding predecessor edge.

Conceptually:

```text
v3 = phi(block1: v1, block2: v2)
```

becomes:

```text
block1 → copy v1 → v3
block2 → copy v2 → v3
```

Copies are emitted before the predecessor block's terminator.

The backend also includes a temporary stack slot for breaking parallel-copy cycles.

## Current Limitations

The project is still under active development.

Known limitations include:

* No complete executable-generation command yet
* No register allocator
* No optimization pipeline
* No dead-code elimination
* No unreachable-block cleanup
* No complete function-call code generation
* No parameter code generation
* No string code generation
* No complete struct code generation
* No advanced type system
* No arrays
* No modules
* No generics
* No exceptions
* No standard library
* No runtime
* No complete error-reporting format
* No cross-platform backend abstraction

The current backend is primarily intended to validate the compiler pipeline and produce simple working assembly.

## Development

Run the compiler's checks:

```bash
cargo check
```

Format the code:

```bash
cargo fmt
```

Run Clippy:

```bash
cargo clippy
```

Build and run tests:

```bash
cargo test
```

## Development Roadmap

Planned milestones include:

1. Finish basic x86-64 code generation
2. Support function parameters
3. Support function calls
4. Implement proper phi lowering
5. Add unreachable-block cleanup
6. Add dead-code elimination
7. Add constant folding
8. Add a simple register allocator
9. Generate assembly files directly
10. Assemble and link generated programs
11. Add structs and field access
12. Add arrays and indexing
13. Add a standard library
14. Improve diagnostics
15. Add a stable command-line interface
16. Add more complete language features

## License

This project does not currently specify a license.
