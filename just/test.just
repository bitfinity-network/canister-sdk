
# Run all tests
[group('test')]
test test_name="":
  cargo test {{test_name}}
  cargo test {{test_name}} --all-features

# Run all tests for the i686 target
[group('test')]
test_i686 test_name="":
  cargo test {{test_name}} --target i686-unknown-linux-gnu
  cargo test {{test_name}} --target i686-unknown-linux-gnu --all-features

