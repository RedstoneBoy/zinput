#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: Pos,
    pub end: Pos,
}

impl Span {
    pub fn index_src<'a>(&self, src: &'a str) -> &'a str {
        &src[self.start.index..self.end.index]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Pos {
    pub index: usize,
    pub line: usize,
    pub col: usize,
}