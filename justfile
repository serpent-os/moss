# The default task is to build moss
default: moss

root-dir := justfile_directory()
build-mode := env_var_or_default("MODE", "packaging")
# Prefix for install tasks
prefix := "./install"

[private]
help:
  @just --list -u

[private]
build package:
  cargo build --profile {{build-mode}} -p {{package}}

# Compile boulder
boulder: (build "boulder")

# Compile moss
moss: (build "moss")

# Fix code issues
fix:
  @echo "Applying clippy fixes..."
  cargo clippy --fix --allow-dirty --allow-staged --workspace -- --no-deps
  @echo "Applying cargo fmt"
  cargo fmt --all
  @echo "Fixing typos"
  typos -w

# Run lints
lint:
  @echo "Running clippy..."
  cargo clippy --workspace -- --no-deps
  @echo "Running cargo fmt.."
  cargo fmt --all -- --check
  @echo "Checking for typos..."
  typos

# Run tests
test: lint
  @echo "Running tests in all packages"
  cargo test --all

# Run all DB migrations
migrate: (diesel "meta" "migration run") (diesel "layout" "migration run") (diesel "state" "migration run")  
# Rerun all DB migrations
migrate-redo: (diesel "meta" "migration redo") (diesel "layout" "migration redo") (diesel "state" "migration redo")  

[private]
diesel db +ARGS:
  diesel \
    --config-file {{root-dir}}/moss/src/db/{{db}}/diesel.toml \
    --database-url sqlite://{{root-dir}}/moss/src/db/{{db}}/test.db \
    {{ARGS}}

install-all: install-boulder install-moss

install-boulder:
  #!/usr/bin/env bash
  set -euxo pipefail
  install -Dm00755 target/{{ build-mode }}/boulder -t {{ prefix }}/usr/bin/

  # Install all the data files
  find boulder/data/ -type f -print0 | sed 's|boulder/data||g' | xargs -0 -I ? xargs install -Dm00644 boulder/data/? {{ prefix }}/usr/share/boulder/?

  # Install shell completions
  export tmpdir=`mktemp -d`
  target/{{ build-mode }}/boulder completions bash > $tmpdir/boulder
  install -Dm00644 $tmpdir/boulder -t {{ prefix }}/usr/share/bash-completion/completions/
  target/{{ build-mode }}/boulder completions zsh > $tmpdir/_boulder
  install -Dm00644 $tmpdir/_boulder -t {{ prefix }}/usr/share/zsh/site-functions/
  target/{{ build-mode }}/boulder completions fish > $tmpdir/boulder.fish
  install -Dm00644 $tmpdir/boulder.fish -t {{ prefix }}/usr/share/fish/vendor_completions.d/

  # License
  install -Dm00644 LICENSES/* -t {{ prefix }}/usr/share/licenses/boulder

  # Cleanup
  rm -rfv $tmpdir

install-moss:
  #!/usr/bin/env bash
  set -euxo pipefail
  install -Dm00755 target/{{ build-mode }}/moss -t {{ prefix }}/usr/bin/

  # Install shell completions
  export tmpdir=`mktemp -d`
  target/{{ build-mode }}/moss completions bash > $tmpdir/moss
  install -Dm00644 $tmpdir/moss -t {{ prefix }}/usr/share/bash-completion/completions/
  target/{{ build-mode }}/moss completions zsh > $tmpdir/_moss
  install -Dm00644 $tmpdir/_moss -t {{ prefix }}/usr/share/zsh/site-functions/
  target/{{ build-mode }}/moss completions fish > $tmpdir/moss.fish
      install -Dm00644 $tmpdir/moss.fish -t {{ prefix }}/usr/share/fish/vendor_completions.d/

  # License
  install -Dm00644 LICENSES/* -t {{ prefix }}/usr/share/licenses/moss

  # Cleanup
  rm -rfv $tmpdir

create-release-tar:
  scripts/create-release-tar.sh
