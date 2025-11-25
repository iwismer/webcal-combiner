# Build stage
FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static

WORKDIR /build

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy main to cache dependencies
# RUN mkdir src && \
#     echo "fn main() {}" > src/main.rs && \
#     cargo build --release || true && \
#     rm -rf src

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM alpine:3

# Install runtime dependencies
RUN apk add --no-cache ca-certificates

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /build/target/release/webcal-combiner /app/webcal-combiner

# Expose port
EXPOSE 5000

# Run the application
CMD ["/app/webcal-combiner"]
