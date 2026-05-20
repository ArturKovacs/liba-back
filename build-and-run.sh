#!/bin/bash

set -eux

project_root=$(pwd)
back_static_folder="$project_root/back/static"

cd front
elm make src/Main.elm --output="$back_static_folder/main.js"
mkdir -p "$back_static_folder"

cp static/* "$back_static_folder"

cd "$project_root/back"
cargo run --release
