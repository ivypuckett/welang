use crate::errors::Span;

/// A complete program: the root of the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item in the program.
///
/// This enum will be extended with function definitions, type declarations,
/// etc. as the language grows.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// Placeholder — will be replaced by real language constructs.
    _Placeholder(Span),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Span;

    #[test]
    fn empty_program() {
        let program = Program { items: vec![] };
        assert!(program.items.is_empty());
    }

    #[test]
    fn program_debug_format() {
        let program = Program { items: vec![] };
        let debug = format!("{:?}", program);
        assert!(debug.contains("Program"));
    }

    #[test]
    fn program_clone() {
        let program = Program { items: vec![] };
        let cloned = program.clone();
        assert_eq!(program, cloned);
    }

    #[test]
    fn item_placeholder_equality() {
        let a = Item::_Placeholder(Span { start: 0, end: 1 });
        let b = Item::_Placeholder(Span { start: 0, end: 1 });
        assert_eq!(a, b);
    }
}
