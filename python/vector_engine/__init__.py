"""Vector Geometry Engine python package entrypoint."""

try:
    from vector_engine._native import process_svg_outset
except ImportError:
    process_svg_outset = None

try:
    from vector_engine.config import load_config, EngineConfig
except ImportError:
    load_config = None
    EngineConfig = None

try:
    from vector_engine.logging import configure_logging
except ImportError:
    configure_logging = None

__all__ = [
    "load_config",
    "EngineConfig",
    "configure_logging",
    "process_svg_outset",
]
