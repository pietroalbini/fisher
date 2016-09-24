#!/bin/bash
# Make a new release build of Fisher
# Copyright (C) 2016 Pietro Albini
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


# In order to do a build on Ubuntu 64bit you need to execute first
# sudo apt install gcc-multilib g++-multilib libssl-dev:i386


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
        cargo build --release --target "${target}"
        cp "${SOURCE_DIRECTORY}/target/${target}/release/${BIN_NAME}" \
           "${BINARIES_DIRECTORY}/${target}"
    done
}


# This optimizes the binaries, stripping away debug stuff
optimize_binaries() {
    cd "${BINARIES_DIRECTORY}"

    for bin in *; do
        echo -e "${BOLD}Optimizing target${RESET} ${bin}..."
        strip --strip-debug "${bin}"
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

    for package in *; do
        echo -e "${BOLD}Signing file${RESET} ${package}..."
        gpg --sign --detach --armor "${package}"
        gpg --verify "${package}.asc" "${package}"
    done
}


_main() {
    revision="$1"; shift

    cleanup
    prepare_source "${revision}"
    build_binaries
    optimize_binaries
    make_packages "${revision}"
    sign_packages

    echo -e "${BOLD}Congrats for the release!${RESET}"
}
_main $@
