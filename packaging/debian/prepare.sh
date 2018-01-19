#!/bin/bash
# Prepare the directory for the debian package of Fisher
# Copyright (c) 2018 Pietro Albini <pietro@pietroalbini.org>

set -euo pipefail
IFS=$'\n\t'


INCLUDE_FILES=( "src" "Cargo.toml" "Cargo.lock" "config-example.toml" )
PACKAGE="fisher"


base="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"


check_dependency() {
    name="$1"; shift

    if ! which "${name}" >/dev/null 2>&1; then
        echo "The binary '${name}' is required to be in your path."
        echo "Please install it (or update \$PATH)."
        exit 1
    fi
}


check_dependency python3
check_dependency cargo
check_dependency cargo-vendor
check_dependency rsync
check_dependency strip-nondeterminism


if [[ $# -ne 1 ]]; then
    echo "Usage: ./prepare.sh <distro>"
    echo "Example: ./prepare.sh xenial"
    exit 1
fi

target_distro="${1}"


# Get the package version
json="$(cd "${base}/../.." && cargo metadata --no-deps --format-version 1)"
version="$(echo "${json}" | python3 -c "if True:
    import json, sys
    data = json.load(sys.stdin)
    print(data['packages'][0]['version'])
")"

content_dir="${base}/src/${PACKAGE}-${version}"


# Cleanup the `src` directory
echo "Cleaning up the package directory..."
for file in $(ls -a "${base}/src"); do
    if [[ "${file}" != "." ]] && [[ "${file}" != ".." ]]  && [[ "${file}" != "debian" ]]
    then
        rm -rf "${base}/src/${file}"
    fi
done


# Copy files to the `src` directory
echo "Copying source files..."
for file in ${INCLUDE_FILES[@]}; do
    src="${base}/../../${file}"
    dest="${content_dir}/${file}"

    mkdir -p "$(dirname "${dest}")"
    if [[ -d "${src}" ]]; then
        rsync -a --delete "${src}/" "${dest}"
    else
        cp "${src}" "${dest}"
    fi
done


# Vendor dependencies
echo "Vendoring dependencies..."
(cd "${content_dir}" && cargo vendor -q)


# Tweak crate configuration file
mkdir -p "${content_dir}/.cargo"
cat >> "${content_dir}/.cargo/config" << EOF
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF


# Create the source tarball
echo "Creating the source tarball..."
tar --sort=name \
    --mtime="1970-01-01 00:00Z" \
    --owner=0 --group=0 --numeric-owner \
    -czf "${base}/src/${PACKAGE}_${version}.orig.tar.gz" \
    -C "${base}/src" "${PACKAGE}-${version}"
strip-nondeterminism "${base}/src/${PACKAGE}_${version}.orig.tar.gz"


# Copy the debian directory
echo "Copying the debian/ directory..."
rsync -a --delete "${base}/src/debian" "${content_dir}"


# Some tweaks to the debian/ directory
echo "Tweaking the debian/ directory..."
sed -i "s/DISTRO/${target_distro}/g" "${content_dir}/debian/changelog"
