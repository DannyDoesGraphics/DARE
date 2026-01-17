use std::{fmt::Debug, hash::Hash};

pub enum Either<A, B> {
    Left(A),
    Right(B),
}

impl<A: Debug, B: Debug> Debug for Either<A, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Either::Left(a) => f.debug_tuple("Left").field(a).finish(),
            Either::Right(b) => f.debug_tuple("Right").field(b).finish(),
        }
    }
}
impl<A: PartialEq, B: PartialEq> PartialEq for Either<A,B> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Left(a), Self::Left(b)) => a == b,
            (Self::Right(a), Self::Right(b)) => a == b,
            _ => false
        }
    }
}
impl<A: Eq, B: Eq> Eq for Either<A,B> {}
impl<A: PartialOrd + PartialOrd<B>, B: PartialOrd + PartialOrd<A>> PartialOrd for Either<A,B> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::Left(a), Self::Left(b)) => a.partial_cmp(b),
            (Self::Left(a), Self::Right(b)) => a.partial_cmp(b),
            (Self::Right(a), Self::Left(b)) => a.partial_cmp(b),
            (Self::Right(a), Self::Right(b)) => a.partial_cmp(b)
        }
    }
}


impl<A: Clone, B: Clone> Clone for Either<A, B> {
    fn clone(&self) -> Self {
        match self {
            Either::Left(a) => Either::Left(a.clone()),
            Either::Right(b) => Either::Right(b.clone()),
        }
    }
}
impl<A: Copy, B: Copy> Copy for Either<A, B> {}
impl<A: Hash, B: Hash> Hash for Either<A, B> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Either::Left(a) => {
                0.hash(state);
                a.hash(state);
            }
            Either::Right(b) => {
                1.hash(state);
                b.hash(state);
            }
        }
    }
}
impl<A, B> Either<A, B> {
    pub fn from_left(a: A) -> Self {
        Self::Left(a)
    }
    
    pub fn from_right(b: B) -> Self {
        Self::Right(b)
    }
    
    pub fn left_ref(&self) -> Option<&A> {
        match self {
            Self::Left(a) => Some(&a),
            Self::Right(_) => None,
        }
    }

    pub fn right_ref(&self) -> Option<&B> {
        match self {
            Self::Left(_) => None,
            Self::Right(b) => Some(&b),
        }
    }

    pub fn left_mut(&mut self) -> Option<&mut A> {
        match self {
            Self::Left(a) => Some(a),
            Self::Right(_) => None,
        }
    }

    pub fn right_mut(&mut self) -> Option<&mut B> {
        match self {
            Self::Left(_) => None,
            Self::Right(b) => Some(b),
        }
    }

    pub fn is_left(&self) -> bool {
        match self {
            Self::Left(_) => true,
            _ => false,
        }
    }
}
