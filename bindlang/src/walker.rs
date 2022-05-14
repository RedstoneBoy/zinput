use std::collections::HashMap;

use crate::ast::{Module, Stmt, StmtKind};

#[derive(Copy, Clone, PartialEq, Eq)]
enum VType {
    IntLiteral,
    FloatLiteral,
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

struct Value {
    ty: VType,
    id: u16,
}

pub struct Walker<'a> {
    src: &'a str,
    vars: HashMap<&'a str, Value>,
}

impl<'a> Walker<'a> {
    pub fn new(src: &'a str) -> Self {
        Walker {
            src,
            vars: HashMap::new(),
        }
    }

    pub fn walk(mut self, module: &Module) {

    }

    fn stmt(&mut self, stmt: &Stmt) {
        let stmt = &stmt.kind;

        match stmt {
            StmtKind::Let { name, expr } => {
                let name = name.index_src(&self.src);
            }
            StmtKind::Expr(_) => {}
        }
    }
}

pub enum WalkerError {

}