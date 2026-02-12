# codec-eval justfile

# Default corpus for all commands
corpus := "~/work/codec-corpus/CID22/CID22-512/training"

# Quick eval (zenjpeg default config, tiny tier, quick quality)
eval format="jpeg" limit="3":
    cargo run --release -p codec-iter -- eval --format {{format}} --limit {{limit}} --corpus {{corpus}}

# Eval with XYB color space
eval-xyb limit="3":
    cargo run --release -p codec-iter -- eval --format jpeg --xyb --limit {{limit}} --corpus {{corpus}}

# Eval with 4:4:4 subsampling
eval-444 limit="3":
    cargo run --release -p codec-iter -- eval --format jpeg --subsampling 444 --limit {{limit}} --corpus {{corpus}}

# Save baseline
eval-baseline format="jpeg" limit="3":
    cargo run --release -p codec-iter -- baseline save --format {{format}} --limit {{limit}} --corpus {{corpus}}

# Show baseline
eval-baseline-show format="jpeg":
    cargo run --release -p codec-iter -- baseline show --format {{format}}

# Sweep over subsampling modes
eval-sweep limit="3":
    cargo run --release -p codec-iter -- sweep --format jpeg --subsampling 420,444 --limit {{limit}} --corpus {{corpus}}

# Sweep over subsampling + XYB
eval-sweep-full limit="3":
    cargo run --release -p codec-iter -- sweep --format jpeg --subsampling 420,444 --xyb on,off --limit {{limit}} --corpus {{corpus}}

# Standard quality evaluation (more quality points)
eval-standard format="jpeg" limit="5":
    cargo run --release -p codec-iter -- eval --format {{format}} --quality standard --limit {{limit}} --corpus {{corpus}}

# Dense quality evaluation
eval-dense format="jpeg" limit="3":
    cargo run --release -p codec-iter -- eval --format {{format}} --quality dense --limit {{limit}} --corpus {{corpus}}
