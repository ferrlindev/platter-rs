# platter-rs

A templating engine for [Scout](https://scout-lang.netlify.app/) (`.sct`) web-crawling scripts, written in Rust.

Scout scripts are powerful but repetitive — the same CSS selectors, base URLs, and depth limits appear across every file.  
`platter` lets you write a single parameterised template and render it into any number of ready-to-run `.sct` files.

---

## Quick start

```bash
# Install (requires Rust / Cargo)
cargo install --path .

# Render a template
platter examples/product-listing.sct.tmpl \
  --var base_url="" \
  --var category=computer/laptops \
  --var page=1 \
  --var max_pages="3" \
  --var debug=true  \
  --var name_selector="" \
  --var description_selector="" \
  --var product_selector="" \
  --var scrape_ratings=true \
  --var price_selector="" \
  --var rating_selector="" \
  --var max_items=2 \
  --out examples/product-listing.sct

# Run the rendered script
scout product-listing.sct
```

---

## Template syntax

Platter Templates are ordinary `.sct` files with extra markers that are expanded at render time. Everything outside a marker is passed through verbatim, so all valid Scout syntax is preserved exactly.

### Variable interpolation — `{{ }}`

```
goto "{{ base_url }}/{{ category }}"
el = $"{{ selector }}"
```

### Filters — `{{ value | filter }}`

Filters are chained with `|`:

```
goto "{{ url | trim | lower }}"
el = $"{{ css | escape_selector }}"
el = {{ css | selector }}       {# expands to: el = $".my-class" #}
```

| Filter | Input | Description |
|---|---|---|
| `upper` / `lower` | Str | Change case |
| `trim` | Str | Strip whitespace |
| `capitalize` | Str | First char uppercase |
| `truncate(N)` | Str | Truncate to N chars, append `…` |
| `replace("from,to")` | Str | String replacement |
| `quote` | Str | Wrap in `"…"` (Scout string literal) |
| `escape_selector` | Str | CSS-escape special characters |
| `selector` | Str | Produce `$"…"` Scout selector expression |
| `multi_selector` | Str | Produce `$$"…"` Scout multi-selector expression |
| `default("fallback")` | Any | Use fallback if value is falsy |
| `first` / `last` | List | First or last element |
| `length` | List\|Str | Element / character count |
| `join(", ")` | List | Join list into string |
| `reverse` | List\|Str | Reverse order |
| `abs` | Int\|Float | Absolute value |
| `int` / `string` | Any | Type coercion |

### Comments — `{# #}`

Template comments are stripped from the output entirely:

```
{# This comment never appears in the rendered .sct file #}
goto "{{ url }}"  {# inline comment #}
```

### Conditionals — `{% if %}`

```
{% if scrape_text %}
el = $".article" |> textContent() |> print()
{% elif print_url %}
url() |> print()
{% else %}
results()
{% endif %}
```

Conditions support equality comparisons:

```
{% if env == "prod" %}
// production path
{% endif %}
```

Comparison operators: `==`, `!=`, `<`, `>`, `<=`, `>=`.

### For loops — `{% for %}`

Iterate over a list variable provided in the context:

```
{% for keyword in keywords %}
goto "https://search.example.com?q={{ keyword }}"
$".result-title" |> textContent() |> print()
{% endfor %}
```

### Set — `{% set %}`

Define or redefine a variable within the template:

```
{% set base = "https://example.com" %}
goto "{{ base }}/products"
```

### Include — `{% include %}`

Emit a Scout `use` import statement:

```
{% include "std/utils.sct" %}
{# renders as: use std::utils #}
```

### Raw blocks — `{% raw %}`

Pass content through completely unprocessed — useful for documenting template syntax inside a template:

```
{% raw %}
{{ this_will_not_be_expanded }}
{% endraw %}
```

---

## CLI reference

```
platter <template> [OPTIONS]

OPTIONS:
  -v, --var key=value    Set a template variable (repeatable)
  -o, --out file.sct     Write output to file (default: stdout)
  -h, --help             Show help

ENVIRONMENT:
  Any env var prefixed with SCOUT_ is available as a lowercase template variable.
  SCOUT_BASE_URL=https://example.com  →  {{ base_url }}
  
  CLI --var flags override environment variables.
```

---

## Library usage

```rust
use platter_rs::{Context, Value, render_str};

let mut ctx = Context::new();
ctx.set("url",      "https://shop.example.com/laptops");
ctx.set("selector", ".product-card");
ctx.set("keywords", Value::List(vec![
    Value::Str("laptop".into()),
    Value::Str("notebook".into()),
]));

let template = r#"
goto "{{ url }}"
for row in $$"{{ selector }}" do
  scrape { name: $(row)"h2" |> textContent() }
end
"#;

let script = render_str(template, &ctx)?;
std::fs::write("scraper.sct", script)?;
```

---

## Examples

| Template | Description |
|---|---|
| `examples/product-listing.sct.tmpl` | Paginated product listing scraper |
| `examples/site-crawl.sct.tmpl` | Depth-limited site crawler with content extraction |
| `examples/authenticated-scrape.sct.tmpl` | Login flow then scrape behind auth |

---

## Architecture

```
src/
  lexer.rs    — Tokenises template source into Raw / Expr / Block / Comment tokens
  parser.rs   — Builds an AST (Node enum) from the token stream
  context.rs  — Scoped variable store with Value enum
  renderer.rs — Walks the AST, evaluates expressions, applies filters, emits Scout source
  error.rs    — TemplateError type with Display impl
  lib.rs      — Public API: render_str(), render(), Context, Value
  main.rs     — CLI binary
tests/
  integration.rs — 29 end-to-end tests covering all features
```

---

## License

MIT OR Apache-2.0*
