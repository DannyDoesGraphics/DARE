use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};

#[derive(thiserror::Error, Debug, Clone)]
pub enum EitherError {
    #[error("Expected left side, got right")]
    LeftSideExpected,
    #[error("Expected right side, got left")]
    RightSideExpected,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, PartialOrd)]
pub enum EitherSide {
    Left,
    Right,
}

pub enum Either<A, B> {
    Left(A),
    Right(B),
}

impl<A: Clone, B: Clone> Clone for Either<A, B> {
    fn clone(&self) -> Self {
        match self {
            Either::Left(A) => Either::Left(A.clone()),
            Either::Right(B) => Either::Right(B.clone()),
        }
    }
}

impl<A: Copy, B: Copy> Copy for Either<A, B> {}

impl<A: Debug, B: Debug> Debug for Either<A, B> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Either::Left(a) => f.debug_tuple("Left").field(a).finish(),
            Either::Right(b) => f.debug_tuple("Right").field(b).finish(),
        }
    }
}

impl<A: Default, B: Default> Default for Either<A, B> {
    /// By default, will always pick left A for [`Self::default`]
    fn default() -> Self {
        Self::Left(A::default())
    }
}

impl<A: Hash, B: Hash> Hash for Either<A, B> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Either::Left(a) => {
                a.hash(state);
            }
            Either::Right(b) => {
                b.hash(state);
            }
        }
    }
}

impl<A: PartialEq, B: PartialEq> PartialEq for Either<A, B> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Either::Left(a), Either::Left(b)) => a == b,
            (Either::Right(a), Either::Right(b)) => a == b,
            _ => false,
        }
    }
}
impl<A: Eq, B: Eq> Eq for Either<A, B> {}

impl<A: PartialOrd<A> + PartialOrd<B>, B: PartialOrd<B> + PartialOrd<A>> PartialOrd
    for Either<A, B>
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Either::Left(a), Either::Left(b)) => a.partial_cmp(b),
            (Either::Right(a), Either::Right(b)) => a.partial_cmp(b),
            (Either::Left(a), Either::Right(b)) => a.partial_cmp(b),
            (Either::Right(a), Either::Left(b)) => a.partial_cmp(b),
        }
    }
}

impl<A, B> Either<A, B>
where
    B: From<A>,
{
    pub fn as_ref(&self) -> Either<&A, &B> {
        match self {
            Either::Left(a) => Either::Left(a),
            Either::Right(b) => Either::Right(b),
        }
    }
}
