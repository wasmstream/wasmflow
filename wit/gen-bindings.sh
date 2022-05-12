#! /bin/bash -eu

OUT_DIR=/tmp/wit-bindings
rm -fr "${OUT_DIR}"
mkdir -p "${OUT_DIR}"

function generate_bindings {
    type=$1
    direction=$2
    wit_file=$3
    outfile="./wit/bindings-${wit_file}-${type}-${direction}.rs"
    wit-bindgen "${type}" "--${direction}" "./wit/${wit_file}.wit" --out-dir "${OUT_DIR}"
    mv "${OUT_DIR}/bindings.rs" "${outfile}"
    rustfmt "${outfile}"
}

generate_bindings "rust-wasm" "export" "record-processor"
generate_bindings "wasmtime" "import" "record-processor"
generate_bindings "rust-wasm" "import" "s3-sink"
generate_bindings "wasmtime" "export" "s3-sink"
rm -fr "${OUT_DIR}"
