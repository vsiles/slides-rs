# Welcome to Slides

A terminal-based presentation tool written in Rust

---

# Features

## Core Capabilities

+ Parse markdown into slides
+ Beautiful terminal UI with ratatui
+ Simple navigation
+ Syntax highlighting for code

## Advanced Features

+ Sequential reveals (like reveal.js!)
+ Inline code formatting
+ Image support with flexible sizing

---

# Getting Started

## Installation

Build from source with Cargo:

```bash
cargo build --release
```

## Running Your First Presentation

```bash
./target/release/slides example.md
```

---

# Markdown Syntax

## Slide Separators

Slides are separated by horizontal rules (`---`)

## Supported Elements

Each slide can contain:

+ Headings (# and ##)
+ Paragraphs
+ Lists (ordered and unordered)
+ Code blocks with syntax highlighting
+ Images

---

# Code Examples

## Rust

Here's some Rust code:

```rust
fn main() {
    println!("Hello, slides!");
}
```

## Python

And here's Python:

```python
def greet(name):
    print(f"Hello, {name}!")
```

---

# Navigation

## Keyboard Shortcuts

### Moving Between Slides

+ Arrow keys (← →) to move between slides
+ Space bar to go forward
+ h/l (vim-style) also work

### Other Controls

+ q or Esc to quit
+ Progressive reveals with right arrow

---

# Sequential Reveals

## How They Work

Lines prefixed with `+` will be revealed incrementally:

+ First item appears
+ Then the second
+ And finally the third

This is perfect for bullet points you want to reveal one at a time!

## Benefits

+ Keeps audience focused
+ Controls information flow
+ Creates engaging presentations

---

# Images

## Basic Syntax

You can display PNG images in your slides!

Just use standard markdown image syntax:

`![](./atuin.png)`

## Sizing Options

### Width only (maintains aspect ratio)
`![80x](image.png)` or `![80](image.png)`

### Height only (maintains aspect ratio)
`![x40](image.png)` or `![:40](image.png)`

### Both dimensions
`![80x40](image.png)` or `![80:40](image.png)`

---

# Image Example

## Atuin Logo


![80x](./atuin.png)

A cute turtle mascot!

---

# Advanced Topics

## Terminal Compatibility

### Supported Protocols

+ Kitty graphics protocol
+ iTerm2 inline images
+ Sixel
+ Halfblocks (fallback)

### Best Experience

Use modern terminals like Kitty, WezTerm, or iTerm2 for optimal image quality.

---

# Tips & Tricks

## Presentation Design

### Keep It Simple

+ One main idea per slide
+ Use reveals to control pacing
+ Images should support your message

### Code Examples

+ Syntax highlighting is automatic
+ Keep code snippets short and focused
+ Explain complex code with bullets

---

# That's It!

## Thank You!

Thanks for trying out the slides application!

## Get Started

Build your own presentations in markdown.

### Resources

+ GitHub repository
+ Documentation
+ Example presentations
