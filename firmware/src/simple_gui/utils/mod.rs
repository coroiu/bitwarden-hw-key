pub trait LowerBound {
    fn lower_bound(&self, other: Self) -> Self;
}

impl<T> LowerBound for T
where
    T: Ord + Copy,
{
    fn lower_bound(&self, other: Self) -> Self {
        other.max(*self)
    }
}

pub trait UpperBound {
    fn upper_bound(&self, other: Self) -> Self;
}

impl<T> UpperBound for T
where
    T: Ord + Copy,
{
    fn upper_bound(&self, other: Self) -> Self {
        other.min(*self)
    }
}
