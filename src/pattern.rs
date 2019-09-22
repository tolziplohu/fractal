use crate::ast::*;
use crate::vm::Value;
use string_interner::Sym;

#[derive(PartialEq, Debug)]
pub enum MatchResult {
    Pass(Vec<(Sym, Value)>),
    Fail,
}
impl MatchResult {
    pub fn is_pass(&self) -> bool {
        match self {
            MatchResult::Pass(_) => true,
            MatchResult::Fail => false,
        }
    }
}

/// Three-value logic
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum TBool {
    True,
    False,
    Maybe,
}
/// Top-level True, False, Maybe
pub use TBool::*;

impl std::ops::BitOr for TBool {
    type Output = TBool;
    fn bitor(self, rhs: TBool) -> TBool {
        match (self, rhs) {
            (True, _) => True,
            (_, True) => True,
            (False, False) => False,
            _ => Maybe,
        }
    }
}

impl std::ops::BitAnd for TBool {
    type Output = TBool;
    fn bitand(self, rhs: TBool) -> TBool {
        match (self, rhs) {
            (False, _) => False,
            (_, False) => False,
            (True, True) => True,
            _ => Maybe,
        }
    }
}

pub trait Match {
    fn match_strict(&self, other: &Value) -> MatchResult;
    fn match_conservative(&self, _other: &Term) -> TBool {
        Maybe
    }
}

impl Match for Term {
    fn match_strict(&self, other: &Value) -> MatchResult {
        match (self, other) {
            (Term::Var(s), x) => MatchResult::Pass(vec![(*s, x.clone())]),
            (Term::Int(x), Value::Int(y)) if x == y => MatchResult::Pass(Vec::new()),
            (Term::Float(x), Value::Float(y)) if x == y => MatchResult::Pass(Vec::new()),
            (Term::Tuple(a, b), Value::Tuple(a2, b2)) => {
                if let MatchResult::Pass(mut a) = a.match_strict(a2) {
                    if let MatchResult::Pass(mut b) = b.match_strict(b2) {
                        a.append(&mut b);
                        return MatchResult::Pass(a);
                    }
                }
                MatchResult::Fail
            }
            (Term::Union(a, b), x) => {
                let a = a.match_strict(x);
                if a.is_pass() {
                    a
                } else {
                    b.match_strict(x)
                }
            }
            _ => MatchResult::Fail,
        }
    }

    fn match_conservative(&self, other: &Term) -> TBool {
        match (self, other) {
            (Term::Var(_), _) => True,
            (Term::Int(x), Term::Int(y)) => {
                if x == y {
                    True
                } else {
                    False
                }
            }
            (Term::Float(x), Term::Float(y)) => {
                if x == y {
                    True
                } else {
                    False
                }
            }
            (Term::Int(_), Term::Float(_)) => False,
            (Term::Float(_), Term::Int(_)) => False,
            (Term::Tuple(a, b), Term::Tuple(a2, b2)) => {
                a.match_conservative(a2) & b.match_conservative(b2)
            }
            (Term::Union(a, b), x) => a.match_conservative(x) | b.match_conservative(x),
            _ => Maybe,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_var() -> Sym {
        use string_interner::StringInterner;
        StringInterner::new().get_or_intern("a")
    }

    fn fresh_file() -> codespan::FileId {
        codespan::Files::new().add("a", "a")
    }

    fn fresh_span() -> codespan::Span {
        codespan::Span::default()
    }

    #[test]
    fn test_var() {
        assert_eq!(
            Term::Var(fresh_var()).match_conservative(&Term::Int(3)),
            True
        );
        assert_eq!(
            Term::Var(fresh_var()).match_strict(&Value::Int(3)),
            MatchResult::Pass(vec![(fresh_var(), Value::Int(3))])
        );
    }

    #[test]
    fn test_tuple() {
        let file = fresh_file();
        let span = fresh_span();

        let i = Box::new(Node {
            file,
            span,
            val: Term::Int(2),
        });
        let j = Box::new(Node {
            file,
            span,
            val: Term::Int(3),
        });
        let v = Box::new(Node {
            file,
            span,
            val: Term::Var(fresh_var()),
        });

        assert_eq!(
            Term::Tuple(i.clone(), j.clone())
                .match_conservative(&Term::Tuple(i.clone(), j.clone())),
            True
        );
        assert_eq!(
            Term::Tuple(i.clone(), j.clone())
                .match_conservative(&Term::Tuple(i.clone(), v.clone())),
            Maybe
        );
        assert_eq!(
            Term::Tuple(i.clone(), j.clone())
                .match_conservative(&Term::Tuple(j.clone(), v.clone())),
            False
        );

        let iv = Box::new(Value::Int(2));
        let jv = Box::new(Value::Int(3));

        assert_eq!(
            Term::Tuple(i.clone(), j.clone()).match_strict(&Value::Tuple(iv.clone(), jv.clone())),
            MatchResult::Pass(vec![])
        );
        assert_eq!(
            Term::Tuple(i, j).match_strict(&Value::Tuple(jv, iv)),
            MatchResult::Fail
        );
    }

    #[test]
    fn test_union() {
        let file = fresh_file();
        let span = fresh_span();

        let i = Box::new(Node {
            file,
            span,
            val: Term::Int(2),
        });
        let j = Box::new(Node {
            file,
            span,
            val: Term::Int(3),
        });
        let v = Box::new(Node {
            file,
            span,
            val: Term::Var(fresh_var()),
        });

        assert_eq!(
            Term::Union(i.clone(), j.clone()).match_conservative(&i),
            True
        );
        assert_eq!(
            Term::Union(i.clone(), j.clone()).match_conservative(&v),
            Maybe
        );
        assert_eq!(
            Term::Union(i.clone(), v.clone())
                .match_conservative(&Term::Tuple(j.clone(), v.clone())),
            True
        );

        let iv = Box::new(Value::Int(2));
        let jv = Box::new(Value::Int(3));

        assert_eq!(
            Term::Union(i.clone(), j.clone()).match_strict(&Value::Int(3)),
            MatchResult::Pass(vec![])
        );
        assert_eq!(
            Term::Union(i.clone(), v.clone()).match_strict(&Value::Int(3)),
            MatchResult::Pass(vec![(fresh_var(), Value::Int(3))])
        );
        assert_eq!(
            Term::Union(i, j).match_strict(&Value::Tuple(jv, iv)),
            MatchResult::Fail
        );
    }
}
