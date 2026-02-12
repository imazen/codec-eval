//! GPU-accelerated SSIMULACRA2 via CUDA.
//!
//! Wraps ssimulacra2-cuda with a simple interface for codec-iter's eval loop.
//! All images must be the same dimensions (true for CID22-512 corpus).

use cudarse_driver::CuStream;
use cudarse_npp::image::isu::Malloc;
use cudarse_npp::image::{C, Image, Img};
use cudarse_npp::set_stream;

/// Initialize CUDA driver and primary context.
pub fn init_cuda() -> anyhow::Result<()> {
    cudarse_driver::init_cuda_and_primary_ctx()
        .map_err(|e| anyhow::anyhow!("CUDA init failed: {e:?}"))
}

/// GPU-accelerated SSIMULACRA2 context.
///
/// Allocates GPU buffers once for a fixed image size, then reuses them
/// across all quality levels and images.
pub struct GpuSsim2 {
    stream: CuStream,
    width: u32,
    height: u32,

    // Shared u8 GPU buffers for upload
    gpu_ref: Option<Image<u8, C<3>>>,
    gpu_dis: Option<Image<u8, C<3>>>,

    // Linear f32 GPU buffers for SSIM2
    linear_ref: Option<Image<f32, C<3>>>,
    linear_dis: Option<Image<f32, C<3>>>,

    // SSIM2 context
    ssim2: Option<ssimulacra2_cuda::Ssimulacra2>,
}

impl GpuSsim2 {
    /// Create a new GPU SSIM2 context for images of the given dimensions.
    pub fn new(width: u32, height: u32) -> anyhow::Result<Self> {
        let stream =
            CuStream::new().map_err(|e| anyhow::anyhow!("CUDA stream creation failed: {e:?}"))?;

        // Set NPP global stream context
        #[allow(clippy::ptr_as_ptr)]
        set_stream(stream.inner() as _)
            .map_err(|e| anyhow::anyhow!("NPP set_stream failed: {e:?}"))?;

        // Allocate shared u8 image buffers
        let gpu_ref: Image<u8, C<3>> = Image::malloc(width, height)
            .map_err(|e| anyhow::anyhow!("GPU malloc ref failed: {e:?}"))?;
        let gpu_dis: Image<u8, C<3>> = gpu_ref
            .malloc_same_size()
            .map_err(|e| anyhow::anyhow!("GPU malloc dis failed: {e:?}"))?;

        // Allocate linear f32 buffers
        let linear_ref: Image<f32, C<3>> = Image::malloc(width, height)
            .map_err(|e| anyhow::anyhow!("GPU malloc linear ref failed: {e:?}"))?;
        let linear_dis: Image<f32, C<3>> = linear_ref
            .malloc_same_size()
            .map_err(|e| anyhow::anyhow!("GPU malloc linear dis failed: {e:?}"))?;

        // Create SSIM2 context
        let ssim2 = ssimulacra2_cuda::Ssimulacra2::new(&linear_ref, &linear_dis, &stream)
            .map_err(|e| anyhow::anyhow!("SSIM2 CUDA init failed: {e:?}"))?;

        Ok(Self {
            stream,
            width,
            height,
            gpu_ref: Some(gpu_ref),
            gpu_dis: Some(gpu_dis),
            linear_ref: Some(linear_ref),
            linear_dis: Some(linear_dis),
            ssim2: Some(ssim2),
        })
    }

    /// Compute SSIMULACRA2 score from packed RGB u8 bytes in CPU memory.
    ///
    /// Both `reference` and `distorted` must be `width * height * 3` bytes,
    /// packed as RGBRGBRGB.
    pub fn compute(&mut self, reference: &[u8], distorted: &[u8]) -> anyhow::Result<f64> {
        let expected = self.width as usize * self.height as usize * 3;
        if reference.len() != expected || distorted.len() != expected {
            anyhow::bail!(
                "Image size mismatch: expected {} bytes ({}x{}x3), got ref={} dis={}",
                expected,
                self.width,
                self.height,
                reference.len(),
                distorted.len()
            );
        }

        self.ssim2
            .as_mut()
            .expect("ssim2 context")
            .compute_from_cpu_srgb_sync(
                reference,
                distorted,
                self.gpu_ref.as_mut().expect("gpu_ref"),
                self.gpu_dis.as_mut().expect("gpu_dis"),
                self.linear_ref.as_mut().expect("linear_ref"),
                self.linear_dis.as_mut().expect("linear_dis"),
                &self.stream,
            )
            .map_err(|e| anyhow::anyhow!("SSIM2 CUDA compute failed: {e:?}"))
    }

    /// Returns the expected image dimensions.
    #[allow(dead_code)]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl Drop for GpuSsim2 {
    fn drop(&mut self) {
        // Sync before dropping to ensure all GPU operations complete
        let _ = self.stream.sync();

        // Drop SSIM2 context before GPU buffers it references
        self.ssim2 = None;
        self.linear_ref = None;
        self.linear_dis = None;

        // Sync context before dropping GPU images
        let _ = cudarse_driver::sync_ctx();

        self.gpu_ref = None;
        self.gpu_dis = None;
    }
}
