# vscode-rust

I decided to write a port of [VS Code](https://github.com/microsoft/vscode)
in Rust.

Honestly — I have no idea how big this is going to get, how well I'll
pull it off, or whether I'll pull it off at all. Realistically, nothing
commercially serious is going to come out of this: VS Code is tens of
thousands of TypeScript files, thousands of features, LSP, debugger,
extensions, terminal. A solo project from the outside is unlikely to
match all of that.

But I'm just curious. I want to see how far you can get by methodically
porting things file by file from TypeScript + Electron to Rust + egui.
Which components I can match pixel-for-pixel, where I'll have to give
up. Where Rust ends up being a better fit, and where it just slows me
down.

A side-project in the spirit of "because it's fun".

## Status

This code isn't even close to an alpha — it's in an embryonic state.
There are plenty of bugs, lots of half-finished pieces, and only a vague
plan for what to do next. Treat it as a sketch, not a product.

## Run

```bash
cargo run                              # the editor itself
cargo run --example widget_showcase    # interactive UI component storybook
cargo test                             # tests + pixel-diff snapshots
```
