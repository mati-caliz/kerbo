#!/usr/bin/env bash
# Gate del estándar para un crate o workspace de Rust. Lo copia quality/sync.sh como
# scripts/verify-rust.sh y no se edita en el repo. El host no tiene Rust: todo corre en una imagen
# armada acá mismo sobre la oficial fijada por digest, con clippy, rustfmt, llvm-tools, git (lo
# usa cargo-deny para bajar la base de RustSec), cargo-deny y cargo-llvm-cov (los binarios
# verificados por sha256). La memoria va acotada y el registro de crates y los target/ viven en un
# volumen compartido, así el repo no se llena de archivos de root. Se llama envuelto en build-lock.sh, como cualquier gate pesado del server.
#
#   scripts/verify-rust.sh
set -euo pipefail

readonly RUST_IMAGE="rust:1.99.1-slim-trixie@sha256:4df2ae8cfdfebdfc13a65da7f253b73d68983dc0339f2719bc17a279c913b13d"
readonly CARGO_DENY_URL="https://github.com/EmbarkStudios/cargo-deny/releases/download/0.20.2/cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz"
readonly CARGO_DENY_SHA256="9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f"
readonly LLVM_COV_URL="https://github.com/taiki-e/cargo-llvm-cov/releases/download/v0.9.1/cargo-llvm-cov-x86_64-unknown-linux-musl.tar.gz"
readonly LLVM_COV_SHA256="3fca950394a3c49457657c158b1619cec8dfd2647ae5b48746734c0ab969a522"
readonly MEMORY_LIMIT="1500m"
# Un solo job: con dos, el clippy de un crate grande compilado de cero (pasó con kerbo al subir
# cc el 2026-10-26) junta dos clippy-driver de ~750 MB y el tope del contenedor lo mata.
readonly BUILD_JOBS=1
readonly CACHE_VOLUME="rust-quality-cache"
readonly VERIFY_CGROUP="builds.slice"
readonly MAX_FILE_LINES=400

# Todo clippy (pedantic, nursery y cargo) más los de restriction que hacen cumplir el estándar:
# sin unwrap ni panic fuera de los tests, sin casteos con `as`, sin `allow` (una excepción va con
# `#[expect(lint, reason = "...")]`, que además falla si ya no hace falta) y sin unsafe. Queda
# afuera `unreachable_pub` de rustc: pide `pub(crate)` donde `redundant_pub_crate` de clippy pide
# `pub`, y lo inalcanzable de una biblioteca ya lo marca `dead_code`.
readonly LINT_FLAGS="
  -D warnings
  -D clippy::all -D clippy::pedantic -D clippy::nursery -D clippy::cargo
  -D clippy::allow_attributes -D clippy::allow_attributes_without_reason
  -D clippy::as_conversions -D clippy::indexing_slicing -D clippy::unwrap_used
  -D clippy::expect_used -D clippy::panic -D clippy::todo -D clippy::unimplemented
  -D clippy::dbg_macro -D clippy::exit -D clippy::unwrap_in_result -D clippy::get_unwrap
  -D clippy::excessive_nesting -D clippy::wildcard_enum_match_arm -D clippy::shadow_unrelated
  -D clippy::mod_module_files -D clippy::clone_on_ref_ptr -D clippy::float_cmp_const
  -D clippy::lossy_float_literal -D clippy::mem_forget -D clippy::rc_buffer
  -D clippy::str_to_string -D clippy::try_err -D clippy::verbose_file_reads
  -D clippy::if_then_some_else_none -D clippy::empty_structs_with_brackets
  -D clippy::rest_pat_in_fully_bound_structs -D clippy::unneeded_field_pattern
  -D clippy::let_underscore_must_use -D clippy::unused_result_ok -D clippy::format_push_string
  -D clippy::error_impl_error -D clippy::multiple_inherent_impl -D clippy::partial_pub_fields
  -D clippy::same_name_method -D clippy::infinite_loop -D clippy::tests_outside_test_module
  -D clippy::redundant_type_annotations -D clippy::cfg_not_test
  -F unsafe_code -D rust_2018_idioms -D unused_qualifications
  -D missing_debug_implementations -D let_underscore_drop -D unused_lifetimes
  -D single_use_lifetimes -D redundant_lifetimes -D trivial_casts -D trivial_numeric_casts
"

crate_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && { pwd -W 2>/dev/null || pwd; })"
recipe="FROM ${RUST_IMAGE}
RUN apt-get update && apt-get install -y --no-install-recommends git && rm -rf /var/lib/apt/lists/* \\
 && rustup component add clippy rustfmt llvm-tools
ADD --checksum=sha256:${CARGO_DENY_SHA256} ${CARGO_DENY_URL} /tmp/cargo-deny.tar.gz
ADD --checksum=sha256:${LLVM_COV_SHA256} ${LLVM_COV_URL} /tmp/cargo-llvm-cov.tar.gz
RUN tar -xzf /tmp/cargo-deny.tar.gz -C /usr/local/cargo/bin --strip-components=1 --wildcards '*/cargo-deny' \\
 && tar -xzf /tmp/cargo-llvm-cov.tar.gz -C /usr/local/cargo/bin cargo-llvm-cov \\
 && rm /tmp/*.tar.gz"
image_tag="quality-rust:$(printf '%s' "${recipe}" | sha256sum | cut -c1-12)"

if ! docker image inspect "${image_tag}" >/dev/null 2>&1; then
  printf '%s\n' "${recipe}" | docker build --cgroup-parent="${VERIFY_CGROUP}" -q -t "${image_tag}" - >/dev/null
  docker images --format '{{.Repository}}:{{.Tag}}' quality-rust | grep -vxF "${image_tag}" |
    xargs -r docker rmi >/dev/null
fi

docker run --rm -i -m "${MEMORY_LIMIT}" --cgroup-parent="${VERIFY_CGROUP}" \
  -v "${crate_dir}:/src:ro" -w /src \
  -v "${CACHE_VOLUME}:/cache" \
  -e CARGO_HOME=/cache/cargo -e CARGO_TARGET_DIR="/cache/target/$(basename "${crate_dir}")" \
  -e CARGO_BUILD_JOBS="${BUILD_JOBS}" -e CARGO_TERM_COLOR=never \
  -e LINT_FLAGS="${LINT_FLAGS}" -e MAX_FILE_LINES="${MAX_FILE_LINES}" \
  "${image_tag}" bash -euo pipefail -s <<'GATE'
cargo fmt --all --check
read -r -d "" -a lint_flags <<<"${LINT_FLAGS}" || true
cargo clippy --locked --workspace --all-targets --all-features -- "${lint_flags[@]}"
oversized_files="$(find . -path ./target -prune -o -name '*.rs' -exec awk -v max="${MAX_FILE_LINES}" \
  'END { if (NR > max) printf "%s: %d líneas (máximo %d)\n", FILENAME, NR, max }' {} \;)"
if [ -n "${oversized_files}" ]; then
  echo "${oversized_files}" >&2
  exit 1
fi
cargo llvm-cov --locked --workspace --all-features --lcov --output-path /tmp/coverage.lcov
if [ -f scripts/check-rust-coverage.sh ]; then bash scripts/check-rust-coverage.sh /tmp/coverage.lcov; fi
cargo deny --locked --workspace --all-features check
GATE
