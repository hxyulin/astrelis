//! GPU command queue trait.

use crate::device::GpuDevice;

/// Submits recorded commands to the GPU for execution.
///
/// Separated from [`GpuDevice`] because submission is conceptually
/// different from resource creation — some advanced patterns need to
/// submit from a different context than where resources are created.
pub trait GpuQueue {
    /// The device type this queue is paired with.
    type Device: GpuDevice;

    /// Submits command encoders for execution.
    ///
    /// The encoders are consumed and their commands are submitted in order.
    fn submit(
        &self,
        encoders: impl IntoIterator<Item = <Self::Device as GpuDevice>::Encoder>,
    );
}
