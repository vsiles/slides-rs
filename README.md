# slides-rs

A terminal-based presentation tool for Markdown slides, inspired by [slides](https://github.com/maaslalani/slides).

## Usage

```bash
slides presentation.md
```

### Navigation
- `←`/`→` or `h`/`l` - Navigate between slides
- `Space` - Next slide
- `q` or `Esc` - Quit

### Slide Format
Separate slides with `---`:

```markdown
# Slide 1
Content here

---

# Slide 2
More content
```

## Image Support

### Basic Usage
```markdown
![](image.png)
```

### Sizing Options
```markdown
![80x](image.png)     # Width only (maintains aspect ratio)
![x40](image.png)     # Height only (maintains aspect ratio)  
![80x40](image.png)   # Both dimensions
```

### Image Restrictions
- **Images must be on their own line** - inline images are not supported
- **Relative paths only** - images are resolved relative to the markdown file
- Supported formats: PNG, JPEG, GIF, etc.

**❌ Invalid (inline):**
```markdown
Text ![](image.png) more text
```

**✅ Valid (separate line):**
```markdown
Text

![](image.png)

More text
```

## Sequential Reveals
Use `+` prefix for incremental reveals:

```markdown
+ First point
+ Second point  
+ Third point
```

## Building

```bash
# With Cargo
cargo build --release

# With Nix
nix build
```