"""Logging initialization module syncing Python structlog with Rust tracing."""

import logging
import sys
from typing import Optional
import structlog

from vector_engine.config import LoggingConfig


def configure_logging(config: Optional[LoggingConfig] = None) -> None:
    """Initialize structured logging pipeline for Python and bridge to Rust subscriber."""
    cfg = config or LoggingConfig()
    log_level = getattr(logging, cfg.level.upper(), logging.INFO)

    # Configure standard Python logging
    logging.basicConfig(
        format="%(message)s",
        stream=sys.stdout,
        level=log_level,
    )

    processors = [
        structlog.stdlib.add_log_level,
        structlog.stdlib.add_logger_name,
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.UnicodeDecoder(),
    ]

    if cfg.format.lower() == "json":
        processors.append(structlog.processors.JSONRenderer())
    else:
        processors.append(structlog.dev.ConsoleRenderer())

    structlog.configure(
        processors=processors,
        logger_factory=structlog.stdlib.LoggerFactory(),
        wrapper_class=structlog.stdlib.BoundLogger,
        cache_logger_on_first_use=True,
    )

    # Bridge to Rust tracing subscriber if enabled
    if cfg.bridge_rust_tracing:
        try:
            from vector_engine import _native
            _native.init_tracing(cfg.level)
        except ImportError:
            # Native module might not be compiled yet during early setup
            pass
