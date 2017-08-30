# Installing Fisher

If you want to use Fisher in a new machine you need to install it. Fisher is
written in Rust, and it's available as a single binary you can drop into your
path.

Unfortunately, no precompiled packages for any Linux distribution are available
yet. In the future they might become available.

## Precompiled binaries

Official precompiled binaries are available from
[files.pietroalbini.org](https://files.pietroalbini.org/releases/fisher). You
can download the latest version from it and extract the binary contained in it
in your `${PATH}` (usually `/usr/local/bin`). There are also GPG signatures
available if you want to check them.

## Install from source

If you want to build Fisher from source, you need to have the Rust 1.17 (or
greater) toolchain installed on the target machine. Keep in mind this might
take a while to complete.

The easiest way to build from source is to build the package uploaded in the
Rust's package registry, [crates.io](https://crates.io/crates/fisher):

```
$ cargo install fisher
```

Instead, if you want to compile directly from the source code you need to fetch
the code from the git repository, and then build it with Cargo.

```
$ git clone https://github.com/pietroalbini/fisher
$ cd fisher
$ cargo build --release
```

The binary will be available in `target/release/fisher`.
