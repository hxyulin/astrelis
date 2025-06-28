use std::fmt::Debug;

pub struct Extent2D<T> {
    pub width: T,
    pub height: T,
}

pub struct Extent3D<T> {
    pub width: T,
    pub height: T,
    pub depth: T,
}

impl<T> Debug for Extent2D<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extent2D")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl<T> Clone for Extent2D<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            width: self.width.clone(),
            height: self.height.clone(),
        }
    }
}

impl<T> Copy for Extent2D<T> where T: Copy {}

impl<T> Debug for Extent3D<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extent3D")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("depth", &self.depth)
            .finish()
    }
}

impl<T> Clone for Extent3D<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            width: self.width.clone(),
            height: self.height.clone(),
            depth: self.depth.clone(),
        }
    }
}

impl<T> Copy for Extent3D<T> where T: Copy {}

impl Into<wgpu::Extent3d> for Extent3D<u32> {
    fn into(self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: self.depth,
        }
    }
}
