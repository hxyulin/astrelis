use std::ops::Mul;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect<T> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> 
{
    pub fn new(width: T, height: T) -> Self {
        Size { width, height }
    }

    pub fn cast<U: From<T>>(self) -> Size<U> {
        Size {
            width: U::from(self.width),
            height: U::from(self.height),
        }
    }
}

impl<T: Mul + Copy> Mul<T> for Size<T> {
    type Output = Size<<T as Mul>::Output>;

    fn mul(self, rhs: T) -> Self::Output {
        Size {
            width: self.width * rhs,
            height: self.height * rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pos<T> {
    pub x: T,
    pub y: T,
}
