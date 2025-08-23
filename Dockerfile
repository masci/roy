# Stage 1: Build the application
FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev

# Create a new empty shell project
WORKDIR /usr/src/roy
COPY . .

# Build for release
RUN cargo build --release

# Stage 2: Create the final, lightweight image
FROM alpine:latest

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/roy/target/release/roy /usr/local/bin/roy

# Expose the port the app runs on
EXPOSE 8000

# Set the entrypoint
CMD ["roy", "-vvv"]
