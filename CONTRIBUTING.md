# Contributing to Solana Transaction Bundler

Thank you for your interest in contributing to the Solana Transaction Bundler! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Code Standards](#code-standards)
- [Testing Guidelines](#testing-guidelines)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)
- [Issue Reporting](#issue-reporting)
- [Security Vulnerabilities](#security-vulnerabilities)
- [Community](#community)

## Code of Conduct

This project adheres to a code of conduct that we expect all contributors to follow. Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md) to help us maintain a welcoming and inclusive community.

## Getting Started

### Prerequisites

- **Rust 1.75+** (stable toolchain)
- **Git** for version control
- **Docker** (optional, for testing)
- **Solana CLI** (optional, for testing)

### Setting Up Development Environment

1. **Fork and clone the repository:**
   ```bash
   git clone https://github.com/your-username/solana-bundler.git
   cd solana-bundler
   ```

2. **Set up the development environment:**
   ```bash
   make setup
   ```

3. **Verify the setup:**
   ```bash
   make check
   make test
   ```

4. **Start development server:**
   ```bash
   make dev
   ```

### Project Structure

```
solana-bundler/
├── crates/
│   ├── bundler-types/     # Core type definitions
│   ├── bundler-config/    # Configuration management
│   ├── bundler-core/      # Core business logic
│   ├── bundler-cli/       # Command-line interface
│   └── bundler-service/   # HTTP service
├── tests/
│   ├── unit/             # Unit tests
│   ├── integration/      # Integration tests
│   └── e2e/              # End-to-end tests
├── benchmarks/           # Performance benchmarks
├── docs/                 # Documentation
├── examples/             # Usage examples
└── .github/              # CI/CD workflows
```

## Development Workflow

### Branching Strategy

We use **Git Flow** with the following branch types:

- `main`: Production-ready code
- `develop`: Integration branch for features
- `feature/*`: New features
- `bugfix/*`: Bug fixes
- `hotfix/*`: Critical production fixes
- `release/*`: Release preparation

### Creating a Feature Branch

```bash
# Start from develop branch
git checkout develop
git pull origin develop

# Create feature branch
git checkout -b feature/your-feature-name

# Make your changes
# ...

# Commit changes
git add .
git commit -m "feat: add your feature description"

# Push branch
git push origin feature/your-feature-name
```

### Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks
- `perf`: Performance improvements
- `ci`: CI/CD changes

**Examples:**
```
feat(core): add adaptive fee calculation
fix(rpc): handle connection timeout gracefully
docs(api): update endpoint documentation
test(bundler): add integration tests for atomic transactions
```

## Code Standards

### Rust Code Style

We follow standard Rust conventions with some additional guidelines:

1. **Use `rustfmt` for formatting:**
   ```bash
   cargo fmt --all
   ```

2. **Pass `clippy` lints:**
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

3. **Follow naming conventions:**
   - Types: `PascalCase`
   - Functions/variables: `snake_case`
   - Constants: `SCREAMING_SNAKE_CASE`
   - Modules: `snake_case`

4. **Documentation requirements:**
   - All public APIs must have documentation
   - Include examples for complex functions
   - Document error conditions

### Code Quality Guidelines

1. **Error Handling:**
   ```rust
   // Good: Use structured error types
   #[derive(Debug, thiserror::Error)]
   pub enum BundlerError {
       #[error("RPC connection failed: {0}")]
       RpcConnection(String),
       #[error("Invalid configuration: {0}")]
       Config(String),
   }
   
   // Avoid: Generic error types
   fn bad_function() -> Result<(), Box<dyn std::error::Error>> { ... }
   ```

2. **Async Code:**
   ```rust
   // Good: Use structured concurrency
   async fn process_bundle(&self, bundle: Bundle) -> Result<Response, BundlerError> {
       let simulation = self.simulate(&bundle).await?;
       let signed = self.sign(&bundle).await?;
       self.submit(&signed).await
   }
   
   // Avoid: Blocking in async context
   async fn bad_function() {
       std::thread::sleep(Duration::from_secs(1)); // Don't do this
   }
   ```

3. **Resource Management:**
   ```rust
   // Good: Use RAII and proper cleanup
   pub struct RpcClient {
       client: reqwest::Client,
       endpoints: Vec<Endpoint>,
   }
   
   impl Drop for RpcClient {
       fn drop(&mut self) {
           // Cleanup resources
       }
   }
   ```

4. **Testing:**
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[tokio::test]
       async fn test_bundle_processing() {
           let bundler = create_test_bundler().await;
           let bundle = create_test_bundle();
           
           let result = bundler.process(bundle).await;
           
           assert!(result.is_ok());
           assert_eq!(result.unwrap().status, BundleStatus::Success);
       }
   }
   ```

### Performance Guidelines

1. **Avoid unnecessary allocations:**
   ```rust
   // Good: Use string slices when possible
   fn process_signature(sig: &str) -> Result<Signature, Error> { ... }
   
   // Avoid: Unnecessary String allocation
   fn bad_process(sig: String) -> Result<Signature, Error> { ... }
   ```

2. **Use appropriate data structures:**
   ```rust
   // Good: Use HashMap for key-value lookups
   use std::collections::HashMap;
   let mut cache: HashMap<String, CachedValue> = HashMap::new();
   
   // Avoid: Linear search in Vec for lookups
   let mut cache: Vec<(String, CachedValue)> = Vec::new();
   ```

3. **Profile performance-critical code:**
   ```bash
   cargo bench
   ```

## Testing Guidelines

### Test Categories

1. **Unit Tests** (`tests/unit/`):
   - Test individual functions and modules
   - Mock external dependencies
   - Fast execution (< 1ms per test)

2. **Integration Tests** (`tests/integration/`):
   - Test component interactions
   - Use real dependencies where possible
   - Medium execution time (< 100ms per test)

3. **End-to-End Tests** (`tests/e2e/`):
   - Test complete workflows
   - Use real Solana devnet
   - Slower execution (< 10s per test)

### Writing Tests

```rust
// Unit test example
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    
    #[test]
    fn test_fee_calculation() {
        let fee_manager = FeeManager::new(FeeConfig::default());
        let accounts = vec![Pubkey::new_unique()];
        
        let fee = fee_manager.calculate_base_fee(&accounts);
        
        assert!(fee > 0);
        assert!(fee <= MAX_FEE);
    }
    
    #[tokio::test]
    async fn test_rpc_failover() {
        let mut mock_client = MockRpcClient::new();
        mock_client
            .expect_get_latest_blockhash()
            .times(1)
            .returning(|| Ok(Hash::new_unique()));
        
        let result = mock_client.get_latest_blockhash().await;
        assert!(result.is_ok());
    }
}
```

### Test Data

Create reusable test utilities:

```rust
// tests/common/mod.rs
pub fn create_test_bundle() -> BundleRequest {
    BundleRequest {
        request_id: Uuid::new_v4(),
        atomic: true,
        compute: ComputeConfig::default(),
        instructions: vec![create_test_instruction()],
        signers: vec![],
        metadata: HashMap::new(),
    }
}

pub fn create_test_instruction() -> InstructionData {
    InstructionData {
        program_id: system_program::ID,
        keys: vec![
            AccountMeta {
                pubkey: Pubkey::new_unique(),
                is_signer: true,
                is_writable: true,
            }
        ],
        data_b64: base64::encode(&[1, 2, 3, 4]),
    }
}
```

### Running Tests

```bash
# Run all tests
make test

# Run specific test category
make test-unit
make test-integration

# Run tests with coverage
make coverage

# Run benchmarks
make bench
```

## Documentation

### Code Documentation

1. **Public APIs must be documented:**
   ```rust
   /// Processes a bundle of transactions atomically.
   /// 
   /// # Arguments
   /// 
   /// * `bundle` - The bundle request containing instructions to process
   /// 
   /// # Returns
   /// 
   /// Returns a `BundleResponse` with transaction results and metrics.
   /// 
   /// # Errors
   /// 
   /// This function will return an error if:
   /// - Any instruction fails validation
   /// - RPC connection is unavailable
   /// - Signing fails
   /// 
   /// # Examples
   /// 
   /// ```rust
   /// let bundle = BundleRequest::new(instructions);
   /// let response = bundler.process_bundle(bundle).await?;
   /// assert_eq!(response.status, BundleStatus::Success);
   /// ```
   pub async fn process_bundle(&self, bundle: BundleRequest) -> Result<BundleResponse, BundlerError> {
       // Implementation
   }
   ```

2. **Update documentation for changes:**
   - Update README.md for user-facing changes
   - Update API.md for API changes
   - Update DEPLOYMENT.md for deployment changes

3. **Generate and check documentation:**
   ```bash
   make doc
   ```

### Writing Documentation

1. **Use clear, concise language**
2. **Include practical examples**
3. **Document error conditions**
4. **Keep documentation up-to-date with code**

## Pull Request Process

### Before Submitting

1. **Ensure all tests pass:**
   ```bash
   make check
   make test
   ```

2. **Update documentation if needed**

3. **Add changelog entry** (if applicable)

4. **Rebase on latest develop:**
   ```bash
   git checkout develop
   git pull origin develop
   git checkout feature/your-feature
   git rebase develop
   ```

### Pull Request Template

When creating a PR, use this template:

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] No new warnings introduced
```

### Review Process

1. **Automated checks must pass**
2. **At least one maintainer review required**
3. **Address all review comments**
4. **Squash commits before merge** (if requested)

## Issue Reporting

### Bug Reports

Use the bug report template:

```markdown
**Describe the bug**
A clear description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Configure bundler with '...'
2. Submit bundle with '...'
3. See error

**Expected behavior**
What you expected to happen.

**Environment:**
- OS: [e.g. Ubuntu 20.04]
- Rust version: [e.g. 1.75.0]
- Bundler version: [e.g. 0.1.0]

**Additional context**
Add any other context about the problem here.
```

### Feature Requests

Use the feature request template:

```markdown
**Is your feature request related to a problem?**
A clear description of what the problem is.

**Describe the solution you'd like**
A clear description of what you want to happen.

**Describe alternatives you've considered**
Alternative solutions or features you've considered.

**Additional context**
Add any other context or screenshots about the feature request here.
```

## Security Vulnerabilities

**Do not report security vulnerabilities through public GitHub issues.**

Instead, please email security@yourorg.com with:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will respond within 48 hours and work with you to address the issue.

## Community

### Communication Channels

- **GitHub Discussions**: For general questions and discussions
- **GitHub Issues**: For bug reports and feature requests
- **Discord**: [Join our Discord server](https://discord.gg/your-server)
- **Twitter**: [@SolanaBundler](https://twitter.com/SolanaBundler)

### Getting Help

1. **Check existing documentation**
2. **Search existing issues**
3. **Ask in GitHub Discussions**
4. **Join our Discord for real-time help**

### Recognition

Contributors will be recognized in:
- **CONTRIBUTORS.md** file
- **Release notes** for significant contributions
- **Annual contributor highlights**

## Development Tips

### Useful Commands

```bash
# Development workflow
make dev              # Start development server
make check            # Run all checks
make test             # Run all tests
make bench            # Run benchmarks

# Code quality
make fmt              # Format code
make clippy           # Run lints
make audit            # Security audit

# Documentation
make doc              # Generate docs
make doc-private      # Include private items

# Docker
make docker-build     # Build Docker image
make docker-run       # Run container
```

### IDE Setup

**VS Code Extensions:**
- rust-analyzer
- CodeLLDB (for debugging)
- Better TOML
- GitLens

**Settings:**
```json
{
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.checkOnSave.command": "clippy"
}
```

### Debugging

```bash
# Debug with logs
RUST_LOG=debug cargo run --bin bundler-service

# Debug with debugger
cargo build
lldb target/debug/bundler-service
```

## Thank You

Thank you for contributing to the Solana Transaction Bundler! Your contributions help make this project better for everyone in the Solana ecosystem.

---

**Questions?** Feel free to reach out to the maintainers or ask in our community channels.
