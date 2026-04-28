//! platter-rs CLI
//!
//! Usage:
//!     platter <template.sct> [--var key=value ...] [--out output.sct]
//!     platter --help
//!
//! Variables can also be passed via environment variables prefixed with SCOUT_.

use platter_rs::Context;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    let mut template_path: Option<&str> = None;
    let mut output_path: Option<&str> = None;
    let mut vars: HashMap<String, String> = HashMap::new();
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--var" | "-v" => {
                i += 1;
                if let Some(kv) = args.get(i) {
                    if let Some((k, v)) = kv.split_once('=') {
                        vars.insert(k.to_string(), v.to_string());
                    } else {
                        eprintln!("Warning: ignoring malformed --var '{kv}' (expected key=value)");
                    }
                }
            }
            "--out" | "-o" => {
                i += 1;
                output_path = args.get(i).map(|s| s.as_str());
            }
            path => {
                if template_path.is_none() {
                    template_path = Some(path);
                }
            }
        }
        i += 1;
    }

    let template_path = template_path.ok_or("No template file provided. Use --help for usage.")?;
    let source = fs::read_to_string(template_path)
        .map_err(|e| format!("Could not read `{template_path}` : {e} "))?;

    // Build context: CLI vars override env vars
    let mut ctx: Context = Context::new();

    // Collect SCOUT_* env vars
    for (k, v) in env::vars() {
        if let Some(name) = k.strip_prefix("SCOUT_") {
            ctx.set(name.to_lowercase(), parse_cli_value(&v));
        }
    }

    // CLI --var overrides - parse booleans, ints, floats automatically
    for (k, v) in &vars {
        let val = parse_cli_value(v);
        ctx.set(k.clone(), val);
    }

    let output = platter_rs::render_str(&source, &ctx).map_err(|e| format!("{e}"))?;

    match output_path {
        Some(path) => {
            fs::write(path, &output).map_err(|e| format!("Could not write `{path}`: {e}"))?;
            eprintln!("Rendered to `{path}`");
        }
        None => println!("{output}"),
    }

    Ok(())
}

fn parse_cli_value(s: &str) -> platter_rs::Value {
    use platter_rs::Value;
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                return Value::Int(n);
            }
            if let Ok(f) = s.parse::<f64>() {
                return Value::Float(f);
            }
            Value::Str(s.to_string())
        }
    }
}

fn print_help() {
    let help = concat!(
        "platter-rs — A templating engine for Scout (.sct) scripts\n",
        "\n",
        "USAGE:\n",
        "    platter <template.sct> [OPTIONS]\n",
        "\n",
        "OPTIONS:\n",
        "    -v, --var key=value     Set a template variable (repeatable)\n",
        "    -o, --out output.sct    Write rendered output to file (default: stdout)\n",
        "    -h, --help              Print this help message\n",
        "\n",
        "TEMPLATE SYNTAX:\n",
        "    {{ variable }}                   Interpolate a variable\n",
        "    {{ variable | filter }}          Apply a filter\n",
        "    {{ variable | default(\"x\") }}    Use fallback if variable is falsy\n",
        "    {# comment #}                    Template comment (stripped from output)\n",
        "    {% if condition %}...{% endif %} Conditional block\n",
        "    {% elif condition %}             Else-if branch\n",
        "    {% else %}                       Else branch\n",
        "    {% for item in list %}           Loop over a list variable\n",
        "    {% endfor %}\n",
        "    {% set var = expr %}             Define a template variable\n",
        "    {% include \"path.sct\" %}         Emit a Scout `use` import\n",
        "    {% raw %}...{% endraw %}         Pass through verbatim\n",
        "\n",
        "FILTERS:\n",
        "    upper, lower, trim, capitalize\n",
        "    replace(\"from,to\"), truncate(80)\n",
        "    quote, escape_selector, selector, multi_selector\n",
        "    first, last, length, join(\", \"), reverse\n",
        "    abs, int, string, default(\"fallback\")\n",
        "\n",
        "ENVIRONMENT:\n",
        "    Variables prefixed with SCOUT_ are automatically available.\n",
        "    e.g. SCOUT_TARGET=https://example.com makes `target` available.\n",
        "\n",
        "EXAMPLE:\n",
        "    platter crawl.sct.tmpl --var target=https://example.com -o crawl.sct\n",
        "    scout my-file.sct\n",
    );
    print!("{help}");
}
