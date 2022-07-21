use std::collections::HashMap;

use crate::{ir::{Module, Block, Body, Instruction}, ast::{Module as AstModule, Block as AstBlock, DeviceIn, Stmt, StmtKind}};


pub struct IrCompiler<'a> {
    src: &'a str,
}

impl<'a> IrCompiler<'a> {
    pub fn new(src: &'a str) -> Self {
        IrCompiler {
            src,
        }
    }

    pub fn compile(mut self, module: AstModule) -> Module {
        let inputs = module.inputs
            .into_iter()
            .map(|input| self.compile_input(input))
            .collect();
        
        Module { inputs }
    }

    fn compile_input(&mut self, input: DeviceIn) -> Body {
        let block = self.compile_block(input.body);

        Body {
            block,
            max_var_index: todo!(),
        }
    }

    fn compile_block(&mut self, block: AstBlock) -> Block {
        Block(block.stmts.into_iter().flat_map(|stmt| self.compile_stmt(stmt)).collect())
    }

    fn compile_stmt(&mut self, stmt: Stmt) -> Vec<Instruction> {
        match stmt.kind {
            StmtKind::Let { name, expr } => {
                todo!()
            }
            _ => todo!(),
        }
    }
}