variable "VERGEN_GIT_SHA" {
  default = ""
}

variable "VERGEN_GIT_SHA_SHORT" {
  default = ""
}

group "default" {
  targets = ["magnus", "magnus-bench", "magnus-sidecar", "magnus-xtask"]
}

target "docker-metadata" {}

# Base image with all dependencies pre-compiled
target "chef" {
  dockerfile = "Dockerfile.chef"
  context = "."
  platforms = ["linux/amd64", "linux/arm64"]
  args = {
    RUST_PROFILE = "profiling"
    RUST_FEATURES = "asm-keccak,jemalloc,otlp"
  }
}

target "_common" {
  dockerfile = "Dockerfile"
  context = "."
  contexts = {
    chef = "target:chef"
  }
  args = {
    CHEF_IMAGE = "chef"
    RUST_PROFILE = "profiling"
    RUST_FEATURES = "asm-keccak,jemalloc,otlp"
    VERGEN_GIT_SHA = "${VERGEN_GIT_SHA}"
    VERGEN_GIT_SHA_SHORT = "${VERGEN_GIT_SHA_SHORT}"
  }
  platforms = ["linux/amd64", "linux/arm64"]
}

target "magnus" {
  inherits = ["_common", "docker-metadata"]
  target = "magnus"
}

target "magnus-bench" {
  inherits = ["_common", "docker-metadata"]
  target = "magnus-bench"
}

target "magnus-sidecar" {
  inherits = ["_common", "docker-metadata"]
  target = "magnus-sidecar"
}

target "magnus-xtask" {
  inherits = ["_common", "docker-metadata"]
  target = "magnus-xtask"
}
