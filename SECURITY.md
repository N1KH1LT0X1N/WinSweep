# Security Policy

## Supported Versions

| Version | Supported          |
|---------| ------------------ |
| 1.0.x   | :white_check_mark: |
| < 1.0   | :x:                |

## Scope

We welcome reports for vulnerabilities that affect the confidentiality, integrity, or availability of WinSweep or its users. This includes, but is not limited to:

- Remote code execution (RCE)
- Privilege escalation
- Unauthorized data loss or deletion
- Path traversal or sandbox escape
- Injection vulnerabilities in configuration or log parsing

## Reporting a Vulnerability

**Do not open public issues for security vulnerabilities.**

Instead, use GitHub's private vulnerability reporting:

1. Go to **Security → Advisories**
2. Click **"New draft security advisory"**
3. Fill in the details and submit

Please include:
- A clear description of the vulnerability
- Steps to reproduce
- Potential impact and affected versions
- Any suggested mitigations or patches

## Response Process

- **Acknowledgment**: We will acknowledge receipt within **48 hours**
- **Investigation**: We will investigate and validate the report within **7 days**
- **Fix & Disclosure**: We follow coordinated disclosure. Once a fix is ready, we will:
  1. Prepare a patch release
  2. Request a CVE if warranted
  3. Publish a security advisory on GitHub
  4. Credit the reporter (unless anonymity is requested)

## Security Best Practices for Users

- Always download WinSweep from the [official GitHub Releases](https://github.com/N1KH1LT0X1N/WinSweep/releases)
- Verify SHA-256 checksums of downloaded binaries
- Run with the least privileges necessary (admin is only required for system-wide cleaning)
- Keep WinSweep updated to the latest supported version

## History

See [GitHub Security Advisories](https://github.com/N1KH1LT0X1N/WinSweep/security/advisories) for past disclosures.
