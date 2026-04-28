use platter_rs::{Context, Value, render_str};

fn ctx_with(vars: &[(&str, &str)]) -> Context {
    let mut ctx: Context = Context::new();
    for (k, v) in vars {
        ctx.set(*k, *v);
    }
    ctx
}

// ── Raw passthrough ───────────────────────────────────────────────────────────

#[test]
fn raw_scout_source_passes_through() {
    let src: &'static str = r#"goto "https://example.com"
el = $".title"
el |> textContent() |> print()"#;
    let ctx = Context::new();
    assert_eq!(render_str(src, &ctx).unwrap(), src);
}

// ── Variable interpolation ────────────────────────────────────────────────────

#[test]
fn interpolates_string_variable() {
    let src: &'static str = r#"goto "{{ url }}""#;
    let ctx: Context = ctx_with(&[("url", "https://example.com")]);
    assert_eq!(
        render_str(src, &ctx).unwrap(),
        r#"goto "https://example.com""#
    );
}

#[test]
fn undefined_variable_returns_error() {
    let src: &'static str = "{{ missing }}";
    let ctx: Context = Context::new();
    assert!(render_str(src, &ctx).is_err());
}

#[test]
fn multi_variable_template() {
    let src: &'static str = r#"goto "{{ base }}/{{ path }}"
el = $"{{ selector }}"
el |> textContent() |> print()"#;
    let ctx = ctx_with(&[
        ("base", "https://example.com"),
        ("path", "products"),
        ("selector", ".item-title"),
    ]);
    let out = render_str(src, &ctx).unwrap();
    assert!(out.contains("https://example.com/products"));
    assert!(out.contains(".item-title"));
}

// ── Filters ───────────────────────────────────────────────────────────────────

#[test]
fn filter_upper() {
    let src: &'static str = "{{ name | upper }}";
    let ctx: Context = ctx_with(&[("name", "scout")]);
    assert_eq!(render_str(src, &ctx).unwrap(), "SCOUT");
}

#[test]
fn filter_lower() {
    let src: &'static str = "{{ NAME | lower }}";
    let ctx: Context = ctx_with(&[("NAME", "SCOUT")]);
    assert_eq!(render_str(src, &ctx).unwrap(), "scout");
}

#[test]
fn filter_trim() {
    let src: &'static str = "{{ padded | trim }}";
    let ctx: Context = ctx_with(&[("padded", "  hello  ")]);
    assert_eq!(render_str(src, &ctx).unwrap(), "hello");
}

#[test]
fn filter_capitalize() {
    let src: &'static str = "{{ word | capitalize }}";
    let ctx: Context = ctx_with(&[("word", "scout")]);
    assert_eq!(render_str(src, &ctx).unwrap(), "Scout");
}

#[test]
fn filter_truncate() {
    let src: &'static str = "{{ text | truncate(5) }}";
    let ctx: Context = ctx_with(&[("text", "Hello, World!")]);
    let out = render_str(src, &ctx).unwrap();
    assert!(out.starts_with("Hello"));
    assert!(out.contains("..."));
}

#[test]
fn filter_default_uses_fallback_on_empty() {
    let src: &'static str = "{{ name | default(\"unknown\") }}";
    let ctx: Context = ctx_with(&[("name", "")]);
    assert_eq!(render_str(src, &ctx).unwrap(), "unknown");
}

#[test]
fn filter_default_passes_value_if_truthy() {
    let src: &'static str = "{{ name | default(\"unknown\") }}";
    let ctx: Context = ctx_with(&[("name", "alice")]);
    assert_eq!(render_str(src, &ctx).unwrap(), "alice");
}

#[test]
fn filter_quote_wraps_in_double_quotes() {
    let src: &'static str = "el = ${{ selector | quote }}";
    let ctx: Context = ctx_with(&[("selector", ".item-title")]);
    assert_eq!(render_str(src, &ctx).unwrap(), r#"el = $".item-title""#);
}

#[test]
fn filter_selector_produces_scout_selector() {
    let src: &'static str = "{{ css | selector }}";
    let ctx: Context = ctx_with(&[("css", ".price")]);
    assert_eq!(render_str(src, &ctx).unwrap(), r#"$".price""#);
}

#[test]
fn filter_multi_selector_produces_scout_multi_selector() {
    let src: &'static str = "{{ css | multi_selector }}";
    let ctx: Context = ctx_with(&[("css", "tr")]);
    assert_eq!(render_str(src, &ctx).unwrap(), r#"$$"tr""#);
}

#[test]
fn chained_filters() {
    let src: &'static str = "{{ name | trim | upper }}";
    let ctx: Context = ctx_with(&[("name", "  scout  ")]);
    assert_eq!(render_str(src, &ctx).unwrap(), "SCOUT");
}

#[test]
fn unknown_filter_returns_error() {
    let src: &'static str = "{{ name | nonexistent }}";
    let ctx: Context = ctx_with(&[("name", "test")]);
    assert!(render_str(src, &ctx).is_err());
}

// ── Comments ──────────────────────────────────────────────────────────────────

#[test]
fn template_comments_are_stripped() {
    let src: &'static str =
        "goto \"https://webscraper.io/test-sites/e-commerce/allinone {# navigate to target #}";
    let ctx: Context = Context::new();
    let out = render_str(src, &ctx).unwrap();
    assert!(!out.contains("{#"));
    assert!(!out.contains("#}"));
    assert!(out.contains("goto"));
}

// ── If/elif/else ──────────────────────────────────────────────────────────────

#[test]
fn if_block_renders_when_true() {
    let src = r#"{% if logged_in %}goto "https://dashboard.example.com"{% endif %}"#;
    let mut ctx = Context::new();
    ctx.set("logged_in", true);
    assert!(render_str(src, &ctx).unwrap().contains("goto"));
}

#[test]
fn if_block_skipped_when_false() {
    let src = r#"{% if logged_in %}goto "https://dashboard.example.com"{% endif %}"#;
    let mut ctx = Context::new();
    ctx.set("logged_in", false);
    assert!(render_str(src, &ctx).unwrap().is_empty());
}

#[test]
fn if_else_block() {
    let src = r#"{% if use_https %}https://example.com{% else %}http://example.com{% endif %}"#;
    let mut ctx_true = Context::new();
    ctx_true.set("use_https", true);
    let mut ctx_false = Context::new();
    ctx_false.set("use_https", false);

    assert_eq!(render_str(src, &ctx_true).unwrap(), "https://example.com");
    assert_eq!(render_str(src, &ctx_false).unwrap(), "http://example.com");
}

#[test]
fn if_elif_else_block() {
    let src = r#"{% if env == "prod" %}production{% elif env == "stage" %}staging{% else %}local{% endif %}"#;
    let mut ctx = Context::new();

    ctx.set("env", "prod");
    assert_eq!(render_str(src, &ctx).unwrap(), "production");

    ctx.set("env", "stage");
    assert_eq!(render_str(src, &ctx).unwrap(), "staging");

    ctx.set("env", "dev");
    assert_eq!(render_str(src, &ctx).unwrap(), "local");
}

// ── For loops ─────────────────────────────────────────────────────────────────

#[test]
fn for_loop_over_list() {
    let src = r#"{% for url in urls %}goto "{{ url }}"
{% endfor %}"#;
    let mut ctx = Context::new();
    ctx.set(
        "urls",
        Value::List(vec![
            Value::Str("https://a.com".to_string()),
            Value::Str("https://b.com".to_string()),
        ]),
    );
    let out = render_str(src, &ctx).unwrap();
    assert!(out.contains("https://a.com"));
    assert!(out.contains("https://b.com"));
}

// ── Set ───────────────────────────────────────────────────────────────────────

#[test]
fn set_defines_variable_for_later_use() {
    let src = r#"{% set domain = "example.com" %}goto "https://{{ domain }}""#;
    let ctx = Context::new();
    let out = render_str(src, &ctx).unwrap();
    assert!(out.contains("https://example.com"));
}

// ── Include ───────────────────────────────────────────────────────────────────

#[test]
fn include_emits_scout_use_statement() {
    let src = r#"{% include "std/utils.sct" %}"#;
    let ctx = Context::new();
    let out = render_str(src, &ctx).unwrap();
    assert!(out.contains("use std::utils"));
}

// ── Raw blocks ────────────────────────────────────────────────────────────────

#[test]
fn raw_block_passes_through_template_syntax_verbatim() {
    let src = r#"{% raw %}{{ not_a_variable }}{% endraw %}"#;
    let ctx = Context::new();
    let out = render_str(src, &ctx).unwrap();
    assert_eq!(out, "{{ not_a_variable }}");
}

// ── Realistic .sct templates ──────────────────────────────────────────────────

#[test]
fn realistic_scraper_template() {
    let src: &'static str = r#"{# platter template for scraping a product listing page #}
{# Generated by platter-rs #}

goto "{{ base_url }}/{{ category }}"

{# Collect all product rows #}
for row in $$"{{ row_selector }}" do
  scrape {
    name: $(row)"{{ name_selector }}" |> textContent(),
    price: $(row)"{{ price_selector }}" |> textContent() |> trim()
  }
end
"#;

    let ctx: Context = ctx_with(&[
        ("base_url", "https://webscraper.io/test-sites/"),
        ("category", "e-commerce/static"),
        ("row_selector", ".product-item"),
        ("name_selector", ".product-name"),
        ("price_selector", ".product-price"),
    ]);

    let out = render_str(src, &ctx).unwrap();

    // Template comments stripped
    assert!(!out.contains("{#"));
    // Variables interpolated
    assert!(out.contains("https://webscraper.io/test-sites/"));
    assert!(out.contains(".product-item"));
    assert!(out.contains(".product-name"));
    assert!(out.contains(".product-price"));
    // Scout for-loop syntax preserved
    assert!(out.contains("for row in"));
    assert!(out.contains("scrape {"));
}

#[test]
fn crawl_template_with_conditional_depth() {
    let src: &'static str = r#"goto "{{ start_url }}"

crawl link, depth
  where link |> contains("{{ domain }}") and depth < {{ max_depth }}
do
  {% if scrape_text %}
  el = $".content"
  el |> textContent() |> print()
  {% else %}
  url() |> print()
  {% endif %}
end
"#;

    let mut ctx: Context = ctx_with(&[
        ("start_url", "https://example.com"),
        ("domain", "example.com"),
        ("max_depth", "5"),
    ]);
    ctx.set("scrape_text", true);

    let out = render_str(src, &ctx).unwrap();
    assert!(out.contains("https://example.com"));
    assert!(out.contains("example.com"));
    assert!(out.contains("depth < 5"));
    assert!(out.contains("textContent()"));
    // else branch should not appear
    assert!(!out.contains("url() |> print()"));
}

#[test]
fn unclosed_tag_returns_error() {
    let src: &'static str = "goto \"{{ url\"";
    let ctx: Context = Context::new();
    assert!(render_str(src, &ctx).is_err());
}

#[test]
fn unclosed_if_returns_error() {
    let src: &'static str = "{% if cond %}goto \"x\"";
    let mut ctx: Context = Context::new();
    ctx.set("cond", true);
    assert!(render_str(src, &ctx).is_err());
}
