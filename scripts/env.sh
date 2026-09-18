#!/bin/sh
# Use the Rust and Flutter already on PATH. clang/swift module caches embed the
# checkout path, so they stay local instead of colliding across clones.
_checkout="$(basename "$PWD")"
export CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$PWD/.tools/$_checkout-clang-cache}"
export SWIFT_MODULE_CACHE_PATH="${SWIFT_MODULE_CACHE_PATH:-$PWD/.tools/$_checkout-swift-cache}"
