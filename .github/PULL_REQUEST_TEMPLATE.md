## Description
Briefly explain the goal and scope of this pull request.

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] New frontend preset or platform mapping
- [ ] Documentation update
- [ ] Performance improvement or refactor

## Invariants & Safety Verification
- [ ] **Non-Destructive Guarantee**: Verified that input directory operations remain read-only.
- [ ] **Atomic Compression**: Verified that `.part` staging and `MComprHD` magic verification are preserved.
- [ ] **No ROMs / BIOS Bundled**: Verified no proprietary or copyrighted binaries/dumps are committed.
- [ ] **Local Inference**: Any classifier changes preserve offline execution and zero-telemetry rules.

## Testing Checklist
- [ ] `npm test` passes (all Vitest frontend tests green)
- [ ] `cargo test` (`cd src-tauri && cargo test` or `--manifest-path src-tauri/Cargo.toml`) passes (all backend suites green)
