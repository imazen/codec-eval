# Multi-codec comparison Docker image
#
# Includes all supported encoders:
# - MozJPEG (libjpeg-turbo based)
# - jpegli (from libjxl)
# - libwebp
# - libavif with aom, rav1e, and SVT-AV1
#
# Build: docker build -t codec-compare .
# Run:   docker run -v $(pwd)/corpus:/corpus -v $(pwd)/reports:/reports \
#          codec-compare run --corpus /corpus --output /reports

# =============================================================================
# Stage 1: Build dependencies
# =============================================================================
FROM rust:1.85-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    ninja-build \
    nasm \
    pkg-config \
    git \
    meson \
    # For libavif
    libaom-dev \
    libdav1d-dev \
    # For mozjpeg
    libturbojpeg0-dev \
    # For webp
    libwebp-dev \
    # For jpegli (libjxl)
    libbrotli-dev \
    libhwy-dev \
    liblcms2-dev \
    libpng-dev \
    zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

# Build SVT-AV1 (not in Debian repos)
WORKDIR /build
RUN git clone --depth 1 --branch v2.3.0 https://gitlab.com/AOMediaCodec/SVT-AV1.git && \
    cd SVT-AV1/Build && \
    cmake .. -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=ON && \
    ninja && \
    ninja install && \
    ldconfig

# Build rav1e (Rust AV1 encoder)
RUN cargo install rav1e --version 0.7.1 --features asm

# Build libjxl for jpegli
WORKDIR /build
RUN git clone --depth 1 --branch v0.11.1 https://github.com/libjxl/libjxl.git && \
    cd libjxl && \
    git submodule update --init --recursive --depth 1 && \
    mkdir build && cd build && \
    cmake .. -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DBUILD_TESTING=OFF \
        -DJPEGXL_ENABLE_BENCHMARK=OFF \
        -DJPEGXL_ENABLE_EXAMPLES=OFF \
        -DJPEGXL_ENABLE_SJPEG=OFF \
        -DJPEGXL_ENABLE_SKCMS=ON \
        -DJPEGXL_ENABLE_VIEWERS=OFF \
        -DJPEGXL_FORCE_SYSTEM_BROTLI=ON \
        -DJPEGXL_FORCE_SYSTEM_HWY=ON \
        -DJPEGXL_FORCE_SYSTEM_LCMS2=ON && \
    ninja && \
    ninja install && \
    ldconfig

# Build libavif with all encoders
WORKDIR /build
RUN git clone --depth 1 --branch v1.1.1 https://github.com/AOMediaCodec/libavif.git && \
    cd libavif && \
    mkdir build && cd build && \
    cmake .. -G Ninja \
        -DCMAKE_BUILD_TYPE=Release \
        -DAVIF_CODEC_AOM=SYSTEM \
        -DAVIF_CODEC_DAV1D=SYSTEM \
        -DAVIF_CODEC_SVT=SYSTEM \
        -DAVIF_CODEC_RAV1E=LOCAL \
        -DAVIF_BUILD_APPS=ON && \
    ninja && \
    ninja install && \
    ldconfig

# Build codec-compare
WORKDIR /app
COPY . .
RUN cargo build --release --package codec-compare --features all

# =============================================================================
# Stage 2: Runtime image
# =============================================================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libaom3 \
    libdav1d7 \
    libturbojpeg0 \
    libwebp7 \
    libwebpdemux2 \
    libwebpmux3 \
    libbrotli1 \
    libhwy1 \
    liblcms2-2 \
    libpng16-16 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy built libraries
COPY --from=builder /usr/local/lib/*.so* /usr/local/lib/
COPY --from=builder /usr/local/bin/avifenc /usr/local/bin/
COPY --from=builder /usr/local/bin/avifdec /usr/local/bin/
COPY --from=builder /usr/local/bin/cjpegli /usr/local/bin/
COPY --from=builder /usr/local/bin/djpegli /usr/local/bin/

# Copy codec-compare binary
COPY --from=builder /app/target/release/codec-compare /usr/local/bin/

# Update library cache
RUN ldconfig

# Create directories for corpus and reports
RUN mkdir -p /corpus /reports

WORKDIR /work

# Default entrypoint
ENTRYPOINT ["codec-compare"]
CMD ["--help"]

# Labels
LABEL org.opencontainers.image.title="codec-compare"
LABEL org.opencontainers.image.description="Multi-codec image comparison with Pareto analysis"
LABEL org.opencontainers.image.source="https://github.com/imazen/codec-eval"
