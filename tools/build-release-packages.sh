#!/bin/bash
# Make a new release build of Fisher
# Copyright (C) 2016-2019 Pietro Albini <pietro@pietroalbini.org>
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <http:#www.gnu.org/licenses/>.


# Detect source directory
# Thanks to http://stackoverflow.com/a/246128/2204144
get_base() {
    source="$0"
    while [ -h "${source}" ]; do
        dir="$( cd -P "$( dirname "${source}" )" && pwd )"
        source="$(readlink "${source}")"
    done
    directory="$( cd "$( dirname "${source}" )/.." && pwd )"

    echo "${directory}"
}


BASE="$( get_base )"
BUILD_DIRECTORY="${BASE}/build"
SOURCE_DIRECTORY="${BUILD_DIRECTORY}/src"
TARGET_DIRECTORY="${BUILD_DIRECTORY}/target"
BINARIES_DIRECTORY="${BUILD_DIRECTORY}/binaries"
PACKAGES_DIRECTORY="${BUILD_DIRECTORY}/packages"

PROJECT_NAME="fisher"
BIN_NAME="fisher"
declare -A TARGETS=( \
    [x86_64-unknown-linux-gnu]="linux-amd64" \
    [i686-unknown-linux-gnu]="linux-i686" \
    [arm-unknown-linux-gnueabi]="linux-armv6" \
    [armv7-unknown-linux-gnueabihf]="linux-armv7" \
    [aarch64-unknown-linux-gnu]="linux-armv8" \
)
PACKAGES_INCLUDE_FILES=(
    "LICENSE"
    "CHANGELOG.md"
    "README.md"
)


RESET="\033[m"
BOLD="\033[1m"


# Cleanup things before starting
cleanup() {
    mkdir -p "${BUILD_DIRECTORY}"
    mkdir -p "${TARGET_DIRECTORY}"
    rm -rf "${SOURCE_DIRECTORY}"; mkdir -p "${SOURCE_DIRECTORY}"
    rm -rf "${BINARIES_DIRECTORY}"; mkdir -p "${BINARIES_DIRECTORY}"
    rm -rf "${PACKAGES_DIRECTORY}"; mkdir -p "${PACKAGES_DIRECTORY}"
}


# Prepare the source code to be built
prepare_source() {
    revision="$1"; shift

    cd "${BASE}"
    git "--work-tree=${SOURCE_DIRECTORY}" checkout "${revision}" -- .
    ln -s "${TARGET_DIRECTORY}" "${SOURCE_DIRECTORY}/target"
}


# This builds the binaries for all the targets
build_binaries() {
    cd "${SOURCE_DIRECTORY}"

    for target in "${!TARGETS[@]}"; do
        echo -e "${BOLD}Building target${RESET} ${target}..."
        cargo clean
        cross build --release --target "${target}"
        cp "${SOURCE_DIRECTORY}/target/${target}/release/${BIN_NAME}" \
           "${BINARIES_DIRECTORY}/${target}"
    done
}


# This makes the packages
make_packages() {
    revision="$1"; shift

    package_building="${PACKAGES_DIRECTORY}/building"

    fname="${PROJECT_NAME}_${revision}"
    subdir="${PROJECT_NAME}-${revision}"

    # Create source packages
    echo -e "${BOLD}Creating source package${RESET}..."
    git -C "${BASE}" archive "${revision}" --format tar.gz --prefix \
        "${fname}/" -o "${PACKAGES_DIRECTORY}/${fname}.tar.gz"

    for binary in "${BINARIES_DIRECTORY}"/*; do
        target="$( basename "${binary}" )"
        echo -e "${BOLD}Creating package for target${RESET} ${target}..."

        dest="${package_building}/${subdir}"

        rm -rf "${package_building}"
        mkdir -p "${dest}"

        cp "${binary}" "${dest}/${BIN_NAME}"
        for src in "${PACKAGES_INCLUDE_FILES[@]}"; do
            cp "${SOURCE_DIRECTORY}/${src}" "${dest}"
        done

        cd "${package_building}"

        # Calculate the tar options
        mtime="`git log -1 --format=%cd "--date=format:%Y-%m-%d %H:%m:%S"`"
        archive="${PACKAGES_DIRECTORY}/${fname}_${TARGETS[$target]}.tar.gz"

        find "${subdir}" -print0 \
            | LC_ALL=C sort -z \
            | tar --null -T - --no-recursion -czf "${archive}" \
              --owner=root --group=root --numeric-owner --mtime="${mtime}"

        rm -rf "${package_building}"
    done
}


# This signs all the packages
sign_packages() {
    cd "${PACKAGES_DIRECTORY}"

    gpg_bin="gpg"
    if which gpg2 >/dev/null 2>&1; then
        gpg_bin="gpg2"
    fi

    for package in *; do
        echo -e "${BOLD}Signing file${RESET} ${package}..."
        "${gpg_bin}" --sign --detach --armor "${package}"
        "${gpg_bin}" --verify "${package}.asc" "${package}"
    done
}


_main() {
    if [[ $# -ne 1 ]]; then
        echo -e "${BOLD}usage:${RESET} $0 <revision>"
        exit 1
    fi
    revision="$1"; shift

    if ! which cross >/dev/null 2>&1; then
        echo -e "installing ${BOLD}cross${RESET}..."
        cargo install cross
    fi

    cleanup
    prepare_source "${revision}"
    build_binaries
    make_packages "${revision}"
    sign_packages

    echo -e "${BOLD}Congrats for the release!${RESET}"
}
_main $@
