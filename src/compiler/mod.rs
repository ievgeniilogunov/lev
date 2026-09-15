pub mod diagnostics;
pub mod source;

use crate::{
    ir,
    lexer,
    parser,
    semantic,
};

use diagnostics::Diagnostic;

pub fn compile(
    source: &str,
) -> Result<ir::IrProgram, Vec<Diagnostic>> {
    let source_file =
        source::SourceFile::new(source);

    let tokens =
        lexer::lex(&source_file)?;

    let ast =
        parser::parse(&tokens)?;

    let hir =
        semantic::analyze(ast)?;

    let mut ir =
        ir::lower::lower_to_ir(hir)?;

    /*
     * Validate the CFG before SSA construction.
     */
    ir::validate(&ir)?;

    /*
     * Convert local-variable based IR into SSA.
     */
    ir::construct_ssa(&mut ir)?;

    /*
     * Validate the resulting SSA IR.
     */
    ir::validate_ssa(&ir)?;

    Ok(ir)
}