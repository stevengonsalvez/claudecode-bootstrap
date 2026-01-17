"""
Configuration management for Langfuse integration.

This module handles environment variable configuration without importing
the Langfuse SDK, ensuring zero overhead when not configured.
"""

import os
import re
from pathlib import Path
from typing import Optional


def _load_secrets_file():
    """
    Load secrets from ~/.secrets file if env vars not set.

    Parses shell export statements like:
        export LANGFUSE_PUBLIC_KEY="value"
        export LANGFUSE_SECRET_KEY='value'

    Only loads LANGFUSE_* variables to avoid side effects.
    """
    secrets_file = Path.home() / '.secrets'
    if not secrets_file.exists():
        return

    # Only load if LANGFUSE keys aren't already set
    if os.getenv('LANGFUSE_PUBLIC_KEY') and os.getenv('LANGFUSE_SECRET_KEY'):
        return

    try:
        with open(secrets_file, 'r') as f:
            content = f.read()

        # Pattern matches: export VAR="value" or export VAR='value' or export VAR=value
        pattern = r'^export\s+(LANGFUSE_\w+)=["\']?([^"\'#\n]+)["\']?\s*(?:#.*)?$'

        for match in re.finditer(pattern, content, re.MULTILINE):
            key, value = match.groups()
            value = value.strip()
            if not os.getenv(key):  # Don't override existing env vars
                os.environ[key] = value
    except Exception:
        pass  # Fail silently - secrets file is optional


# Load secrets on module import
_load_secrets_file()


class LangfuseConfig:
    """
    Configuration for Langfuse integration.

    Reads from environment variables:
    - LANGFUSE_PUBLIC_KEY: Required to enable Langfuse
    - LANGFUSE_SECRET_KEY: Required to enable Langfuse
    - LANGFUSE_HOST: Custom Langfuse host (default: https://cloud.langfuse.com)
    - LANGFUSE_ENABLED: Explicit enable/disable flag (default: true)
    - LANGFUSE_ENVIRONMENT: Environment tag (default: development)
    - LANGFUSE_RELEASE: Release version tag
    - LANGFUSE_DEBUG: Enable debug logging (default: false)
    """

    def __init__(self):
        self.public_key: Optional[str] = os.getenv('LANGFUSE_PUBLIC_KEY')
        self.secret_key: Optional[str] = os.getenv('LANGFUSE_SECRET_KEY')
        self.host: str = os.getenv('LANGFUSE_HOST', 'https://cloud.langfuse.com')
        self.enabled: bool = os.getenv('LANGFUSE_ENABLED', 'true').lower() == 'true'
        self.environment: str = os.getenv('LANGFUSE_ENVIRONMENT', 'development')
        self.release: Optional[str] = os.getenv('LANGFUSE_RELEASE', 'claude-code-hooks-v1')
        self.debug: bool = os.getenv('LANGFUSE_DEBUG', 'false').lower() == 'true'

    def is_available(self) -> bool:
        """
        Check if Langfuse is properly configured and enabled.

        Returns True only if:
        - Both public_key and secret_key are set
        - LANGFUSE_ENABLED is not explicitly set to 'false'
        """
        return bool(
            self.enabled and
            self.public_key and
            self.secret_key
        )

    def to_dict(self) -> dict:
        """Return configuration as dictionary for Langfuse client initialization."""
        return {
            'public_key': self.public_key,
            'secret_key': self.secret_key,
            'host': self.host,
            'environment': self.environment,
            'release': self.release,
            'debug': self.debug,
        }

    def __repr__(self) -> str:
        return (
            f"LangfuseConfig(available={self.is_available()}, "
            f"host={self.host}, environment={self.environment})"
        )


# Singleton configuration instance
_config: Optional[LangfuseConfig] = None


def get_config() -> LangfuseConfig:
    """Get the singleton configuration instance."""
    global _config
    if _config is None:
        _config = LangfuseConfig()
    return _config
