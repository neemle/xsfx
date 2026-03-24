# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| latest  | Yes       |

## Reporting a Vulnerability

If you discover a security vulnerability in xsfx, please report it responsibly.

**Do NOT open a public issue.** Instead, use one of these methods:

1. **GitHub Security Advisories** (preferred): [Report a vulnerability](https://github.com/ratushnyi-labs/xsfx/security/advisories/new)
2. **Email**: Contact the maintainers privately

## Response Timeline

- **Acknowledgment**: within 48 hours
- **Initial assessment**: within 7 days
- **Fix or mitigation**: based on severity, targeting the next release

## Scope

The following are considered security issues:

- Panics or crashes on malformed/untrusted input (SFX binaries, PE files, Mach-O files)
- Buffer overflows or out-of-bounds reads in binary parsing
- Integer overflows in offset/size calculations
- Arbitrary code execution beyond the intended payload execution
- Memory safety violations in unsafe blocks
- Resource exhaustion (unbounded allocation from crafted input)

The following are NOT security issues:

- The payload itself being malicious (xsfx is a packer, not an antivirus)
- Performance issues
- Feature requests
