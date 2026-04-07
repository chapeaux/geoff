# Contributing to Geoff

We welcome contributions! Geoff is built by the community, and we're excited to work with you.

## Getting Started

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Run tests and linters (see below)
5. Commit with a clear message
6. Push to your fork and open a pull request

## Code Quality Requirements

All submissions must pass:

### Format
```bash
cargo fmt --all
```

### Lint
```bash
cargo clippy --all -- -D warnings
```

### Tests
```bash
cargo test --all
```

Please ensure these pass before submitting a PR. We run the same checks in CI.

## Code Style

- Follow Rust naming conventions (snake_case for functions/variables, PascalCase for types)
- Keep functions focused and well-documented
- Add tests for new functionality
- Update relevant documentation

## IP Policy

**By submitting a pull request, you agree that:**

1. Your contribution is submitted under the MIT license (see LICENSE file)
2. You have the right to license your contribution under the MIT license
3. You are not submitting code that violates any third-party intellectual property rights
4. You understand that your contribution may be used commercially without royalty

**Developer Certificate of Origin:** By contributing, you certify that:

```
By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it.

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it) is maintained indefinitely
    and may be redistributed consistent with this project or the open
    source license(s) involved.
```

## Reporting Issues

- Use GitHub Issues to report bugs
- Include reproduction steps for bugs
- Suggest features with clear use cases
- Reference relevant documentation

## Questions?

Open an issue or start a discussion in the repository. We're happy to help!

Thank you for contributing to Geoff! 🎩
