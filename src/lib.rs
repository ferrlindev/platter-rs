pub mod context;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod renderer;
//
pub use context::{Context, Value};
pub use error::{TemplateError, TemplateResult};
pub use renderer::render;

//
pub fn render_str(template: &str, ctx: &Context) -> TemplateResult<String> {
    let ast = parser::parse(template)?;
    renderer::render(&ast, ctx)
}
