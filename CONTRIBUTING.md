# Contributing to uncompress

Thank you for considering contributing to `uncompress`! We welcome all kinds of contributions, including bug reports, feature requests, documentation improvements, and code contributions.

## Code of Conduct

Please be respectful and constructive in your interactions. We aim to foster an inclusive and welcoming community.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/your-username/uncompress.git
   cd uncompress
   ```
3. **Set up the development environment**:
   ```bash
   # Ensure you have Rust 1.70+ installed
   rustc --version
   
   # Build the project
   cargo build
   
   # Run tests
   cargo test
   ```

## Making Changes

1. **Create a branch** for your changes:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** following the coding guidelines below

3. **Ensure code quality**:
   ```bash
   # Format your code
   cargo fmt
   
   # Run clippy linter
   cargo clippy -- -W clippy::all
   
   # Run tests
   cargo test
   ```

4. **Commit your changes** with clear, descriptive commit messages:
   ```bash
   git commit -m "feat: add support for new file format"
   ```

5. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

6. **Open a Pull Request** on GitHub

## Coding Guidelines

### Code Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` to format code (configuration in `rustfmt.toml`)
- Avoid clippy warnings (`cargo clippy`)
- Write clear, self-documenting code with minimal comments

### Documentation

- Add doc comments (`///`) to public functions and types
- Include examples in documentation when helpful
- Update README.md if adding new features

### Testing

- Write unit tests for new functionality
- Include integration tests for CLI behavior
- Ensure all tests pass before submitting PR

### Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `style:` - Code style changes (formatting, etc.)
- `refactor:` - Code refactoring
- `test:` - Test additions or modifications
- `chore:` - Maintenance tasks

Example:
```
feat: add support for tar.gz files

Added processing for tar.gz archives with zero compression.
Closes #42
```

## Pull Request Process

1. Ensure your PR description clearly describes the changes
2. Link any related issues
3. Ensure all CI checks pass
4. Request review from maintainers
5. Address review feedback

## Reporting Issues

### Bug Reports

When reporting a bug, please include:

- **Description**: Clear description of the issue
- **Steps to Reproduce**: How to reproduce the behavior
- **Expected Behavior**: What you expected to happen
- **Actual Behavior**: What actually happened
- **Environment**: OS, Rust version, uncompress version
- **Additional Context**: Any other relevant information

### Feature Requests

When requesting a feature, please include:

- **Description**: Clear description of the feature
- **Use Case**: Why this feature would be useful
- **Examples**: How the feature would be used
- **Alternatives**: Any alternative solutions you've considered

## Questions?

Feel free to open an issue for questions or discussions.

## License

By contributing to this project, you agree that your contributions will be licensed under the project's dual MIT/Apache-2.0 license.
