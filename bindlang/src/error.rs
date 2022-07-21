use std::{
    error::Error,
    fmt::{Display, Formatter, Result},
};

use crate::{
    lexer::{LexerError, LexerErrorKind},
    parser::ParserError,
    span::{Pos, Span}, typecheck::TypeError,
};

#[derive(Clone, Debug)]
pub struct Errors<'a> {
    src: &'a str,
    lexer_errors: Vec<LexerError>,
    parser_errors: Vec<ParserError>,
    type_errors: Vec<TypeError>,
}

impl<'a> Errors<'a> {
    pub(crate) fn new(
        src: &'a str,
        lexer_errors: Vec<LexerError>,
        parser_errors: Vec<ParserError>,
        type_errors: Vec<TypeError>,
    ) -> Self {
        Errors {
            src,
            lexer_errors,
            parser_errors,
            type_errors,
        }
    }

    fn num_errors(&self) -> usize {
        self.lexer_errors.len() + self.parser_errors.len() + self.type_errors.len()
    }

    fn write_context(f: &mut Formatter, src: &'a str, span: Span) -> Result {
        writeln!(f, "at {}:{}: ", span.start.line, span.start.col)?;

        let left_col_width = format!("{}", span.end.line).len() + 1;

        let lines: Vec<&str> = src.lines().collect();

        writeln!(f, "{:>1$}", "| ", left_col_width + 2)?;

        for line_num in span.start.line..=span.end.line {
            if line_num == 0 {
                continue;
            }
            let i = line_num - 1;

            let line = lines[i];

            write!(f, "{:<1$}| ", line_num, left_col_width)?;
            writeln!(f, "{}", line)?;

            write!(f, "{:>1$}", "| ", left_col_width + 2)?;
            let col_start = if line_num == span.start.line {
                span.start.col
            } else {
                1
            };
            let col_end = if line_num == span.end.line {
                span.end.col
            } else {
                line.len()
            };

            write!(f, "{:1$}", "", col_start - 1)?;
            writeln!(f, "{:^<1$}", "^", col_end - col_start)?;
        }

        Ok(())
    }
}

impl<'a> Display for Errors<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{} errors found\n", self.num_errors())?;

        for err in &self.lexer_errors {
            write!(f, "error: ")?;

            match &err.kind {
                LexerErrorKind::InvalidCharacter(ch) => write!(f, "invalid character '{}'", ch)?,
            }

            writeln!(f)?;

            Self::write_context(f, self.src, err.span)?;

            write!(f, "\n")?;
        }

        for err in &self.parser_errors {
            write!(f, "error: ")?;

            match err {
                ParserError::UnexpectedToken { got, expected } => {
                    write!(
                        f,
                        "unexpected token '{}'",
                        &self.src[got.span.start.index..got.span.end.index]
                    )?;
                    if expected.len() == 1 {
                        write!(f, ", expected '{}'", expected.last().unwrap())?;
                    } else if expected.len() > 1 {
                        write!(f, ", expected one of '{}'", expected.first().unwrap())?;
                        for kind in &expected[1..] {
                            write!(f, ", '{}'", kind)?;
                        }
                    }

                    writeln!(f)?;

                    Self::write_context(f, self.src, got.span)?;
                }
                ParserError::ExpectedIdentKeyWord { got, expected } => {
                    write!(
                        f,
                        "unexpected token '{}', expected '{}'",
                        &self.src[got.span.start.index..got.span.end.index],
                        expected
                    )?;

                    writeln!(f)?;

                    Self::write_context(f, self.src, got.span)?;
                }
                ParserError::UnexpectedEof => {
                    write!(f, "unexpected end of file")?;
                    writeln!(f)?;

                    let mut lines = self.src.lines();
                    let mut last_line = "";
                    let mut last_line_num = 0;
                    while let Some(line) = lines.next() {
                        last_line = line;
                        last_line_num += 1;
                    }
                    let start = Pos {
                        index: self.src.len() - 1,
                        line: last_line_num,
                        col: last_line.len() - 1,
                    };
                    let end = Pos {
                        index: self.src.len(),
                        line: last_line_num,
                        col: last_line.len(),
                    };

                    Self::write_context(f, self.src, Span { start, end })?;
                }
            }

            write!(f, "\n")?;
        }

        for err in &self.type_errors {
            write!(f, "error: ")?;

            match err {
                TypeError::NotLVal(span) => {
                    writeln!(f, "this expression cannot be assigned a value")?;

                    Self::write_context(f, self.src, *span)?;
                }
                TypeError::NotAssignable { left, left_ty, right, right_ty } => {
                    writeln!(f, "a value of type '{right_ty}' cannot be assigned to '{left_ty}'")?;

                    Self::write_context(
                        f,
                        self.src,
                        Span {
                            start: left.start,
                            end: right.end,
                        }
                    )?;
                }
                TypeError::TypeMismatch { expected, got, span } => {
                    writeln!(f, "type mismatch: expected '{expected}', got '{got}'")?;

                    Self::write_context(f, self.src, *span)?;
                }
                TypeError::InvalidVariable(span) => {
                    writeln!(f, "variable does not exist")?;

                    Self::write_context(f, self.src, *span)?;
                }
                TypeError::InvalidField { ty, field } => {
                    writeln!(f, "type '{ty}' does not have field {}", field.index_src(self.src))?;

                    Self::write_context(f, self.src, *field)?;
                }
                TypeError::NotIndexable { ty, expr } => {
                    writeln!(f, "type '{ty}' cannot be indexed")?;

                    Self::write_context(f, self.src, *expr)?;
                }
                TypeError::NotAnIndex { ty, expr } => {
                    writeln!(f, "type '{ty}' cannot be used as an index")?;

                    Self::write_context(f, self.src, *expr)?;
                }
                TypeError::InvalidUnOp { op, ty, expr } => {
                    writeln!(f, "operator '{op}' cannot be used on a value of type '{ty}'")?;

                    Self::write_context(f, self.src, *expr)?;
                }
                TypeError::InvalidBinOp { left, op, right, expr } => {
                    writeln!(f, "operator '{op}' cannot be used on values of type '{left}' and '{right}'")?;

                    Self::write_context(f, self.src, *expr)?;
                }
                TypeError::DeviceAlreadyExists { old, new } => {
                    writeln!(f, "device '{}' already exists\n", old.index_src(self.src))?;
                    writeln!(f, "device was first defined here")?;
                    Self::write_context(f, self.src, *old)?;
                    writeln!(f, "\nbut then redefined here")?;
                    Self::write_context(f, self.src, *new)?;
                }
            }

            write!(f, "\n")?;
        }

        Ok(())
    }
}

impl<'a> Error for Errors<'a> {}
