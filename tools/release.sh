#!/bin/bash
# Publish a release of Fisher
# Copyright (C) 2017 Pietro Albini <pietro@pietroalbini.org>
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
# along with this program.  If not, see <http://www.gnu.org/licenses/>.


RELEASE_BRANCH_PREFIX="release/"


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


build_packages() {
    tag="$( git describe --abbrev=0 )"
    "${BASE}/tools/build-release-packages.sh" "${tag}"
}


upload_to_crates() {
    git -c commit.gpgsign=false stash save --include-untracked

    cargo publish
    git stash pop
}


upload_releases() {
    short_version="$(git describe --abbrev=0 | awk '{split(substr($0,2),s,".");print s[1] "." s[2]}')"
    branch="${RELEASE_BRANCH_PREFIX}${short_version}"

    old_branch="$(git rev-parse --abbrev-ref HEAD)"
    git checkout $(git show-ref --verify --quiet "refs/heads/${branch}" || echo '-b') "${branch}"
    git merge master
    git push origin "${branch}"
    git checkout "${old_branch}"
}


main() {
    cd "${base}"
    tag="$( git describe --abbrev=0 )"

    echo "Please be sure:"
    echo "- The changelog has been updated"
    echo "- The version bump has been committed to master"
    echo "- The release tag (starting with a v) has been created"
    echo

    read -p "Are you sure do you want to publish tag ${tag}? [y/N] " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$  ]]; then
        build_packages
        upload_to_crates
        upload_releases

        echo
        echo "Remaining manual steps:"
        echo " - Draft a new release on GitHub"
    else
        echo "Aborted."
    fi
}


main
